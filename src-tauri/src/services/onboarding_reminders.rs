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

#![allow(dead_code)] // D1 骨架：drain_in_tx / SqliteConnection / Transaction 在 D2 消费。

// D2 将消费这三个 sqlx 类型完成 drain_in_tx；D1 阶段先 import 占位避免 D2 commit 噪声。
#[allow(unused_imports)]
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

// drain_in_tx + instantiate_onboarding_reminders 在 Task D2 / D3 实现。
// D2 会消费 `Transaction<'_, Sqlite>` 把 KV 转 reminder 行；D3 在 lib.rs setup 钩子里 sync 调用。

#[cfg(test)]
mod tests {
    use super::*;

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
}
