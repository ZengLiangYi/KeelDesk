# KeelDesk 架构设计与演进路线图

> **设计愿景**：面向 **Windows 11** 构建一套安全可控、体验原生的桌面级 MCP（Model Context Protocol）运行底座与本机开发环境工作台，让本地桌面应用既能向外部 AI 智能体暴露高可信系统工具，又能通过原生桌面交互守护高危操作边界。

---

## 1. 架构分层与单向依赖契约

```text
┌───────────────────────────────────────────────────────────────────┐
│                     业务应用层 (Workbench App)                     │
│  Rust: src-tauri/src/workbench/   │  React: src/App.tsx           │
│  - Windows 11 环境扫描与端口治理  │  - Windows 11 原生工作台界面  │
│  - 向 KeelToolRegistry 注册业务工具│  - 消费 keel-sdk 提供的 Hooks │
└───────────────────────┬───────────┴──────────────┬────────────────┘
                        │ 单向依赖（零反向耦合）   │
                        ▼                          ▼
┌───────────────────────────────────────────────────────────────────┐
│                     龙骨运行底座 (Keel Spine)                      │
│  Rust: src-tauri/src/keel_core/   │  React: src/keel-sdk/         │
│  - 内嵌 rmcp + axum HTTP 服务端   │  - useKeelRuntime() 等状态抽象│
│  - Loopback 防重绑定与会话生命周期 │  - 强类型 tauri-specta 事件流 │
│  - Safe / ConfirmRequired 安全分级│  - 托盘常驻与优雅停机钩子     │
└───────────────────────────────────────────────────────────────────┘
```

**核心工程原则**：
1. **严格单向依赖**：`src-tauri/src/keel_core/` 与 `src/keel-sdk/` 不包含任何具体业务领域的硬编码依赖，为后续独立发布多包工作空间保持清晰边界。
2. **Windows 11 原生桌面体验（`shadcn/ui` + `modern-web-guidance` 规范）**：采用原生系统窗口装饰（保留 Snap Layouts 分屏交互与系统默认描边）、DWM 沉浸式暗色标题栏无缝融合（`#111116`）、`100dvh` 受限视口布局（消除网页式整页滚动条）、`shadcn/ui` (`radix-nova`) 组件体系、高对比度与键盘无障碍支持，以及双阶段无闪烁首屏骨架屏（完整规范见 [`AGENTS.md`](../AGENTS.md)）。

---

## 2. 阶段路线图（Phase 1 ~ Phase 5）

### ✅ Phase 1：运行底座与最小闭环（Runtime Spine）

- **目标**：建立 `keel_core` 与 `workbench` 的清晰架构边界，交付单实例系统托盘常驻的内嵌 Streamable HTTP MCP 服务端及 Windows 11 原生暗色工作台。
- **已落地能力**：
  - `src-tauri/src/keel_core/`：`KeelConfig`（默认 `127.0.0.1:19527/mcp`，支持 `KEELDESK_MCP_PORT`）、基于 `rmcp v3.10` + `axum v0.8` 的 `KeelMcpServer`、`Host`/`Origin` 防 DNS Rebinding 校验、`/healthz` 探针、`keel_ping` 工具与回环自检客户端；
  - `src-tauri/src/workbench/`：业务工具注册入口与基于注册表构建号的 Windows 11 宿主环境识别；
  - 单实例守护（`tauri-plugin-single-instance`）、关闭窗口最小化至托盘、托盘 `Quit` 优雅停机；
  - Windows 11 原生 DWM 暗色窗体、`shadcn/ui` (`radix-nova`) 双栏工作台与双阶段首屏骨架屏。

---

### ⏳ Phase 2：本机环境工具与人机协同安全审批（Host Tools & Approval Gate）

- **目标**：发挥桌面 GUI 应用相比纯命令行脚本的核心优势——**高危操作的实时可视化人工确认（Human-in-the-loop）**。
- **关键规划**：
  1. **本机开发环境工具集（`workbench`）**：
     - `scan_dev_env`（`Safe`）：探测本机 Node.js、Rust、Python、Git、pnpm 等版本与可执行路径；
     - `list_active_ports`（`Safe`）：采集本机监听端口、PID、进程名与命令行摘要；
     - `kill_port_process`（`ConfirmRequired`）：终止指定占用端口的目标进程。
  2. **异步挂起权限拦截器（`keel_core`）**：
     - 工具执行前校验 `ToolSafetyTier`：`Safe` 直接放行；`ConfirmRequired` 通过 `tokio::sync::oneshot` 挂起当前 MCP 请求协程，并经由 `tauri-specta` 事件向桌面端推送审批请求（含超时自动拒绝保护）。
  3. **桌面审批交互（`keel-sdk` & UI）**：
     - 提供 `useToolApproval()` Hook 与高危调用确认卡片，完整打通「外部客户端发起敏感工具调用 -> 桌面端弹出确认卡片 -> 用户授权放行 -> 唤醒协程执行并返回结构化结果」闭环。

---

### 📋 Phase 3：Windows 11 原生 ODR 集成与子进程托管（ODR & Process Host）

- **目标**：对接 Windows 11 原生智能体注册表生态，并解决桌面端托管外部三方 MCP 子进程时的控制台闪烁与残留痛点。
- **关键规划**：
  1. **`keel_core::win_odr` 模块**：
     - 自动检测 Windows 11 Build 版本、开发者模式状态及系统 `odr.exe` 可用性；
     - 支持一键向 Windows On-Device Agent Registry 注册/注销 MCP 服务清单，并在环境未就绪时平滑回退至标准客户端配置导出。
  2. **`keel_core::process_host` 子进程宿主**：
     - 通过 `CREATE_NO_WINDOW`（`0x08000000`）消除 Windows 下拉起 `npx` / `uvx` / `.cmd` 子进程时的控制台黑框闪烁；
     - 注入完整用户级环境变量 `PATH`，并绑定 **Windows Job Object（`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`）**，确保宿主退出或异常终止时子进程树被系统内核干净回收。

---

### 📋 Phase 4：前端 React SDK 完善与 `<KeelDevTools />`（SDK & DevTools）

- **目标**：沉淀标准化的前端状态管理 Hooks 与实时协议调试工具。
- **关键规划**：
  1. **`src/keel-sdk/` 标准 Hooks**：
     - `useMcpServerState()`：服务运行状态、监听端口、活跃会话与健康度控制；
     - `useToolApproval()`：待审批请求队列、单次放行/会话白名单/拒绝控制；
     - `useExternalMcpClients()`：外部子进程 MCP 服务启停、状态与错误日志流。
  2. **`<KeelDevTools />` 实时报文调试抽屉**：
     - 可折叠的桌面内嵌协议面板，支持实时检视 JSON-RPC 请求/响应负载、耗时分布与异常上下文。

---

### 📋 Phase 5：多包工作空间模块化拆分（Workspace Modularization）

- **目标**：完成 Cargo Workspace 与 `pnpm` Workspace 拆分，将核心运行时、Tauri 插件、React SDK 与本机环境工作台解耦为可独立复用的开源模块：

```text
keeldesk/
├── crates/
│   ├── keeldesk-core/          # 纯 Rust 桌面 MCP 运行时、安全拦截器、Win11 ODR 与进程托管
│   └── tauri-plugin-keeldesk/  # 标准 Tauri v2 插件封装
├── packages/
│   └── react/                  # @keeldesk/react (Hooks + <KeelDevTools />)
└── apps/
    └── env-workbench/          # 龙骨本机开发环境管理工作台
```
