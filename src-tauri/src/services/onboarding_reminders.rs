//! Onboarding reminder intent 实例化（#29 闭合 #21 ADR-019 step 4）。
//!
//! 启动期把 onboarding 期写入的 KV `onboarding:reminder_intents` 消化成真实 reminders 表
//! 数据；批量 reminder.create_internal_tx + preferences.delete_tx 在同一 Transaction
//! 内执行 → 原子性（任一失败 tx drop → rollback → 等价"上次没运行过"，下次启动 KV 还在重试）。
//!
//! REMINDER_TEMPLATES 与 `src/types/reminder.ts:80+` 双写约束（lessons.md 条目）：
//! 扩 template 时必须同步两份。drain 时不在表内的 id 静默 skip。
//!
//! 字段映射：Rust `title` <- ts `label`（用户可读名）；`emoji`/`hint`/`label` 等纯 UI
//! 字段不落库（reminders 表无对应列）。

use sqlx::{Sqlite, SqliteConnection, Transaction};

pub(crate) const ONBOARDING_KV_KEY: &str = "onboarding:reminder_intents";

#[derive(Debug, Clone)]
pub(crate) struct ReminderTemplate {
    pub id: &'static str,
    pub title: &'static str,
    pub trigger_type: &'static str,
    pub trigger_spec: &'static str,
    pub priority: &'static str,
}

/// 5 条模板，逐字段 mirror `src/types/reminder.ts` REMINDER_TEMPLATES（双写约束 — lessons.md）。
/// 字段映射：Rust `title` ← ts `label`；emoji / hint 纯 UI 不落库。
const TEMPLATES: &[ReminderTemplate] = &[
    ReminderTemplate {
        id: "water",
        title: "喝水",
        trigger_type: "daily",
        trigger_spec: "*/30 * *",
        priority: "soft",
    },
    ReminderTemplate {
        id: "sit_long",
        title: "久坐起身",
        trigger_type: "daily",
        trigger_spec: "*/60 * *",
        priority: "soft",
    },
    ReminderTemplate {
        id: "focus_study",
        title: "学习专注",
        trigger_type: "daily",
        trigger_spec: "09:00",
        priority: "hard",
    },
    ReminderTemplate {
        id: "stretch",
        title: "伸展活动",
        trigger_type: "daily",
        trigger_spec: "*/90 * *",
        priority: "soft",
    },
    ReminderTemplate {
        id: "early_sleep",
        title: "早睡",
        trigger_type: "daily",
        trigger_spec: "23:00",
        priority: "soft",
    },
];

/// 解析 KV 值，返回 None 表示"无需 instantiate"（kv 不存在 / null sentinel / [] / 无效 JSON）。
///
/// 设计取舍：无效 JSON 不抛错只 None — 启动期 drain 容错优先，下次启动 KV 仍在表内可重试；
/// 抛错会让整个 setup 钩子失败影响主态可达性。
fn parse_intent_ids(raw: Option<&str>) -> Option<Vec<String>> {
    let s = raw?;
    if s == "null" || s.is_empty() {
        return None;
    }
    let parsed: Result<Vec<String>, _> = serde_json::from_str(s);
    match parsed {
        Ok(v) if v.is_empty() => None,
        Ok(v) => Some(v),
        Err(_) => None,
    }
}

/// 按 id 反查模板；未注册 id（前端/后端版本错位）返回 None，调用方静默 skip。
fn lookup_template(id: &str) -> Option<&'static ReminderTemplate> {
    TEMPLATES.iter().find(|t| t.id == id)
}

// instantiate_onboarding_reminders + lib.rs setup hook 在 Task D3 实现（sync entry）。

/// 读取单个 KV 值 — `drain_in_tx` 唯一调用方；接 `&mut SqliteConnection` 让它可在 tx
/// 内 reborrow（`&mut **tx`）。返回 None 表示 KV 不存在。
async fn read_kv(conn: &mut SqliteConnection, key: &str) -> Result<Option<String>, String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM memory WHERE key=?")
        .bind(key)
        .fetch_optional(conn)
        .await
        .map_err(|e| format!("read kv {key}: {e}"))?;
    Ok(row.map(|r| r.0))
}

