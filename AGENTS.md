# AGENTS.md

本仓库正在按 `docs/` 里的设计从零重写：把自己的几台机器组成一个 agent 环境池
（每节点 `harnessd` + Cloudflare coordinator + 原生 Rust 客户端）。
上一版实现（Slint / Tauri 的脚本生成工具）只保留在 git 历史（`1a54956` 及之前），
不要参考或恢复它。

## 先读

1. `docs/01-overview.md`：目标与非目标、术语、总体架构、组件划分。
2. `docs/02-session-and-failover.md`：会话日志与检查点、租约与自动替补、外部副作用与重做规则。这是正确性的核心。
3. `docs/07-milestones.md`：按 M0 → M5 实现，每步先过验收门槛再进入下一步。

其余文档（`03-browser.md`、`04-environment-and-credentials.md`、`05-client.md`、
`06-local-models.md`、`08-open-questions.md`、`09-ui.md`）按需查阅。
做客户端时以 `docs/09-ui.md` 和 `docs/ui/mockups/tokens.css` 为准，界面稿在 `docs/ui/`。
完整的约束清单见 `CLAUDE.md`。设计和实现冲突时，先改文档并说明理由，不要让代码悄悄偏离文档。

## 硬约束（摘要）

- 我们自己写的程序全部用 Rust：`harnessd`、coordinator（workers-rs 编译为 WASM）、客户端（egui + wgpu）；agent 在容器里用的工具不受此限。
- agent 循环、模型客户端、对话压缩用 goose 的 GDK（`goose-agent`、`goose-providers`、`goose-context-management`），锁定精确版本。
- 会话日志的外层格式自己定义，goose 的 `Message` 只作为记录里的内容。
- 会话内容、工作区快照、浏览器状态、凭据只在节点之间点对点同步，不能经 Cloudflare 存储。
- 每个里程碑的验收门槛要做成可自动运行的测试（故障注入：`kill -9`、断网、重启 coordinator）。
