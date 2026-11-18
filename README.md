# Jita

Jita 把自己的几台机器组成一个 agent 环境池。agent 在池里跑起来像在一台机器上；任一台掉线，另一台在约 30 秒内从最近的检查点接着跑。你在原生客户端上随时看进度、下指令。

> 本仓库目前只有设计文档，代码待按文档重写。上一版实现（Slint 界面的脚本生成工具）在 git 历史里，最后一个提交是 `1a54956`。

## 文档

| 文档 | 内容 |
| --- | --- |
| [概览](docs/01-overview.md) | 目标与非目标、术语、总体架构、组件划分 |
| [会话与替补](docs/02-session-and-failover.md) | 会话日志与检查点、租约与自动替补、外部副作用与重做 |
| [浏览器](docs/03-browser.md) | 给 agent 用的浏览器，登录状态跨机器同步 |
| [环境与凭据](docs/04-environment-and-credentials.md) | 容器环境、节点入池、凭据同步 |
| [客户端](docs/05-client.md) | 原生桌面客户端与控制面 |
| [端侧模型服务](docs/06-local-models.md) | 语音转文字、文字转语音、AI 绘图（MVP 之后） |
| [里程碑](docs/07-milestones.md) | M0–M5，每步一个可自动化的验收门槛 |
| [未决问题与风险](docs/08-open-questions.md) | 待验证的技术点和已接受的风险 |

建议阅读顺序：概览 → 会话与替补 → 里程碑，其余按需。

## 一句话架构

- **coordinator**（Cloudflare Workers + Durable Objects，Rust 编译为 WASM）：只管"谁能写"——租约、节点名册、中继。不存任何会话内容。
- **harnessd**（每台节点一个 Rust 二进制）：agent 循环（基于 goose 的 GDK）、会话日志、点对点复制、容器、凭据。
- **客户端**（原生 Rust，egui + wgpu）：作为 iroh 节点直连池里的节点。

会话日志、工作区快照、浏览器状态和凭据密文只在你自己的机器之间点对点同步，不上云。

## 许可证

[GPL v3](LICENSE)
