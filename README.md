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
- **端到端回环自检（`keel_ping`）**：基于 Windows 原生 SChannel TLS（`reqwest` `native-tls`）执行真实 HTTP MCP 握手与工具调用验证，并在桌面端实时展示耗时与载荷摘要。
- **Windows 11 原生暗色体验（`shadcn/ui` + `modern-web-guidance` 规范）**：保留系统原生标题栏交互与默认窗口描边，支持 `100dvh` 零整页滚动条受限视口布局、键盘无障碍与高对比度适配，以及双阶段零闪烁首屏骨架屏（完整前端工程规范沉淀于 [`AGENTS.md`](AGENTS.md)）。

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
│       ├── keel_core/           # 框架核心层：MCP HTTP 服务、工具注册表、状态快照与自检
│       ├── workbench/           # 业务工作台层：工具挂载入口与 Windows 11 宿主环境摘要
│       ├── win11.rs             # Win32 DWM 沉浸式暗色标题栏与注册表版本识别
│       ├── tray.rs              # 系统托盘菜单与常驻生命周期管理
│       └── lib.rs               # 单实例守护装配与 tauri-specta 绑定导出
└── src/
    ├── keel-sdk/                # 前端框架 SDK：useKeelRuntime 状态订阅与自检 Hook
    ├── components/
    │   ├── ui/                  # shadcn/ui (radix-nova) 基础组件
    │   └── WorkbenchSkeleton.tsx# 首屏 React 同构骨架屏
    ├── bindings.ts              # tauri-specta 自动生成的强类型命令与事件契约
    └── App.tsx                  # Windows 11 龙骨 MCP 工作台主界面
```

## 演进路线图

完整架构设计与阶段规划见 [`docs/ROADMAP.md`](docs/ROADMAP.md)：

| 里程碑 | 核心目标 | 状态 |
| :--- | :--- | :--- |
| **Phase 1 — Runtime Spine** | 框架与工作台解耦、单实例托盘常驻、内嵌 Streamable HTTP MCP 服务端与 Windows 11 原生工作台 | ✅ 已完成 |
| **Phase 2 — Host Tools & Approval Gate** | 本机开发环境扫描与端口治理工具、高危工具 `ConfirmRequired` 托盘级人工确认拦截闭环 | ⏳ 进行中 |
| **Phase 3 — Windows 11 ODR & Process Host** | Windows 11 原生 Agent Registry（`odr.exe`）注册集成、外部 MCP 子进程无黑框与 Job Object 托管 | 📋 规划中 |
| **Phase 4 — React SDK & DevTools** | 标准化 `useMcpServerState` / `useToolApproval` / `useExternalMcpClients` 与 `<KeelDevTools />` 报文抽屉 | 📋 规划中 |
| **Phase 5 — Workspace Modularization** | Cargo Workspace + `pnpm` Monorepo 模块化拆分（独立发布核心 Crates、React SDK 与工作台应用） | 📋 规划中 |

## 开源许可

本项目基于 [MIT License](LICENSE) 开源。
