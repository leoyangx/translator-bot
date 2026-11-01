# 翻译机器人 · Translator Bot

> 一个跑在自己电脑上的多语言翻译机器人，带桌面控制台。
> Telegram 里转发即翻，14 种语言互译，模型和功能都能在界面上配置。

![界面截图](docs/screenshot.png)

> 架构：Tauri 2（Rust）+ 系统 WebView2 + Python 后端。不打包浏览器内核，安装包 17 MB，解压后约 38 MB。

## 特性

- **14 种语言**：中、英、日、韩、泰、越、印尼、俄、西、法、德、葡、阿、印地
- **自动判断语向**：外语进来译成中文；你说中文，它译成对方语言
- **桌面控制台**：加模型、改功能、调提示词，改完点重启就生效
- **任意 OpenAI 兼容模型**：DeepSeek、通义、智谱、Moonshot、Ollama、OpenRouter
- **术语表**：人名、公司名、游戏术语固定译法
- **可配置**：双语对照、语气按钮、解释按钮、上下文、缓存、群聊响应方式
- **托盘常驻**：关窗不退出，开机可自启
- **自动更新**：从 GitHub Release 检查并安装新版本

## 快速开始

### 方式一：安装包（推荐）

1. 下载 `TranslatorBot-Setup-0.0.1.exe`
2. 双击运行，**安装目录可以自己选**（默认装在当前用户目录，不需要管理员权限）
3. 安装完成会自动启动，开始菜单和桌面都有快捷方式

### 方式二：绿色版

1. 下载 `TranslatorBot-v0.0.1-win64.zip`，解压到**英文路径**
2. 双击 `TranslatorBot.exe`（**不要改文件名**，中文名会导致内核崩溃）

### 首次配置

1. 在「设置」里填 Bot Token，在「模型」里填 API Key
2. 保存 → 重启
3. 在 Telegram 里给机器人发一条消息

### 从源码运行

```bash
git clone https://github.com/<owner>/<repo>.git
cd <repo>
python -m venv .venv
.venv\Scripts\pip install -r requirements.txt
.venv\Scripts\python -m app.console.window
```

## 界面

| 页签 | 作用 |
|---|---|
| 概览 | 运行状态、今日调用、启动/停止、快捷操作 |
| 语言 | 我的语言、对方语言、自动识别开关 |
| 模型 | 多模型管理、测试连接、设默认 |
| 功能 | 双语、语气按钮、解释按钮、上下文、缓存、群聊策略 |
| 提示词 | 翻译/讲解提示词，配试译面板 |
| 日志 | 实时日志、导出诊断包 |
| 设置 | Bot Token、开机自启、关闭行为、维护、关于 |

## 配置

配置文件是 `config.yaml`，与 exe 同级。控制台保存时会自动备份为 `config.yaml.bak`。

```yaml
languages:
  mine: zh
  partners: [ko, ja, en]

features:
  bilingual: true
  tone_buttons: true
  explain_button: true
  group_mode: reply_or_mention   # all | mention | reply_or_mention
  max_input_chars: 6000
```

完整示例见 [config.example.yaml](config.example.yaml)。

### 在群里用

1. 把机器人拉进群
2. 默认只有 **@它** 或 **回复它的消息** 才会响应（避免刷屏）
3. 想翻译某个人的消息：回复那条消息并 @机器人

> 机器人看不到其他机器人的消息；这是 Telegram 的限制。

## 自动更新

- 每次启动/手动点击时读取 GitHub Releases 最新版本
- 下载 zip → 校验 `checksums.txt` 里的 SHA256 → 由 `updater.exe` 替换文件并重启
- 不覆盖 `config.yaml`、`data/`、`logs/`

发布新版本：

```bash
git tag v0.0.2 && git push origin v0.0.2
```

GitHub Actions 会自动构建并上传 `TranslatorBot-v0.0.2-win64.zip` + `checksums.txt`。

## 开发

```
host-tauri/     Rust 宿主（Tauri 2 + 系统 WebView2）：单开、托盘、拉起后端
app/
├─ core/        语言识别、提示词、翻译编排
├─ providers/   OpenAI 兼容适配
├─ storage/     SQLite
├─ bot/         Telegram 层
├─ console/     HTTP 服务、桥接 API、更新、无界面后端
└─ main.py      机器人入口
```

运行机器人（无界面）：`python -m app.main`
运行后端服务：`python -m app.console.serve`

打包分两步：

```powershell
# 1. 后端（PyInstaller，约 34 MB）
python -m PyInstaller --noconfirm --windowed --name TranslatorBotBackend ^
  --add-data "app\console\web;app\console\web" --add-data "assets;assets" ^
  --collect-all langdetect run.py

# 2. 宿主（Tauri / Rust，约 3.5 MB）
.\build-host.ps1
```

宿主构建依赖：`rustup`（`stable-x86_64-pc-windows-gnu`）+ [w64devkit](https://github.com/skeeto/w64devkit) 解压到 `C:\Temp\w64devkit`，无需 Visual Studio。
`build-host.ps1` 会自动镜像到英文路径再编译（中文路径会让 `windres` 失败）。

最终目录只有三项：

```
TranslatorBot.exe           宿主（Tauri + WebView2）
WebView2Loader.dll          WebView2 加载器
backend\                    后端程序、_internal
```

## 常见问题

**Q：提示"未授权"？**
只有第一个给机器人发消息的人会成为主人。要换人，删除 `data/bot.db` 后重新发消息。

**Q：连不上 Telegram？**
大陆网络需要代理，在 `config.yaml` 里填 `proxy: http://127.0.0.1:7890`。
注意 Windows 系统代理对程序无效，必须写进配置。

**Q：窗口一片空白？**
控制台会以 `--disable-gpu` 启动浏览器内核，若仍异常，请更新 Edge / Chrome。

**Q：翻译质量不满意？**
换更强的模型（如 `deepseek-v4-pro`），或在「提示词」里加约束，加术语表。

## License

MIT
