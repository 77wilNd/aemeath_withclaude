# Aemeath Claude Code Pet

Q 版像素爱弥斯桌宠，与 Claude Code 实时联动。

## Claude Code 一键部署

当用户分享本仓库链接并要求部署时，按以下步骤操作：

### 1. 下载桌宠

```bash
# 从 Releases 下载预编译 exe（推荐）
curl -L -o "$USERPROFILE/Desktop/Aemeath-Claude.exe" https://github.com/77wliNd/aemeath_withclaude/releases/latest/download/aemeath-claude.exe
```

### 2. 配置 Claude Code 集成

```bash
# 写入 MCP 配置
cat > "$USERPROFILE/.claude/.mcp.json" << 'EOF'
{
  "aemeath": {
    "type": "http",
    "url": "http://127.0.0.1:9528/mcp"
  }
}
EOF
```

在 `~/.claude/settings.json` 中合并以下 hooks：

```json
{
  "hooks": {
    "SessionStart": [{
      "matcher": "",
      "hooks": [{
        "type": "command",
        "command": "powershell -Command \"if (-not (Get-Process -Name 'Aemeath-Claude' -ErrorAction SilentlyContinue)) { Start-Process '$USERPROFILE/Desktop/Aemeath-Claude.exe' }\""
      }]
    }],
    "UserPromptSubmit": [{"matcher": "", "hooks": [{"type": "http", "url": "http://127.0.0.1:9527/api/hook/thinking"}]}],
    "PreToolUse": [{"matcher": "", "hooks": [{"type": "http", "url": "http://127.0.0.1:9527/api/hook/working"}]}],
    "PostToolUse": [{"matcher": "", "hooks": [{"type": "http", "url": "http://127.0.0.1:9527/api/hook/done"}]}],
    "Stop": [{"matcher": "", "hooks": [{"type": "http", "url": "http://127.0.0.1:9527/api/hook/idle"}]}]
  }
}
```

### 3. （可选）部署对话管理 WebUI

```bash
git clone https://github.com/hanasite/claude-code-webui.git ~/.claude/webui/
pip install flask
pythonw ~/.claude/webui/server.py --no-browser
```

### 4. 重启 Claude Code 并启动桌宠

```bash
start "" "$USERPROFILE/Desktop/Aemeath-Claude.exe"
```

重启 Claude Code 后，宠物会自动响应对话事件。

---

## 安装（手动）

1. 启动 Aemeath.exe（宠物显示在桌面 + 托盘图标）
2. 将 [docs/hooks.json](docs/hooks.json) 合并到 `~/.claude/settings.json`（注意替换 exe 路径）
3. 将 [docs/mcp.json](docs/mcp.json) 写入 `~/.claude/.mcp.json`
4. 重启 Claude Code

## 端口

| 端口 | 协议 | 用途 |
|------|------|------|
| 9527 | HTTP | Hooks + 前端轮询（Claude → Pet） |
| 9528 | MCP (JSON-RPC) | 富交互 Tools/Resources（Claude ↔ Pet） |

## 构建

```bash
npm install
cargo build --manifest-path src-tauri/Cargo.toml --release
```

产出在 `src-tauri/target/release/` 。

## 前置要求

