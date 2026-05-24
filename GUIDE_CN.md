# API Switch 使用指南

## 快速开始

### 1. 添加渠道

进入「渠道管理」，点击添加渠道，填写：

| 配置项 | 说明 |
|--------|------|
| 名称 | 自定义，方便识别 |
| API 类型 | 选择对应的服务商类型 |
| Base URL | 上游 API 地址（不需要带 `/v1`） |
| API Key | 服务商提供的密钥 |

添加后点击「拉取模型」获取可用模型列表，勾选需要使用的模型。

### 2. 启用模型

进入「API 管理」，开启需要使用的模型条目（绿点 = 可用）。

### 3. 开始使用

将客户端的 API Base URL 设为：

```
http://127.0.0.1:9090/v1
```

API Key 留空即可（未开启访问密钥验证时）。

请求时 model 填 `auto` 即可自动匹配最优渠道，也可以填具体模型名称。

---

## 上游 API 类型

API Switch 支持接入以下类型的上游 API 服务：

| API 类型 | 说明 | 默认 Base URL |
|----------|------|---------------|
| `OpenAI` | 标准 OpenAI API | `https://api.openai.com` |
| `Anthropic` | Claude 系列模型，自动做协议转换 | `https://api.anthropic.com` |
| `Google Gemini` | Gemini 系列模型，自动做协议转换 | `https://generativelanguage.googleapis.com` |
| `Azure OpenAI` | Azure 部署，需填写部署名称 | — |
| `OpenAI-compatible` | 兼容 OpenAI 协议的第三方服务（中转站、硅基流动等自定义服务） | — |
| `OpenAI Responses (Beta)` | OpenAI Responses API，支持 `/v1/responses` 端点 | `https://api.openai.com` |

> 添加渠道时，Base URL 不需要带 `/v1` 后缀。如使用 CODING PLAN 等中转站选择 `OpenAI-compatible` 类型。

---

## 无头服务（Headless）模式

无需桌面环境，仅启动 API 转发服务和 Web 管理界面，适合部署在服务器上。

### 启动方式

```bash
# 方式一：命令行参数
./api-switch --headless

# 或
./api-switch --standalone
# --nodisktop 同样效果
```

```bash
# 方式二：环境变量
API_SWITCH_HEADLESS=1 ./api-switch
```

> **Linux 无桌面环境**：系统没有 `DISPLAY` 或 `WAYLAND_DISPLAY` 时会自动进入 Headless 模式，无需手动指定参数。

启动后终端输出示例：

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  API Switch is running
  Proxy:      http://127.0.0.1:9090/v1/...
  Web Admin:  http://127.0.0.1:9090
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Press Ctrl+C to stop
```

### Web 管理界面

无头模式下，API 转发服务和 Web 管理界面共用一个端口（默认 `9090`）。

| 项目 | 说明 |
|------|------|
| 访问地址 | `http://<服务器IP>:9090/admin` |
| 默认用户名 | `admin` |
| 默认密码 | `admin` |

登录后可查看仪表盘、管理渠道和 API、查看日志等。默认用户/密码可在「系统设置 → Web 管理」中修改。

> **环境变量覆盖**：启动前设置 `API_SWITCH_ADMIN_USER` 和 `API_SWITCH_ADMIN_PASS` 可覆盖默认管理员账号密码。

---

## 下游接入说明

客户端通过代理端口（默认 `9090`）接入，支持如下协议端点：

### 代理端口（默认 9090）

| 端点 | 方法 | 说明 |
|------|------|------|
| `/v1/chat/completions` | POST | OpenAI 标准聊天补全（stream 与非 stream） |
| `/v1/messages` | POST | Anthropic Claude 协议格式（自动转换） |
| `/v1/responses` | POST | OpenAI Responses API |
| `/v1/models` | GET | OpenAI 格式模型列表 |
| `/anthropic/v1/models` | GET | Anthropic 格式模型列表 |
| `/v1beta/models` | GET | Gemini 格式模型列表 |
| `/openai/deployments` | GET | Azure 格式部署列表 |
| `/v1beta/models/{model}:generateContent` | POST | Gemini 原生格式 |
| `/openai/deployments/{deployment}/chat/completions` | POST | Azure 原生格式 |
| `/health` | GET | 健康检查 |

