use crdb_harness::CockroachInstance;
use crdb_harness::CockroachStarterBuilder;
use sqlx::{Acquire, PgPool, Pool, Postgres};

async fn create_crdb_and_sqlx_pool() -> anyhow::Result<(CockroachInstance, Pool<Postgres>)> {
    let crdb_builder = CockroachStarterBuilder::new().build()?;
    let db = crdb_builder.start().await?;
    let pool = PgPool::connect(&db.pg_config().url).await?;

    Ok((db, pool))
}

#[tokio::test]
async fn nested_transaction_for_repro() {
    let (_db, pool) = create_crdb_and_sqlx_pool().await.unwrap();
    let mut conn = pool.acquire().await.unwrap();

    // Context:
    //
    // https://www.cockroachlabs.com/docs/stable/advanced-client-side-transaction-retries#customizing-the-retry-savepoint-name
    sqlx::raw_sql("SET force_savepoint_restart=true")
        .execute(&mut *conn)
        .await
        .unwrap();

    // BEGIN
    let mut txn = conn.begin().await.unwrap();
    // SAVEPOINT _sqlx_savepoint_1
    let mut txn2 = txn.begin().await.unwrap();
    // SAVEPOINT _sqlx_savepoint_2
    //
    // This SHOULD fail, because nested crdb transactions are disallowed.
    // However, in reality, it silently succeeds.
    let txn3 = txn2.begin().await.unwrap();

    // XXX: This fails, because "_sqlx_savepoint_2" cannot be found.
    txn3.commit().await.unwrap();
    txn2.commit().await.unwrap();
    txn.commit().await.unwrap();
}
