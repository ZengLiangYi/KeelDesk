<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="public/logo-readme-dark.svg">
    <img src="public/logo.svg" alt="KeelDesk logo" width="112">
  </picture>
</p>

<h1 align="center">KeelDesk · 龙骨桌面</h1>

<p align="center">
  面向 <strong>Windows 11</strong> 的桌面级 MCP（Model Context Protocol）运行底座与本机开发环境工作台。
</p>

---

## 架构概览

**KeelDesk** 采用框架底座与桌面业务单向解耦的分层设计：

- **龙骨运行底座（`src-tauri/src/keel_core` + `src/keel-sdk`）**：提供单实例系统托盘守护、内嵌 `rmcp` + `axum` Streamable HTTP MCP 服务端、本地回环 DNS Rebinding 防护、强类型 `tauri-specta` IPC 契约以及实时工具调用遥测事件流，与上层业务完全解耦。
- **本机环境工作台（`src-tauri/src/workbench` + `src/App.tsx`）**：专注 **Windows 11 原生桌面体验**（DWM 沉浸式暗色标题栏、原生 Snap Layouts 分屏支持、`shadcn/ui` 组件体系、零闪烁首屏骨架屏），面向本机开发环境检测、端口治理与外部 Agent 客户端连接管理。

## 核心特性

- **内嵌 Streamable HTTP MCP 服务端**：默认监听 `http://127.0.0.1:19527/mcp`（可通过环境变量 `KEELDESK_MCP_PORT` 自定义端口），内置 `/healthz` 健康检查端点。
- **单实例系统托盘常驻**：基于 `tauri-plugin-single-instance` 保证全局唯一守护实例；关闭主窗口时自动最小化至系统托盘，后台 MCP 服务持续在线，仅在托盘菜单选择 `Quit` 时优雅释放端口退出。
- **本地回环安全基线**：仅监听 `127.0.0.1` 并严格校验 `Host` 与 `Origin` 本地白名单，阻断跨站 DNS Rebinding 请求。
- **人机协同安全审批网关（`ConfirmRequired`）**：对高危 MCP 工具（如终止端口占用进程）自动基于 `tokio::sync::oneshot` 挂起请求协程；若主窗口隐藏于系统托盘则自动唤醒聚焦并弹出 `AlertDialog` 人工确认卡片，支持单次放行、当前 MCP 会话免审白名单、主动拒绝、调用方取消即刻回收以及超时自动拦截。
- **Windows 11 原生宿主工具集**：无控制台黑框闪烁（`CREATE_NO_WINDOW`）并发探测 10 类主流开发工具链，并通过 Win32 内核级 `GetExtendedTcpTable`（IPv4/IPv6）+ `QueryFullProcessImageNameW` 毫秒级枚举监听端口与进程镜像路径。
- **端到端回环自检与拦截演练**：基于 Windows 原生 SChannel TLS（`reqwest` `native-tls`）内置一键 `keel_ping` 自检与 `kill_port_process`（`dry_run`）高危审批全链路演练。
- **Windows 11 原生暗色体验（`shadcn/ui` + `modern-web-guidance` 规范）**：保留系统原生标题栏交互与默认窗口描边，支持 `100dvh` 零整页滚动条受限视口布局、键盘无障碍与高对比度适配，以及双阶段零闪烁首屏骨架屏（完整前端工程规范沉淀于 [`AGENTS.md`](AGENTS.md)）。

## 内置 MCP 工具目录

| 工具名 | 归属层 | 安全分级 | 功能说明 |
| :--- | :--- | :--- | :--- |
| `keel_ping` | `keel_core` | `Safe` | 验证本地 Streamable HTTP MCP 连通性并返回服务运行状态与回显载荷 |
| `scan_dev_env` | `workbench` | `Safe` | 无黑框并发探测 Windows 11 开发工具链（`git`/`node`/`pnpm`/`rustc`/`cargo`/`python`/`uv`/`pwsh`/`winget`/`wsl`）版本与路径 |
| `list_active_ports` | `workbench` | `Safe` | 基于 Win32 `GetExtendedTcpTable` 枚举本机 TCPv4/TCPv6 监听端口、PID 与完整进程路径 |
| `kill_port_process` | `workbench` | `ConfirmRequired` | 终止指定端口监听进程（受桌面人工审批、TOCTOU `expected_pid` 校验及自身/系统进程防误杀保护，支持 `dry_run` 演练） |

## 快速开始

### 环境要求

- **操作系统**：Windows 11（Build 22000+，推荐 24H2）
- **工具链**：Rust（stable）、Node.js 20+、`pnpm`

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

## 外部 Agent 客户端接入

当 KeelDesk 运行（或常驻于系统托盘）时，可将本地 AI 客户端接入当前 MCP 端点：

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
├── docs/
│   └── ROADMAP.md               # 架构分层说明与演进路线图
├── src-tauri/
│   └── src/
│       ├── keel_core/           # 框架核心层：MCP HTTP 服务、异步审批网关、工具注册表与回环客户端
│       ├── workbench/           # 业务工作台层：Win11 工具链无黑框探测、Win32 端口治理与宿主摘要
│       ├── win11.rs             # Win32 DWM 沉浸式暗色标题栏与注册表版本识别
│       ├── tray.rs              # 系统托盘菜单与常驻生命周期管理
│       └── lib.rs               # 单实例守护装配与 tauri-specta 绑定导出
└── src/
    ├── keel-sdk/                # 前端框架 SDK：useKeelRuntime 与 useToolApproval 状态订阅 Hooks
    ├── components/
    │   ├── ui/                  # shadcn/ui (radix-nova) 基础组件
    │   ├── ToolApprovalDialog.tsx # ConfirmRequired 高危调用人工确认弹窗
    │   ├── PortsGovernancePanel.tsx # Win32 监听端口检索与审批演练面板
    │   ├── DevEnvPanel.tsx      # Windows 11 本机开发工具链探测面板
    │   └── WorkbenchSkeleton.tsx# 首屏 React 同构骨架屏
    ├── bindings.ts              # tauri-specta 自动生成的强类型命令与事件契约
    └── App.tsx                  # Windows 11 龙骨 MCP 工作台主界面
```

## 演进路线图

完整架构设计与阶段规划见 [`docs/ROADMAP.md`](docs/ROADMAP.md)：

| 里程碑 | 核心目标 | 状态 |
| :--- | :--- | :--- |
| **Phase 1 — Runtime Spine** | 框架与工作台解耦、单实例托盘常驻、内嵌 Streamable HTTP MCP 服务端与 Windows 11 原生工作台 | ✅ 已完成 |
| **Phase 2 — Host Tools & Approval Gate** | 本机开发环境扫描与端口治理工具、高危工具 `ConfirmRequired` 托盘级人工确认拦截闭环 | ✅ 已完成 |
| **Phase 3 — Windows 11 ODR & Process Host** | Windows 11 原生 Agent Registry（`odr.exe`）注册集成、外部 MCP 子进程无黑框与 Job Object 托管 | ⏳ 进行中 |
| **Phase 4 — React SDK & DevTools** | 标准化 `useMcpServerState` / `useToolApproval` / `useExternalMcpClients` 与 `<KeelDevTools />` 报文抽屉 | 📋 规划中 |
| **Phase 5 — Workspace Modularization** | Cargo Workspace + `pnpm` Monorepo 模块化拆分（独立发布核心 Crates、React SDK 与工作台应用） | 📋 规划中 |

## 开源许可

本项目基于 [MIT License](LICENSE) 开源。
