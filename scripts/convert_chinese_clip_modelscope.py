#!/usr/bin/env python3
"""Convert Chinese-CLIP (ModelScope format) to ONNX for Rust/ort inference.

The ModelScope version (damo/multi-modal_clip-vit-base-patch16_zh) uses
the original OpenAI CLIP naming convention, not HuggingFace transformers.
This script builds the PyTorch modules directly and exports to ONNX.
"""
import os, sys, json, math
from pathlib import Path
import torch
import torch.nn as nn
import torch.nn.functional as F

PROJECT_ROOT = Path(__file__).parent.parent
MODEL_DIR = PROJECT_ROOT / "models" / "chinese-clip-vit-base-patch16"
OUTPUT_DIR = PROJECT_ROOT / "src-tauri" / "src" / "ai" / "models"
OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

IMAGE_SIZE = 224
MAX_TEXT_LEN = 52

# ============================================================
# BERT Text Encoder
# ============================================================

class BertConfig:
    vocab_size = 21128
    hidden_size = 768
    intermediate_size = 3072
    num_hidden_layers = 12
    num_attention_heads = 12
    max_position_embeddings = 512
    type_vocab_size = 2
    hidden_dropout_prob = 0.1
    attention_probs_dropout_prob = 0.1
    layer_norm_eps = 1e-12
    hidden_act = "gelu"

