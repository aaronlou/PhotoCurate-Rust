#!/bin/bash
set -e

# ==========================================================
# PhotoCurate — Chinese-CLIP 本地模型一键安装脚本
# ==========================================================
# 用法: bash scripts/setup_local_model.sh
#
# 这会自动完成：
# 1. 创建 Python 虚拟环境
# 2. 安装依赖
# 3. 从 ModelScope 下载模型
# 4. 转换为 ONNX 格式
# 5. 复制到运行时目录
# ==========================================================

RUNTIME_DIR="$HOME/Library/Application Support/com.photocurate/models"

echo "============================================"
echo " PhotoCurate 本地模型安装"
echo "============================================"
echo ""
echo "运行时目录: $RUNTIME_DIR"
echo ""

# Step 1: Python venv
if [ ! -d ".venv" ]; then
    echo "[1/4] 创建 Python 虚拟环境..."
    python3 -m venv .venv
fi

source .venv/bin/activate

# Step 2: Install dependencies
echo "[2/4] 安装 Python 依赖..."
pip install -q modelscope torch transformers onnx onnxscript Pillow

# Step 3: Download model
if [ ! -f "models/chinese-clip-vit-base-patch16/pytorch_model.bin" ]; then
    echo "[3/4] 从 ModelScope 下载 Chinese-CLIP 模型（约 720 MB）..."
    python3 -c "
from modelscope import snapshot_download
snapshot_download('damo/multi-modal_clip-vit-base-patch16_zh',
                  local_dir='models/chinese-clip-vit-base-patch16')
"
else
    echo "[3/4] 模型文件已存在，跳过下载"
fi

# Step 4: Convert and copy
echo "[4/4] 转换为 ONNX 并安装到运行时目录..."
python3 scripts/export_chinese_clip_onnx.py

echo ""
echo "============================================"
echo " 完成！模型已安装到："
echo " $RUNTIME_DIR"
echo ""
echo " 现在启动 PhotoCurate 即可使用本地模型。"
echo "============================================"
