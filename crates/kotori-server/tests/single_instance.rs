//! ユーザーごとの単一インスタンス保証のテスト(REQ-4-1)。

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::ErrorKind;

use kotori_server::InstanceGuard;

#[cfg(unix)]
#[test]
fn lock_file_allows_only_one_holder() {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("kotori-lock-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("server.lock");

    let first = InstanceGuard::acquire_lock_file(&path).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let err = InstanceGuard::acquire_lock_file(&path).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::AlreadyExists);

    // 解放すれば、残ったロックファイルがあっても再び取れる。
    drop(first);
    let _again = InstanceGuard::acquire_lock_file(&path).unwrap();

    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(windows)]
#[test]
fn named_mutex_allows_only_one_holder() {
    let name = format!(r"Local\kotori-test-{}", std::process::id());
    let first = InstanceGuard::acquire_named(&name).unwrap();
    let err = InstanceGuard::acquire_named(&name).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::AlreadyExists);

    drop(first);
    let _again = InstanceGuard::acquire_named(&name).unwrap();
}