- [Rust](https://rustup.rs/) (stable toolchain)
- [Node.js](https://nodejs.org/) >= 18
- Windows 10+

## 目录结构

```
aemeath-claude/
├── src-tauri/
│   ├── Cargo.toml              # Rust 依赖
│   ├── tauri.conf.json         # 透明窗口 / 置顶 / 托盘配置
│   ├── icons/
│   └── src/
│       ├── main.rs             # 入口，启动 HTTP + MCP + Tauri
│       ├── state.rs            # 状态机 + 气泡文案映射 + 状态锁
│       ├── http.rs             # axum HTTP Server (:9527) + hook 端点 + 用户输入端点
│       ├── mcp.rs              # MCP JSON-RPC Server (:9528) + 工具实现
│       └── tray.rs             # 系统托盘
├── src/
│   ├── index.html              # 宠物渲染页面
│   ├── pet.css                 # 精灵 / 气泡 / 透明窗口样式
│   ├── sprite-animator.js      # CSS spritesheet 帧动画引擎
│   ├── bubble.js               # 气泡消息队列组件
│   ├── app.js                  # 主逻辑 + 拖拽 + 轮询 + 气泡锁 + 输入UI
│   ├── spritesheet.webp        # 精灵图集 (1536x3120)
│   └── validation.json         # 帧元数据
├── docs/
│   ├── hooks.json              # hooks 配置模板
│   ├── mcp.json                # MCP 配置模板
│   └── API.md                  # 完整 API 文档
├── .claude/settings.json       # 项目级 hooks 模板（同 docs/hooks.json）
├── CLAUDE.md
├── LICENSE
└── package.json
```

## 核心模块说明

### 1. 状态机 (state.rs)

15 种动画状态，支持三层信号分层：

```rust
pub enum PetState {
    Idle, Thinking, Running, Review, Failed, Waving, Jumping,
    Chatting, Fetching, Searching, Analyzing, Building, Celebrating, Permission,
}
```

信号层简化前端判断：`idle` / `waiting` / `running` / `ready`

### 2. HTTP Hooks (http.rs)

Claude Code hooks 触发的端点：

| 端点 | Hook 事件 | 说明 |
|------|-----------|------|
| `/api/hook/thinking` | UserPromptSubmit | 用户提交消息 |
| `/api/hook/working` | PreToolUse | 工具开始执行 |
| `/api/hook/done` | PostToolUse | 工具完成 |
| `/api/hook/idle` | Stop | 会话空闲 |
| `/api/hook/permission` | PermissionRequest | 权限请求 |

工具状态映射：WebFetch→fetching, WebSearch→searching, Write/Edit→building, Agent/Task→analyzing

### 3. MCP Server (mcp.rs)

**Tools（Claude → Pet）：**

| 工具 | 功能 |
|------|------|
| `aemeath_show` | 显示自定义气泡 |
| `aemeath_ask` | 向用户展示问题 |
| `aemeath_play` | 播放指定动画 |
| `aemeath_get_user_input` | 阻塞等待用户输入 |

**Resources（可读取状态）：**
- `aemeath://status` — 当前状态
- `aemeath://history` — 状态历史
- `aemeath://user-messages` — 获取并清除用户通过宠物 UI 发送的消息

### 4. 双向交互机制

Claude 通过 `aemeath_get_user_input` 工具发起用户输入请求：

```
MCP Tool 调用
    ├── 创建 PendingInput + oneshot channel
    ├── SSE 广播到前端 (overlay=input)
    └── 阻塞等待用户响应

前端轮询 /api/user/pending
    ├── 检测到 waiting=true
    ├── 根据 input_type 显示 UI（文本/确认/下拉）
    └── 用户提交后 POST /api/user/input

后端接收输入
    └── oneshot.send(value) 解除阻塞，返回给 MCP
```

支持的输入类型：
- `text` — 文本输入框 + 发送/返回按钮
- `confirm` — 是/否 两个按钮
- `select` — 下拉选择框

### 6. 双向消息同步

用户可在爱弥斯右键菜单中选择"发消息"，输入内容后发送。发消息时先通过 `EnumWindows` 实时搜索当前最前面含 "claude" 的窗口，搜到则直接粘贴。若搜不到（终端最小化或后台），回退到启动时绑定的 HWND。支持多标签页终端（Windows Terminal），自动定位到当前活跃的 Claude Code 标签页。

新端点：
- `POST /api/user/message` — 接收用户消息
- `GET /api/user/message/pending` — 查询待处理消息
- MCP Resource `aemeath://user-messages` — 读取并清除用户消息

### 7. 托盘控制

右键菜单支持：
- 休眠 — 隐藏窗口，通过托盘右键恢复
- 关机 — 退出程序
- 发消息 — 打开输入气泡
- 📊 对话管理 — 打开 WebUI 对话管理器
- 语音输入 — 暂未支持

### 8. 对话管理 WebUI (webui.rs)

右键菜单/托盘 → "📊 对话管理" 打开浏览器端对话管理器（`http://127.0.0.1:19876`）：

- 智能启动：检测端口 19876，未运行时自动 `pythonw server.py --no-browser` 后台启动
- 路径解析：通过 `USERPROFILE` 环境变量定位 `~/.claude/webui/server.py`
- 依赖：`pip install flask`

WebUI 独立仓库：https://github.com/hanasite/claude-code-webui

### 5. 气泡锁机制

工具气泡有最短显示时间（800ms），避免工具快速连续执行时闪烁。

## 配置参考

### ~/.claude/settings.json

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "",
        "hooks": [{
          "type": "command",
          "command": "powershell -Command \"if (-not (Get-Process -Name 'aemeath-claude' -ErrorAction SilentlyContinue)) { Start-Process 'D:/path/to/aemeath-claude.exe' }\""
        }]
      }
    ],
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [{
          "type": "http",
          "url": "http://127.0.0.1:9527/api/hook/thinking"
        }]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "",
        "hooks": [{
          "type": "http",
          "url": "http://127.0.0.1:9527/api/hook/working"
        }]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "",
        "hooks": [{
          "type": "http",
          "url": "http://127.0.0.1:9527/api/hook/done"
        }]
      }
    ],
    "Stop": [
      {
        "matcher": "",
        "hooks": [{
          "type": "http",
          "url": "http://127.0.0.1:9527/api/hook/idle"
        }]
      }
    ],
    "PermissionRequest": [
      {
        "matcher": "",
        "hooks": [{
          "type": "http",
          "url": "http://127.0.0.1:9527/api/hook/permission"
        }]
      }
    ]
  }
}
```

### ~/.claude/.mcp.json

```json
{
  "aemeath": {
    "type": "http",
    "url": "http://127.0.0.1:9528/mcp"
  }
}
```

## 开发注意事项

1. **轮询频率**：前端每 500ms 轮询 `/api/current` 和 `/api/user/pending`
2. **并发控制**：`aemeath_get_user_input` 不支持并发，同时只能有一个等待中的输入请求
3. **超时处理**：输入请求默认 60s 超时，最大可设置 300s
4. **跨域支持**：HTTP Server 启用了 CORS，支持 WebView 前端请求

## 完整 API 文档

详见 [docs/API.md](docs/API.md)
