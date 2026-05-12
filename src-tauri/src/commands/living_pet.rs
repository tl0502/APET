// LivingPet IPC commands（#21 收尾增强）
//
// - living_pet_cancel_wander：前端 PetCanvas pointerdown 主键判定后调用 → 取消正在
//   进行的 wander tween（L1 drag interrupt）。tween 在 select! 内监听 token，立即退出
//   保留当前位置（capture current state，不 snap 到段终点）。
//
// 实现极薄：直接 delegate 给 services::living_pet::LivingPet::cancel_wander。

use crate::services::living_pet::LivingPet;
use tauri::{AppHandle, Manager};

/// 取消当前正在进行的 wander tween。无 wander 进行时为 no-op。
/// 不阻塞，不返业务错误（用户输入路径要快）；上锁失败 / state 缺失只 eprintln。
#[tauri::command]
pub async fn living_pet_cancel_wander(app: AppHandle) -> Result<(), String> {
    match app.try_state::<LivingPet>() {
        Some(living) => {
            living.cancel_wander();
            Ok(())
        }
        None => {
            // setup 未完成的极端时序（理论上不会到这里）；不报错让前端流程继续
            eprintln!("[living_pet] cancel_wander: state not managed yet");
            Ok(())
        }
    }
}
