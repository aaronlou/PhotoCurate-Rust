#!/usr/bin/env python3
"""Export Chinese-CLIP to ONNX using HuggingFace transformers.

Downloads the ModelScope checkpoint, maps weights to HF format,
and exports standalone ONNX files for the Rust ort runtime.
"""
import os, sys
from pathlib import Path
from collections import OrderedDict
import torch
import torch.nn as nn

PROJECT_ROOT = Path(__file__).parent.parent
MODEL_DIR = PROJECT_ROOT / "models" / "chinese-clip-vit-base-patch16"
OUTPUT_DIR = PROJECT_ROOT / "src-tauri" / "src" / "ai" / "models"
RUNTIME_DIR = Path.home() / "Library" / "Application Support" / "com.photocurate" / "models"

MAX_TEXT_LEN = 52


def download_model():
    """Download Chinese-CLIP from ModelScope if not already present."""
    if (MODEL_DIR / "pytorch_model.bin").exists():
        print(f"Model already exists at {MODEL_DIR}")
        return

    print(f"Downloading model from ModelScope to {MODEL_DIR}...")
    from modelscope import snapshot_download

    snapshot_download(
        "damo/multi-modal_clip-vit-base-patch16_zh",
        local_dir=str(MODEL_DIR),
    )
    print("Download complete.")


def map_weights():
    """Map ModelScope state dict keys to HuggingFace ChineseCLIPModel keys."""
    checkpoint = torch.load(str(MODEL_DIR / "pytorch_model.bin"), map_location="cpu")
    ms = {
        k[7:] if k.startswith("module.") else k: v
        for k, v in checkpoint["state_dict"].items()
    }

    hf = OrderedDict()

    # Text model: bert.* → text_model.*
    for k, v in ms.items():
        if k.startswith("bert."):
            hf[k.replace("bert.", "text_model.", 1)] = v

    # Text projection: [768, 512] param → [512, 768] Linear weight
    hf["text_projection.weight"] = ms["text_projection"].t().contiguous()

    # Vision model: simple renames
    rename = {
        "visual.class_embedding": "vision_model.embeddings.class_embedding",
        "visual.conv1.weight": "vision_model.embeddings.patch_embedding.weight",
        "visual.positional_embedding": "vision_model.embeddings.position_embedding.weight",
        "visual.ln_pre.weight": "vision_model.pre_layrnorm.weight",
        "visual.ln_pre.bias": "vision_model.pre_layrnorm.bias",
        "visual.ln_post.weight": "vision_model.post_layernorm.weight",
        "visual.ln_post.bias": "vision_model.post_layernorm.bias",
    }
    for ms_key, hf_key in rename.items():
        hf[hf_key] = ms[ms_key]

    # Vision: split fused QKV and map transformer layers
    for i in range(12):
        ms_p = f"visual.transformer.resblocks.{i}"
        hf_p = f"vision_model.encoder.layers.{i}"

        w = ms[f"{ms_p}.attn.in_proj_weight"]
        b = ms[f"{ms_p}.attn.in_proj_bias"]
        for proj, w_chunk, b_chunk in [
            ("q", w.chunk(3)[0], b.chunk(3)[0]),
            ("k", w.chunk(3)[1], b.chunk(3)[1]),
            ("v", w.chunk(3)[2], b.chunk(3)[2]),
        ]:
            hf[f"{hf_p}.self_attn.{proj}_proj.weight"] = w_chunk
            hf[f"{hf_p}.self_attn.{proj}_proj.bias"] = b_chunk

        hf[f"{hf_p}.self_attn.out_proj.weight"] = ms[f"{ms_p}.attn.out_proj.weight"]
        hf[f"{hf_p}.self_attn.out_proj.bias"] = ms[f"{ms_p}.attn.out_proj.bias"]

        for hf_ln, ms_ln in [("layer_norm1", "ln_1"), ("layer_norm2", "ln_2")]:
            hf[f"{hf_p}.{hf_ln}.weight"] = ms[f"{ms_p}.{ms_ln}.weight"]
            hf[f"{hf_p}.{hf_ln}.bias"] = ms[f"{ms_p}.{ms_ln}.bias"]

        for hf_mlp, ms_mlp in [("fc1", "c_fc"), ("fc2", "c_proj")]:
            hf[f"{hf_p}.mlp.{hf_mlp}.weight"] = ms[f"{ms_p}.mlp.{ms_mlp}.weight"]
            hf[f"{hf_p}.mlp.{hf_mlp}.bias"] = ms[f"{ms_p}.mlp.{ms_mlp}.bias"]

    # Visual projection: [768, 512] param → [512, 768] Linear weight
    hf["visual_projection.weight"] = ms["visual.proj"].t().contiguous()
    hf["logit_scale"] = ms["logit_scale"]

    return hf