客户端配置示例：

```
# OpenAI 兼容客户端
API Base URL: http://127.0.0.1:9090/v1
API Key:      <留空或访问密钥>

# Claude 客户端（如 Claude Code）
API Base URL: http://127.0.0.1:9090
API Key:      <留空或访问密钥>
```

### Web 管理端口

| 模式 | 默认端口 | 说明 |
|------|---------|------|
| 合并模式（默认） | `9090`（与代理同端口） | 访问 `http://127.0.0.1:9090/admin` |
| 独立模式 | `9099` | 访问 `http://127.0.0.1:9099/admin` |

Web 管理端口可在「系统设置 → Web 管理」中修改。

---

## CODING PLAN / API-Switch 推荐配置

### MiniMax（硅基流动）

| 配置项 | 值 |
|--------|-----|
| API 类型 | `openai` 或 `anthropic` |
| Base URL | `https://api.minimaxi.com` 或 `https://api.minimax.chat` |
| API Key | 你的 Key |
| API 管理添加模型 | `MiniMax-M2.7`（需手动填写） |

### CODING PLAN

| 配置项 | 值 |
|--------|-----|
| API 类型 | `openai` |
| Base URL | `https://api.rcouyi.com` |
| API Key | 你的 Key |
| 拉取模型 | 不支持，需手动添加 |
| API 管理添加模型 | `gemini-2.0-flash`、`gemini-2.5-pro` 等（需手动填写） |

> 此类中转站的模型列表接口不可用，拉取会失败。在渠道中添加 Key 后，直接到「API 管理」点击「添加 API」手动填写模型名称即可正常使用。

---

## 常见问题

### 代理启动失败：端口被占用

修改「系统设置 → 代理设置 → 监听端口」为其他端口，重启代理。

### 请求返回 401

1. 如果开启了「强制验证访问密钥」，请求必须携带 Access Key：
   ```
   Authorization: Bearer sk-xxx
   ```
2. 如果未开启，检查渠道的 API Key 是否正确。

### 请求返回 "No available provider"

1. 确认渠道已启用。
2. 确认对应的模型条目已启用（绿点）。
3. 确认模型未处于冷却中（红点）。冷却中的模型不参与路由。

### 模型显示红点（冷却中）

模型请求失败后会被自动冷却，冷却期间不参与路由。默认冷却时间 300 秒，可在「系统设置 → 熔断机制」中调整。冷却到期后模型会自动恢复。

### 拉取模型失败

1. 检查 Base URL 是否正确（不需要带 `/v1` 后缀）。
2. 检查 API Key 是否有效。
3. 检查网络是否能访问上游 API。

### Claude 渠道配置

- API 类型选择 `claude`
- Base URL 填写 `https://api.anthropic.com`
- 模型名称需要与上游一致，如 `claude-sonnet-4-20250514`

### Gemini 渠道配置

- API 类型选择 `gemini`
- Base URL 填写 `https://generativelanguage.googleapis.com`
- API Key 填写 Google AI Studio 的 Key

### Azure OpenAI 配置

- API 类型选择 `azure`
- Base URL 填写 Endpoint 地址
- 模型名称填写部署名称（Deployment Name）

### 托盘菜单模型顺序不对

托盘显示 AUTO 组中前 5 个启用条目，顺序跟随 API 管理页。在 API 管理页拖拽即可调整 AUTO 组优先级。

### 日志中看到 (auto) 前缀

表示请求的 model 为 `auto`，括号后是实际命中的模型名称。

---

> 本文档会持续更新，如果遇到其他问题欢迎提 [Issue](https://github.com/wang1970/API-Switch/issues)。
