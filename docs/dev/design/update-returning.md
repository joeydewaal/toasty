# Returning Models from Updates

## Summary

Query updates return the backend's affected count by default. The
`.returning_*()` methods update every matching row but control how many models
are returned and whether they contain old or new values.

## Motivation

Query updates currently discard the result even when the backend reports an
affected count. Returning it by default exposes useful information without a
second query. Drivers that cannot determine a count return `0`.

Returning models avoids a second query, its extra round trip, and races with
other writers. Old values let callers remove replaced keys from caches or
search indexes.

## User-facing API

Executing a query update directly returns the affected count:

```rust
let count: u64 = User::filter_by_active(false)
    .update()
    .active(true)
    .exec(&mut db)
    .await?;
```

Existing calls that ignore the result continue to work:

```rust
User::filter_by_active(false)
    .update()
    .active(true)
    .exec(&mut db)
    .await?;
```

Call `.returning_all()`, `.returning_first()`, or `.returning_one()` to return new models:

```rust
let users: Vec<User> = User::filter_by_active(false)
    .update()
    .active(true)
    .returning_all()
    .exec(&mut db)
    .await?;
```

All three methods update every matching row. They control only result
collection: `.returning_all()` collects every updated model, while
`.returning_first()` and `.returning_one()` collect one returned model.
`.returning_first()` returns `None` for no match; `.returning_one()` returns a
record-not-found error.

### Returning old values

Call `.returning_all_old()`, `.returning_first_old()`, or
`.returning_one_old()` to return pre-update values:

```rust
let previous: User = User::update_by_id(id)
    .email(new_email)
    .returning_one_old()
    .exec(&mut db)
    .await?;

cache.remove(&previous.email).await?;
```

The corresponding methods without `_old` return post-update values. All six
result forms compose with `toasty::batch()`. Instance updates remain unchanged:
they return `()` and reload the borrowed model.

## Behavior

| Builder | Result | Default model version |
|---|---|---|
| `update.exec(&mut db)` | `Result<u64>` | N/A |
| `update.returning_all().exec(&mut db)` | `Result<Vec<Model>>` | New |
| `update.returning_first().exec(&mut db)` | `Result<Option<Model>>` | New |
| `update.returning_one().exec(&mut db)` | `Result<Model>` | New |
| `update.returning_all_old().exec(&mut db)` | `Result<Vec<Model>>` | Old |
| `update.returning_first_old().exec(&mut db)` | `Result<Option<Model>>` | Old |
| `update.returning_one_old().exec(&mut db)` | `Result<Model>` | Old |

Affected-count semantics follow the backend:

| Backend | Affected count |
|---|---|
| PostgreSQL | Updated rows, including unchanged values |
| SQLite, Turso | Directly updated rows; side effects excluded |
| MySQL | Matched rows, using `CLIENT_FOUND_ROWS` |
| DynamoDB | Successful root-item mutations, summed by Toasty |
| Drivers without a native count | `0` |

Counts exclude changes caused by relation updates. A driver that cannot report
an affected count returns `0` instead of rejecting the update.

Result collection never limits the update. `.returning_first()` and
`.returning_one()` update every match, then collect one model from the rows
returned by the backend. The backend may return any updated model.
`.returning_one()` does not reject a query that matches multiple rows; it
differs from `.returning_first()` only when no rows match.

Updates do not support `order_by`. An ordered update returns
`Error::unsupported_feature` before writing.

Returned models include no-op assignments. Deferred primitive and embedded
fields remain deferred, and relations remain unloaded.

| Backend | New values | Old values |
|---|---|---|
| PostgreSQL 18+ | Native | Native |
| PostgreSQL before 18 | Native | `Error::unsupported_feature` |
| SQLite, Turso | Native | `Error::unsupported_feature` |
| MySQL | `Error::unsupported_feature` | `Error::unsupported_feature` |
| DynamoDB `UpdateItem` | `ALL_NEW` | `ALL_OLD` |
| DynamoDB `TransactWriteItems` | `Error::unsupported_feature` | `Error::unsupported_feature` |

SQLite returns values from before subsequent `AFTER` triggers; PostgreSQL
returns values after update triggers. Toasty preserves these semantics.

DynamoDB updates that require `TransactWriteItems`, including changes to a
Toasty-managed unique-index field, cannot return models. Unsupported forms fail
before writing.

## Edge cases

- `.returning_all()` and `.returning_all_old()` have no defined result order.
- No matches produce `0`, an empty vector, `None`, or a record-not-found error
  for the default, `returning_all` variants, `returning_first` variants, and
  `returning_one` variants.
- Partial embedded and engine-managed assignments return complete models,
  subject to deferred fields.
- Relation assignments return only root models.
- Non-transactional DynamoDB multi-row updates retain existing partial-write
  behavior.

## Driver integration

Affected counts require no capability. Drivers report support for new and old
model values separately; unsupported model-return requests fail before writing.

Drivers return the same row stream for `.returning_all()`, `.returning_first()`,
and `.returning_one()`. Toasty collects that stream according to the selected
result type.

SQL drivers derive counts from statement-completion metadata. Key-value drivers
count successful root-item mutations without reading rows. Drivers without
either source return `0`. SQL model returns use projected `RETURNING` columns;
old values require native pre-update row references.

MySQL's non-atomic update-then-select fallback does not satisfy the model-return
contract. DynamoDB maps instance reloads, new models, and old models to
`UPDATED_NEW`, `ALL_NEW`, and `ALL_OLD`, and rejects model returns when planning
requires `TransactWriteItems`.

Out-of-tree drivers must preserve existing instance reloads and return their
native affected count or `0` from query updates. They may implement or reject
each model-returning mode.

## Alternatives considered

### Return new models by default

This transfers and decodes unused rows and makes ordinary updates unsupported
where model returning is unavailable.

### Infer cardinality from `update_by_*`

Unique filters can match no row. Explicit `.returning_first()` and `.returning_one()`
keep the zero-row policy visible.

## Open questions

None.

## Out of scope

- Selected fields, eager-loaded relations, and deleted models.
- Separate matched-row and physically-changed-row metadata.
- Owned or borrowed return values from instance updates.
