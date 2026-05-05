#!/usr/bin/env bash
# 一次性批量创建 AIPET 项目的 GitHub labels。
# 使用前提：已 cd 到接入了 origin 的 git 仓库，且 gh auth login 完成。
# 用法：bash docs/scripts/init-labels.sh
# 幂等：--force 让重复执行也安全（覆盖颜色/描述）。

set -euo pipefail

echo ">>> 创建 type:* labels（6 个）"
gh label create "type:feat"     --color "1f883d" --description "新功能"               --force
gh label create "type:fix"      --color "d1242f" --description "修 bug"               --force
gh label create "type:refactor" --color "8957e5" --description "重构（不改行为）"     --force
gh label create "type:spike"    --color "fbca04" --description "调研 / 验证"          --force
gh label create "type:chore"    --color "656d76" --description "工具链 / 依赖 / 配置" --force
gh label create "type:docs"     --color "0969da" --description "仅文档变更"           --force

echo ">>> 创建 module:* labels（17 + infra = 18 个）"
gh label create "module:A-shell"     --color "fef2c0" --description "桌宠壳层" --force
gh label create "module:B-chat"      --color "fef2c0" --description "对话"     --force
gh label create "module:C-reminder"  --color "fef2c0" --description "提醒"     --force
gh label create "module:D-pomodoro"  --color "fef2c0" --description "番茄钟"   --force
gh label create "module:E-todo"      --color "fef2c0" --description "待办"     --force
gh label create "module:F-memory"    --color "fef2c0" --description "记忆"     --force
gh label create "module:G-settings"  --color "fef2c0" --description "设置"     --force
gh label create "module:H-persona"   --color "fef2c0" --description "人格系统" --force
gh label create "module:I-living"    --color "fef2c0" --description "生命感"   --force
gh label create "module:J-care"      --color "fef2c0" --description "情境关心" --force
gh label create "module:K-bosskey"   --color "fef2c0" --description "摸鱼模式" --force
gh label create "module:L-filedrop"  --color "fef2c0" --description "文件拖入" --force
gh label create "module:M-pledge"    --color "fef2c0" --description "灵魂宣誓" --force
gh label create "module:N-interact"  --color "fef2c0" --description "物理交互" --force
gh label create "module:O-wardrobe"  --color "fef2c0" --description "装扮"     --force
gh label create "module:P-voice"     --color "fef2c0" --description "声音表情" --force
gh label create "module:Q-game"      --color "fef2c0" --description "小游戏"   --force
gh label create "module:infra"       --color "c5def5" --description "基础设施" --force

echo ">>> 创建 priority:* labels（2 个）"
gh label create "priority:p0" --color "b60205" --description "阻塞当前 milestone" --force
gh label create "priority:p1" --color "ff9f1c" --description "重要但不阻塞"       --force

echo ">>> 创建 status:* labels（1 个）"
gh label create "status:blocked" --color "000000" --description "被外部因素阻塞" --force

echo ""
echo "✓ 完成：6 type + 18 module + 2 priority + 1 status = 27 labels"
echo ""
echo "下一步：建 milestones（详见 docs/github-workflow.md §8.1）"
