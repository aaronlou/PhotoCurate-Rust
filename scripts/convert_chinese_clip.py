#!/usr/bin/env python3
"""Convert Chinese-CLIP PyTorch model to ONNX format for Rust/ort inference."""
import os, sys, torch, onnx, json
from pathlib import Path

PROJECT_ROOT = Path(__file__).parent.parent
MODEL_DIR = PROJECT_ROOT / "models" / "chinese-clip-vit-base-patch16"
OUTPUT_DIR = PROJECT_ROOT / "src-tauri" / "src" / "ai" / "models"
OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

IMAGE_SIZE = 224
MAX_TEXT_LEN = 52

def export_image_encoder():
    print(f"Loading full Chinese-CLIP model from {MODEL_DIR}...")
    from transformers import ChineseCLIPModel, ChineseCLIPProcessor
    processor = ChineseCLIPProcessor.from_pretrained(str(MODEL_DIR), local_files_only=True)
    full_model = ChineseCLIPModel.from_pretrained(str(MODEL_DIR), local_files_only=True)
    full_model.eval()
    
    class ImageEncoder(torch.nn.Module):
        def __init__(self, vision_model, visual_projection):
            super().__init__()
            self.vision_model = vision_model
            self.visual_projection = visual_projection
        def forward(self, pixel_values):
            vision_out = self.vision_model(pixel_values=pixel_values)
            pooled = vision_out.pooler_output
            projected = self.visual_projection(pooled)
            return projected
    
    wrapped = ImageEncoder(full_model.vision_model, full_model.visual_projection)
    wrapped.eval()
    
    dummy_input = torch.randn(1, 3, IMAGE_SIZE, IMAGE_SIZE)
    output_path = OUTPUT_DIR / "chinese_clip_image.onnx"
    
    torch.onnx.export(wrapped, (dummy_input,), str(output_path),
        input_names=["pixel_values"], output_names=["image_embeds"],
        dynamic_axes={"pixel_values": {0: "batch_size"}, "image_embeds": {0: "batch_size"}},
        opset_version=18, external_data=False, do_constant_folding=True)
    onnx.checker.check_model(str(output_path))
    print(f"  Image encoder: {output_path} ({output_path.stat().st_size / 1024 / 1024:.1f} MB)")
    return processor

def export_text_encoder():
    print(f"Extracting text encoder...")
    from transformers import ChineseCLIPModel, BertTokenizer
    tokenizer = BertTokenizer.from_pretrained(str(MODEL_DIR), local_files_only=True)
    full_model = ChineseCLIPModel.from_pretrained(str(MODEL_DIR), local_files_only=True)
    full_model.eval()
    
    class TextEncoder(torch.nn.Module):
        def __init__(self, text_model, text_projection):
            super().__init__()
            self.text_model = text_model
            self.text_projection = text_projection
        def forward(self, input_ids, attention_mask):
            text_out = self.text_model(input_ids=input_ids, attention_mask=attention_mask)
            cls_embedding = text_out.last_hidden_state[:, 0, :]
            projected = self.text_projection(cls_embedding)
            return projected
    
    wrapped = TextEncoder(full_model.text_model, full_model.text_projection)
    wrapped.eval()
    
    dummy_input_ids = torch.zeros(1, MAX_TEXT_LEN, dtype=torch.long)
    dummy_attention_mask = torch.ones(1, MAX_TEXT_LEN, dtype=torch.long)
    output_path = OUTPUT_DIR / "chinese_clip_text.onnx"
    
    torch.onnx.export(wrapped, (dummy_input_ids, dummy_attention_mask), str(output_path),
        input_names=["input_ids", "attention_mask"], output_names=["text_embeds"],
        dynamic_axes={"input_ids": {0: "batch_size"}, "attention_mask": {0: "batch_size"}, "text_embeds": {0: "batch_size"}},
        opset_version=18, external_data=False, do_constant_folding=True)
    onnx.checker.check_model(str(output_path))
    print(f"  Text encoder: {output_path} ({output_path.stat().st_size / 1024 / 1024:.1f} MB)")
    return tokenizer

def save_processor_config(processor, tokenizer):
    config = {
        "version": "1.0",
        "image_size": IMAGE_SIZE,
        "max_text_length": MAX_TEXT_LEN,
        "do_normalize": True,
        "do_rescale": True,
        "do_resize": True,
        "rescale_factor": 1.0 / 255.0,
        "image_mean": processor.image_processor.image_mean if hasattr(processor, "image_processor") else [0.48145466, 0.4578275, 0.40821073],
        "image_std": processor.image_processor.image_std if hasattr(processor, "image_processor") else [0.26862954, 0.26130258, 0.27577711],
        "vocab_size": tokenizer.vocab_size,
        "pad_token_id": tokenizer.pad_token_id,
        "cls_token_id": tokenizer.cls_token_id,
        "sep_token_id": tokenizer.sep_token_id,
        "unk_token_id": tokenizer.unk_token_id,
    }
    with open(OUTPUT_DIR / "chinese_clip_config.json", "w", encoding="utf-8") as f:
        json.dump(config, f, ensure_ascii=False, indent=2)
    print(f"  Config saved")
    vocab_path = OUTPUT_DIR / "vocab.txt"
    with open(vocab_path, "w", encoding="utf-8") as f:
        for token, idx in sorted(tokenizer.vocab.items(), key=lambda x: x[1]):
            f.write(token + "\n")
    print(f"  Vocab saved ({len(tokenizer.vocab)} tokens)")

def main():
    print("=" * 60)
    print("Chinese-CLIP ONNX Converter")
    print("=" * 60)
    if not MODEL_DIR.exists():
        print(f"Model directory not found: {MODEL_DIR}")
        sys.exit(1)
    try:
        processor = export_image_encoder()
        tokenizer = export_text_encoder()
        save_processor_config(processor, tokenizer)
        print("\nAll models converted successfully!")
        print(f"\nOutput files in: {OUTPUT_DIR}")
        for f in sorted(OUTPUT_DIR.iterdir(), key=lambda x: x.stat().st_size, reverse=True):
            print(f"  {f.name} ({f.stat().st_size / 1024 / 1024:.1f} MB)")
    except Exception as e:
        print(f"\nConversion failed: {e}")
        import traceback; traceback.print_exc()
        sys.exit(1)

if __name__ == "__main__":
    main()