/// 批量原子化 drain：所有 reminder.create_internal_tx + KV delete 共享调用方 tx；
/// 任一步骤 `?` 失败 → 调用方 drop(tx) → 整组自动 rollback → 等价"上次没运行过"，
/// 下次启动 KV 仍在表内可重试（idempotent at boundary）。
///
/// 注意：本函数 **只接 tx，不自己 commit** — commit / rollback 由调用方 D3 entry fn 决定。
/// 这是 #29 spec §5.2 tx-injection 模式的第二个 use case（第一个是 todo.rs C9）。
///
/// 行为：
/// - KV 不存在 / null / [] / 无效 JSON → 删 KV（若存在）后返回 Ok，无 reminder 创建。
/// - id 不在 TEMPLATES → 静默 skip + eprintln warn（前后端版本错位容错）。
/// - reminder.create_internal_tx 失败 → 直接传播 → 调用方 rollback。
///
/// `#[allow(dead_code)]`: D3 (`instantiate_onboarding_reminders` + lib.rs setup hook)
/// 会是首个真实调用方；移除本 allow 是 D3 commit 自检清单的一项（届时 read_kv /
/// parse_intent_ids / lookup_template / TEMPLATES / ReminderTemplate / ONBOARDING_KV_KEY
/// 都会随调用链变 live，无需各自加 allow）。
#[allow(dead_code)]
pub(crate) async fn drain_in_tx(tx: &mut Transaction<'_, Sqlite>) -> Result<(), String> {
    let raw = read_kv(&mut **tx, ONBOARDING_KV_KEY).await?;
    let ids = match parse_intent_ids(raw.as_deref()) {
        Some(ids) => ids,
        None => {
            // 脏数据 / null / [] / 无 KV → 收尾删 KV（若存在）后返回。
            // KV 不存在时 delete_tx 是 no-op（DELETE WHERE 不存在的 key 行影响 0）。
            crate::services::preferences::delete_tx(tx, ONBOARDING_KV_KEY)
                .await
                .map_err(|e| format!("delete kv: {e}"))?;
            return Ok(());
        }
    };

    for id in &ids {
        let template = match lookup_template(id) {
            Some(t) => t,
            None => {
                eprintln!("[onboarding-reminders] skipping unknown template id: {id}");
                continue;
            }
        };
        crate::services::reminder::create_internal_tx(
            tx,
            crate::services::reminder::CreateInput {
                title: template.title.into(),
                trigger_type: template.trigger_type.into(),
                trigger_spec: template.trigger_spec.into(),
                priority: Some(template.priority.into()),
            },
        )
        .await
        .map_err(|e| format!("create reminder {id}: {e}"))?;
    }

    crate::services::preferences::delete_tx(tx, ONBOARDING_KV_KEY)
        .await
        .map_err(|e| format!("delete kv: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::test_db::fresh_db;
    use sqlx::Connection;

    #[test]
    fn parse_array_returns_ids() {
        let v = parse_intent_ids(Some(r#"["water","sit_long"]"#));
        assert_eq!(v, Some(vec!["water".to_string(), "sit_long".to_string()]));
    }

    #[test]
    fn parse_null_sentinel_returns_none() {
        assert_eq!(parse_intent_ids(Some("null")), None);
    }

    #[test]
    fn parse_empty_array_returns_none() {
        assert_eq!(parse_intent_ids(Some("[]")), None);
    }

    #[test]
    fn parse_invalid_json_returns_none() {
        assert_eq!(parse_intent_ids(Some("garbage")), None);
    }

    #[test]
    fn parse_missing_kv_returns_none() {
        assert_eq!(parse_intent_ids(None), None);
    }

    #[test]
    fn lookup_known_id_returns_template() {
        let t = lookup_template("water").expect("water template must exist");
        assert_eq!(t.id, "water");
        assert_eq!(t.title, "喝水");
        assert_eq!(t.trigger_type, "daily");
        assert_eq!(t.trigger_spec, "*/30 * *");
        assert_eq!(t.priority, "soft");
    }

    #[test]
    fn lookup_unknown_id_returns_none() {
        assert!(lookup_template("unknown_xyz").is_none());
    }

    #[tokio::test]
    async fn drain_in_tx_creates_reminders_and_deletes_kv() {
        let (_dir, mut conn) = fresh_db().await;
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO memory (key,value,source,updated_at) VALUES (?, ?, 'inferred', ?)")
            .bind(ONBOARDING_KV_KEY)
            .bind(r#"["water","sit_long"]"#)
            .bind(&now)
            .execute(&mut conn)
            .await
            .unwrap();

        let mut tx = conn.begin().await.unwrap();
        drain_in_tx(&mut tx).await.unwrap();
        tx.commit().await.unwrap();

        // reminders 表 +2
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(count.0, 2);
        // KV 已删
        let kv: Option<(String,)> = sqlx::query_as("SELECT value FROM memory WHERE key=?")
            .bind(ONBOARDING_KV_KEY)
            .fetch_optional(&mut conn)
            .await
            .unwrap();
        assert!(kv.is_none());
    }

    #[tokio::test]
    async fn drain_in_tx_skips_unknown_ids() {
        let (_dir, mut conn) = fresh_db().await;
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO memory (key,value,source,updated_at) VALUES (?, ?, 'inferred', ?)")
            .bind(ONBOARDING_KV_KEY)
            .bind(r#"["water","unknown_xyz","sit_long"]"#)
            .bind(&now)
            .execute(&mut conn)
            .await
            .unwrap();

        let mut tx = conn.begin().await.unwrap();
        drain_in_tx(&mut tx).await.unwrap();
        tx.commit().await.unwrap();

        // 只 water + sit_long 创建（unknown 静默 skip）
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(count.0, 2);
    }

    #[tokio::test]
    async fn drain_in_tx_rollback_keeps_kv_and_reminders_clean() {
        // 验证 caller drop(tx) 不 commit → reminders + KV 都未变
        let (_dir, mut conn) = fresh_db().await;
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO memory (key,value,source,updated_at) VALUES (?, ?, 'inferred', ?)")
            .bind(ONBOARDING_KV_KEY)
            .bind(r#"["water"]"#)
            .bind(&now)
            .execute(&mut conn)
            .await
            .unwrap();

        let mut tx = conn.begin().await.unwrap();
        drain_in_tx(&mut tx).await.unwrap();
        // 不 commit，直接 drop → rollback
        drop(tx);

        // KV 应该仍在（未 commit）
        let kv: Option<(String,)> = sqlx::query_as("SELECT value FROM memory WHERE key=?")
            .bind(ONBOARDING_KV_KEY)
            .fetch_optional(&mut conn)
            .await
            .unwrap();
        assert!(kv.is_some(), "KV should still exist after rollback");
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reminders")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(
            count.0, 0,
            "no reminders should be persisted after rollback"
        );
    }
}
