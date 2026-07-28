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

#[driver_test(id(ID), scenario(crate::scenarios::user_with_age))]
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
        .returning_all()
        .exec(&mut db)
        .await?;

    assert_struct!(users, #(
        { age: 1, name: "Alice" },
        { age: 1, name: "Bob" },
    ));

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
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
        .returning_all()
        .exec(&mut db)
        .await?;

    assert_struct!(todos, #(
        { title: "updated" },
        { title: "updated" },
    ));

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn query_update_return_model_leaves_relations_unloaded(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let user = toasty::create!(User {
        name: "Alice",
        todos: [{ title: "write tests" }],
    })
    .exec(&mut db)
    .await?;

    let users = User::update_by_id(user.id)
        .name("Alicia")
        .returning_all()
        .exec(&mut db)
        .await?;
    let user = &users[0];

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

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn query_update_relation_only_returns_model(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let user = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    let returned = User::update_by_id(user.id)
        .todos(toasty::stmt::insert(Todo::create().title("write tests")))
        .returning_all()
        .exec(&mut db)
        .await?;

    assert_eq!(returned[0].id, user.id);
    assert!(returned[0].todos.is_unloaded());
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
    let user = toasty::create!(User {
        id: uuid::Uuid::from_u128(1),
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    let count = User::update_by_id(user.id)
        .name("Alicia")
        .exec(&mut db)
        .await?;
    assert_eq!(count, 1);

    let id = uuid::Uuid::from_u128(2);

    let count = User::update_by_id(id).name("missing").exec(&mut db).await?;

    assert_eq!(count, 0);
    assert!(User::get_by_id(&mut db, &id).await.is_err());

    Ok(())
}

#[driver_test]
pub async fn query_update_missing_exact_key_returns_no_models(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: uuid::Uuid,

        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    let all_id = uuid::Uuid::from_u128(3);
    let one_id = uuid::Uuid::from_u128(4);

    let users = User::update_by_id(all_id)
        .name("missing")
        .returning_all()
        .exec(&mut db)
        .await?;
    assert!(users.is_empty());
    assert!(User::get_by_id(&mut db, &all_id).await.is_err());

    let users = User::update_by_id(one_id)
        .name("missing")
        .returning_all()
        .exec(&mut db)
        .await?;
    assert!(users.is_empty());
    assert!(User::get_by_id(&mut db, &one_id).await.is_err());

    Ok(())
}
