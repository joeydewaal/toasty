use crate::prelude::*;

#[driver_test(id(ID), scenario(crate::scenarios::user_with_age), requires(scan))]
pub async fn query_update_returns_affected_count(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(User::[
        { name: "Alice", age: 0 },
        { name: "Bob", age: 0 },
    ])
    .exec(&mut db)
    .await?;

    let count = User::filter_by_age(0).update().age(1).exec(&mut db).await?;
    assert_eq!(count, 2);

    let count = User::filter_by_age(0).update().age(1).exec(&mut db).await?;
    assert_eq!(count, 0);

    Ok(())
}

#[driver_test(
    id(ID),
    scenario(crate::scenarios::user_with_age),
    requires(update_returning_new)
)]
pub async fn query_update_return_all(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(User::[
        { name: "Alice", age: 0 },
        { name: "Bob", age: 0 },
    ])
    .exec(&mut db)
    .await?;

    let users = User::filter_by_age(0)
        .update()
        .age(1)
        .return_all()
        .exec(&mut db)
        .await?;

    assert_struct!(users, #(
        { age: 1, name: "Alice" },
        { age: 1, name: "Bob" },
    ));

    Ok(())
}

#[driver_test(
    id(ID),
    scenario(crate::scenarios::user_with_age),
    requires(update_returning_new)
)]
pub async fn query_update_return_first_and_one(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(User::[
        { name: "Alice", age: 0 },
        { name: "Bob", age: 0 },
    ])
    .exec(&mut db)
    .await?;

    let user = User::filter_by_age(0)
        .update()
        .age(1)
        .return_first()
        .exec(&mut db)
        .await?;
    assert!(user.is_some());
    assert_eq!(User::filter_by_age(1).exec(&mut db).await?.len(), 2);

    let user = User::filter_by_age(1)
        .update()
        .age(0)
        .return_one()
        .exec(&mut db)
        .await?;
    assert_eq!(user.age, 0);
    assert_eq!(User::filter_by_age(0).exec(&mut db).await?.len(), 2);

    let user = User::filter_by_age(1)
        .update()
        .age(0)
        .return_first()
        .exec(&mut db)
        .await?;
    assert_none!(user);

    let error = assert_err!(
        User::filter_by_age(1)
            .update()
            .age(0)
            .return_one()
            .exec(&mut db)
            .await
    );
    assert!(error.is_record_not_found());

    Ok(())
}

#[driver_test(
    id(ID),
    scenario(crate::scenarios::user_with_age),
    requires(update_returning_new)
)]
pub async fn query_update_return_first_uses_query_order(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(User::[
        { name: "Alice", age: 0 },
        { name: "Bob", age: 0 },
    ])
    .exec(&mut db)
    .await?;

    let expected = User::filter_by_age(0)
        .order_by(User::fields().name().desc())
        .first()
        .exec(&mut db)
        .await?
        .unwrap();

    let user = User::filter_by_age(0)
        .order_by(User::fields().name().desc())
        .update()
        .age(1)
        .return_first()
        .exec(&mut db)
        .await?
        .unwrap();

    assert_eq!(user.id, expected.id);
    assert_eq!(User::filter_by_age(1).exec(&mut db).await?.len(), 2);

    Ok(())
}

#[driver_test(
    id(ID),
    scenario(crate::scenarios::in_list_item),
    requires(and(sql, update_returning_new))
)]
pub async fn query_update_return_first_uses_pre_update_backend_order(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(Item::[
        { name: "not-null", n: 0, bio: Some("z".to_string()) },
        { name: "null", n: 0, bio: None },
    ])
    .exec(&mut db)
    .await?;

    let expected = Item::all()
        .order_by(Item::fields().bio().asc())
        .first()
        .exec(&mut db)
        .await?
        .unwrap();

    let returned = Item::all()
        .order_by(Item::fields().bio().asc())
        .update()
        .bio(Some("updated".to_string()))
        .return_first()
        .exec(&mut db)
        .await?
        .unwrap();

    assert_eq!(returned.id, expected.id);

    Ok(())
}

#[driver_test(
    scenario(crate::scenarios::fixed_item_name),
    requires(and(sql, update_returning_new))
)]
pub async fn ordered_returns_reject_primary_key_updates(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(Item::[
        { id: 1, name: "first" },
        { id: 3, name: "one" },
    ])
    .exec(&mut db)
    .await?;

    let error = assert_err!(
        Item::all()
            .filter(Item::fields().id().eq(1))
            .order_by(Item::fields().id().asc())
            .update()
            .id(2)
            .return_first()
            .exec(&mut db)
            .await
    );
    assert!(error.is_unsupported_feature());

    let error = assert_err!(
        Item::all()
            .filter(Item::fields().id().eq(3))
            .order_by(Item::fields().id().asc())
            .update()
            .id(4)
            .return_one()
            .exec(&mut db)
            .await
    );
    assert!(error.is_unsupported_feature());

    let items = Item::all().exec(&mut db).await?;
    assert_eq_unordered!(items.iter().map(|item| &item.id), [&1, &3]);

    Ok(())
}

#[driver_test(
    id(ID),
    scenario(crate::scenarios::composite_has_many_belongs_to),
    requires(update_returning_new)
)]
pub async fn query_update_return_all_by_partial_composite_key(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let user = toasty::create!(User {
        name: "Alice",
        todos: [{ title: "one" }, { title: "two" }],
    })
    .exec(&mut db)
    .await?;

    let todos = Todo::filter_by_user_id(user.id)
        .update()
        .title("updated")
        .return_all()
        .exec(&mut db)
        .await?;

    assert_struct!(todos, #(
        { title: "updated" },
        { title: "updated" },
    ));

    Ok(())
}

