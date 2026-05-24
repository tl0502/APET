//! Energy IPC（#41）— 单 IPC 读取当前精力值。
//!
//! energy 状态机详见 [services/energy.rs](../services/energy.rs)；本模块仅 IPC 端封装。
//!
//! 锁定项：energy 全 transient 不持久（PRD line 1073）；启动 initial=80。

use serde::Serialize;
use tauri::State;

use crate::services::energy::EnergyState;

#[derive(Serialize)]
pub struct EnergySnapshot {
    pub value: u8,
}

#[tauri::command]
pub fn energy_get(state: State<'_, EnergyState>) -> EnergySnapshot {
    EnergySnapshot {
        value: state.get(),
    }
}
