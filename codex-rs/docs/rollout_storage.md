# Rollout storage (JSONL vs PostgreSQL)

Codex persists a thread's “rollout” as an ordered stream of `RolloutItem` events (session metadata, user/assistant messages, tool calls, etc.). This history is what Codex replays/resumes and what it re-sends to the model when building context for the next turn.

## Default: JSONL rollouts

By default, Codex appends rollout items to a JSONL file under the Codex home directory:

- `~/.codex/sessions/YYYY/MM/DD/rollout-<timestamp>-<thread_id>.jsonl`

Each line contains:

- a top-level `timestamp` field, plus
- the serialized `RolloutItem` flattened at the top level.

## PostgreSQL: `CODEX_ROLLOUT_POSTGRES_URL`

If `CODEX_ROLLOUT_POSTGRES_URL` is set (and non-empty), Codex switches rollout persistence from JSONL files to PostgreSQL.

- Env var: `CODEX_ROLLOUT_POSTGRES_URL`
- Value: a PostgreSQL connection string usable by `sqlx` (e.g. `postgres://user:pass@host:5432/dbname`)

On first use, Codex creates an idempotent schema:

```sql
CREATE TABLE IF NOT EXISTS codex_rollout_items (
  id BIGSERIAL PRIMARY KEY,
  thread_id UUID NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  item JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS codex_rollout_items_thread_id_id_idx
  ON codex_rollout_items(thread_id, id);
```

### Stored format

`codex_rollout_items.item` is the JSON serialization of `codex_protocol::protocol::RolloutItem`.

This matches the JSONL line format **minus** the top-level `timestamp` field. Ordering is preserved by `id` (`ORDER BY id ASC`), and `created_at` captures insertion time.

### Querying a thread’s history

```sql
SELECT item
FROM codex_rollout_items
WHERE thread_id = '00000000-0000-0000-0000-000000000000'
ORDER BY id ASC;
```

### Web server resume/fork behavior

When `CODEX_ROLLOUT_POSTGRES_URL` is set for `codex-web-server`, these endpoints load history from PostgreSQL:

- `POST /api/v2/threads/{id}/resume`
- `POST /api/v2/threads/{id}/fork`

## Notes / limitations

- Codex’s local SQLite “state db” is designed around JSONL rollouts; PostgreSQL rollouts currently do not backfill or update that SQLite metadata store.
