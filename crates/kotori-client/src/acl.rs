//! 名前付きパイプのアクセス制御(REQ-10-4、docs/adr/0011)。
//!
//! OS に依存しない部分(SDDL の組み立てとアクセス権の値)をここに置き、Linux でもテストする。

/// `FILE_GENERIC_READ`(読み取り、属性の読み取り、同期)。
const FILE_GENERIC_READ: u32 = 0x0012_0089;
/// `FILE_WRITE_DATA`。
const FILE_WRITE_DATA: u32 = 0x0000_0002;
/// パイプでは `FILE_APPEND_DATA` と同じ値で、パイプのインスタンスを作る権限になる。
pub const FILE_CREATE_PIPE_INSTANCE: u32 = 0x0000_0004;

/// クライアントがパイプを開くときに求めるアクセス権。読み取りとデータの書き込みだけにする。
/// `GENERIC_WRITE` は `FILE_APPEND_DATA`(= インスタンスの作成)を含むので使わない。
pub const PIPE_CLIENT_ACCESS: u32 = FILE_GENERIC_READ | FILE_WRITE_DATA;

/// パイプのセキュリティ記述子(SDDL)。
///
/// - 保護付き DACL で、ユーザー本人には全権を与える。
/// - 「すべてのアプリケーションパッケージ」(AC)には [`PIPE_CLIENT_ACCESS`] だけを与え、
///   ストアアプリ(AppContainer)からもつなげるようにする。
/// - 整合性ラベルを低(LW、書き込みは上へ行かない)にして、低整合性のプロセスからもつなげる。
///
/// 同じユーザーの別のプロセスかどうかは、接続を受けたあとにトークンで確かめる。
pub fn pipe_sddl(user_sid: &str) -> String {
    format!("D:P(A;;GA;;;{user_sid})(A;;0x{PIPE_CLIENT_ACCESS:x};;;AC)S:(ML;;NW;;;LW)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_sddl_grants_app_containers_read_write_only() {
        assert_eq!(
            pipe_sddl("S-1-5-21-1-2-3-1001"),
            "D:P(A;;GA;;;S-1-5-21-1-2-3-1001)(A;;0x12008b;;;AC)S:(ML;;NW;;;LW)"
        );
        // インスタンスを作る権限は与えない(パイプの乗っ取りを防ぐ)。
        assert_eq!(PIPE_CLIENT_ACCESS & FILE_CREATE_PIPE_INSTANCE, 0);
    }
}