class ImageEncoder(nn.Module):
    def __init__(self, vision_model, visual_projection):
        super().__init__()
        self.vision_model = vision_model
        self.visual_projection = visual_projection

    def forward(self, pixel_values):
        out = self.vision_model(pixel_values=pixel_values)
        return self.visual_projection(out.pooler_output)


class TextEncoder(nn.Module):
    def __init__(self, text_model, text_projection):
        super().__init__()
        self.text_model = text_model
        self.text_projection = text_projection

    def forward(self, input_ids, attention_mask):
        out = self.text_model(input_ids=input_ids, attention_mask=attention_mask)
        cls_emb = out.last_hidden_state[:, 0, :]
        return self.text_projection(cls_emb)


def main():
    print("=" * 60)
    print("Chinese-CLIP ONNX Exporter (HF transformers)")
    print("=" * 60)

    # 1. Download
    download_model()

    # 2. Map weights
    print("Mapping weights...")
    state_dict = map_weights()
    print(f"  {len(state_dict)} keys mapped")

    # 3. Create HF model
    from transformers import ChineseCLIPModel, ChineseCLIPConfig

    config = ChineseCLIPConfig(
        text_config={"vocab_size": 21128},
        vision_config={"image_size": 224, "patch_size": 16},
        projection_dim=512,
    )
    model = ChineseCLIPModel(config).eval()
    model.load_state_dict(state_dict, strict=False)
    print("  Model loaded")

    # 4. Export Image ONNX
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    img_path = OUTPUT_DIR / "chinese_clip_image.onnx"

    img_encoder = ImageEncoder(model.vision_model, model.visual_projection).eval()
    torch.onnx.export(
        img_encoder,
        torch.randn(1, 3, 224, 224),
        str(img_path),
        input_names=["pixel_values"],
        output_names=["image_embeds"],
        dynamic_axes={"pixel_values": {0: "batch_size"}, "image_embeds": {0: "batch_size"}},
        opset_version=18,
        do_constant_folding=True,
        external_data=False,
    )
    print(f"  Image encoder: {img_path} ({img_path.stat().st_size / 1024 ** 2:.1f} MB)")

    # 5. Export Text ONNX
    text_path = OUTPUT_DIR / "chinese_clip_text.onnx"

    text_encoder = TextEncoder(model.text_model, model.text_projection).eval()
    torch.onnx.export(
        text_encoder,
        (torch.zeros(1, MAX_TEXT_LEN, dtype=torch.long), torch.ones(1, MAX_TEXT_LEN, dtype=torch.long)),
        str(text_path),
        input_names=["input_ids", "attention_mask"],
        output_names=["text_embeds"],
        dynamic_axes={
            "input_ids": {0: "batch_size"},
            "attention_mask": {0: "batch_size"},
            "text_embeds": {0: "batch_size"},
        },
        opset_version=18,
        do_constant_folding=True,
        external_data=False,
    )
    print(f"  Text encoder: {text_path} ({text_path.stat().st_size / 1024 ** 2:.1f} MB)")

    # 6. Copy to runtime directory
    RUNTIME_DIR.mkdir(parents=True, exist_ok=True)
    import shutil

    shutil.copy2(img_path, RUNTIME_DIR)
    shutil.copy2(text_path, RUNTIME_DIR)
    print(f"\n  Copied to: {RUNTIME_DIR}")

    # 7. Verify
    with torch.no_grad():
        img_out = img_encoder(torch.randn(1, 3, 224, 224))
        text_out = text_encoder(
            torch.zeros(1, MAX_TEXT_LEN, dtype=torch.long),
            torch.ones(1, MAX_TEXT_LEN, dtype=torch.long),
        )
    assert img_out.shape == (1, 512), f"Unexpected image output: {img_out.shape}"
    assert text_out.shape == (1, 512), f"Unexpected text output: {text_out.shape}"

    hf_text = model.get_text_features(
        input_ids=torch.zeros(1, MAX_TEXT_LEN, dtype=torch.long),
        attention_mask=torch.ones(1, MAX_TEXT_LEN, dtype=torch.long),
    )
    cos_sim = torch.nn.functional.cosine_similarity(hf_text.pooler_output, text_out, dim=-1)
    assert cos_sim.item() > 0.999, f"HF-ONNX mismatch: {cos_sim.item()}"

    print(f"\n  Verification: HF-ONNX cosine similarity = {cos_sim.item():.6f}")
    print("\nAll done. Models ready for use.")


if __name__ == "__main__":
    main()
