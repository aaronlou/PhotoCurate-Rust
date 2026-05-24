# PhotoCurate (Tauri 重构版)

基于 **Tauri (Rust + React/TypeScript)** 的 AI 照片管理与评分工具。

## 项目架构

```
PhotoCurate_Rust/
├── src/                          # 前端 (React + TypeScript)
│   ├── components/               # 共享 UI 组件
│   ├── views/                    # 页面视图 (Library, Scoring, Search, Export)
│   ├── hooks/                    # IPC 调用封装
│   ├── stores/                   # Zustand 状态管理
│   └── types/                    # TypeScript 类型定义
├── src-tauri/                    # 后端 (Rust)
│   ├── src/
│   │   ├── commands/             # Tauri IPC 命令
│   │   ├── db/                   # SQLite 数据库与迁移
│   │   ├── fs/                   # 文件扫描、EXIF、缩略图
│   │   ├── ai/                   # Gemini API 服务
│   │   ├── vector/               # 内存向量检索 (BruteForce)
│   │   └── models/               # 数据模型
│   ├── icons/                    # 应用图标
│   ├── tauri.conf.json           # Tauri 配置
│   └── Entitlements.plist        # macOS 沙盒权限
├── package.json
├── vite.config.ts
└── tailwind.config.js
```

## 技术栈

| 层级 | 技术 |
|------|------|
| 前端框架 | React 19 + TypeScript |
| UI 样式 | Tailwind CSS |
| 状态管理 | Zustand |
| 图标 | Lucide React |
| 后端框架 | Tauri v2 (Rust) |
| 数据库 | SQLite + sqlx |
| 文件监控 | notify crate |
| AI 服务 | Gemini API |
| 向量检索 | 内存 BruteForce + 余弦相似度 |

## 快速开始

### 开发环境要求

- **Node.js** >= 20
- **Rust** >= 1.78
- **Tauri CLI**: `npm install -g @tauri-apps/cli`
- macOS 14.0+ (用于 App Store 上架)

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run tauri-dev
```

这会同时启动 Vite 前端 dev server 和 Rust 后端。

### 构建生产包

```bash
npm run tauri-build
```

构建产物位于 `src-tauri/target/release/bundle/`:
- **DMG**: `PhotoCurate_0.1.0_aarch64.dmg`
- **APP**: `PhotoCurate.app`

## 核心功能

### 1. 图库管理
- 添加本地文件夹（支持 Security-Scoped Bookmark）
- 自动递归扫描图片（JPG/PNG/HEIC/TIFF/RAW）
- FSEvents 实时文件监控
- 三种浏览模式：浏览器 / 网格 / 列表

### 2. AI 评分
- 基于 Gemini API 的照片美学评分 (0-100)
- 自动生成图像描述向量
- 批量评分，支持进度显示

### 3. 智能检索
- 自然语言搜索照片内容
- 基于向量相似度的语义匹配

### 4. 精选导出
- 按评分阈值筛选照片
- 一键导出到指定文件夹

## 与原 Swift 项目的对比

| 特性 | Swift 版 | Tauri 版 |
|------|---------|---------|
| 本地 AI (Core ML) | MobileCLIP + Neural Engine | **Gemini API** (V1) |
| 数据持久化 | SwiftData | SQLite + sqlx |
| 文件监控 | FSEvents (C API) | notify crate (跨平台) |
| UI | SwiftUI | React + Tailwind |
| 跨平台 | 仅 macOS | **macOS/Windows/Linux** |
| AI 辅助开发 | 较弱 | **极强** |

## App Store 上架配置

### 1. 代码签名

在 `src-tauri/tauri.conf.json` 中配置：

```json
{
  "bundle": {
    "macOS": {
      "signingIdentity": "Developer ID Application: Your Name (TEAM_ID)",
      "providerShortName": "TEAM_ID"
    }
  }
}
```

### 2. 沙盒权限

`src-tauri/Entitlements.plist` 已配置：
- `com.apple.security.app-sandbox`
- `com.apple.security.files.user-selected.read-write`
- `com.apple.security.network.client`

### 3. 上架步骤

```bash
# 1. 构建并签名
tauri build --target aarch64-apple-darwin

# 2. 公证 (dmg 版本)
xcrun notarytool submit src-tauri/target/release/bundle/dmg/*.dmg \
  --apple-id your@email.com \
  --team-id TEAM_ID \
  --wait

# 3. App Store 版本需要额外配置 (使用 app 目标而非 dmg)
```

## 已知限制与 TODO

- [ ] **本地 AI 模型**: V1 使用 Gemini API，后续可集成 ONNX Runtime + MobileCLIP 实现离线推理
- [ ] **Security-Scoped Bookmark**: 当前为占位实现，需补充 macOS 原生 bookmark 创建/解析
- [ ] **RAW 格式缩略图**: 依赖 macOS `sips` 命令，Windows 需额外配置
- [ ] **增量文件监控**: 当前 FSEvents 仅记录日志，未实现增量更新
- [ ] **进度流式推送**: 评分进度目前为前端模拟，后续可用 Tauri Event 实现真实进度

## 许可证

MIT
