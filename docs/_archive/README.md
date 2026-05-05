# 历史归档目录

本目录用于保留 v0.x 系列历史版本（实施前的迭代草案），**实施期不应参考**。

---

## 当前归档状态

> 本目录目前为空。项目主要历史信息以 git 提交记录为准。

如未来需要离线快照，约定如下命名：

```
_archive/
├── prd/
│   ├── prd-v0.1.md
│   ├── prd-v0.3.md
│   └── ...
├── architecture/
│   ├── system-architecture-v0.1.md
│   └── ...
├── flows/
├── persona/
└── telemetry-uat/
```

---

## 历史版本清单（出处：CHANGELOG）

| 文档 | 已归档版本 |
|---|---|
| PRD | v0.1, v0.3, v0.4, v0.5, v0.6, v0.7 |
| 架构 | v0.1, v0.2, v0.3, v0.4 |
| flows | v0.3, v0.4, v0.5, v0.6 |
| 埋点 UAT | v0.3, v0.5, v0.6 |
| 人格设计 | v0.1, v0.2 |

完整版本演化轨迹见 [../CHANGELOG.md](../CHANGELOG.md)。

---

## 归档原则

1. **入档触发**：基线文档升 MAJOR（v1.x → v2.0）时，把 v1.x 整体归档到此处。
2. **入档形式**：原文件不动复制为 `<doc>-v<old>.md`，frontmatter 加 `status: superseded`。
3. **不入档**：MINOR 升级（v1.0 → v1.1）不入档；变化通过 git 历史 + CHANGELOG 追溯。
4. **不修改**：归档文件一经入档不再修改，错误也不修，保持历史快照属性。

---

## 工具配置提示

- **Obsidian 用户**：`Settings → Files and links → Excluded files`，添加 `_archive/` 避免污染搜索与图谱。
- **Git 检索**：用 `git log --follow <path>` 查看具体文件的历史版本演化。
- **VS Code 用户**：可在 `.vscode/settings.json` 添加 `"search.exclude": { "**/_archive": true }`。
