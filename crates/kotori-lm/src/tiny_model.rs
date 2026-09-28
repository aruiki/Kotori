//! テスト用の小さな llama 形式の GGUF を書き出す。重みは決まった擬似乱数で、語彙は
//! 制御トークン 3 個とバイト 256 個(SentencePiece のバイト代替)。モデルファイルを
//! リポジトリに置かずに、推論の配線を CI で確かめるために使う。

#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::path::Path;

const N_EMBD: u64 = 32;
const N_HEAD: u32 = 2;
const N_FF: u64 = 64;
const N_CTX: u32 = 256;
pub const N_VOCAB: u64 = 3 + 256;

enum Value {
    U32(u32),
    F32(f32),
    Str(String),
    Strs(Vec<String>),
    F32s(Vec<f32>),
    I32s(Vec<i32>),
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u64).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn put_kv(out: &mut Vec<u8>, key: &str, value: &Value) {
    put_str(out, key);
    match value {
        Value::U32(v) => {
            out.extend_from_slice(&4u32.to_le_bytes());
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::F32(v) => {
            out.extend_from_slice(&6u32.to_le_bytes());
            out.extend_from_slice(&v.to_le_bytes());
        }
        Value::Str(s) => {
            out.extend_from_slice(&8u32.to_le_bytes());
            put_str(out, s);
        }
        Value::Strs(v) => {
            out.extend_from_slice(&9u32.to_le_bytes());
            out.extend_from_slice(&8u32.to_le_bytes());
            out.extend_from_slice(&(v.len() as u64).to_le_bytes());
            v.iter().for_each(|s| put_str(out, s));
        }
        Value::F32s(v) => {
            out.extend_from_slice(&9u32.to_le_bytes());
            out.extend_from_slice(&6u32.to_le_bytes());
            out.extend_from_slice(&(v.len() as u64).to_le_bytes());
            v.iter()
                .for_each(|x| out.extend_from_slice(&x.to_le_bytes()));
        }
        Value::I32s(v) => {
            out.extend_from_slice(&9u32.to_le_bytes());
            out.extend_from_slice(&5u32.to_le_bytes());
            out.extend_from_slice(&(v.len() as u64).to_le_bytes());
            v.iter()
                .for_each(|x| out.extend_from_slice(&x.to_le_bytes()));
        }
    }
}

/// 決まった擬似乱数(xorshift)で重みを作る。
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 0.2
    }
}

/// `path` にテスト用のモデルを書く。
pub fn write(path: &Path) {
    let mut tokens = vec!["<unk>".to_owned(), "<s>".to_owned(), "</s>".to_owned()];
    tokens.extend((0..=255u8).map(|b| format!("<0x{b:02X}>")));
    let mut types = vec![2, 3, 3];
    types.extend(std::iter::repeat(6).take(256));
    let kvs: Vec<(&str, Value)> = vec![
        ("general.architecture", Value::Str("llama".into())),
        ("general.alignment", Value::U32(32)),
        ("llama.context_length", Value::U32(N_CTX)),
        ("llama.embedding_length", Value::U32(N_EMBD as u32)),
        ("llama.block_count", Value::U32(1)),
        ("llama.feed_forward_length", Value::U32(N_FF as u32)),
        ("llama.attention.head_count", Value::U32(N_HEAD)),
        ("llama.attention.head_count_kv", Value::U32(N_HEAD)),
        (
            "llama.rope.dimension_count",
            Value::U32(N_EMBD as u32 / N_HEAD),
        ),
        ("llama.attention.layer_norm_rms_epsilon", Value::F32(1e-5)),
        ("tokenizer.ggml.model", Value::Str("llama".into())),
        ("tokenizer.ggml.tokens", Value::Strs(tokens)),
        (
            "tokenizer.ggml.scores",
            Value::F32s(vec![0.0; N_VOCAB as usize]),
        ),
        ("tokenizer.ggml.token_type", Value::I32s(types)),
        ("tokenizer.ggml.unknown_token_id", Value::U32(0)),
        ("tokenizer.ggml.bos_token_id", Value::U32(1)),
        ("tokenizer.ggml.eos_token_id", Value::U32(2)),
    ];
    // (名前, 形(ne0 から), 1 で埋めるか)
    let tensors: Vec<(&str, Vec<u64>, bool)> = vec![
        ("token_embd.weight", vec![N_EMBD, N_VOCAB], false),
        ("blk.0.attn_norm.weight", vec![N_EMBD], true),
        ("blk.0.attn_q.weight", vec![N_EMBD, N_EMBD], false),
        ("blk.0.attn_k.weight", vec![N_EMBD, N_EMBD], false),
        ("blk.0.attn_v.weight", vec![N_EMBD, N_EMBD], false),
        ("blk.0.attn_output.weight", vec![N_EMBD, N_EMBD], false),
        ("blk.0.ffn_norm.weight", vec![N_EMBD], true),
        ("blk.0.ffn_gate.weight", vec![N_EMBD, N_FF], false),
        ("blk.0.ffn_up.weight", vec![N_EMBD, N_FF], false),
        ("blk.0.ffn_down.weight", vec![N_FF, N_EMBD], false),
        ("output_norm.weight", vec![N_EMBD], true),
        ("output.weight", vec![N_EMBD, N_VOCAB], false),
    ];

    let mut out = Vec::new();
    out.extend_from_slice(b"GGUF");
    out.extend_from_slice(&3u32.to_le_bytes());
    out.extend_from_slice(&(tensors.len() as u64).to_le_bytes());
    out.extend_from_slice(&(kvs.len() as u64).to_le_bytes());
    for (k, v) in &kvs {
        put_kv(&mut out, k, v);
    }
    let mut offset = 0u64;
    let mut data = Vec::new();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for (name, shape, ones) in &tensors {
        put_str(&mut out, name);
        out.extend_from_slice(&(shape.len() as u32).to_le_bytes());
        shape
            .iter()
            .for_each(|d| out.extend_from_slice(&d.to_le_bytes()));
        out.extend_from_slice(&0u32.to_le_bytes()); // F32
        out.extend_from_slice(&offset.to_le_bytes());
        let n: u64 = shape.iter().product();
        for _ in 0..n {
            let x = if *ones { 1.0f32 } else { rng.next() };
            data.extend_from_slice(&x.to_le_bytes());
        }
        while data.len() % 32 != 0 {
            data.push(0);
        }
        offset = data.len() as u64;
    }
    while out.len() % 32 != 0 {
        out.push(0);
    }
    out.extend_from_slice(&data);
    std::fs::File::create(path)
        .unwrap()
        .write_all(&out)
        .unwrap();
}
