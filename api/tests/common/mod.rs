use sqlx::PgPool;

#[allow(dead_code)]
pub async fn setup_test_db() -> PgPool {
    let database_url =
        std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
            "postgres://dbsaas:testpassword@localhost:5432/dbsaas_test".to_string()
        });

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    // Run migrations
    let migrations = [
        include_str!("../../migrations/001_create_users.sql"),
        include_str!("../../migrations/002_create_databases.sql"),
        include_str!("../../migrations/003_create_invitations.sql"),
        include_str!("../../migrations/004_create_migrations.sql"),
    ];

    for sql in &migrations {
        let _ = sqlx::query(sql).execute(&pool).await;
    }

    pool
}
