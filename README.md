# PhotoCurate

**AI 驱动的照片管理与精选工具。** 自动为你的照片打分，用自然语言搜索照片内容，一键导出高分作品。

<p align="center">
  <a href="https://github.com/aaronlou/PhotoCurate-Rust/releases/latest/download/PhotoCurate_0.1.0_aarch64.dmg">
    <img src="https://img.shields.io/badge/下载-macOS_DMG-7c3aed?style=for-the-badge&logo=apple" alt="下载 macOS DMG">
  </a>
  <a href="https://github.com/aaronlou/PhotoCurate-Rust/releases">
    <img src="https://img.shields.io/badge/查看-Release_Notes-6d28d9?style=for-the-badge&logo=github" alt="Release Notes">
  </a>
</p>

> macOS 14.0+（Apple Silicon）| 首次运行如提示「无法验证开发者」，请到 **系统设置 → 隐私与安全性** 中点击「仍要打开」

---

## 这个工具能做什么？

如果你有成千上万张照片，你可能会遇到这些问题：

- **拍了太多，不知道哪些值得保留** → AI 自动美学评分（0-100 分），一眼看到最好的照片
- **想找某张照片，但翻半天找不到** → 用中文描述画面内容直接搜索，比如「夕阳下的海滩」「穿红衣服的小孩」
- **想导出最好的照片，但一张张挑选太累** → 设定评分阈值，一键导出所有高分照片

PhotoCurate 就是帮你解决这些问题的桌面应用。

---

## 准备工作

### 系统要求

| | 最低要求 |
|---|---|
| 操作系统 | macOS 14.0+ / Windows 10+ / Linux |
| Node.js | >= 20 |
| Rust | >= 1.78 |

### 配置 AI 能力（二选一）

PhotoCurate 的评分和语义搜索需要 AI 能力，你有两种选择：

**方式 A：Gemini API（推荐，零配置）**

1. 打开 [Google AI Studio](https://aistudio.google.com/apikey)
2. 点击「Create API Key」获取免费 Key
3. 打开 PhotoCurate 后，在「评分」页面填入 Key 即可

> 优点：无需下载模型，开箱即用。免费额度足够个人日常使用。

**方式 B：本地 Chinese-CLIP 模型（离线，无需网络）**

如果你希望完全不依赖网络和 API Key，可以使用本地模型。本地模型仅支持语义搜索，评分仍需 Gemini API：

```bash
# 1. 创建 Python 虚拟环境并安装依赖
python3 -m venv .venv && source .venv/bin/activate
pip install modelscope torch transformers onnx onnxscript Pillow

# 2. 从 ModelScope 下载模型（约 720 MB）
python3 -c "
from modelscope import snapshot_download
snapshot_download('damo/multi-modal_clip-vit-base-patch16_zh',
                  local_dir='models/chinese-clip-vit-base-patch16')
"

# 3. 运行转换脚本，生成约 720 MB 的 ONNX 文件并自动复制到运行时目录
python3 scripts/export_chinese_clip_onnx.py
```

> 脚本会自动将 ONNX 文件复制到 `~/Library/Application Support/com.photocurate/models/`。应用启动时检测到模型文件即自动加载，否则回退到 Gemini API。

---

## 启动应用

```bash
# 安装前端依赖
npm install

# 启动开发模式（同时启动前端和后端）
npm run tauri-dev
```

应用窗口会自动打开，首次运行会提示授予文件夹访问权限。

---

## 使用流程

1. **导入照片** — 点击左侧「图库」，添加你的照片文件夹。应用会自动扫描照片，并在后台生成搜索索引
2. **AI 评分** — 切换到「评分」页，配置 Gemini API Key 后开始评分，进度条实时更新
3. **智能检索** — 照片索引完成后，在「搜索」页输入中文描述即可找到匹配的画面，无需手动操作
4. **精选导出** — 在「导出」页设置评分门槛，一键导出高分照片

---

## 技术架构

```
src-tauri/src/
├── domain/          # 领域模型（Photo、Directory 等核心类型）
├── application/     # 应用层（评分、搜索、导出等用例编排）
├── infrastructure/  # 基础设施（数据库、文件系统、AI 服务、向量索引）
└── interface/       # 接口层（Tauri IPC 命令）
```

| 层级 | 技术 |
|---|---|
| 前端 | React 19 + TypeScript + Tailwind CSS + Zustand |
| 后端 | Tauri v2 (Rust) |
| 数据库 | SQLite + sqlx |
| AI 评分 | Gemini API |
| 语义搜索 | Chinese-CLIP (ONNX + CoreML) 或 Gemini Embedding |
| 向量检索 | 内存 BruteForce + 余弦相似度 |

## 许可证

MIT
