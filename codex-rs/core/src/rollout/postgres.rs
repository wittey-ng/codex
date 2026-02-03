use std::io::Error as IoError;
use std::io::ErrorKind;

use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use sqlx::PgPool;
use sqlx::Postgres;
use sqlx::QueryBuilder;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json;
use uuid::Uuid;

pub(crate) const CODEX_ROLLOUT_POSTGRES_URL_ENV: &str = "CODEX_ROLLOUT_POSTGRES_URL";

pub(crate) fn rollout_postgres_url_from_env() -> Option<String> {
    std::env::var(CODEX_ROLLOUT_POSTGRES_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) async fn connect_rollout_pool() -> std::io::Result<PgPool> {
    let Some(url) = rollout_postgres_url_from_env() else {
        return Err(IoError::new(
            ErrorKind::NotFound,
            format!("{CODEX_ROLLOUT_POSTGRES_URL_ENV} is not set"),
        ));
    };

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(url.as_str())
        .await
        .map_err(|err| {
            IoError::other(format!(
                "failed to connect to Postgres for rollout persistence: {err}"
            ))
        })?;

    ensure_schema(&pool).await?;
    Ok(pool)
}

async fn ensure_schema(pool: &PgPool) -> std::io::Result<()> {
    // Keep this fully idempotent so Codex can safely start against an empty DB.
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS codex_rollout_items (
            id BIGSERIAL PRIMARY KEY,
            thread_id UUID NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            item JSONB NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|err| IoError::other(format!("failed to ensure codex_rollout_items table: {err}")))?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS codex_rollout_items_thread_id_id_idx
        ON codex_rollout_items(thread_id, id)
        "#,
    )
    .execute(pool)
    .await
    .map_err(|err| IoError::other(format!("failed to ensure rollout index: {err}")))?;

    Ok(())
}

pub(crate) async fn append_rollout_items(
    pool: &PgPool,
    thread_id: ThreadId,
    items: &[RolloutItem],
) -> std::io::Result<()> {
    if items.is_empty() {
        return Ok(());
    }

    let thread_uuid = thread_uuid(thread_id)?;

    let mut values = Vec::with_capacity(items.len());
    for item in items {
        let json = serde_json::to_value(item)
            .map_err(|err| IoError::other(format!("failed to serialize rollout item: {err}")))?;
        values.push(json);
    }

    let mut tx = pool.begin().await.map_err(|err| {
        IoError::other(format!(
            "failed to begin Postgres transaction for rollout persistence: {err}"
        ))
    })?;

    let mut builder: QueryBuilder<Postgres> =
        QueryBuilder::new("INSERT INTO codex_rollout_items (thread_id, item) ");
    builder.push_values(values, |mut row, item| {
        row.push_bind(thread_uuid);
        row.push_bind(Json(item));
    });

    builder
        .build()
        .execute(&mut *tx)
        .await
        .map_err(|err| IoError::other(format!("failed to insert rollout items: {err}")))?;

    tx.commit()
        .await
        .map_err(|err| IoError::other(format!("failed to commit rollout transaction: {err}")))?;

    Ok(())
}

pub(crate) async fn load_rollout_items(thread_id: ThreadId) -> std::io::Result<Vec<RolloutItem>> {
    let pool = connect_rollout_pool().await?;
    let thread_uuid = thread_uuid(thread_id)?;

    let rows: Vec<Json<serde_json::Value>> = sqlx::query_scalar(
        r#"
        SELECT item
        FROM codex_rollout_items
        WHERE thread_id = $1
        ORDER BY id ASC
        "#,
    )
    .bind(thread_uuid)
    .fetch_all(&pool)
    .await
    .map_err(|err| IoError::other(format!("failed to load rollout items from Postgres: {err}")))?;

    if rows.is_empty() {
        return Err(IoError::new(
            ErrorKind::NotFound,
            format!("no rollout history found in Postgres for thread {thread_id}"),
        ));
    }

    let mut items = Vec::with_capacity(rows.len());
    for Json(value) in rows {
        let item: RolloutItem = serde_json::from_value(value)
            .map_err(|err| IoError::other(format!("failed to decode rollout item: {err}")))?;
        items.push(item);
    }

    Ok(items)
}

fn thread_uuid(thread_id: ThreadId) -> std::io::Result<Uuid> {
    Uuid::parse_str(thread_id.to_string().as_str()).map_err(|err| {
        IoError::new(
            ErrorKind::InvalidInput,
            format!("invalid thread id {thread_id}: {err}"),
        )
    })
}
