#!/usr/bin/env bash
# Phase 2 汇总：收集所有评审发现报告，按严重度归并。不跑 cargo。
set -u
cd "$(dirname "$0")/.."
echo "================ Σ⁴ 边界审计 · 发现汇总 ================"
echo
for f in .boundary-audit/A*.md; do
  [ -f "$f" ] || continue
  echo "──────── $(basename "$f") ────────"
  # 抽取表格行（以 | 开头）与 P0/P1/P2 标记
  grep -nE "^\|" "$f" 2>/dev/null | sed 's/^/  /' | head -40
  echo
done
echo "================ 严重度计数 ================"
for sev in P0 P1 P2; do
  n=$(grep -rhoE "\\b${sev}\\b" .boundary-audit/A*.md 2>/dev/null | wc -l | tr -d ' ')
  echo "  ${sev}: ${n} 处提及"
done
echo
echo "================ 新增测试文件 ================"
ls -la tests/*.rs 2>/dev/null || echo "  (tests/ 尚无文件)"
