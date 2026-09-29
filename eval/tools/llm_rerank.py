"""汎用 LLM で、Kotori の候補を文全体の自然さで選び直す(学習なしの下見、段階 B)。

文節ごとに候補を入れ替えた文を作り、log P(文 | 前の文) が最大のものを残す(左から 2 巡)。
使い方: python llm_rerank.py <model> <cands.json> [--4bit] [--no-context] [--out res.json]
"""
import argparse, json, time

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig


class Scorer:
    def __init__(self, name, four_bit):
        self.tok = AutoTokenizer.from_pretrained(name)
        kw = {"dtype": torch.bfloat16, "device_map": "cuda"}
        if four_bit:
            kw = {"device_map": "cuda", "quantization_config": BitsAndBytesConfig(
                load_in_4bit=True, bnb_4bit_compute_dtype=torch.bfloat16, bnb_4bit_quant_type="nf4")}
        self.model = AutoModelForCausalLM.from_pretrained(name, **kw).eval()
        bos = self.tok.bos_token_id
        self.head = [bos] if bos is not None else self.tok("\n", add_special_tokens=False).input_ids
        # zenz(azooKey の変換用モデル)は読みを入れたプロンプトで採点する(crates/kotori-lm/src/zenz.rs と同じ)。
        self.zenz = "zenz" in name
        if self.zenz:
            self.head = []
            self.eos = self.tok.convert_tokens_to_ids("</s>")

    @torch.no_grad()
    def score(self, context, sentences, reading=""):
        """各文の log P(文 | 前の文)。前の文と文の境目はトークンの切れ目にする。"""
        if self.zenz:
            kata = "".join(chr(ord(c) + 0x60) if "ぁ" <= c <= "ゖ" else c for c in reading)
            prompt = "" + kata + ("" + context[-40:] if context else "") + ""
            ctx = self.tok(prompt, add_special_tokens=False).input_ids
            seqs = [ctx + self.tok(s, add_special_tokens=False).input_ids + [self.eos] for s in sentences]
        else:
            ctx = self.head + self.tok(context, add_special_tokens=False).input_ids if context else self.head
            seqs = [ctx + self.tok(s, add_special_tokens=False).input_ids for s in sentences]
        n = max(map(len, seqs))
        pad = self.tok.pad_token_id if self.tok.pad_token_id is not None else 0
        ids = torch.tensor([s + [pad] * (n - len(s)) for s in seqs], device="cuda")
        att = torch.tensor([[1] * len(s) + [0] * (n - len(s)) for s in seqs], device="cuda")
        logp = torch.log_softmax(self.model(input_ids=ids, attention_mask=att).logits.float(), -1)
        out = []
        for b, s in enumerate(seqs):
            tgt = torch.tensor(s[len(ctx):], device="cuda")
            pos = torch.arange(len(ctx) - 1, len(s) - 1, device="cuda")
            out.append(logp[b, pos, tgt].sum().item())
        return out


def rerank(scorer, item, use_context, prior=0.0, passes=2):
    segs = item["segments"]
    choice = [0] * len(segs)
    ctx = item["context"] if use_context else ""
    reading = "".join(seg["key"] for seg in segs)
    for _ in range(passes):
        for i, seg in enumerate(segs):
            if len(seg["cands"]) < 2:
                continue
            sents = []
            for c in range(len(seg["cands"])):
                ch = choice[:]; ch[i] = c
                sents.append("".join(s["cands"][j] for s, j in zip(segs, ch)))
            sc = scorer.score(ctx, sents, reading)
            # Kotori(Mozc + zenz)の順位を事前分布として足す。
            sc = [v - prior * c for c, v in enumerate(sc)]
            choice[i] = max(range(len(sc)), key=sc.__getitem__)
    return "".join(s["cands"][j] for s, j in zip(segs, choice))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("model"); ap.add_argument("cands", nargs="+")
    ap.add_argument("--4bit", dest="four_bit", action="store_true")
    ap.add_argument("--no-context", action="store_true")
    ap.add_argument("--prior", type=float, nargs="+", default=[0.0])
    args = ap.parse_args()
    t0 = time.time(); sc = Scorer(args.model, args.four_bit); load = time.time() - t0
    for prior in args.prior:
      for path in args.cands:
        items = json.load(open(path, encoding="utf-8"))
        t0 = time.time(); hit = 0; wrong = []
        for it in items:
            top = rerank(sc, it, not args.no_context, prior)
            if top in it["expected"]:
                hit += 1
            else:
                wrong.append((it["index"], top))
        dt = (time.time() - t0) / len(items)
        print(f"{args.model} prior={prior} ctx={not args.no_context} {path.split('/')[-1]}: "
              f"Acc@1 {hit}/{len(items)} = {100*hit/len(items):.1f}%  ({dt*1000:.0f} ms/問, 読込 {load:.0f}s)", flush=True)
        if len(items) <= 30:
            for w in wrong:
                print("   x", *w, flush=True)


main()
