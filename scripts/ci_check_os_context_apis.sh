#!/usr/bin/env bash
# Phase A0 CI 黑名单: src-tauri crate 不允许出现 OS context API。
# Spec: Constitution #9 (Privacy by Default) / §14.1 A0 DoD。
# 仅允许出现在 docs/ 与 plans/ 注释中, 不允许在 .rs 源代码 import / 调用。

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/src-tauri/src"

BLACKLIST=(
    "GetForegroundWindow"
    "GetWindowTextW"
    "GetWindowTextA"
    "BitBlt"
    "getUserMedia"
    "MediaRecorder"
    "GetCursorPos"
    "ReadClipboardText"
)

FAIL=0
for needle in "${BLACKLIST[@]}"; do
    # 排除 permission_service.rs 顶部的黑名单文档注释 (它本身就是文档化的禁止清单)
    HITS=$(grep -rn "$needle" "$SRC" --include='*.rs' \
        | grep -v "^[^:]*:[0-9]*://.*[❌]" \
        || true)
    if [ -n "$HITS" ]; then
        echo "FAIL: blacklisted API '$needle' found in src-tauri:"
        echo "$HITS"
        echo ""
        FAIL=1
    fi
done

if [ $FAIL -eq 1 ]; then
    echo "❌ Constitution #9 violation: Privacy by Default. OS context APIs forbidden in Phase A0."
    echo "   See docs/superpowers/specs/2026-05-24-companion-agent-runtime-design.md §3 / §14.1"
    exit 1
fi

echo "✅ CI check passed: no OS context APIs in src-tauri/src/."
