<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="public/logo-readme-dark.svg">
    <img src="public/logo.svg" alt="KeelDesk logo" width="104">
  </picture>
</p>

<h1 align="center">KeelDesk</h1>

<p align="center">
  面向 <strong>Windows 11</strong> 的原生桌面级 MCP（Model Context Protocol）运行底座与本机开发环境工作台。
</p>

<p align="center">
  <a href="https://github.com/ZengLiangYi/KeelDesk/actions/workflows/ci.yml"><img src="https://github.com/ZengLiangYi/KeelDesk/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-7c66ff.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-Windows%2011%20(Build%2022000%2B)-0078D4.svg" alt="Platform: Windows 11 (Build 22000+)">
  <img src="https://img.shields.io/badge/Stack-Tauri%20v2%20%C2%B7%20Rust%20%C2%B7%20React%2019-18181b.svg" alt="Stack: Tauri v2 · Rust · React 19">
</p>

<p align="center">
  <a href="README.md">English</a> · <strong>简体中文</strong>
</p>

---

## 架构概览

**KeelDesk** 采用运行底座与桌面业务严格单向解耦的分层架构：

- **KeelDesk 运行底座（`src-tauri/src/keel_core/` + `src/keel-sdk/`）**：提供单实例系统托盘守护、内嵌 `rmcp` + `axum` Streamable HTTP MCP 服务端、本地回环 DNS Rebinding 防护、`Safe` / `ConfirmRequired` 工具安全注册表、基于 `tokio::sync::oneshot` 的人机协同审批网关、强类型 `tauri-specta` IPC 契约、实时遥测事件流与前端状态 Hooks。**`keel_core` 与 `keel-sdk` 严禁引用或依赖任何 `workbench` 业务模块。**
- **KeelDesk 工作台应用（`src-tauri/src/workbench/` + `src/App.tsx` + `src/components/`）**：向 `KeelToolRegistry` 注册 Windows 11 宿主环境探测与端口治理工具，并构建原生桌面工作台界面（DWM 沉浸式暗色标题栏、Snap Layouts 原生分屏、`shadcn/ui` + `ds/` 领域组件与双阶段零闪烁首屏骨架屏）。

## 核心特性

- **内嵌 Streamable HTTP MCP 服务端**：默认监听 `http://127.0.0.1:19527/mcp`（可通过环境变量 `KEELDESK_MCP_PORT` 自定义端口），内置 `/healthz` 就绪探针。
- **单实例系统托盘常驻**：基于 `tauri-plugin-single-instance` 保证全局唯一守护实例；关闭主窗口时自动隐藏至 Windows 11 系统托盘，后台 MCP 服务持续在线，仅在托盘菜单点击 `Quit` 时优雅关闭 `KeelRuntime` 并释放端口退出。
- **本地回环 DNS Rebinding 防护**：仅绑定 `127.0.0.1` 并严格校验 `Host` 与 `Origin` 白名单，阻断跨站 DNS Rebinding 请求。
- **人机协同 `ConfirmRequired` 审批网关**：高危 MCP 工具调用会自动挂起在 `tokio::sync::oneshot` 通道上；若主窗口隐藏于托盘，将自动唤醒聚焦并弹出 `AlertDialog` 人工确认弹窗，支持 **单次放行**、**当前会话免审白名单**、**主动拒绝**、调用方取消即刻回收（`RequestContext.ct`）以及超时自动拦截（默认 `60s`）。
- **Windows 11 原生宿主探测工具集**：使用 `CREATE_NO_WINDOW` 并发探测本机开发工具链（无控制台黑框闪烁），并通过 Win32 `GetExtendedTcpTable` 与 `QueryFullProcessImageNameW` 毫秒级枚举 TCPv4/TCPv6 监听端口与完整进程路径。
- **端到端回环自检与安全演练**：基于 Windows 原生 SChannel TLS（`reqwest` `native-tls`）内置一键 `keel_ping` 连通性自检与 `kill_port_process`（`dry_run: true`）高危审批全链路演练。
- **The Graphite Spine 设计系统**：保留真实 Windows 11 原生窗口边框与标题栏（`decorations: true`，`DWMWA_CAPTION_COLOR` 像素锁定 `#111116`），配备 `100dvh` 零整页滚动条受限视口、`ScrollArea` 内置滚动、六阶桌面排版体系、`4px` 间距网格与 `12px` 垂直基准线、同心 `2px` 圆角层级及 Windows 高对比度（`forced-colors`）适配。规范详情见 [`PRODUCT.md`](PRODUCT.md)、[`DESIGN.md`](DESIGN.md) 与 [`AGENTS.md`](AGENTS.md)。

## 内置 MCP 工具目录

| 工具名 | 归属层 | 安全分级 | 功能说明 |
| :--- | :--- | :--- | :--- |
| `keel_ping` | `keel_core` | `Safe` | 验证本地 Streamable HTTP MCP 连通性并返回服务运行状态与回显载荷。 |
| `scan_dev_env` | `workbench` | `Safe` | 无黑框并发探测 Windows 11 开发工具链（`git`、`node`、`pnpm`、`rustc`、`cargo`、`python`、`uv`、`pwsh`、`winget`、`wsl`）版本与路径，支持 UTF-16LE 输出解码与 `WindowsApps` 占位符过滤。 |
| `list_active_ports` | `workbench` | `Safe` | 基于 Win32 `GetExtendedTcpTable` 枚举本机 TCPv4/TCPv6 监听端口、PID、进程名与完整可执行文件路径。 |
| `kill_port_process` | `workbench` | `ConfirmRequired` | 终止指定端口监听进程，受桌面人工确认、TOCTOU `expected_pid` 校验、自身/系统进程防误杀保护以及 `dry_run` 演练模式约束。 |

