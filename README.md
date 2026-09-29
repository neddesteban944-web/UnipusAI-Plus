# UnipusAI-Plus

U校园 AI 版课程的自动答题 / 刷课工具（Rust 实现），**任意账号、任意课程**都能用：
粘贴一条浏览器请求即可完成配置，内置听力音频识别（ffmpeg + 本地 Whisper）。

> 本站基于 [Zzj-klwgxdz/UnipusAI](https://github.com/Zzj-klwgxdz/UnipusAI) 修改而来（GPL-3.0），
> 修复了若干解析/转写问题，并新增了「一键配置」「课程列表」「选课」等命令。

## ⚠️ 免责声明

本项目**仅供学习交流与技术研究**（HTTP 接口分析、异步 Rust、本地语音识别等）。
请勿用于商业用途、代刷牟利或违反你所在学校规定的场景。
使用本工具产生的一切后果（包括但不限于成绩、纪律处分）由使用者自行承担。

## 特点

- **一行配置**：`UnipusAI init` 粘贴浏览器复制的 cURL（或 Cookie 那一行），
  自动解析出 `cookie / open_id / class_id / curricula_id`，并按页面算法生成
  `x-annotator-auth-token`，无需手工到处找字段
- **任意账号任意课程**：`UnipusAI courses` 列出账号下的课程（含 `course_id`），`UnipusAI use <序号>` 选定
- **听力题型支持**：内嵌脚本 → WEBVTT 字幕 → **本地 Whisper 转写**（不依赖在线语音识别）
- **无材料题自动补材料**：题目没有任何材料时，自动取同单元微课视频的文字/转写喂给大模型
- **长材料不截断**：材料上限可配（默认 16000 字符）
- **健壮性**：接口返回小数/`null` 也能解析；命中服务端限频自动冷却重试；已通过任务自动跳过
- **纯命令行、无需浏览器、无需 WebDriver**

## 快速开始

### 方式一：下载编译好的程序（推荐）

1. 到 [Releases](../../releases) 下载 `UnipusAI.exe`（Windows x64）
2. 新建一个文件夹（例如 `D:\UnipusAI`），把 exe 放进去
3. 双击 `首次配置.cmd`（若没随包提供，直接双击 `UnipusAI.cmd` 选 0）
4. 按提示粘贴浏览器里的请求，然后选课、填大模型 API Key
5. 双击 `UnipusAI.cmd` → 菜单里选「全量自动答题」

### 方式二：自行编译

需要 Rust 1.85+（本工程使用 edition 2024）与 MSVC 生成工具（Windows）：

```bash
cargo build --release
# 产物：target/release/UnipusAI.exe
```

Windows 上也可以双击 `scripts\build.cmd`。

## 三步完成配置

### 1. 粘贴请求，生成 config.json

```
UnipusAI.exe init
```

它提示你粘贴内容，获取方式：

1. 浏览器登录 U校园（`ucontent.unipus.cn`）并进入任意课程
2. 按 `F12` → **网络 / Network** → 点 **Fetch/XHR** 过滤
3. 刷新页面，右键任意 `ucontent.unipus.cn` 的请求 → **复制 → 以 cURL 格式复制**
4. 回到终端粘贴（可多行），按 `Ctrl+Z` 再回车结束输入

> 只复制请求头里的 `Cookie:` 那一整行也可以，程序同样能识别。
> 详细步骤与字段含义见 [docs/配置教程.md](docs/配置教程.md)。

### 2. 选择课程

```
UnipusAI.exe courses     # 列出账号下的课程与 course_id
UnipusAI.exe use 2       # 选择第 2 门课程（自动写入 course_id 等字段）
```

`init` 结束时也会直接把课程列表打出来，照着序号执行 `use` 即可。

### 3. 填大模型配置

编辑 `config.json`：

```json
{
  "api_key": "sk-xxxx",
  "base_url": "https://api.deepseek.com",
  "model": "deepseek-flash"
}
```

任何 OpenAI 兼容接口都可以（DeepSeek、Kimi、通义、本地 Ollama 等）。

## 命令一览

| 命令 | 说明 |
| --- | --- |
| `init` | 首次配置：解析粘贴内容，生成/更新 `config.json` |
| `courses` | 列出账号下的课程（含 `course_id` / `class_id` / `curricula_id`） |
| `use <序号>` | 选定要刷的课程，写入配置 |
| `progress [--names]` | 打印课程全部单元/任务树（按 `learning_strategy` 过滤） |
| `run [--names] [--interval <毫秒>] [unitId...]` | 自动完成课程（默认全部单元，也可指定单元） |
| `group <groupId> [--force]` | 直接提交指定任务组；已通过默认跳过，`--force` 强制重做 |
| `debug <groupId> [--force]` | 本地求解指定任务组，**不提交**（调试用） |
| `test-types` | 每种题型抽一题测试答题链路，不提交 |
| `transcribe <url>` | 测试媒体转写链路（下载 → ffmpeg → whisper） |
| `dump-text [--names] [--force] [unitId...]` | 导出题目文本与媒体转写到 `dump_text/`，不答题 |

组合使用建议：

```bash
UnipusAI.exe progress --names     # 1. 看结构，确认配置正确
UnipusAI.exe dump-text --names    # 2. 导出题目（顺便把听力转写缓存下来）
UnipusAI.exe test-types           # 3. 题型自测
UnipusAI.exe debug <groupId>      # 4. 单组预览，满意再提交
UnipusAI.exe run --interval 3000  # 5. 全量答题
```

## 配置项说明

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `cookie` | ✅ | 浏览器请求头里的 Cookie（含 `jwt=`），`init` 会自动填 |
| `authorization` | | 登录 JWT，留空则自动用 cookie 里的 `jwt=` |
| `x_annotator_auth_token` | | `init` 会自动生成（复刻前端 `_generateJwtToken`），也可留空由程序生成 |
| `course_id` | ✅ | `course-v2:...`，用 `courses` + `use` 自动填 |
| `class_id` / `curricula_id` | | 讨论题需要的班级/课程 id，`use` 会自动填 |
| `open_id` | ✅ | 用户 open id，`init` 从 jwt 里解析 |
| `api_key` / `base_url` / `model` | ✅ | 大模型（OpenAI 兼容）配置 |
| `learning_strategy` | | `learn_all_compulsory_course`（只做必修，默认）/ `learn_all`（全部） |
| `max_material_chars` | | 单模块材料上限，默认 16000（过长材料会被截断） |
| `max_tokens` / `temperature` | | 大模型参数，默认 4096 / 0.3 |
| `fallback_on_llm_failure` | | LLM 失败时是否随机作答兜底，默认 true |
| `interval_ms` | | 两次提交间隔，默认 3000ms |
| `whisper_enabled` / `whisper_model` / `whisper_language` | | 本地语音转写开关/模型/语种（`auto` 自动检测） |
| `timeout` | | HTTP 超时（秒），音频下载多时建议 30+ |

## 听力与音频识别

取听力材料的优先级：

1. 题目里内嵌的文本 / `WEBVTT` 字幕（多数听力题自带脚本，直接使用，速度最快）
2. 都没有时：下载音频/视频 → **ffmpeg** 转 16k 单声道 wav → **本地 Whisper** 转写（结果按 URL + 语种缓存）

安装 ffmpeg（二选一）：

- 把 ffmpeg 的 `bin` 目录加入系统 `PATH`
- 或把 `ffmpeg.exe` 放到本目录的 `tools\ffmpeg\bin\`（启动器会自动加入 PATH）

Whisper 模型首次使用会自动从 HuggingFace 下载并缓存到 `~/.cache/whisper-candle/`（可用 `HF_HOME` 改路径）。
国内网络建议设置镜像（启动器已默认设置）：

```bash
set HF_ENDPOINT=https://hf-mirror.com
```

中文微课 + 英文听力混合的课程，`whisper_language` 保持 `auto` 即可；
纯英文课程可以设为 `en`，纯中文设为 `zh`。想要更准可以把 `whisper_model` 改成 `small`。

## 常见问题

见 [docs/常见问题.md](docs/常见问题.md)，常见几类：

- Cookie 过期（`jwt` 有效期通常 1~2 天）→ 重新 `init` 粘贴一次
- 提示 401 / `unAuthenticated` → 先看看 `courses` 能不能列出课程
- 提示「操作过于频繁」→ 程序会自动等待冷却重试，也可以把 `interval_ms` 调大
- 成绩不满分/任务没通过 → 用 `debug <groupId>` 看具体作答，必要时换更强模型
- 想只做必修 → `learning_strategy` 设为 `learn_all_compulsory_course`

## 目录结构

```
src/
├── main.rs          # CLI 入口与各子命令
├── setup.rs         # 首次配置：解析 cURL/Cookie、生成 x-annotator 令牌、课程列表
├── config.rs        # config.json 读写与校验
├── llm.rs           # OpenAI 兼容接口调用
├── solve.rs         # 各题型提示词构造与答案解析
├── transcribe.rs    # 媒体转写：字幕解析 + ffmpeg + whisper，带缓存
├── dump.rs          # dump-text 导出与状态汇总
├── api/             # 会话、课程、内容、提交、讨论区接口
└── core/            # 计划与执行器
```

## 与上游的差异

上游 `Zzj-klwgxdz/UnipusAI` 的基础上做了以下修复与增强：

1. **新增 `init` / `courses` / `use`**：粘贴一次请求即可完成配置，并自动选择课程
2. **宽松数值解析**：`min_score_pct` 可能是小数（如 `0.6`）、`start_time`/`end_time` 可能是 `null`，
   上游会直接 `反序列化失败` 导致整门课刷不动
3. **媒体相对路径支持**：部分课程（如四级听力）返回 `qs-prod/...` 这类相对路径，自动补全
   `https://birdflock.unipus.cn/`
4. **无材料题自动补材料**：取同单元微课视频的文字/转写作为材料，避免大模型凭空猜
5. **材料长度上限可配**：上游硬编码 4000 字符，长阅读/长对话会被截断；现默认 16000 且可配置
6. **转写缓存按语种区分**：切换 `whisper_language` 后会重新识别（避免复用错误语种的缓存）
7. **显式使用 rustls**：避免依赖系统 schannel，跨环境更稳定

## 致谢与许可

- 原项目：[Zzj-klwgxdz/UnipusAI](https://github.com/Zzj-klwgxdz/UnipusAI)（GPL-3.0）
- 本项目同样以 **GPL-3.0** 发布，详见 [LICENSE](LICENSE)

如果这个项目对你有帮助，请给原项目也点个 Star。
