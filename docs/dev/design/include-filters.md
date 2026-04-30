# Filtering included relations

## Summary

Extend relation paths with a `.filter(...)` combinator so `include(...)`
can load a subset of a relation's records. A user who today writes
`.include(User::fields().todos())` can write
`.include(User::fields().todos().filter(Todo::fields().completed().eq(false)))`
to preload only the unfinished todos for each user. Works the same way
for `HasOne` / `BelongsTo` relations.

## Motivation

`include(...)` currently has no way to restrict which related records
get preloaded. Users have to choose between:

- Loading every related row and filtering in memory — wasteful, and for
  large relations effectively impossible.
- Issuing a separate query for the relation — loses the batching the
  engine already does for `include`, and forces the user to stitch
  results back to parents by hand.

The parent-side combinators `.any(...)` and `.all(...)` already accept a
predicate over the relation's fields, so the building blocks exist; they
just filter *which parents come back*, not *which children load*. The
two are complementary — see "Behavior" — and users routinely want both.

## User-facing API

Every relation path supports `.filter(predicate)`. The predicate is an
`Expr<bool>` written in terms of the relation target's own fields, the
same scope `.any(...)` and `.all(...)` already use.

### Filtering a `HasMany` include

```rust
// Load each user with only their incomplete todos preloaded.
let users: Vec<User> = User::all()
    .include(
        User::fields()
            .todos()
            .filter(Todo::fields().completed().eq(false)),
    )
    .exec(&mut db)
    .await?;

for user in &users {
    // `user.todos.get()` contains only incomplete todos.
    for todo in user.todos.get() {
        assert!(!todo.completed);
    }
}
```

A user with no matching todos still comes back — their `todos` is loaded
as an empty `Vec`, distinct from "not loaded".

### Filtering a `HasOne` / `BelongsTo` include

```rust
// Preload the profile only if it is public; otherwise it loads as None.
let user = User::filter_by_id(id)
    .include(
        User::fields()
            .profile()
            .filter(Profile::fields().public().eq(true)),
    )
    .get(&mut db)
    .await?;

match user.profile.get() {
    Some(profile) => { /* loaded and matches the filter */ }
    None => { /* either no profile exists, or it failed the filter */ }
}
```

The relation is still considered loaded; `.get()` does not panic. From
the parent's perspective a filtered-out 1-1 looks the same as a missing
relation.

### Composing with parent-side filters

`.filter(...)` on the included path is independent of `.any(...)` /
`.all(...)` on the parent query. Users frequently want both:

```rust
// Users who have at least one incomplete todo, with only their
// incomplete todos preloaded.
let users: Vec<User> = User::all()
    .filter(
        User::fields()
            .todos()
            .any(Todo::fields().completed().eq(false)),
    )
    .include(
        User::fields()
            .todos()
            .filter(Todo::fields().completed().eq(false)),
    )
    .exec(&mut db)
    .await?;
```

The parent filter decides which users come back; the include filter
decides which todos travel with each user. Toasty does not deduplicate
the predicate — if the same condition appears in both places, write it
in both places.

### Before and after

```rust
// Before: load everything, filter in memory.
let users: Vec<User> = User::all()
    .include(User::fields().todos())
    .exec(&mut db)
    .await?;
for user in &mut users {
    user.todos.get_mut().retain(|t| !t.completed);
}

// After: filter in the database.
let users: Vec<User> = User::all()
    .include(
        User::fields()
            .todos()
            .filter(Todo::fields().completed().eq(false)),
    )
    .exec(&mut db)
    .await?;
```

Existing `.include(User::fields().todos())` calls keep working
unchanged.

## Behavior

- **Happy path.** For `HasMany`, the preloaded `Vec` contains exactly
  the related rows matching the predicate, in whatever order the engine
  already produces for an unfiltered include. For `HasOne` /
  `BelongsTo`, the relation loads as `Some(record)` if the (single)
  related row matches, otherwise `None`.
- **Empty matches.** A `HasMany` parent with no matching children is
  still returned with an empty preloaded `Vec`. The include filter
  never removes parents.
- **Errors.** A predicate that references fields outside the relation's
  model is a compile error (the typed path machinery already enforces
  this for `.any` / `.all`). Runtime errors from the driver propagate
  as `toasty::Error` exactly as for unfiltered includes.
- **Interaction with parent-side `any` / `all`.** Independent. See the
  composition example above.
- **Interaction with nested `include`.** Out of scope for this design;
  see "Out of scope".
- **Interaction with transactions.** None specific. Filtered includes
  use the same statements as unfiltered ones plus an extra `WHERE`
  clause; transactional semantics are unchanged.

## Edge cases

- **Predicate that is statically `false`.** Loads as empty `Vec` for
  `HasMany`, `None` for `HasOne`. No special-casing.
- **Null comparisons in the predicate.** Standard SQL three-valued
  logic — a row whose comparison evaluates to NULL is excluded, same as
  in `.filter(...)` on a top-level query.
- **`HasOne` predicate matching multiple rows.** Cannot happen: the
  relation type asserts at most one row. If the schema is wrong and
  multiple rows exist, the engine surfaces the same error it does for
  an unfiltered `HasOne` preload.
- **DynamoDB.** The filter is applied as a `FilterExpression` on the
  query that fetches the relation, the same machinery the driver
  already uses for top-level filters. No new capability needed.

## Driver integration

Nothing for driver implementors to do. The change is entirely in the
query engine: lowering attaches the user's predicate to the include
subquery's `WHERE` clause before handing it to the driver. Drivers
receive the same `Operation` variants as today, with one extra
predicate inside the statement.

## Alternatives considered

- **Closure-based include builder** —
  `.include(|u| u.todos.filter(...).limit(10))`. More extensible: a
  natural place to grow `.limit`, `.order_by`, and nested `.include`.
  Rejected for now because each relation would need a generated
  sub-query builder type, doubling the macro surface, and we do not
  yet have concrete demand for limit/order on includes. Revisit if
  those land.
- **Pass a pre-built query as the source** —
  `.include_query(User::fields().todos(), Todo::filter(...))`.
  Cheapest to implement, but introduces a second include entry point
  and reads worse than a single fluent chain. The engine still has to
  inject the parent join, so the user-supplied query is effectively
  just an extra predicate — exactly what `.filter(...)` on the path
  already expresses.
- **Macro DSL** — `include!(todos where !completed)`. Compact but
  introduces a separate parser, hides type errors, and diverges from
  the rest of the query API.

## Open questions

- **Filter expressions that reference parent fields.** Should
  `.filter(Todo::fields().user_id().eq(User::fields().id()))` (or similar
  cross-scope references) be allowed? `.any` / `.all` do not support
  this today. *Deferrable* — start scoped to the relation's own fields
  and lift the restriction later if a real use case appears.
- **Surface the filter in error messages.** When a driver reports a
  query error on an include, do we show the user the include path that
  produced it? *Blocking implementation* of good ergonomics, not
  blocking acceptance.

## Out of scope

- **`.limit` / `.order_by` on includes.** Worth doing, but bigger than
  filtering — the result shape changes (e.g. "top 3 todos per user")
  and SQL drivers need lateral joins or window functions. Separate
  design.
- **Nested `.include` inside a filtered include.** Already supported
  through chained paths; this design does not change it. Filtering at
  deeper levels follows naturally once `.filter(...)` exists on every
  relation path.
- **Aggregations over filtered relations** (`count`, `sum`, etc.).
  Tracked separately; filtering is a prerequisite but not the same
  feature.
