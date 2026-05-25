// Kernel — Companion Agent Runtime 7 件套 (Phase A0 子集: safety_guard / state_store /
// permission_service / grant_broker / crypto / lifecycle_manager)。
// Spec: docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md v3 §4.2 / §8。
//
// Phase A0 不上 EventBus / Scheduler / 完整 SubsystemRegistry。

pub mod crypto;
pub mod grant_broker;
pub mod lifecycle_manager;
pub mod permission_service;
pub mod repos;
pub mod runtime;
pub mod safety_guard;
pub mod state_store;

pub use runtime::Kernel;
