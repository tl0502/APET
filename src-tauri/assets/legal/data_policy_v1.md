# 数据策略 v1.0

> 这份文档说明 AIPET 桌宠如何处理你的数据。**默默**在灵魂宣誓页用 5 句话讲了承诺；这里给完整版本。
>
> 适用版本：v1.0（与 `consent.version = 1` 绑定）。本策略变更时 `version` 自增，下次启动会请你重新确认。

---

## 1. 数据范围

所有数据**只保存在你本机**，不上传到任何 AIPET 服务器。具体路径：

| 类别 | 位置 |
|---|---|
| 对话历史 / 偏好 / 配置 | `%APPDATA%/com.aipet.desktop/aipet.db`（SQLite） |
| 人格文件（`.soul.md`） | `%APPDATA%/com.aipet.desktop/personas/` |
| 内置人格 | 安装目录 `resources/personas/_builtin/`（只读） |
| 日志（出错时） | `%APPDATA%/com.aipet.desktop/logs/`（默认关，启用后本地保留 7 天） |

AIPET 项目**没有**任何后端服务器、没有云端账号、没有遥测埋点。

## 2. 网络使用

仅在以下情况会主动发起网络请求：

- **聊天**：你配置了 LLM Provider 后，对话会发送到该 Provider（OpenAI / DeepSeek / Moonshot / Qwen / Ollama 等）。请求内容包含：当前对话历史、激活人格的系统提示、你的 API Key。
- **AI 待办拆解 / LLM 小游戏**（M5 阶段功能）：同上路径，使用同一 Provider。
- **自动更新检查**（M3 阶段功能）：默认关，开启后定期向 GitHub Releases API 查询新版本号（不发送任何用户数据）。

**没有 LLM Provider 配置时**，应用完全离线运行：本地小游戏、装扮、心情系统、空闲信号、Onboarding 全部可用。

## 3. 第三方服务

你主动配置的 LLM Provider 各自有隐私政策，AIPET 不代理、不缓存、不转发你的 API Key 到任何中间服务：

- API Key 通过 Windows DPAPI 加密后存于本地（M3 上线；M1 阶段临时明文存放，请勿用于生产 Key）
- 请求直连 Provider 的 base_url（如 `https://api.openai.com/v1`）
- AIPET 不在请求路径中插入任何 telemetry header

## 4. 权限边界

| 权限 | 默认 | 说明 |
|---|---|---|
| 截图 | 关 | 需要时由你在设置中显式开启；M5 阶段才用到 |
| 剪贴板 | 关 | 同上 |
| 麦克风 | **永不申请** | AIPET 没有语音输入功能 |
| 摄像头 | **永不申请** | AIPET 没有视频功能 |
| 应用名 / 窗口标题 | **永不读取** | 主动陪伴功能基于本地空闲信号（鼠标键盘空闲时长），不读屏幕内容 |
| 输入内容 | **永不读取** | 同上，不接 keylogger |
| 网络 | 仅 LLM Provider + 自动更新（可关） | 不会无故发起请求 |

## 5. 数据控制

你随时可以在「设置 → 数据治理」（M3 上线）中：

- 删除全部对话历史 / 删除指定 conversation
- 导出全部数据为 JSON
- 删除指定记忆字段（M3 主体记忆）
- 重置全部数据（等同卸载重装）

直接删除 `%APPDATA%/com.aipet.desktop/` 目录也可以，等同卸载用户数据。

## 6. 加密

- API Key：M1 临时明文 → M3 改 Windows DPAPI 加密
- 对话历史：明文存于 SQLite（无加密；如需保护可对 `%APPDATA%/com.aipet.desktop/` 目录设权限）
- 不上传任何加密 / 明文数据到外部

## 7. 更新

本数据策略变更时：

- 文档版本号 + 1（如 v1.0 → v1.1）
- `consent.version` 自增
- 启动时检测到版本不匹配 → 你会被请回灵魂宣誓页确认（按钮文案改为「我重新确认」）

## 8. 联系方式

本项目是个人 vibecoding 项目，无组织实体。问题 / 建议请到 GitHub Issues：

[https://github.com/tl0502/APET/issues](https://github.com/tl0502/APET/issues)

---

*更新于 2026-05-08。* 如果你读完了，谢谢你愿意花时间。
