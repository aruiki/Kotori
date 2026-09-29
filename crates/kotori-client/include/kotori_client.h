/*
 * Kotori IME の IPC クライアントの C ABI(docs/SPEC.md 10章、4.2)。
 * 実装は crates/kotori-client/src/ffi.rs。文字列はすべて NUL 終端の UTF-8。
 * 関数は状態コード(KOTORI_OK など)を返し、結果は出力引数に書く。
 */
#ifndef KOTORI_CLIENT_H
#define KOTORI_CLIENT_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define KOTORI_OK 0
/* キーイベントが 200ms 以内に返らなかった。キーはアプリへ渡し、表示は保つ。 */
#define KOTORI_TIMEOUT 1
#define KOTORI_ERR_ARGUMENT (-1)
/* 接続が切れた、またはプロトコルのバージョンが合わない。接続し直す。 */
#define KOTORI_ERR_DISCONNECTED (-2)
#define KOTORI_ERR_SERVER (-3)

#define KOTORI_MOD_SHIFT 1u
#define KOTORI_MOD_CTRL 2u
#define KOTORI_MOD_ALT 4u
#define KOTORI_MOD_META 8u

/* IPC の InputScope・CommandKind・SegmentAttribute・InputMode の値(kotori.proto)。 */
#define KOTORI_INPUT_SCOPE_DEFAULT 0u
#define KOTORI_INPUT_SCOPE_PASSWORD 1u
#define KOTORI_COMMAND_COMMIT 2u
#define KOTORI_COMMAND_CANCEL 3u
#define KOTORI_SEGMENT_INPUT 0u
#define KOTORI_SEGMENT_CONVERTED 1u
#define KOTORI_SEGMENT_FOCUSED 2u

typedef struct KotoriClient KotoriClient;
typedef struct KotoriOutput KotoriOutput;

/* address は UNIX ドメインソケットのパスか名前付きパイプ名。NULL なら既定。失敗なら NULL。 */
KotoriClient *kotori_client_connect(const char *address);
void kotori_client_free(KotoriClient *client);

int32_t kotori_create_session(KotoriClient *client, const char *app_id, uint32_t input_scope,
                              uint64_t *session_id);
int32_t kotori_delete_session(KotoriClient *client, uint64_t session_id);
int32_t kotori_set_context(KotoriClient *client, uint64_t session_id, const char *left_context);
/* KOTORI_OK なら *out に表示の内容を書く。呼び出し側が kotori_output_free で解放する。 */
int32_t kotori_send_key(KotoriClient *client, uint64_t session_id, uint32_t virtual_key,
                        const char *text, uint32_t modifiers, int32_t key_up, KotoriOutput **out);
int32_t kotori_send_command(KotoriClient *client, uint64_t session_id, uint32_t kind,
                            uint32_t argument, KotoriOutput **out);

void kotori_output_free(KotoriOutput *out);
int32_t kotori_output_consumed(const KotoriOutput *out);
size_t kotori_output_preedit_count(const KotoriOutput *out);
/* 文字列は out を解放するまで有効。範囲外なら NULL。 */
const char *kotori_output_preedit_text(const KotoriOutput *out, size_t i);
uint32_t kotori_output_preedit_attribute(const KotoriOutput *out, size_t i);
uint32_t kotori_output_cursor(const KotoriOutput *out);
const char *kotori_output_committed(const KotoriOutput *out);
int32_t kotori_output_candidate_visible(const KotoriOutput *out);
size_t kotori_output_candidate_count(const KotoriOutput *out);
const char *kotori_output_candidate(const KotoriOutput *out, size_t i);
uint32_t kotori_output_candidate_focused(const KotoriOutput *out);
uint32_t kotori_output_input_mode(const KotoriOutput *out);

#ifdef __cplusplus
}
#endif

#endif /* KOTORI_CLIENT_H */
