# Release Guide / 发布指南

本文档描述如何发布 Smart Command (sc) 的新版本。

## 自动发布流程

当推送带有 `v` 前缀的 tag 时，GitHub Actions 会自动：

1. **构建多平台二进制文件**
   - macOS Intel (x86_64-apple-darwin)
   - macOS Apple Silicon (aarch64-apple-darwin)
   - Linux x86_64 (x86_64-unknown-linux-gnu)
   - Linux ARM64 (aarch64-unknown-linux-gnu)
   - Windows x86_64 (x86_64-pc-windows-msvc)

2. **生成校验和**
   - 为每个 artifact 生成 SHA256 校验和文件

3. **构建 Debian 包**
   - 生成 `.deb` 安装包

4. **创建 GitHub Release**
   - 自动生成 Release Notes
   - 上传所有构建产物

## 发布步骤

### 1. 更新版本号

编辑 `Cargo.toml`：

```toml
[package]
version = "0.2.0"  # 更新版本号
```

### 2. 提交更改

```bash
git add Cargo.toml
git commit -m "chore: bump version to 0.2.0"
git push origin main
```

### 3. 创建并推送 Tag

```bash
# 创建 tag
git tag v0.2.0

# 推送 tag（触发自动发布）
git push origin v0.2.0
```

### 4. 监控构建

```bash
# 查看 Actions 状态
gh run list --limit 5

# 查看构建日志
gh run view <run-id> --log

# 查看失败日志
gh run view <run-id> --log-failed
```

### 5. 验证发布

- 访问 https://github.com/skingford/smart-command/releases
- 确认所有平台的二进制文件都已上传
- 确认 SHA256 校验和文件存在

## 重新发布（修复失败的发布）

如果构建失败需要重新发布：

```bash
# 1. 删除本地 tag
git tag -d v0.2.0

# 2. 删除远程 tag
git push origin :refs/tags/v0.2.0

# 3. 修复问题并提交
git add .
git commit -m "fix: ..."
git push origin main

# 4. 重新创建并推送 tag
git tag v0.2.0
git push origin v0.2.0
```

## 版本号规范

遵循 [Semantic Versioning](https://semver.org/)：

- **MAJOR** (x.0.0): 不兼容的 API 变更
- **MINOR** (0.x.0): 向后兼容的新功能
- **PATCH** (0.0.x): 向后兼容的问题修复

## 发布检查清单

发布前确认：

- [ ] 版本号已更新
- [ ] CHANGELOG 已更新（如有）
- [ ] 所有测试通过 (`cargo test`)
- [ ] 本地构建成功 (`cargo build --release`)
- [ ] README 文档已更新（如有新功能）

## 常见问题

### Q: 构建失败怎么办？

1. 查看失败日志：`gh run view <run-id> --log-failed`
2. 修复问题
3. 按照"重新发布"步骤操作

### Q: 如何查看发布的 artifacts？

```bash
gh release view v0.2.0
```

### Q: 如何手动上传文件到 Release？

```bash
gh release upload v0.2.0 ./path/to/file
```

### Q: 如何创建预发布版本？

```bash
git tag v0.2.0-beta.1
git push origin v0.2.0-beta.1
```

预发布版本会被标记为 Pre-release。

## 工作流配置

工作流配置文件位于 `.github/workflows/release.yml`，主要配置：

```yaml
on:
  push:
    tags:
      - 'v*'  # 匹配所有 v 开头的 tag

env:
  BINARY_NAME: sc  # 二进制文件名

jobs:
  build:        # 构建多平台二进制
  build-deb:    # 构建 Debian 包
  release:      # 创建 GitHub Release
```
