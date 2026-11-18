# CLAUDE.md

本仓库正在按 `docs/` 里的设计从零重写。上一版实现只在 git 历史里（提交 `1a54956` 及之前），不要参考或恢复它。

## 先读

1. `docs/01-overview.md`：目标、术语、架构。
2. `docs/10-services.md`：框架与服务两层、服务契约。
3. `docs/02-session-and-failover.md`：会话日志、检查点、租约、重做规则。这是正确性的核心。
4. `docs/07-milestones.md`：按 M0 → M5 的顺序实现，每步先过验收门槛再进入下一步。

做客户端时以 `docs/09-ui.md` 和 `docs/ui/mockups/tokens.css` 为准，界面稿在 `docs/ui/`。其余文档按需查阅。设计和实现冲突时，先改文档并说明理由，不要让代码悄悄偏离文档。

## 约束

- 我们自己写的程序全部用 Rust：harnessd、coordinator（workers-rs，编译为 WASM）、客户端（egui + wgpu）。agent 在容器里用的工具不受此限。
- agent 循环、模型客户端、对话压缩用 goose 的 GDK（`goose-agent`、`goose-providers`、`goose-context-management`），锁定精确版本。
- 会话日志的外层格式自己定义，goose 的 `Message` 只作为记录内容。
- 会话内容、工作区快照、浏览器状态、凭据只在节点之间点对点同步，不能经 Cloudflare 存储。
- 单机必须完整可用：不配置 Cloudflare、断网时，除云端模型和替补外全部功能正常。Cloudflare 只在多机时需要。
- 框架核心不依赖任何具体服务，尤其不依赖 goose；goose 只出现在 agent 会话服务里。
- 仲裁逻辑（租约状态机、epoch、名册）写在不依赖运行时的 crate 里，同时编进 harnessd（单机）和 coordinator（WASM）。
- 每个里程碑的验收门槛要做成可自动运行的测试（故障注入：`kill -9`、断网、重启 coordinator）。

## 建议的仓库结构

Cargo workspace，按组件分 crate，例如：

- `crates/protocol`：节点、仲裁者、客户端共用的协议类型（不能依赖 tokio，要能编译到 WASM）。
- `crates/arbiter`：仲裁逻辑（租约状态机、epoch、名册），不依赖运行时，要能编译到 WASM。
- `crates/core`：框架核心（名册、租约客户端、复制、放置、路由、容器、凭据），不依赖 goose。
- `crates/svc-session`：agent 会话服务（会话日志、goose-agent、工具、快照与还原、浏览器状态）。
- `crates/harnessd`：节点守护进程二进制，组装 core、arbiter（单机时）和各服务。
- `crates/coordinator`：Cloudflare Workers + Durable Objects，包装 arbiter。
- `crates/client`：原生客户端。

具体划分可以调整，调整后同步更新本文件。
