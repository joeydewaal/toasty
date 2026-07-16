# Database-generated defaults

## Summary

Add `toasty::stmt::now() -> Now`. `Now` works with `jiff::Timestamp` and
`jiff::civil::{DateTime, Date, Time}` fields. Supporting drivers evaluate the
matching native expression. With `#[default]`, migrations also define a column
default. When a driver cannot evaluate `Now` as a `jiff::Timestamp`, Toasty
evaluates `jiff::Timestamp::now()` in the engine.

## Motivation

Timestamp defaults currently run in the application:

```rust
#[default(jiff::Timestamp::now())]
created_at: jiff::Timestamp,
```

The create builder sends the value to the database, and migrations do not
define a column default. Toasty needs a database expression so Toasty writes,
triggers, and external inserts can use the same clock.

## User-facing API

### Database-generated date and time values

Use `toasty::stmt::now()` when the database should choose the value. The field
type determines the result:

```rust
#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: u64,

    #[default(toasty::stmt::now())]
    created_at: jiff::Timestamp,

    #[default(toasty::stmt::now())]
    created_on: jiff::civil::Date,
}
```

`now()` returns a `Now` struct rather than a generic expression. It works with:

- `jiff::Timestamp`
- `jiff::civil::DateTime`
- `jiff::civil::Date`
- `jiff::civil::Time`

Imported forms such as `#[default(now())]` behave the same as the fully
qualified call. The `Now` value, not the source spelling, identifies a database
default.

An explicit value overrides the default. Passing `now()` directly to a setter
uses the database clock when available without defining a column default:

```rust
post.update()
    .published_at(toasty::stmt::now())
    .exec(&mut db)
    .await?;
```

`#[update(toasty::stmt::now())]` emits a database expression on create and
update when the driver supports it. Otherwise, Timestamp fields use an
application value. It does not define an `ON UPDATE` clause. Use both attributes
when a field needs a column default and Toasty-managed updates:

```rust
#[default(toasty::stmt::now())]
#[update(toasty::stmt::now())]
updated_at: jiff::Timestamp,
```

### Automatic fields

Bare `#[auto]` on `jiff::Timestamp` fields named `created_at` and `updated_at`
uses the database clock when the driver supports it:

```rust
// #[auto] on created_at
#[default(toasty::stmt::now())]
created_at: jiff::Timestamp,

// #[auto] on updated_at
#[update(toasty::stmt::now())]
updated_at: jiff::Timestamp,
```

On drivers without database current-time support, bare `#[auto]` retains its
existing application-clock behavior:

```rust
// #[auto] on created_at
#[default(jiff::Timestamp::now())]
created_at: jiff::Timestamp,

// #[auto] on updated_at
#[update(jiff::Timestamp::now())]
updated_at: jiff::Timestamp,
```

This is the same Timestamp fallback used by explicit `toasty::stmt::now()`.
`created_at` uses it when column defaults are unsupported; `updated_at` uses it
when current-time expressions are unsupported.

Use explicit Rust expressions when the application clock is acceptable:

```rust
#[default(jiff::Timestamp::now())]
created_at: jiff::Timestamp,
```

### Before and after

The existing form keeps application-clock behavior:

```rust
#[default(jiff::Timestamp::now())]
created_at: jiff::Timestamp,
```

The new form uses the database clock and defines a migration default when the
driver supports them:

```rust
#[default(toasty::stmt::now())]
created_at: jiff::Timestamp,
```

Existing bare `#[auto]` fields require no source change. Supporting drivers use
the database clock; other drivers use the same `jiff::Timestamp::now()` fallback
as explicit `now()`.

### Generated migrations

Migrations use the column default selected for the field type:

