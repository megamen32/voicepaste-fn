# VoicePaste

跨平台（macOS / Windows / Linux）语音转剪贴板应用，基于 **Rust + Tauri v2** 构建。

从麦克风录制音频，通过 Whisper API 进行转录（支持 3 次自动重试 + whisper.cpp 本地回退），并将结果直接粘贴到剪贴板。

![平台](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue)
![许可证](https://img.shields.io/badge/license-MIT-green)

## 功能特性

- **全局快捷键** — 按住或切换模式录制（默认右 Alt）
- **Whisper API 转录** — 兼容 OpenAI 的端点，支持 3 次自动重试
- **本地回退** — whisper.cpp (whisper-rs) 在服务器失败时进行离线转录
- **浮动覆盖层** — 始终置顶的 HUD，跟随光标显示录制状态和转录预览
- **系统托盘** — 完整的设置菜单，包含所有选项
- **录制队列** — 链接多个录制
- **跨平台** — macOS、Windows、Linux 单一代码库
- **自动启动** — LaunchAgent (macOS)、Registry (Windows)、XDG (Linux)
- **可配置** — 端点、API 密钥、语言、模型、延迟、快捷键、激活模式

## 安装

### 从 DMG 安装 (macOS)

1. 从 [Releases](../../releases) 下载 `VoicePaste_1.0.0_aarch64.dmg`
2. 将 `VoicePaste.app` 拖到 Applications
3. 启动并授予麦克风 + 辅助功能权限

### 从源码构建

```bash
# 前置要求：Rust 工具链、cmake
cargo install tauri-cli --version "^2"

cd VoicePasteTauri/src-tauri
cargo tauri build
```

构建后的应用位置：
- macOS: `target/release/bundle/macos/VoicePaste.app`
- macOS DMG: `target/release/bundle/dmg/VoicePaste_*.dmg`
- Windows: `target/release/bundle/msi/VoicePaste_*.msi`
- Linux: `target/release/bundle/deb/voicepaste_*.deb`

## 使用方法

1. **启动** 应用 — 菜单栏会出现托盘图标
2. **按住** 右 Alt（或配置的快捷键）开始录制
3. **松开** 停止并转录
4. 转录文本自动粘贴到光标位置

### 切换模式

在托盘菜单中，将激活模式切换为 **Toggle**：
- 第一次按下：开始录制
- 第二次按下：停止并转录

### 托盘菜单选项

| 选项 | 描述 |
|------|------|
| Settings > Endpoint | Whisper API 基础 URL |
| Settings > API Key | 您的 API 密钥 |
| Recording delay | 录制开始前的延迟 (0.2–2.0秒) |
| Preview hide delay | 预览保持可见的时间 (0–5秒) |
| Language | ru / en / auto |
| Model | Whisper 模型选择 |
| Realtime preview | 实时转录，可配置间隔 |
| Autostart | 系统启动时运行 |
| Hotkey | 选择全局快捷键 |
| Activation mode | 按住 或 切换 |
| Centre overlay | 将覆盖层固定在屏幕中心 |
| Wake server | 录制前发送静默请求 |
| Local fallback | 服务器失败时使用 whisper.cpp |

## 配置

设置以 JSON 格式存储在平台的应用数据目录中：

- **macOS**: `~/Library/Application Support/com.bezrabotnyi.voicepaste/settings.json`
- **Windows**: `%APPDATA%\com.bezrabotnyi.voicepaste\settings.json`
- **Linux**: `~/.config/com.bezrabotnyi.voicepaste/settings.json`

环境变量在启动时覆盖设置：

| 变量 | 描述 |
|------|------|
| `OPENAI_BASE_URL` | Whisper API 端点 |
| `OPENAI_API_KEY` | API 密钥 |
| `TRANSCRIBE_MODEL` | 模型名称 |

## 开发

```bash
cd VoicePasteTauri/src-tauri

# 检查编译
cargo check

# 运行测试（30 个单元测试）
cargo test

# 开发模式运行
cargo tauri dev

# 生产构建
cargo tauri build
```

## 技术栈

- **Rust** — 后端语言
- **Tauri v2** — 跨平台桌面框架
- **cpal** — 跨平台音频 I/O
- **hound** — WAV 编码
- **whisper-rs** — 本地 whisper.cpp 语音转文字
- **reqwest** — Whisper API HTTP 客户端
- **core-graphics** — 原生光标位置 (macOS)

## 翻译

- [English](README.md)
- [Русский](README_RU.md)

## 许可证

MIT
