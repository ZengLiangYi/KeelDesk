[English](ROADMAP.md) · **简体中文**

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
│                     KeelDesk 运行底座 (Keel Spine)                 │
│  Rust: src-tauri/src/keel_core/   │  React: src/keel-sdk/         │
│  - 内嵌 rmcp + axum HTTP 服务端   │  - useKeelRuntime() 等状态抽象│
│  - Loopback 防重绑定与会话生命周期 │  - 强类型 tauri-specta 事件流 │
│  - Safe / ConfirmRequired 安全分级│  - 托盘常驻与优雅停机钩子     │
└───────────────────────────────────────────────────────────────────┘
```

**核心工程原则**：
1. **严格单向依赖**：`src-tauri/src/keel_core/` 与 `src/keel-sdk/` 不包含任何具体业务领域的硬编码依赖，为后续独立发布多包工作空间保持清晰边界。
2. **Windows 11 原生桌面体验与 The Graphite Spine 规范**：保留原生系统窗口装饰（Snap Layouts 分屏与系统默认描边）、DWM 沉浸式暗色标题栏无缝融合（`#111116`）、`100dvh` 受限视口布局（`overflow: clip`）、`shadcn/ui` (`radix-nova`) + `ds/` 组件体系、高对比度（`forced-colors`）与键盘无障碍支持，以及双阶段无闪烁首屏骨架屏（完整规范见 [`DESIGN.md`](../DESIGN.md)、[`PRODUCT.md`](../PRODUCT.md) 与 [`AGENTS.md`](../AGENTS.md)）。

---

## 2. 阶段路线图（Phase 1 ~ Phase 5）

### ✅ Phase 1：运行底座与最小闭环（Runtime Spine）

- **目标**：建立 `keel_core` 与 `workbench` 的清晰架构边界，交付单实例系统托盘常驻的内嵌 Streamable HTTP MCP 服务端及 Windows 11 原生暗色工作台。
- **已落地能力**：
  - `src-tauri/src/keel_core/`：`KeelConfig`（默认 `127.0.0.1:19527/mcp`，支持 `KEELDESK_MCP_PORT`）、基于 `rmcp` v3.10 + `axum` v0.8 的 `KeelMcpServer`、`Host`/`Origin` 防 DNS Rebinding 校验、`/healthz` 探针、`keel_ping` 工具与回环自检客户端（`reqwest` `native-tls`）；
  - `src-tauri/src/workbench/`：业务工具注册入口与基于注册表构建号的 Windows 11 宿主环境识别；
  - 单实例守护（`tauri-plugin-single-instance`）、关闭窗口隐藏至系统托盘、托盘 `Quit` 优雅停机；
  - Win32 DWM 暗色窗体（`src-tauri/src/win11.rs`）与 `tauri-specta` 强类型命令/事件绑定。

---

### ✅ Phase 2：本机环境工具与人机协同安全审批（Host Tools & Approval Gate）

- **目标**：发挥桌面 GUI 应用相比纯命令行脚本的核心优势——**高危操作的实时可视化人工确认（Human-in-the-loop）**。
- **已落地能力**：
  1. **本机开发环境与端口治理工具集（`workbench`）**：
     - `scan_dev_env`（`Safe`）：基于 `CREATE_NO_WINDOW` 并发探测本机 `git`、`node`、`pnpm`、`rustc`、`cargo`、`python`、`uv`、`pwsh`、`winget`、`wsl` 版本与路径（支持 UTF-16LE 输出解码与 `WindowsApps` 占位符过滤）；
     - `list_active_ports`（`Safe`）：基于 Win32 `GetExtendedTcpTable`（IPv4/IPv6）与 `QueryFullProcessImageNameW` 毫秒级采集监听端口、PID 与完整进程路径；
     - `kill_port_process`（`ConfirmRequired`）：内置 `expected_pid` TOCTOU 校验、系统与自身进程防误杀保护、Win32 `TerminateProcess` 终止及 `dry_run` 安全演练模式。
  2. **异步挂起权限拦截器（`keel_core`）**：
     - `KeelMcpServer::call_tool` 对 `ConfirmRequired` 工具通过 `tokio::sync::oneshot` 挂起请求，支持单次放行（`ApproveOnce`）、会话免审白名单（`ApproveForSession`）、拒绝拦截（`Deny`）、超时自动拒绝以及调用方取消（`RequestContext.ct`）即时回收与托盘主窗口唤醒聚焦。
  3. **桌面审批交互与三合一宿主面板（`keel-sdk` & UI）**：
     - 提供 `useToolApproval()` Hook、`ToolApprovalDialog` 实时倒计时确认弹窗，以及「工具契约 / 端口治理 / 本机环境」三标签工作台视图。

---

### ✅ Design System 基础：The Graphite Spine

- **目标**：统一桌面工作台的每一个视觉原语、布局框架与启动加载状态。
- **已落地能力**：
  - 单轨暗色石墨 Token 引擎（`src/styles/tokens.css`）像素级锁定 Win32 `DWMWA_CAPTION_COLOR`（`#111116`），建立六阶桌面排版体系、`4px` 间距网格与 `12px` 垂直基准线、同心 `2px` 圆角层级、`src/components/ui/` 规范化、`src/components/ds/` 领域原语（`StatusBadge`、`SafetyTierBadge`、`TruncatedText`、`CodeWell`、`CopyButton`、`MetricPill`、`EmptyState`），以及同步化的 Stage 1（`index.html`）/ Stage 2（`WorkbenchSkeleton.tsx`）双阶段首屏骨架屏。

---

