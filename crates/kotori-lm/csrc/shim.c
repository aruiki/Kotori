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

int32_t kotori_lm_meta(const struct llama_model * model, const char * key, char * buf, size_t buf_size) {
    return llama_model_meta_val_str(model, key, buf, buf_size);
}
