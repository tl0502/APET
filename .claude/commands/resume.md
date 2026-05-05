---
description: 召回 AI 桌宠项目上下文，进入开发模式
---

# /resume — 项目上下文召回

读取以下信息并向我汇报当前项目状态：

## 必读（按顺序）

1. `docs/STATUS.md` —— 当前进度快照
2. `docs/README.md` —— 文档地图

## 按需读

3. `docs/roadmap/development-roadmap.md` 中**当前 milestone**对应章节（依据 STATUS 中「当前 milestone」字段）
4. **最近 5 个开放 issue**（如果远端已接入）：

   ```bash
   gh issue list --state=open --limit=5 --json number,title,labels,milestone
   ```

5. `docs/decisions.md`（如果用户的下一步任务涉及已决项，确认不要重新讨论）

## 远端未接入时

如果 `git remote -v` 没有 origin / `gh` 命令报错 / 网络拦截访问不到，跳过第 4 步，只读 STATUS + README + roadmap。

## 汇报格式（< 200 字）

- **当前阶段**：（M? 第?周 / 立项准备 / 自测期）
- **上次到哪**：（一句话）
- **下一步**：（一句话，引用 issue 编号如有）
- **阻塞 / 决策待办**：（如有）
- **建议**：（一句话，可选）

## 不要做

- ✗ 不要重述全部文档内容
- ✗ 不要把所有 issue 都拉进 context（只看前 5 个）
- ✗ 不要主动建议加新流程 / 新工具（用户在 vibecoding，不要打扰）