class BertSelfAttention(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.num_attention_heads = config.num_attention_heads
        self.attention_head_size = config.hidden_size // config.num_attention_heads
        self.all_head_size = self.num_attention_heads * self.attention_head_size
        self.query = nn.Linear(config.hidden_size, self.all_head_size)
        self.key = nn.Linear(config.hidden_size, self.all_head_size)
        self.value = nn.Linear(config.hidden_size, self.all_head_size)
        self.dropout = nn.Dropout(config.attention_probs_dropout_prob)

    def forward(self, hidden_states, attention_mask=None):
        q = self.query(hidden_states)
        k = self.key(hidden_states)
        v = self.value(hidden_states)
        bs = hidden_states.size(0)
        q = q.view(bs, -1, self.num_attention_heads, self.attention_head_size).permute(0, 2, 1, 3)
        k = k.view(bs, -1, self.num_attention_heads, self.attention_head_size).permute(0, 2, 1, 3)
        v = v.view(bs, -1, self.num_attention_heads, self.attention_head_size).permute(0, 2, 1, 3)
        attn_weights = torch.matmul(q, k.transpose(-2, -1)) / math.sqrt(self.attention_head_size)
        if attention_mask is not None:
            attn_weights = attn_weights + attention_mask
        attn_probs = F.softmax(attn_weights, dim=-1)
        attn_probs = self.dropout(attn_probs)
        context = torch.matmul(attn_probs, v)
        context = context.permute(0, 2, 1, 3).contiguous().view(bs, -1, self.all_head_size)
        return context

class BertSelfOutput(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.dense = nn.Linear(config.hidden_size, config.hidden_size)
        self.LayerNorm = nn.LayerNorm(config.hidden_size, eps=config.layer_norm_eps)
        self.dropout = nn.Dropout(config.hidden_dropout_prob)

    def forward(self, hidden_states, input_tensor):
        hidden_states = self.dense(hidden_states)
        hidden_states = self.dropout(hidden_states)
        hidden_states = self.LayerNorm(hidden_states + input_tensor)
        return hidden_states

class BertAttention(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.self = BertSelfAttention(config)
        self.output = BertSelfOutput(config)

    def forward(self, hidden_states, attention_mask=None):
        self_output = self.self(hidden_states, attention_mask)
        attention_output = self.output(self_output, hidden_states)
        return attention_output

class BertIntermediate(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.dense = nn.Linear(config.hidden_size, config.intermediate_size)

    def forward(self, hidden_states):
        return F.gelu(self.dense(hidden_states))

class BertOutput(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.dense = nn.Linear(config.intermediate_size, config.hidden_size)
        self.LayerNorm = nn.LayerNorm(config.hidden_size, eps=config.layer_norm_eps)
        self.dropout = nn.Dropout(config.hidden_dropout_prob)

    def forward(self, hidden_states, input_tensor):
        hidden_states = self.dense(hidden_states)
        hidden_states = self.dropout(hidden_states)
        hidden_states = self.LayerNorm(hidden_states + input_tensor)
        return hidden_states

class BertLayer(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.attention = BertAttention(config)
        self.intermediate = BertIntermediate(config)
        self.output = BertOutput(config)

    def forward(self, hidden_states, attention_mask=None):
        attention_output = self.attention(hidden_states, attention_mask)
        intermediate_output = self.intermediate(attention_output)
        layer_output = self.output(intermediate_output, attention_output)
        return layer_output

class BertEncoder(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.layer = nn.ModuleList([BertLayer(config) for _ in range(config.num_hidden_layers)])

    def forward(self, hidden_states, attention_mask=None):
        for layer in self.layer:
            hidden_states = layer(hidden_states, attention_mask)
        return hidden_states

class BertEmbeddings(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.word_embeddings = nn.Embedding(config.vocab_size, config.hidden_size)
        self.position_embeddings = nn.Embedding(config.max_position_embeddings, config.hidden_size)
        self.token_type_embeddings = nn.Embedding(config.type_vocab_size, config.hidden_size)
        self.LayerNorm = nn.LayerNorm(config.hidden_size, eps=config.layer_norm_eps)
        self.dropout = nn.Dropout(config.hidden_dropout_prob)

    def forward(self, input_ids, token_type_ids=None):
        bs, seq_len = input_ids.shape
        if token_type_ids is None:
            token_type_ids = torch.zeros_like(input_ids)
        position_ids = torch.arange(seq_len, dtype=torch.long, device=input_ids.device).unsqueeze(0).expand(bs, -1)
        words_emb = self.word_embeddings(input_ids)
        position_emb = self.position_embeddings(position_ids)
        token_type_emb = self.token_type_embeddings(token_type_ids)
        embeddings = words_emb + position_emb + token_type_emb
        embeddings = self.LayerNorm(embeddings)
        embeddings = self.dropout(embeddings)
        return embeddings

class BertPooler(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.dense = nn.Linear(config.hidden_size, config.hidden_size)
        self.activation = nn.Tanh()

    def forward(self, hidden_states):
        first_token = hidden_states[:, 0]
        return self.activation(self.dense(first_token))

class BertTextEncoder(nn.Module):
    def __init__(self, config, projection_dim=512):
        super().__init__()
        self.embeddings = BertEmbeddings(config)
        self.encoder = BertEncoder(config)
        self.pooler = BertPooler(config)
        # Raw parameter matching original model format [hidden_size, projection_dim]
        self.text_projection = nn.Parameter(torch.empty(config.hidden_size, projection_dim))

    def forward(self, input_ids, attention_mask=None):
        # Build attention mask from [B, S] boolean/int mask to additive mask
        extended_mask = None
        if attention_mask is not None:
            # attention_mask: [B, S] with 1 for valid tokens, 0 for padding
            extended_mask = attention_mask.unsqueeze(1).unsqueeze(2).to(torch.float32)
            extended_mask = (1.0 - extended_mask) * -10000.0

        emb = self.embeddings(input_ids)
        encoded = self.encoder(emb, extended_mask)
        pooled = self.pooler(encoded)
        projected = pooled @ self.text_projection
        return F.normalize(projected, p=2, dim=-1)


# ============================================================
# ViT Image Encoder (OpenAI CLIP style)
# ============================================================

class QuickGELU(nn.Module):
    def forward(self, x):
        return x * torch.sigmoid(1.702 * x)

class ViTAttention(nn.Module):
    def __init__(self, hidden_size=768, num_heads=12):
        super().__init__()
        self.num_heads = num_heads
        self.head_dim = hidden_size // num_heads
        self.in_proj_weight = nn.Parameter(torch.empty(3 * hidden_size, hidden_size))
        self.in_proj_bias = nn.Parameter(torch.empty(3 * hidden_size))
        self.out_proj = nn.Linear(hidden_size, hidden_size)

    def forward(self, x):
        bs, seq_len, dim = x.shape
        qkv = F.linear(x, self.in_proj_weight, self.in_proj_bias)
        q, k, v = qkv.chunk(3, dim=-1)
        q = q.view(bs, seq_len, self.num_heads, self.head_dim).permute(0, 2, 1, 3)
        k = k.view(bs, seq_len, self.num_heads, self.head_dim).permute(0, 2, 1, 3)
        v = v.view(bs, seq_len, self.num_heads, self.head_dim).permute(0, 2, 1, 3)
        attn = torch.matmul(q, k.transpose(-2, -1)) / math.sqrt(self.head_dim)
        attn = F.softmax(attn, dim=-1)
        ctx = torch.matmul(attn, v)
        ctx = ctx.permute(0, 2, 1, 3).contiguous().view(bs, seq_len, dim)
        return self.out_proj(ctx)

class ViTMLP(nn.Module):
    def __init__(self, hidden_size=768, intermediate_size=3072):
        super().__init__()
        self.c_fc = nn.Linear(hidden_size, intermediate_size)
        self.c_proj = nn.Linear(intermediate_size, hidden_size)

    def forward(self, x):
        return self.c_proj(QuickGELU()(self.c_fc(x)))

class ViTBlock(nn.Module):
    def __init__(self, hidden_size=768, num_heads=12):
        super().__init__()
        self.ln_1 = nn.LayerNorm(hidden_size)
        self.attn = ViTAttention(hidden_size, num_heads)
        self.ln_2 = nn.LayerNorm(hidden_size)
        self.mlp = ViTMLP(hidden_size, hidden_size * 4)

    def forward(self, x):
        x = x + self.attn(self.ln_1(x))
        x = x + self.mlp(self.ln_2(x))
        return x

class ViTImageEncoder(nn.Module):
    def __init__(self, image_size=224, patch_size=16, hidden_size=768,
                 num_layers=12, num_heads=12, projection_dim=512):
        super().__init__()
        self.patch_size = patch_size
        num_patches = (image_size // patch_size) ** 2
        self.conv1 = nn.Conv2d(3, hidden_size, kernel_size=patch_size, stride=patch_size, bias=False)
        self.class_embedding = nn.Parameter(torch.empty(hidden_size))
        self.positional_embedding = nn.Parameter(torch.empty(num_patches + 1, hidden_size))
        self.ln_pre = nn.LayerNorm(hidden_size)
        self.transformer = nn.ModuleList([ViTBlock(hidden_size, num_heads) for _ in range(num_layers)])
        self.ln_post = nn.LayerNorm(hidden_size)
        self.proj = nn.Parameter(torch.empty(hidden_size, projection_dim))

    def forward(self, pixel_values):
        # pixel_values: [B, 3, H, W]
        x = self.conv1(pixel_values)  # [B, hidden, H/patch, W/patch]
        x = x.flatten(2).transpose(1, 2)  # [B, num_patches, hidden]
        cls = self.class_embedding.unsqueeze(0).expand(x.size(0), 1, -1)
        x = torch.cat([cls, x], dim=1)  # [B, 1+num_patches, hidden]
        x = x + self.positional_embedding.unsqueeze(0)
        x = self.ln_pre(x)
        for block in self.transformer:
            x = block(x)
        x = self.ln_post(x)
        cls_out = x[:, 0, :]  # take CLS token
        projected = cls_out @ self.proj  # [B, projection_dim]
        return F.normalize(projected, p=2, dim=-1)


# ============================================================
# Weight loading and ONNX export
# ============================================================

def load_state_dict():
    checkpoint = torch.load(str(MODEL_DIR / "pytorch_model.bin"), map_location="cpu")
    sd = checkpoint["state_dict"]

    # Strip "module." prefix
    new_sd = {}
    for k, v in sd.items():
        if k.startswith("module."):
            new_sd[k[7:]] = v
        else:
            new_sd[k] = v
    return new_sd

def export_image_encoder():
    print("Building ViT image encoder...")
    sd = load_state_dict()
    model = ViTImageEncoder(image_size=IMAGE_SIZE, patch_size=16, hidden_size=768,
                            num_layers=12, num_heads=12, projection_dim=512)
    model.eval()

    # Map state dict keys
    state_dict = {}
    key_map = {
        "conv1.weight": "visual.conv1.weight",
        "class_embedding": "visual.class_embedding",
        "positional_embedding": "visual.positional_embedding",
        "ln_pre.weight": "visual.ln_pre.weight",
        "ln_pre.bias": "visual.ln_pre.bias",
        "ln_post.weight": "visual.ln_post.bias",  # wait, this looks wrong. Let me check...
        # Actually in the state dict, ln_post uses weight and bias consistently
        "ln_post.bias": "visual.ln_post.bias",
        "proj": "visual.proj",
    }

    # Build transformer key map
    for i in range(12):
        for comp in ["ln_1", "ln_2"]:
            for param in ["weight", "bias"]:
                state_dict[f"transformer.{i}.{comp}.{param}"] = sd[f"visual.transformer.resblocks.{i}.{comp}.{param}"]
        state_dict[f"transformer.{i}.attn.in_proj_weight"] = sd[f"visual.transformer.resblocks.{i}.attn.in_proj_weight"]
        state_dict[f"transformer.{i}.attn.in_proj_bias"] = sd[f"visual.transformer.resblocks.{i}.attn.in_proj_bias"]
        state_dict[f"transformer.{i}.attn.out_proj.weight"] = sd[f"visual.transformer.resblocks.{i}.attn.out_proj.weight"]
        state_dict[f"transformer.{i}.attn.out_proj.bias"] = sd[f"visual.transformer.resblocks.{i}.attn.out_proj.bias"]
        state_dict[f"transformer.{i}.mlp.c_fc.weight"] = sd[f"visual.transformer.resblocks.{i}.mlp.c_fc.weight"]
        state_dict[f"transformer.{i}.mlp.c_fc.bias"] = sd[f"visual.transformer.resblocks.{i}.mlp.c_fc.bias"]
        state_dict[f"transformer.{i}.mlp.c_proj.weight"] = sd[f"visual.transformer.resblocks.{i}.mlp.c_proj.weight"]
        state_dict[f"transformer.{i}.mlp.c_proj.bias"] = sd[f"visual.transformer.resblocks.{i}.mlp.c_proj.bias"]

    state_dict["conv1.weight"] = sd["visual.conv1.weight"]
    state_dict["class_embedding"] = sd["visual.class_embedding"]
    state_dict["positional_embedding"] = sd["visual.positional_embedding"]
    state_dict["ln_pre.weight"] = sd["visual.ln_pre.weight"]
    state_dict["ln_pre.bias"] = sd["visual.ln_pre.bias"]
    state_dict["ln_post.weight"] = sd["visual.ln_post.weight"]
    state_dict["ln_post.bias"] = sd["visual.ln_post.bias"]
    state_dict["proj"] = sd["visual.proj"]

    model.load_state_dict(state_dict, strict=True)
    print("  Image encoder weights loaded successfully.")

    dummy_input = torch.randn(1, 3, IMAGE_SIZE, IMAGE_SIZE)
    output_path = OUTPUT_DIR / "chinese_clip_image.onnx"

    torch.onnx.export(model, dummy_input, str(output_path),
        input_names=["pixel_values"], output_names=["image_embeds"],
        dynamic_axes={"pixel_values": {0: "batch_size"}, "image_embeds": {0: "batch_size"}},
        opset_version=18, external_data=False, do_constant_folding=True)
    print(f"  Image encoder: {output_path} ({output_path.stat().st_size / 1024 / 1024:.1f} MB)")

    # Verify output shape
    with torch.no_grad():
        out = model(dummy_input)
    print(f"  Output shape: {out.shape} (expected: [1, 512])")


def export_text_encoder():
    print("Building BERT text encoder...")
    sd = load_state_dict()
    config = BertConfig()
    model = BertTextEncoder(config, projection_dim=512)
    model.eval()

    # Map state dict keys: bert.* -> embeddings.* / encoder.layer.* / pooler.* / text_projection
    state_dict = {}

    # Embeddings
    for param in ["weight", "bias"]:
        state_dict[f"embeddings.LayerNorm.{param}"] = sd[f"bert.embeddings.LayerNorm.{param}"]
    state_dict["embeddings.word_embeddings.weight"] = sd["bert.embeddings.word_embeddings.weight"]
    state_dict["embeddings.position_embeddings.weight"] = sd["bert.embeddings.position_embeddings.weight"]
    state_dict["embeddings.token_type_embeddings.weight"] = sd["bert.embeddings.token_type_embeddings.weight"]

    # Encoder layers
    for i in range(12):
        prefix = f"encoder.layer.{i}."
        src = f"bert.encoder.layer.{i}."
        state_dict[prefix + "attention.self.query.weight"] = sd[src + "attention.self.query.weight"]
        state_dict[prefix + "attention.self.query.bias"] = sd[src + "attention.self.query.bias"]
        state_dict[prefix + "attention.self.key.weight"] = sd[src + "attention.self.key.weight"]
        state_dict[prefix + "attention.self.key.bias"] = sd[src + "attention.self.key.bias"]
        state_dict[prefix + "attention.self.value.weight"] = sd[src + "attention.self.value.weight"]
        state_dict[prefix + "attention.self.value.bias"] = sd[src + "attention.self.value.bias"]
        state_dict[prefix + "attention.output.dense.weight"] = sd[src + "attention.output.dense.weight"]
        state_dict[prefix + "attention.output.dense.bias"] = sd[src + "attention.output.dense.bias"]
        state_dict[prefix + "attention.output.LayerNorm.weight"] = sd[src + "attention.output.LayerNorm.weight"]
        state_dict[prefix + "attention.output.LayerNorm.bias"] = sd[src + "attention.output.LayerNorm.bias"]
        state_dict[prefix + "intermediate.dense.weight"] = sd[src + "intermediate.dense.weight"]
        state_dict[prefix + "intermediate.dense.bias"] = sd[src + "intermediate.dense.bias"]
        state_dict[prefix + "output.dense.weight"] = sd[src + "output.dense.weight"]
        state_dict[prefix + "output.dense.bias"] = sd[src + "output.dense.bias"]
        state_dict[prefix + "output.LayerNorm.weight"] = sd[src + "output.LayerNorm.weight"]
        state_dict[prefix + "output.LayerNorm.bias"] = sd[src + "output.LayerNorm.bias"]

    # Pooler
    state_dict["pooler.dense.weight"] = sd["bert.pooler.dense.weight"]
    state_dict["pooler.dense.bias"] = sd["bert.pooler.dense.bias"]

    # Text projection (raw nn.Parameter, not nn.Linear)
    state_dict["text_projection"] = sd["text_projection"]

    model.load_state_dict(state_dict, strict=True)
    print("  Text encoder weights loaded successfully.")

    dummy_input_ids = torch.zeros(1, MAX_TEXT_LEN, dtype=torch.long)
    dummy_attention_mask = torch.ones(1, MAX_TEXT_LEN, dtype=torch.long)
    output_path = OUTPUT_DIR / "chinese_clip_text.onnx"

    torch.onnx.export(model, (dummy_input_ids, dummy_attention_mask), str(output_path),
        input_names=["input_ids", "attention_mask"], output_names=["text_embeds"],
        dynamic_axes={"input_ids": {0: "batch_size"}, "attention_mask": {0: "batch_size"}, "text_embeds": {0: "batch_size"}},
        opset_version=18, external_data=False, do_constant_folding=True)
    print(f"  Text encoder: {output_path} ({output_path.stat().st_size / 1024 / 1024:.1f} MB)")

    # Verify output shape
    with torch.no_grad():
        out = model(dummy_input_ids, dummy_attention_mask)
    print(f"  Output shape: {out.shape} (expected: [1, 512])")


def save_config_and_vocab():
    config = {
        "version": "1.0",
        "image_size": IMAGE_SIZE,
        "max_text_length": MAX_TEXT_LEN,
        "do_normalize": True,
        "do_rescale": True,
        "do_resize": True,
        "rescale_factor": 0.00392156862745098,
        "image_mean": [0.48145466, 0.4578275, 0.40821073],
        "image_std": [0.26862954, 0.26130258, 0.27577711],
        "vocab_size": 21128,
        "pad_token_id": 0,
        "cls_token_id": 101,
        "sep_token_id": 102,
        "unk_token_id": 100,
    }
    with open(OUTPUT_DIR / "chinese_clip_config.json", "w", encoding="utf-8") as f:
        json.dump(config, f, ensure_ascii=False, indent=2)
    print("  Config saved.")

    # Copy vocab.txt from model directory
    import shutil
    src_vocab = MODEL_DIR / "vocab.txt"
    dst_vocab = OUTPUT_DIR / "vocab.txt"
    shutil.copy(src_vocab, dst_vocab)
    print(f"  Vocab saved ({dst_vocab.stat().st_size / 1024:.1f} KB)")


def main():
    print("=" * 60)
    print("Chinese-CLIP ONNX Converter (ModelScope format)")
    print("=" * 60)
    if not (MODEL_DIR / "pytorch_model.bin").exists():
        print(f"Model not found: {MODEL_DIR / 'pytorch_model.bin'}")
        print("Download it first: python -c \"from modelscope import snapshot_download; snapshot_download('damo/multi-modal_clip-vit-base-patch16_zh', local_dir='models/chinese-clip-vit-base-patch16')\"")
        sys.exit(1)

    try:
        export_image_encoder()
        export_text_encoder()
        save_config_and_vocab()
        print("\nAll models converted successfully!")
        print(f"\nOutput files in: {OUTPUT_DIR}")
        for f in sorted(OUTPUT_DIR.iterdir(), key=lambda x: x.stat().st_size, reverse=True):
            if f.is_file() and f.suffix in [".onnx", ".json", ".txt"]:
                print(f"  {f.name} ({f.stat().st_size / 1024 / 1024:.1f} MB)")
    except Exception as e:
        print(f"\nConversion failed: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
