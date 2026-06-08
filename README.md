# Aemeath Claude Code Pet

Q 版像素爱弥斯桌宠，通过 HTTP hooks 与 MCP 与 Claude Code 实时联动。基于 MIT 像素小人素材制作，参考《鸣潮》爱弥斯官方视觉设定。

> 这是粉丝制作的桌宠项目，不是库洛游戏或《鸣潮》的官方项目。

## ⚡ Claude Code 一键部署

复制以下提示词发送给你的 Claude Code，自动完成部署：

```
请根据 https://github.com/hanasite/aemeath_withclaude/blob/main/CLAUDE.md 部署爱弥斯桌宠，包括下载 exe、配置 hooks 和 MCP、启动桌宠。
```

部署内容：下载 exe → 桌面 → 配置 hooks + MCP → `~/.claude/` → 启动 → （可选）部署对话管理 WebUI

## 功能

- 15 种像素动画状态，随 Claude Code 操作实时切换
- 气泡消息精准反馈 Claude 当前行为，工具间保持最小停留时间不闪烁
- 空闲时随机展示动画（跳跃 / 招手 / 待机变体）
- 透明无边框桌面悬浮窗，始终置顶，可拖拽，不占任务栏
- 系统托盘驻留，左键切换显隐，右键菜单
- 随 Claude Code 自动启动，不重复创建实例
- **双向交互**：通过 MCP 工具让 Claude 向用户发起输入请求（文本/确认/下拉选择）
- **消息同步**：右键精灵 → 发消息，内容自动粘贴到 Claude Code 对话中
- **右键快捷菜单**：发消息 / 对话管理 / 休眠 / 关机
- **📊 对话管理 WebUI**：一键打开 Claude Code 对话管理器，浏览/搜索/导出历史对话、管理 Skills

## 联动效果

| Claude 操作 | 宠物动画 | 气泡 |
|---|---|---|
| 收到消息 | chatting | "正在组织回复..." |
| Read / Grep / Glob | running | "正在读取文件..." |
| Write / Edit | building | "正在构建..." |
| Bash | running | "正在执行命令..." |
| Agent / Task | analyzing | "正在分析..." |
| WebFetch | fetching | "正在获取网络内容..." |
| WebSearch | searching | "正在搜索网络..." |
| 其他工具 | running | "工作中..." |
| 工具执行完毕 | celebrating | "太棒了!" |
| 权限请求 | waving | "等待指示..." |
| 空闲 | idle | — |

## MCP 工具

通过 MCP 协议，Claude 可以：

| 工具 | 功能 |
|------|------|
| `aemeath_show` | 显示自定义气泡消息 |
| `aemeath_ask` | 向用户展示问题（非阻塞） |
| `aemeath_play` | 强制播放指定动画 |
| `aemeath_get_user_input` | 阻塞等待用户输入（支持文本/确认/下拉选择） |

**示例：让 Claude 向用户请求确认**
```
用户: 删除这个文件
Claude: 调用 aemeath_get_user_input(type="confirm", prompt="确定要删除吗？")
宠物: 显示是/否按钮
用户: 点击"是"
Claude: 执行删除操作
```

详细 API 文档见 [docs/API.md](docs/API.md)

## 架构

```
Claude Code
  ├── HTTP hooks → POST :9527/api/hook/*
  └── MCP Client → :9528/mcp

Aemeath Pet (Tauri Desktop App)
  ├── HTTP Server (:9527)   → 接收 hook 推送 + 前端轮询
  ├── MCP Server (:9528)    → 富交互（tools / resources）
  ├── State Manager (Rust)  → 状态机 + 气泡锁
  └── WebView Frontend      → CSS sprite 动画 + 气泡 + 输入 UI
```

## 安装

### 1. 启动桌宠

从 [Releases](../../releases) 下载 `aemeath-claude.exe`，或自己构建：

```bash
npm install
cargo build --manifest-path src-tauri/Cargo.toml --release
```

产出在 `src-tauri/target/release/` 。

### 2. 配置 Claude Code

将 [docs/hooks.json](docs/hooks.json) 合并到 `~/.claude/settings.json`，将 [docs/mcp.json](docs/mcp.json) 写入 `~/.claude/.mcp.json`，然后重启 Claude Code。注意替换 hooks.json 中 `SessionStart` 里 `aemeath-claude.exe` 的实际路径。

## 📊 对话管理 WebUI（本 Fork 新增）

右键宠物或托盘菜单点击"📊 对话管理"即可打开 Claude Code 对话管理器：

- **会话浏览**：按项目分组查看所有历史对话，支持排序和搜索
- **会话详情**：分页查看消息内容，支持删除和 Markdown 导出
- **标签系统**：为重要会话添加自定义标签
- **Skills 管理**：浏览已安装 Skills、搜索 SkillHub 商店、一键安装/卸载
- **智能启动**：自动检测 WebUI 服务状态，未运行时后台启动（通过 `pythonw` 无窗口运行）

### 依赖

```bash
pip install flask
```

WebUI 服务运行在 `http://127.0.0.1:19876`。

## 端口

| 端口 | 用途 | 方向 |
|---|---|---|
| 9527 | HTTP — hooks 推送状态 + 前端轮询 | Claude → Pet |
| 9528 | MCP — 富交互（tools / resources） | Claude ↔ Pet |
| 19876 | WebUI — 对话管理 & Skills 管理 | 用户 → 浏览器 |

## 构建

### 前置要求

- [Rust](https://rustup.rs/) stable toolchain（需 windows-gnu + MinGW-w64）
- [Node.js](https://nodejs.org/) >= 18
- Windows 10+

### 命令

```bash
npm install
cargo build --manifest-path src-tauri/Cargo.toml --release
```

## 目录结构

```
aemeath-claude/
├── src-tauri/        # Rust 后端 (Tauri + axum)
├── src/              # WebView 前端 (HTML/CSS/JS)
├── docs/             # hooks 与 MCP 配置模板 + API 文档
├── CLAUDE.md         # 项目指南
├── LICENSE
└── package.json
```

详细文件说明见 [CLAUDE.md](CLAUDE.md)。

## 来源与授权

- 像素小人素材来源：[lzy-buaa-jdi/ameath](https://gitee.com/lzy-buaa-jdi/ameath)，MIT License
- 爱弥斯、《鸣潮》及相关官方视觉设定归其权利方所有
- 本仓库仅包含整理后的桌宠代码、精灵图集，不含官方立绘原图
