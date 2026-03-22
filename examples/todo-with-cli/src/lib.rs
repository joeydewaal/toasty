#[derive(Debug, toasty::Model)]
pub struct User {
    #[key]
    #[auto]
    pub id: uuid::Uuid,

    pub name: String,

    #[unique]
    pub email: String,

    #[has_many]
    pub todos: toasty::HasMany<Todo>,
}

#[derive(Debug, toasty::Model)]
pub struct Todo {
    #[key]
    #[auto]
    pub id: uuid::Uuid,

    #[index]
    pub user_id: uuid::Uuid,

    #[belongs_to(key = user_id, references = id)]
    pub user: toasty::BelongsTo<User>,

    #[index]
    pub title: String,

    pub description: Option<String>,

    pub completed: bool,

    #[has_many]
    pub tags: toasty::HasMany<Tag>,
}

#[derive(Debug, toasty::Model)]
pub struct Tag {
    #[key]
    #[auto]
    pub id: uuid::Uuid,

    #[index]
    pub todo_id: uuid::Uuid,

    #[belongs_to(key = todo_id, references = id)]
    pub todo: toasty::BelongsTo<Todo>,

    #[index]
    pub name: String,
}

/// Helper function to create a database instance with the schema
pub async fn create_db() -> toasty::Result<toasty::Db> {
    let db = toasty::Db::builder()
        .register::<User>()
        .register::<Todo>()
        .register::<Tag>()
        .connect("sqlite:./test.db")
        .await?;

    Ok(db)
}
