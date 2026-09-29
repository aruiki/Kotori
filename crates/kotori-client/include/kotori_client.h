/*
 * Kotori IME の IPC クライアントの C ABI(docs/SPEC.md 10章、4.1、4.2)。
 * 実装は crates/kotori-client/src/ffi.rs と renderer_ffi.rs。文字列はすべて NUL 終端の UTF-8。
 * 関数は状態コード(KOTORI_OK など)を返し、結果は出力引数に書く。
 * 接続は最初の要求のときに行い、サーバーがなければ起動し、切れたら間隔を空けて
 * つなぎ直す(REQ-4-1、REQ-4-2)。つながっていない間はキーをアプリへ渡させる。
 */
#ifndef KOTORI_CLIENT_H
#define KOTORI_CLIENT_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define KOTORI_OK 0
/* キーを処理できなかった(未接続、または 200ms 以内に応答がない)。キーはアプリへ渡し、表示は保つ。 */
#define KOTORI_PASS_THROUGH 1
#define KOTORI_ERR_ARGUMENT (-1)
/* 内部の不具合。キーはアプリへ渡す。 */
#define KOTORI_ERR_INTERNAL (-3)

#define KOTORI_MOD_SHIFT 1u
#define KOTORI_MOD_CTRL 2u
#define KOTORI_MOD_ALT 4u
#define KOTORI_MOD_META 8u

/* IPC の InputScope・CommandKind・SegmentAttribute・InputMode の値(kotori.proto)。 */
#define KOTORI_INPUT_SCOPE_DEFAULT 0u
#define KOTORI_INPUT_SCOPE_PASSWORD 1u
#define KOTORI_COMMAND_SELECT_CANDIDATE 1u
#define KOTORI_COMMAND_COMMIT 2u
#define KOTORI_COMMAND_CANCEL 3u
#define KOTORI_SEGMENT_INPUT 0u
#define KOTORI_SEGMENT_CONVERTED 1u
#define KOTORI_SEGMENT_FOCUSED 2u

typedef struct KotoriClient KotoriClient;
typedef struct KotoriOutput KotoriOutput;

/*
 * 接続を作る。address は UNIX ドメインソケットのパスか名前付きパイプ名で、NULL なら既定。
 * server はつながらないときに起動するサーバーの実行ファイルで、NULL なら起動しない。
 * 文字列が UTF-8 でなければ NULL。
 */
KotoriClient *kotori_client_open(const char *address, const char *server);
void kotori_client_free(KotoriClient *client);
/* サーバーにつながっていれば 1。 */
int32_t kotori_client_connected(const KotoriClient *client);

int32_t kotori_create_session(KotoriClient *client, const char *app_id, uint32_t input_scope,
                              uint64_t *session_id);
int32_t kotori_delete_session(KotoriClient *client, uint64_t session_id);
int32_t kotori_set_context(KotoriClient *client, uint64_t session_id, const char *left_context);
/* session_id は手元のセッション ID(サーバーが再起動しても変わらない)。 */
/* KOTORI_OK なら *out に表示の内容を書く。呼び出し側が kotori_output_free で解放する。 */
int32_t kotori_send_key(KotoriClient *client, uint64_t session_id, uint32_t virtual_key,
                        const char *text, uint32_t modifiers, int32_t key_up, KotoriOutput **out);
int32_t kotori_send_command(KotoriClient *client, uint64_t session_id, uint32_t kind,
                            uint32_t argument, KotoriOutput **out);
/*
 * 遅れて終わった処理(LM のリランク)で表示が変わったかを尋ねる。変わっていれば KOTORI_OK で
 * *out に新しい表示を書く。変わっていない・つながっていなければ KOTORI_PASS_THROUGH
 * (接続もサーバーの起動もしない)。変換中に定期的に呼ぶ。
 */
int32_t kotori_poll_update(KotoriClient *client, uint64_t session_id, KotoriOutput **out);

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
/* 候補の注釈。注釈がなければ空文字列、範囲外なら NULL。 */
const char *kotori_output_candidate_annotation(const KotoriOutput *out, size_t i);
uint32_t kotori_output_candidate_focused(const KotoriOutput *out);
uint32_t kotori_output_input_mode(const KotoriOutput *out);

/*
 * 候補ウィンドウの renderer への送信(docs/adr/0010、実装は src/renderer_ffi.rs)。
 * 送るだけで応答は待たない。renderer につながらなければ起動し、間隔を空けてつなぎ直す。
 * 届けられなかったときは KOTORI_PASS_THROUGH を返す(候補ウィンドウは出ないが入力は続けられる)。
 */
typedef struct KotoriRenderer KotoriRenderer;

/* 画面上の矩形(物理ピクセル)。 */
typedef struct KotoriRect {
  int32_t left;
  int32_t top;
  int32_t right;
  int32_t bottom;
} KotoriRect;

/* address は名前付きパイプ名で、NULL なら既定。exe は起動する renderer で、NULL なら起動しない。 */
KotoriRenderer *kotori_renderer_open(const char *address, const char *exe);
void kotori_renderer_free(KotoriRenderer *renderer);
int32_t kotori_renderer_connected(const KotoriRenderer *renderer);
/*
 * texts は count 個の候補。annotations は NULL か count 個の注釈(要素が NULL なら注釈なし)。
 * caret は注目文節の矩形。owner_window はアプリのトップレベルウィンドウ、notify_window は
 * クリックした候補の番号を受ける TIP のメッセージ専用ウィンドウ(どちらも HWND の値)。
 */
int32_t kotori_renderer_show(KotoriRenderer *renderer, const char *const *texts,
                             const char *const *annotations, size_t count, uint32_t focused,
                             const KotoriRect *caret, uint64_t owner_window,
                             uint64_t notify_window);
int32_t kotori_renderer_hide(KotoriRenderer *renderer);

#ifdef __cplusplus
}
#endif

#endif /* KOTORI_CLIENT_H */
