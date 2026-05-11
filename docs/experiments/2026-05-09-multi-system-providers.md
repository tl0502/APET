---
title: 多 system 消息跨 provider 实测
updated: 2026-05-09
related:
  - decisions.md (potential ADR-NNN)
  - lessons.md (pending append)
  - 代码评审报告 B2 项 (stateless-sprouting-sunset.md)
---

# 多 system 消息跨 provider 实测

## 背景

`services/chat/prompt.rs::build_messages` 当前把会话历史里 `role='system'` 的转场注入消息（昵称变更通知）以 `Role::System` 透传给 LLM。研究文献（Persona Drift, arxiv 2402.10962）支持这种"history 中段插 system 重置话术"的方法，但 OpenAI 兼容协议下不同 provider 实测行为不一：

- OpenAI 官方 spec：允许多 system，但内部按位置加权
- DeepSeek：兼容多 system，官方建议只 1 条
- Qwen：行为未文档化
- Ollama：依赖具体模型
- Moonshot：要求 system 在最前

本实验目的：拿到经验数据，决定是保留 `Role::System` 透传，还是回退为 `Role::User` 包装（"（系统通知，请遵守）..."）。

## 实测步骤（每个 preset 各跑一次）

### 准备

1. 启动应用 + 打开设置面板
2. LLM Provider 面板添加目标 preset，填真实 API Key + 默认 model
3. 激活该 provider
4. 昵称面板把 user 昵称设为 `Alice`，确保"昵称变更时通知 AI"开关 ON

### 跑步骤

1. 打开 chat 窗口，新建对话
2. 发 3 轮普通对话（任意内容；目的是让历史里有上下文）：
   - 用户：你好，请记住我喜欢喝拿铁
   - 用户：今天天气怎么样
   - 用户：我刚刚说我喜欢喝什么？（验证 LLM 能引用 Alice 的偏好）
3. 切到设置面板昵称表单，把 user 昵称改为 `Bob`，保存
4. 切回 chat 窗口同会话，发：
   - 用户：你还记得我喜欢喝什么吗？
5. 观察回复中 LLM 用 `Alice` 还是 `Bob` 称呼用户

### 判定标准

记录每个 preset 的回复称呼：

| Preset    | 回复称呼     | 是否切换 |
| --------- | ------------ | -------- |
| OpenAI    | Bob / Alice  | ✓ / ✗    |
| DeepSeek  | Bob / Alice  | ✓ / ✗    |
| Moonshot  | Bob / Alice  | ✓ / ✗    |
| Qwen      | Bob / Alice  | ✓ / ✗    |
| Ollama    | Bob / Alice  | ✓ / ✗    |

### 决策矩阵

| 切换 preset 数 | 决策                                                                 |
| -------------- | -------------------------------------------------------------------- |
| ≥ 4 / 5        | 保留 `Role::System` 透传；本实验文档化为已知方案                     |
| 3 / 5          | 加 ADR-NNN 记录折中：保留为默认 + 提供 config 开关回退到 user 包装   |
| ≤ 2 / 5        | 回退方案：把历史 system 全转 user 包装（"（系统通知）..."）          |

## 结果

> （首次跑完后填入；按上面决策矩阵在 [decisions.md](../decisions.md) 写 ADR-NNN）

| Preset    | 回复称呼 | 是否切换 | 备注 |
| --------- | -------- | -------- | ---- |
| OpenAI    |          |          |      |
| DeepSeek  |          |          |      |
| Moonshot  |          |          |      |
| Qwen      |          |          |      |
| Ollama    |          |          |      |

切换数：__ / 5
最终决策：（待填）

## Follow-up

实测完成后：

1. 上方表格填齐
2. 按决策矩阵在 [docs/decisions.md](../decisions.md) 加 ADR-NNN（如需）
3. 在 [docs/lessons.md](../lessons.md) 追加一条"多 system 跨 provider 兼容"的 lesson（含本文件链接）
4. 若决策为「回退 user 包装」：改 [src-tauri/src/services/chat/prompt.rs](../../src-tauri/src/services/chat/prompt.rs) 的 `build_messages` 逻辑，把历史 system role 转成 `Role::User` 并加显式包装文案
