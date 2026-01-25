# BoxLite Sandbox 集成测试指南

## 概述

已成功将 BoxLite 硬件级隔离替换掉 Codex 原有的 OS 级别 sandbox（macOS Seatbelt / Linux Landlock / Windows Restricted Token）。

## 架构变更

### 1. **SandboxType 新增 BoxLite 变体**
```rust
pub enum SandboxType {
    None,
    MacosSeatbelt,      // 旧的 macOS sandbox
    LinuxSeccomp,       // 旧的 Linux sandbox
    WindowsRestrictedToken, // 旧的 Windows sandbox
    #[cfg(feature = "sandbox-tool")]
    BoxLite,            // 新的硬件级隔离 sandbox
}
```

### 2. **自动选择 BoxLite**
当启用 `sandbox-tool` feature 时，`get_platform_sandbox()` 会自动返回 `SandboxType::BoxLite`，覆盖所有平台的 OS 级 sandbox。

文件：`core/src/safety.rs`
```rust
pub fn get_platform_sandbox() -> Option<SandboxType> {
    #[cfg(feature = "sandbox-tool")]
    {
        return Some(SandboxType::BoxLite);
    }
    // ... OS 级 sandbox 逻辑
}
```

### 3. **执行流程**
所有工具执行的命令现在都通过 BoxLite micro-VM：

```
工具调用 → CommandSpec → SandboxManager::transform()
         → ExecEnv(sandbox=BoxLite)
         → exec() → exec_boxlite()
         → BoxLite micro-VM 执行
```

### 4. **智能镜像选择**
BoxLite 会根据命令自动选择合适的容器镜像：
- Python 命令 → `python:3.11-slim`
- Node.js 命令 → `node:20-alpine`
- Rust 命令 → `rust:1.92-alpine`
- Bash/Shell → `alpine:latest`

## 编译

```bash
# 构建启用 BoxLite sandbox 的 Codex
cargo build --bin codex --features sandbox-tool

# Release 构建
cargo build --bin codex --features sandbox-tool --release
```

## 功能测试

### 测试 1：基本命令执行
在 Codex 会话中测试简单命令：
```
用户: "运行 echo 'Hello from BoxLite' 并显示结果"
```

预期：命令在 alpine:latest micro-VM 中执行，返回 "Hello from BoxLite"

### 测试 2：Python 代码执行
```
用户: "执行这段 Python 代码：print(2 + 2)"
```

预期：
- BoxLite 自动选择 `python:3.11-slim` 镜像
- 在 micro-VM 中执行 Python 代码
- 返回输出 "4"

### 测试 3：工作目录支持
```
用户: "在 /tmp 目录下创建一个测试文件"
```

预期：BoxLite 会使用 `cd /tmp && ...` 包装命令

### 测试 4：超时处理
```
用户: "运行 sleep 100"
```

配置较短的超时时间，验证超时机制是否正常。

### 测试 5：多用户隔离
启动多个 Codex 会话，验证：
- 每个会话的命令在独立的 micro-VM 中执行
- 不同用户的文件系统完全隔离
- 没有进程泄露或资源冲突

## 日志验证

启用详细日志查看 BoxLite 执行：
```bash
RUST_LOG=codex_core=debug ./target/debug/codex "test command"
```

关键日志信息：
```
INFO  Executing command in BoxLite sandbox: ["echo", "Hello"]
DEBUG Selected BoxLite image: alpine:latest
DEBUG Sandbox type: BoxLite
```

## 对比测试

### 不启用 sandbox-tool feature
```bash
cargo build --bin codex
# 使用 OS 级 sandbox (Seatbelt/Landlock)
```

### 启用 sandbox-tool feature
```bash
cargo build --bin codex --features sandbox-tool
# 使用 BoxLite 硬件级隔离
```

对比两种模式下的隔离效果、性能和资源消耗。

## 已知限制

1. **工作目录支持**：BoxCommand 不直接支持 `current_dir()`，当前通过包装 `cd` 命令实现
2. **镜像下载**：首次使用某个镜像时需要从 Docker Hub 下载
3. **启动开销**：micro-VM 创建比 OS sandbox 慢，但隔离性强得多

## 性能考虑

- **micro-VM 创建时间**：~1-3秒（首次，缓存后更快）
- **内存占用**：每个 VM 512MB（可配置）
- **并发能力**：支持多个 VM 同时运行，适合多用户场景

## 安全优势

✅ **硬件级隔离**：每个命令在独立的 micro-VM 中执行
✅ **账户隔离**：不同用户的执行环境完全分离
✅ **网络隔离**：micro-VM 可配置网络策略
✅ **资源限制**：内存、CPU 严格限制
✅ **跨平台一致性**：所有平台使用相同的隔离机制

## 故障排查

### 问题：BoxLite VM 创建失败
- 检查 mke2fs/debugfs 工具是否正确捆绑
- 查看日志：`tail -f ~/.codex/log/codex-tui.log`

### 问题：镜像拉取失败
- 确认网络连接
- 检查 Docker Hub 访问
- 尝试使用国内镜像源

### 问题：命令执行超时
- 调整 BoxLite 内存限制（当前 512MB）
- 检查镜像大小和启动时间
- 考虑使用更轻量的 alpine 镜像

## 下一步

1. ✅ 基本集成完成
2. ⏳ 运行时测试（需要配置 BoxLite 依赖）
3. ⏳ 性能优化（VM 复用、镜像预热）
4. ⏳ 生产环境部署验证
