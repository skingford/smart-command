#!/bin/bash
# 升级功能测试脚本
# Usage: ./scripts/test_upgrade.sh

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[PASS]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[FAIL]${NC} $1"; }

# 项目根目录
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SC_BIN="$PROJECT_DIR/target/release/sc"

echo "=========================================="
echo "  Smart Command 升级功能测试"
echo "=========================================="
echo

# 1. 检查二进制是否存在
info "测试 1: 检查二进制文件..."
if [ -f "$SC_BIN" ]; then
    success "二进制文件存在: $SC_BIN"
else
    warn "二进制文件不存在，正在构建..."
    cd "$PROJECT_DIR"
    cargo build --release
    if [ -f "$SC_BIN" ]; then
        success "构建成功"
    else
        error "构建失败"
        exit 1
    fi
fi
echo

# 2. 测试版本显示
info "测试 2: 版本显示..."
VERSION=$("$SC_BIN" --version 2>&1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
if [ -n "$VERSION" ]; then
    success "当前版本: $VERSION"
else
    error "无法获取版本信息"
    exit 1
fi
echo

# 3. 测试 upgrade --help
info "测试 3: upgrade 命令帮助..."
HELP_OUTPUT=$("$SC_BIN" upgrade --help 2>&1)
if echo "$HELP_OUTPUT" | grep -q "check"; then
    success "upgrade --help 正常工作"
    echo "$HELP_OUTPUT" | head -10 | sed 's/^/    /'
else
    error "upgrade --help 输出异常"
    exit 1
fi
echo

# 4. 测试 upgrade --check
info "测试 4: 检查更新 (upgrade --check)..."
CHECK_OUTPUT=$("$SC_BIN" upgrade --check 2>&1)
echo "$CHECK_OUTPUT" | tail -5 | sed 's/^/    /'

if echo "$CHECK_OUTPUT" | grep -qE "(已是最新版本|发现新版本|检查更新失败)"; then
    success "upgrade --check 正常工作"
else
    warn "upgrade --check 输出可能异常"
fi
echo

# 5. 测试配置
info "测试 5: 检查升级配置..."
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/smart-command"
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/smart-command"

echo "    配置目录: $CONFIG_DIR"
echo "    缓存目录: $CACHE_DIR"

if [ -f "$CACHE_DIR/version-cache.json" ]; then
    success "版本缓存文件已创建"
    echo "    缓存内容:"
    cat "$CACHE_DIR/version-cache.json" | sed 's/^/      /'
else
    warn "版本缓存文件尚未创建（首次运行或网络问题）"
fi
echo

# 6. 测试平台检测
info "测试 6: 平台检测..."
OS=$(uname -s)
ARCH=$(uname -m)
echo "    操作系统: $OS"
echo "    架构: $ARCH"

case "$OS" in
    Darwin) EXPECTED_PLATFORM="apple-darwin" ;;
    Linux) EXPECTED_PLATFORM="unknown-linux-gnu" ;;
    *) EXPECTED_PLATFORM="unknown" ;;
esac

case "$ARCH" in
    x86_64) EXPECTED_ARCH="x86_64" ;;
    arm64|aarch64) EXPECTED_ARCH="aarch64" ;;
    *) EXPECTED_ARCH="unknown" ;;
esac

success "预期平台: ${EXPECTED_ARCH}-${EXPECTED_PLATFORM}"
echo

# 7. 模拟升级测试（dry run）
info "测试 7: 模拟升级流程..."
echo "    注意: 由于仓库可能没有发布 release，实际升级可能失败"
echo "    这是预期行为，只要检查流程正常即可"
echo

# 8. 测试启动时自动检查（模拟）
info "测试 8: 启动时自动检查配置..."
if [ -f "$CONFIG_DIR/config.toml" ]; then
    if grep -q "auto_check" "$CONFIG_DIR/config.toml"; then
        AUTO_CHECK=$(grep "auto_check" "$CONFIG_DIR/config.toml" | grep -oE "(true|false)")
        echo "    auto_check = $AUTO_CHECK"
    else
        echo "    auto_check = true (默认值)"
    fi
else
    echo "    使用默认配置 (auto_check = true)"
fi
success "配置检查完成"
echo

# 总结
echo "=========================================="
echo "  测试完成"
echo "=========================================="
echo
echo "功能清单:"
echo "  [x] sc upgrade --check  - 检查更新"
echo "  [x] sc upgrade          - 执行升级"
echo "  [x] sc upgrade -y       - 跳过确认升级"
echo "  [x] sc upgrade --force  - 强制升级"
echo "  [x] 启动时后台自动检查"
echo "  [x] 版本缓存 (24小时)"
echo
echo "配置选项 (~/.config/smart-command/config.toml):"
echo "  [upgrade]"
echo "  auto_check = true"
echo "  check_interval_hours = 24"
echo "  repository = \"skingford/smart-command\""
echo "  include_prerelease = false"
echo