## 快速开始

### 环境要求

- **操作系统**：Windows 11（Build 22000+，推荐 24H2）
- **工具链**：Rust（`stable`）、Node.js 20+、`pnpm`、Microsoft Edge WebView2 Runtime

### 开发与构建命令

```bash
# 安装前端依赖
pnpm install

# 启动 Tauri + Vite 开发环境（含热更新与自动导出 TS 绑定）
pnpm tauri dev

# 根据 Rust 命令与事件重新生成 src/bindings.ts
pnpm bindings

# 前端生产构建（依次执行 pnpm bindings、tsc、vite build）
pnpm build

# 运行 Rust 单元测试与内嵌 MCP HTTP 回环集成测试
cargo test --manifest-path src-tauri/Cargo.toml

# 构建 Windows 11 桌面安装包
pnpm tauri build
```

## 外部 AI 客户端接入

当 KeelDesk 运行（或常驻系统托盘）时，可将本地兼容 MCP 的 AI 客户端接入当前回环端点：

### Claude Code CLI

```bash
claude mcp add --transport http keeldesk http://127.0.0.1:19527/mcp
```

### Cursor (`mcp.json`)

```json
{
  "mcpServers": {
    "keeldesk": {
      "url": "http://127.0.0.1:19527/mcp"
    }
  }
}
```

### PowerShell 健康探针

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:19527/healthz"
```

## 目录结构

```text
KeelDesk/
├── PRODUCT.md                         # 产品定位、设计禁忌与无障碍目标
├── DESIGN.md                          # "The Graphite Spine" 规范化设计系统 Token 与约束
├── AGENTS.md                          # 工程边界、Windows 11 原生窗体与 Specta 契约
├── docs/
│   ├── ROADMAP.md                     # 架构分层与 Phase 1–5 演进路线图（英文）
│   └── ROADMAP.zh-CN.md               # 架构分层与 Phase 1–5 演进路线图（简体中文）
├── src-tauri/
│   └── src/
│       ├── keel_core/                 # 运行底座：MCP HTTP 服务、审批网关、工具注册表与回环客户端
│       ├── workbench/                 # 工作台业务：Win11 工具链无黑框探测、Win32 端口治理与宿主摘要
│       ├── bin/export-bindings.rs     # tauri-specta TypeScript 绑定导出二进制入口
│       ├── win11.rs                   # Win32 DWM 暗色标题栏主题与注册表版本识别
│       ├── tray.rs                    # 系统托盘菜单与后台常驻生命周期管理
│       └── lib.rs                     # 单实例守护装配与 Specta 命令/事件注册
└── src/
    ├── keel-sdk/                      # 前端 SDK：useKeelRuntime 与 useToolApproval 状态 Hooks
    ├── styles/tokens.css              # 单轨暗色石墨 Token 与排版工具类
    ├── components/
    │   ├── ui/                        # shadcn/ui (radix-nova) 基础组件
    │   ├── ds/                        # KeelDesk 领域设计系统组件
    │   ├── ToolApprovalDialog.tsx     # ConfirmRequired 交互式桌面人工审批弹窗
    │   ├── PortsGovernancePanel.tsx   # Win32 监听端口检索与审批演练面板
    │   ├── DevEnvPanel.tsx            # Windows 11 本机开发工具链探测面板
    │   └── WorkbenchSkeleton.tsx      # Stage 2 React 同构骨架屏
    ├── bindings.ts                    # tauri-specta 自动生成的强类型命令与事件契约（切勿手改）
    └── App.tsx                        # KeelDesk Windows 11 工作台主界面
```

## 演进路线图

完整架构设计与阶段规划见 [`docs/ROADMAP.zh-CN.md`](docs/ROADMAP.zh-CN.md)：

| 里程碑 | 核心目标 | 状态 |
| :--- | :--- | :--- |
| **Phase 1 — Runtime Spine** | `keel_core` 与 `workbench` 单向解耦、单实例托盘常驻、内嵌 Streamable HTTP MCP 服务端与 Windows 11 原生工作台 | ✅ 已完成 |
| **Phase 2 — Host Tools & Approval Gate** | 本机开发环境与 Win32 端口治理工具，以及托盘唤醒级 `ConfirmRequired` 人机协同审批闭环 | ✅ 已完成 |
| **Design System — The Graphite Spine** | 暗色 DWM 像素锁定 Token、六阶桌面排版、`12px` 垂直基准线、同心圆角、`ui/` + `ds/` 分层与双阶段骨架屏 | ✅ 已完成 |
| **Phase 3 — Windows 11 ODR & Process Host** | Windows 原生 Agent Registry（`odr.exe`）注册集成与 `CREATE_NO_WINDOW` + Job Object 子进程 MCP 托管 | ⏳ 进行中 |
| **Phase 4 — React SDK & DevTools** | 标准化 `useMcpServerState`、`useToolApproval`、`useExternalMcpClients` 与 `<KeelDevTools />` 报文调试抽屉 | 📋 规划中 |
| **Phase 5 — Workspace Modularization** | Cargo Workspace + `pnpm` Monorepo 模块化拆分，独立发布核心 Crates、React SDK 与工作台应用 | 📋 规划中 |

## 贡献与安全

- 提交 Pull Request 前请先阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md)，重点遵循 `keel_core` 单向依赖边界、`pnpm bindings` 同步流程与 The Graphite Spine 设计规范。
- 社区协作遵循 [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)。
- 发现安全漏洞请参照 [`SECURITY.md`](SECURITY.md) 通过 GitHub 私密安全通告渠道反馈，切勿在公开 Issue 中披露漏洞细节。

## 开源许可

本项目基于 [MIT License](LICENSE) 开源。Copyright (c) 2026 Leon Zeng.
