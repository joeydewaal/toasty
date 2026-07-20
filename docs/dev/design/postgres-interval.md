# Interval Fields

## Summary

Toasty accepts `jiff::Span` as a model field and statement value. PostgreSQL
stores it as a native `INTERVAL` by default; MySQL, SQLite, Turso, and DynamoDB
use an ISO 8601 string. Native PostgreSQL intervals decode into `jiff::Span`,
but Toasty does not encode a plain `jiff::Span` as `INTERVAL`. Use
`toasty::postgres::Interval` for native interval writes.

## Motivation

Applications cannot currently map a PostgreSQL `INTERVAL` column to a Toasty
model. They must use a string field and parse it outside Toasty, which loses
typed create, update, and load behavior.

`jiff::Span` is the portable model type because it represents years through
nanoseconds and works with databases that lack a native interval type.
PostgreSQL instead stores three values: months, days, and microseconds.
Encoding an arbitrary span into those fields requires a relative datetime;
there is no context-free conversion that preserves its meaning.

This follows [`jiff-sqlx`'s PostgreSQL contract]: its span wrapper implements
decoding but not encoding, and directs writes through SQLx's exact
`PgInterval { months, days, microseconds }` type.

[`jiff-sqlx`'s PostgreSQL contract]: https://docs.rs/jiff-sqlx/latest/jiff_sqlx/struct.Span.html#postgresql-limited-support

## User-facing API

Enable Toasty's `jiff` feature and use `jiff::Span` in a portable model:

```rust
#[derive(toasty::Model)]
struct Job {
    #[key]
    id: u64,

    retry_after: jiff::Span,
}
```

Reads return `jiff::Span` on every driver. Generated setters also accept a
plain span, but execution depends on the column's storage type. Text-backed
columns preserve the span with Jiff's ISO 8601 format:

```rust
Job::create().retry_after(
    jiff::Span::new().days(2).hours(6).minutes(30),
);
```

This works with the default storage on MySQL, SQLite, Turso, and DynamoDB. A
PostgreSQL model can opt into the same behavior explicitly:

```rust
#[derive(toasty::Model)]
struct Job {
    #[key]
    id: u64,

    #[column(type = text)]
    retry_after: jiff::Span,
}
```

With PostgreSQL's default native `INTERVAL` storage, passing a plain
`jiff::Span` compiles but returns `Error::unsupported_feature` when the
statement executes. This includes zero and spans that happen to contain only
months, days, and microseconds. Toasty requires an explicit native interval
value instead of inferring that a particular span is safe to collapse.

### Writing a native PostgreSQL interval

Construct native values with `toasty::postgres::Interval`:

```rust
use toasty::postgres::Interval;

Job::create().retry_after(
    Interval::new().months(1).days(2).minutes(30),
);
```

`Interval` implements `IntoExpr<jiff::Span>`, so the generated setter accepts
it even though `retry_after` is a `jiff::Span` field. The stored value is the
exact PostgreSQL triple; Toasty does not first convert it to a Jiff span.

`Interval` contains public `months: i32`, `days: i32`, and
`microseconds: i64` fields. Its `years`, `months`, `weeks`, `days`, `hours`,
`minutes`, `seconds`, `milliseconds`, and `microseconds` methods add to the
corresponding PostgreSQL field. Infallible methods panic on overflow; matching
`try_*` methods return an error for runtime input. PostgreSQL's independent
field signs are preserved.

PostgreSQL-only models can use `Interval` directly when they need exact reads,
including mixed-sign intervals:

```rust
#[derive(toasty::Model)]
struct Lease {
    #[key]
    id: u64,

    duration: toasty::postgres::Interval,
}
```

For an `Interval` field, generated create and update setters accept
`toasty::postgres::Interval` expressions only. A `jiff::Span` does not
implement `IntoExpr<toasty::postgres::Interval>`, so this is rejected by the
Rust compiler:

```rust
Lease::create().duration(jiff::Span::new().days(2));
```

`toasty::postgres::Interval` is available when the `postgresql` feature is
enabled. Converting it to a `jiff::Span` is fallible because Jiff uses one sign
for the entire span and has a smaller range. Building a schema containing an
`Interval` model field for a non-PostgreSQL driver returns
`Error::unsupported_feature`.

## Behavior

The default storage and codec for `jiff::Span` are:

| Driver | Default storage | Plain `jiff::Span` writes |
| --- | --- | --- |
| PostgreSQL | `INTERVAL` | `unsupported_feature` |
| MySQL | `VARCHAR(191)` | ISO 8601 string |
| SQLite / Turso | `TEXT` | ISO 8601 string |
| DynamoDB | String (`S`) | ISO 8601 string |

Text storage preserves all Jiff units, including nanoseconds. PostgreSQL
`#[column(type = text)]` uses this codec as well.

PostgreSQL native interval behavior depends on the Rust field and input types:

| Model field | Input | Result |
| --- | --- | --- |
| `jiff::Span` | `jiff::Span` | Error before dispatch |
| `jiff::Span` | `postgres::Interval` | Exact write if the value can decode back into a span |
| `postgres::Interval` | `postgres::Interval` | Exact write, including mixed signs |
| `postgres::Interval` | `jiff::Span` | Compile-time type error |

The same rules apply to create, update, upsert, batch, and model default
expressions. Toasty provides no conversion from `jiff::Span` or
`Expr<jiff::Span>` to `postgres::Interval`. Runtime validation errors occur
before any statement in the operation is sent to the database.

When PostgreSQL loads an `INTERVAL` into `jiff::Span`, the span contains only
months, days, and microseconds because those are the fields PostgreSQL sends.
For example, `2 years, 15 months, 100 weeks, 99 hours, and 123456789
milliseconds` decodes as `39 months, 700 days, and 479856789000
microseconds`, matching [`jiff-sqlx`'s decode example].

[`jiff-sqlx`'s decode example]: https://github.com/BurntSushi/jiff/blob/master/examples/sqlx-postgres/main.rs

`Option<jiff::Span>` uses SQL `NULL` or an absent DynamoDB attribute like other
optional fields. `Option<toasty::postgres::Interval>` uses PostgreSQL `NULL`.
Zero is stored as a value, not as `NULL`; native storage requires
`Interval::new()`.

Interval filtering, ordering, and arithmetic are not enabled by this design.
Those operations have backend-specific semantics and return
`Error::unsupported_feature` before dispatch.

## Edge cases

- A Jiff span has one sign for all non-zero units, while PostgreSQL permits
  different signs for months, days, and microseconds. Loading a mixed-sign
  native interval into `jiff::Span` returns a type-conversion error. Loading it
  into `postgres::Interval` preserves it.
- PostgreSQL supports a wider interval range than Jiff. A native value outside
  Jiff's range loads into `postgres::Interval` but not `jiff::Span`.
- PostgreSQL has microsecond resolution. `postgres::Interval` does not expose
  nanoseconds; text-backed `jiff::Span` fields preserve them.
- A `postgres::Interval` passed to a `jiff::Span` field must be representable as
  a span so a later read can succeed. Mixed-sign and out-of-range values are
  rejected before the write. Use an `Interval` model field to preserve them.
- Text-backed drivers reject malformed strings with a type-conversion error.

## Driver integration

`stmt::Value::Span(jiff::Span)` and `stmt::Type::Span` carry portable spans.
`stmt::Value::Interval { months, days, microseconds }` and
`stmt::Type::Interval` carry exact native intervals. The distinct value forms
let the engine distinguish an explicit `postgres::Interval` from a plain span;
it does not infer the source form from the components. `Interval` implements
`IntoExpr<jiff::Span>` for explicit native writes through a `jiff::Span` field,
but `jiff::Span` does not implement `IntoExpr<Interval>`.

Drivers add a default span storage type alongside their timestamp, date, time,
and datetime defaults. `db::Type::Interval` identifies native SQL interval
storage. No new `Operation` variant is required.

The PostgreSQL SQL serializer emits `INTERVAL` for `db::Type::Interval`. The
driver binds and decodes PostgreSQL's 16-byte binary value in microseconds,
days, and months order. When the target statement type is `Span`, it converts a
decoded native value to Jiff and reports conversion failures. An `Interval`
target receives the exact three fields.

MySQL, SQLite, and Turso serialize `Value::Span` with Jiff's ISO 8601 formatter
and parse the same format on load. DynamoDB uses the same codec with a String
attribute. Out-of-tree drivers select a default span storage type and implement
a lossless codec, or reject span fields while building the schema.

Drivers report malformed, out-of-range, and unsupported conversion cases as
Toasty errors rather than panicking.

## Alternatives considered

Automatically collapsing years to months, weeks to days, and clock units to
microseconds would accept many spans, but it would claim a context-free
conversion that Jiff does not provide. Requiring `postgres::Interval` makes
that loss of representation explicit and matches `jiff-sqlx`.

Using `std::time::Duration` would exclude negative intervals and calendar
units. Storing every span as text would preserve Jiff exactly but would give up
PostgreSQL's native interval type and operators.

## Open questions

There are no open questions blocking acceptance. Interval query operators can
be designed separately with backend-specific capability gates.

## Out of scope

- Supplying a relative datetime to convert `jiff::Span` into `INTERVAL`.
- PostgreSQL restrictions such as `INTERVAL DAY TO SECOND` and explicit
  fractional-second precision.
- Interval comparison, sorting, arithmetic, aggregation, and indexes.
- Automatic normalization of mixed-sign PostgreSQL intervals.
- Database-side interval literals and interval expressions in schema defaults.
