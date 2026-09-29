/*
 * llama.cpp の API を、構造体を値渡ししない単純な関数に包む(docs/adr/0006)。
 * Rust 側で llama.cpp の構造体の配置を写さずに済むようにする。
 */
#include <stddef.h>
#include <stdint.h>

#include "llama.h"

static void kotori_lm_quiet(enum ggml_log_level level, const char * text, void * user_data) {
    (void) level;
    (void) text;
    (void) user_data;
}

void kotori_lm_init(void) {
    llama_log_set(kotori_lm_quiet, NULL);
    llama_backend_init();
}

struct llama_model * kotori_lm_load(const char * path, int vocab_only) {
    struct llama_model_params params = llama_model_default_params();
    params.vocab_only = vocab_only != 0;
    params.n_gpu_layers = 0;
    return llama_model_load_from_file(path, params);
}

void kotori_lm_free(struct llama_model * model) {
    llama_model_free(model);
}

int32_t kotori_lm_n_vocab(const struct llama_model * model) {
    return llama_vocab_n_tokens(llama_model_get_vocab(model));
}

int32_t kotori_lm_tokenize(const struct llama_model * model, const char * text, int32_t text_len,
                           int32_t * tokens, int32_t n_tokens_max, int add_special) {
    return llama_tokenize(llama_model_get_vocab(model), text, text_len, tokens, n_tokens_max,
                          add_special != 0, false);
}

/* トークンの文字列(GGUF の tokenizer.ggml.tokens の値)。範囲外なら NULL。 */
const char * kotori_lm_token_text(const struct llama_model * model, int32_t token) {
    const struct llama_vocab * vocab = llama_model_get_vocab(model);
    if (token < 0 || token >= llama_vocab_n_tokens(vocab)) {
        return NULL;
    }
    return llama_vocab_get_text(vocab, token);
}

int32_t kotori_lm_meta(const struct llama_model * model, const char * key, char * buf, size_t buf_size) {
    return llama_model_meta_val_str(model, key, buf, buf_size);
}

/* 推論(6.2 モード A)。候補は前置きを共有するので、KV を1つの領域で持つ。 */
struct llama_context * kotori_lm_ctx_new(struct llama_model * model, uint32_t n_ctx, uint32_t n_seq,
                                         int32_t n_threads) {
    struct llama_context_params params = llama_context_default_params();
    params.n_ctx = n_ctx;
    params.n_batch = n_ctx;
    params.n_ubatch = n_ctx;
    params.n_seq_max = n_seq;
    params.n_threads = n_threads;
    params.n_threads_batch = n_threads;
    params.kv_unified = true;
    return llama_init_from_model(model, params);
}

void kotori_lm_ctx_free(struct llama_context * ctx) {
    llama_free(ctx);
}

void kotori_lm_clear(struct llama_context * ctx) {
    llama_memory_clear(llama_get_memory(ctx), true);
}

void kotori_lm_seq_cp(struct llama_context * ctx, int32_t src, int32_t dst) {
    llama_memory_seq_cp(llama_get_memory(ctx), src, dst, -1, -1);
}

/* 1 トークンに 1 シーケンスを割り当てたバッチを作って評価する。0 なら成功。 */
int32_t kotori_lm_decode(struct llama_context * ctx, int32_t n, const int32_t * tokens,
                         const int32_t * pos, const int32_t * seq, const int8_t * want_logits) {
    struct llama_batch batch = llama_batch_init(n, 0, 1);
    for (int32_t i = 0; i < n; i++) {
        batch.token[i] = tokens[i];
        batch.pos[i] = pos[i];
        batch.n_seq_id[i] = 1;
        batch.seq_id[i][0] = seq[i];
        batch.logits[i] = want_logits[i];
    }
    batch.n_tokens = n;
    int32_t ret = llama_decode(ctx, batch);
    llama_batch_free(batch);
    return ret;
}

/* 直前のバッチの i 番目のトークンのロジット(語彙数ぶん)。なければ NULL。 */
const float * kotori_lm_logits(struct llama_context * ctx, int32_t i) {
    return llama_get_logits_ith(ctx, i);
}

int32_t kotori_lm_bos(const struct llama_model * model) {
    return llama_vocab_bos(llama_model_get_vocab(model));
}

int32_t kotori_lm_eos(const struct llama_model * model) {
    return llama_vocab_eos(llama_model_get_vocab(model));
}