### ✅ Phase 3：Windows 11 原生 ODR 集成与子进程托管（ODR & Process Host）

- **目标**：对接 Windows 11 原生智能体注册表生态，并解决桌面端托管外部三方 MCP 子进程时的控制台闪烁与残留痛点。
- **已落地能力**：
  1. **`keel_core::win32_env` 共享 Win32 环境辅助模块**：
     - [x] 合并 `HKLM` / `HKCU` 注册表 `PATH` 重建（`build_merged_windows_path`）与基于 `PATHEXT` 的可执行文件解析（`resolve_executable_in_path`）；
     - [x] `CREATE_NO_WINDOW`（`0x08000000`）无黑框子进程构建器、Windows 11 构建号/UBR/开发者模式注册表探测（`Win11OdrHostProfile`）以及 UTF-8/UTF-16LE/GBK CLI 输出解码。
  2. **`keel_core::win_odr` Windows 11 ODR 集成**：
     - [x] 探测 OS 构建号（`>= 22000` 与 `26220.7262+` 建议提示）、开发者模式与 `odr.exe` 可用性及版本（`OdrStatusReport`），并解析已注册服务列表；
     - [x] 支持 URI（`odr.exe mcp add --uri ...`）与清单注册/注销，并实时生成包含 `_meta["com.microsoft.windows"].static_responses` 的 MCPB `"0.3"` 清单，在环境未就绪时平滑回退至本地文件导出（`%LOCALAPPDATA%\KeelDesk\odr\keeldesk.mcpb.manifest.json`）。
  3. **`keel_core::process_host` 受控子进程 MCP 宿主**：
     - [x] 以 `CREATE_NO_WINDOW`、合并注册表 `PATH` 及可选工作目录/环境变量（仅暴露脱敏 `env_keys`）拉起外部 `npx` / `uvx` / `.cmd` / 可执行文件 stdio MCP 服务；
     - [x] 为每个子服务绑定 **Win32 Job Object（`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`）**，并在宿主停止或退出时由内核/运行时干净回收整棵子进程树；
     - [x] 执行实时 `rmcp` stdio 握手（`initialize` + `list_tools`），采集 `server_info_*`、`handshake_ms`、`discovered_tools` 与有界 `stderr_tail` 诊断日志。
  4. **前端 SDK 与工作台 UI（`keel-sdk` & `components/`）**：
     - [x] 交付 `useWinOdr` 与 `useProcessHost` React Hooks，消费 `tauri-specta` 强类型命令及 `ProcessHostUpdatedEvent` 事件流；
     - [x] 在工作台中新增 `Windows ODR` 接入页签、`OdrManifestSheet` 清单抽屉，以及 `ProcessHostPanel`（含新增子进程弹窗、内置 echo 探针预设、运行状态表与工具/stderr 详情抽屉）。

---

### ✅ Phase 4：前端 React SDK 完善与 `<KeelDevTools />`（SDK & DevTools）

- **目标**：沉淀标准化的前端状态管理 Hooks、JSON-RPC 协议追踪链路、HTTP 会话监控与实时协议调试工具。
- **已落地能力**：
  1. **`keel_core::devtools` 与协议遥测链路（`src-tauri/src/keel_core/`）**：
     - [x] 有界 JSON-RPC 协议帧环形缓冲区（`MAX_PROTOCOL_TRACES = 128`，单帧载荷上限 `16 KiB`），同步采集入站 Streamable HTTP 请求与出站 Stdio 子进程 RPC 调用（`McpProtocolTraceEntry`），内置 `approval_wait_ms` vs `execution_ms` 耗时拆解（`McpLatencyBreakdown`）与调用序号关联（`linked_call_id`）。
     - [x] Streamable HTTP 活跃会话追踪（`McpSessionSummary`）、`/healthz` 健康探活命令（`check_keel_healthz`）、追踪查询/清空命令（`get_keel_protocol_traces`、`clear_keel_protocol_traces`）及受控 Stdio 子进程交互式工具调用命令（`call_keel_child_server_tool`）。
  2. **`src/keel-sdk/` 标准化 Hooks**：
     - [x] `useMcpServerState()`：服务运行状态、监听端口、`activeSessions` 活跃会话、`/healthz` 探活与回环自检（保留 `useKeelRuntime` 兼容别名）；
     - [x] `useToolApproval()`：待审批请求队列、单次放行/会话免审白名单/拒绝控制与演练模拟；
     - [x] `useExternalMcpClients()`：受控子进程 MCP 服务启停、状态遥测、`stderrStream` 日志流与 `callClientTool()` 工具试调（保留 `useProcessHost` 兼容别名）；
     - [x] `useKeelDevTools()`：实时 `keel_protocol_trace` 事件订阅、传输通道/状态/关键词筛选、暂停/恢复缓冲与 `selectByLinkedCallId()` 帧定位。
  3. **`<KeelDevTools />` 协议调试抽屉与工作台集成**：
     - [x] 支持 `Ctrl+\`` 快捷键唤醒的可折叠底部抽屉，提供双栏 JSON-RPC 请求/响应 `CodeWell`、审批等待与执行耗时分布条、错误/拦截诊断横幅、活跃 HTTP 会话面板及 `/healthz` 探活触发器；
     - [x] 完整集成至 `App.tsx`（顶部会话计数徽标、底部状态栏 `KeelDevTools` 开关、实时调用观测流一键跳转对应协议帧）与 `ProcessHostPanel.tsx`（已发现工具行内 `试调` 按钮及 `查看报文 #seq` 直达调试器）。

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
    └── env-workbench/          # KeelDesk 本机开发环境管理工作台
```
