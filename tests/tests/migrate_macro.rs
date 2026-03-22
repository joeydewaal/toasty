#![cfg(feature = "sqlite")]

#[tokio::test]
async fn migrate_macro_applies_migrations() {
    let migrator = toasty::migrate!("fixtures/migrate/Toasty.toml");

    let mut db = toasty::Db::builder()
        .connect("sqlite::memory:")
        .await
        .unwrap();

    migrator.exec(&mut db).await.unwrap();
}

#[tokio::test]
async fn migrate_macro_is_idempotent() {
    let mut db = toasty::Db::builder()
        .connect("sqlite::memory:")
        .await
        .unwrap();

    toasty::migrate!("fixtures/migrate/Toasty.toml")
        .exec(&mut db)
        .await
        .unwrap();

    // Applying again skips already-applied migrations
    toasty::migrate!("fixtures/migrate/Toasty.toml")
        .exec(&mut db)
        .await
        .unwrap();
}