#[driver_test(
    id(ID),
    scenario(crate::scenarios::user_with_age),
    requires(and(sql, update_returning_new))
)]
pub async fn query_update_return_one_error_rolls_back_batch(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(User {
        name: "Alice",
        age: 0
    })
    .exec(&mut db)
    .await?;

    let error = assert_err!(
        toasty::batch((
            User::filter_by_name("Alice").update().age(1),
            User::filter_by_name("missing").update().age(1).return_one(),
        ))
        .exec(&mut db)
        .await
    );
    assert!(error.is_record_not_found());

    let user = User::filter_by_name("Alice").get(&mut db).await?;
    assert_eq!(user.age, 0);

    Ok(())
}

#[driver_test(
    id(ID),
    scenario(crate::scenarios::user_with_age),
    requires(update_returning_old)
)]
pub async fn query_update_return_old(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let user = toasty::create!(User {
        name: "Alice",
        age: 0,
    })
    .exec(&mut db)
    .await?;

    let previous = User::update_by_id(user.id)
        .name("Bob")
        .return_one()
        .returning_old()
        .exec(&mut db)
        .await?;
    assert_struct!(previous, _ { name: "Alice", age: 0, .. });

    let user = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(user.name, "Bob");

    Ok(())
}

#[driver_test(
    id(ID),
    scenario(crate::scenarios::user_with_age),
    requires(update_returning_new)
)]
pub async fn query_update_return_new_explicit(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let user = toasty::create!(User {
        name: "Alice",
        age: 0,
    })
    .exec(&mut db)
    .await?;

    let updated = User::update_by_id(user.id)
        .name("Bob")
        .return_one()
        .returning_new()
        .exec(&mut db)
        .await?;
    assert_eq!(updated.name, "Bob");

    Ok(())
}

#[driver_test(
    id(ID),
    scenario(crate::scenarios::has_many_belongs_to),
    requires(update_returning_new)
)]
pub async fn query_update_return_model_leaves_relations_unloaded(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let user = toasty::create!(User {
        name: "Alice",
        todos: [{ title: "write tests" }],
    })
    .exec(&mut db)
    .await?;

    let user = User::update_by_id(user.id)
        .name("Alicia")
        .return_one()
        .exec(&mut db)
        .await?;

    assert_eq!(user.name, "Alicia");
    assert!(user.todos.is_unloaded());

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn query_update_relation_only_returns_zero(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let user = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    let count = User::update_by_id(user.id)
        .todos(toasty::stmt::insert(Todo::create().title("write tests")))
        .exec(&mut db)
        .await?;

    assert_eq!(count, 0);
    assert_eq!(user.todos().exec(&mut db).await?.len(), 1);

    Ok(())
}

#[driver_test(
    id(ID),
    scenario(crate::scenarios::has_many_belongs_to),
    requires(update_returning_new)
)]
pub async fn query_update_relation_only_returns_model(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let user = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    let returned = User::update_by_id(user.id)
        .todos(toasty::stmt::insert(Todo::create().title("write tests")))
        .return_one()
        .exec(&mut db)
        .await?;

    assert_eq!(returned.id, user.id);
    assert!(returned.todos.is_unloaded());
    assert_eq!(user.todos().exec(&mut db).await?.len(), 1);

    Ok(())
}

#[driver_test]
pub async fn query_update_missing_exact_key_returns_zero(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: uuid::Uuid,

        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    let id = uuid::Uuid::new_v4();

    let count = User::update_by_id(id).name("missing").exec(&mut db).await?;

    assert_eq!(count, 0);
    assert!(User::get_by_id(&mut db, &id).await.is_err());

    Ok(())
}

#[driver_test(requires(update_returning_new))]
pub async fn query_update_missing_exact_key_returns_no_models(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: uuid::Uuid,

        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    let all_id = uuid::Uuid::new_v4();
    let one_id = uuid::Uuid::new_v4();

    let users = User::update_by_id(all_id)
        .name("missing")
        .return_all()
        .exec(&mut db)
        .await?;
    assert!(users.is_empty());
    assert!(User::get_by_id(&mut db, &all_id).await.is_err());

    let error = assert_err!(
        User::update_by_id(one_id)
            .name("missing")
            .return_one()
            .exec(&mut db)
            .await
    );
    assert!(error.is_record_not_found());
    assert!(User::get_by_id(&mut db, &one_id).await.is_err());

    Ok(())
}

#[driver_test(
    id(ID),
    scenario(crate::scenarios::user_unique_email_with_name),
    requires(and(update_returning_new, not(update_returning_unique)))
)]
pub async fn query_update_return_unique_field_rejected_before_writes(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(User::[
        { email: "same@example.com", name: "Alice" },
        { email: "other@example.com", name: "Bob" },
    ])
    .exec(&mut db)
    .await?;

    let error = assert_err!(
        User::all()
            .update()
            .email("same@example.com")
            .return_all()
            .exec(&mut db)
            .await
    );
    assert!(error.is_unsupported_feature());

    let users = User::all().exec(&mut db).await?;
    assert_struct!(users, #(
        { email: "same@example.com", name: "Alice" },
        { email: "other@example.com", name: "Bob" },
    ));

    Ok(())
}