| Field type | PostgreSQL | MySQL | SQLite |
|---|---|---|---|
| `Timestamp` | `DEFAULT CURRENT_TIMESTAMP` | `DEFAULT (UTC_TIMESTAMP(6))` | `DEFAULT CURRENT_TIMESTAMP` |
| `DateTime` | `DEFAULT LOCALTIMESTAMP` | `DEFAULT CURRENT_TIMESTAMP(6)` | `DEFAULT CURRENT_TIMESTAMP` |
| `Date` | `DEFAULT CURRENT_DATE` | `DEFAULT (CURRENT_DATE)` | `DEFAULT CURRENT_DATE` |
| `Time` | `DEFAULT LOCALTIME` | `DEFAULT (CURRENT_TIME(6))` | `DEFAULT CURRENT_TIME` |

MySQL uses `UTC_TIMESTAMP(6)` for `Timestamp` because Toasty stores the instant
as a UTC `DATETIME(6)`. Civil values use the session timezone. The parenthesized
MySQL defaults require MySQL 8.0.13 or newer.

Adding, removing, or changing the default is a schema change.

## Behavior

The target field determines which value `Now` produces. On create,
`#[default(toasty::stmt::now())]` omits the column and lets the database apply
its column default when supported. Direct setters and
`#[update(toasty::stmt::now())]` emit the native expression when supported. An
explicit value includes the column and bypasses the default or update
expression.

When the required native feature is unavailable for a Timestamp field, the
engine evaluates `jiff::Timestamp::now()` and sends the resulting value. The
fallback applies to defaults, updates, direct setters, and bare `#[auto]`.

Create returns the chosen value as part of the created model.

When a field has both attributes, create uses the column default and update uses
the native expression where supported. Each operation independently uses the
Timestamp fallback when its required feature is unavailable. With only
`#[update]`, create and update both use the expression or fallback.

| Attribute | Evaluation | Migration default |
|---|---|---|
| `#[default(jiff::Timestamp::now())]` | Application | None |
| `#[default(toasty::stmt::now())]` | Database column default, or application fallback for `Timestamp` | Native date or time expression when supported |
| `#[update(toasty::stmt::now())]` | Database statement expression, or application fallback for `Timestamp` | None |

Each database keeps its native timezone, timing, and precision behavior.
PostgreSQL and MySQL civil values normally follow the session timezone; SQLite
uses UTC. PostgreSQL `CURRENT_TIMESTAMP` uses the transaction timestamp.

Civil `DateTime`, `Date`, and `Time` fallbacks would require Toasty to choose a
timezone. Uses of those result types return `unsupported_feature` when the
driver lacks the matching native feature.

## Edge cases

An explicit `NULL` does not invoke the default. It stores `NULL` for an optional
field and violates a required field's not-null constraint.

Each row in a bulk create independently omits a defaulted column or supplies an
explicit value. Toasty may split the create into multiple statements when the
rows omit different columns.

`jiff::Zoned` is not supported because SQL current-time expressions do not
return an IANA timezone identity.

Adding a required column with a `now()` default gives existing rows the value
chosen while applying the migration.

Defaults apply only to scalar columns and cannot reference fields, relations,
or documents.

## Driver integration

Drivers advertise expression support and column-default support for each `Now`
result type. Direct setters and `#[update]` use expression support. `#[default]`
uses column-default support. Missing Timestamp support selects the engine
fallback; other missing result types return `unsupported_feature`. No new
operation is required.

DynamoDB uses the Timestamp fallback and does not receive a `Now` expression.
Out-of-tree drivers may support expressions, column defaults, or both; the same
fallback rules apply.

Schema comparison stores the semantic `Now` expression, not rendered SQL.
Default changes therefore participate in column diffing.
Existing schema snapshots without default metadata deserialize with no column
default.

PostgreSQL uses `SET DEFAULT` and `DROP DEFAULT`. MySQL emits the complete
column definition. SQLite rebuilds the table because it cannot alter a default
or add a dynamic date or time default in place.

## Open questions

There are no blocking questions.

## Out of scope

- `jiff::Zoned` current values.
- `ON UPDATE CURRENT_TIMESTAMP`; `#[update]` manages Toasty updates.
- Arbitrary database functions or raw SQL fragments.
