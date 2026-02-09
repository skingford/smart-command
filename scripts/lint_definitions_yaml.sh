#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if ! command -v rg >/dev/null 2>&1; then
  echo "[lint] error: ripgrep (rg) is required" >&2
  exit 2
fi

echo "[lint] checking definitions/*.yaml for risky unquoted ':' in en/zh description lines..."

PATTERN='^\s+(en|zh): [^"'"'"']*:[^"'"'"']*$'

if rg -n --pcre2 "$PATTERN" definitions/*.yaml >/tmp/sc_yaml_lint_issues.txt; then
  echo "[lint] found potential YAML risks (quote the whole string when ':' is part of text):" >&2
  cat /tmp/sc_yaml_lint_issues.txt >&2
  rm -f /tmp/sc_yaml_lint_issues.txt
  exit 1
fi

rm -f /tmp/sc_yaml_lint_issues.txt
echo "[lint] ok"

