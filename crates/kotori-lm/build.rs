//! llama.cpp(third_party/llama.cpp、コミット固定)を静的ライブラリとして作り、C のシムと
//! つなぐ(docs/SPEC.md 6.4、docs/adr/0006)。
//!
//! `KOTORI_LM_NO_NATIVE=1` なら何も作らない。リンクしない `cargo check` / `clippy` を
//! 別ターゲット向けに回すときに使う(例: Linux 上での windows-gnu の clippy)。

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=csrc/shim.c");
    println!("cargo:rerun-if-env-changed=KOTORI_LM_NO_NATIVE");
    if std::env::var_os("KOTORI_LM_NO_NATIVE").is_some() {
        return;
    }
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default())
        .join("../../third_party/llama.cpp");
    if !root.join("CMakeLists.txt").exists() {
        panic!(
            "{} がない。`git submodule update --init` を実行する",
            root.display()
        );
    }
    println!(
        "cargo:rerun-if-changed={}",
        root.join("include/llama.h").display()
    );

    let dst = cmake::Config::new(&root)
        .profile("Release")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("LLAMA_BUILD_COMMON", "OFF")
        .define("LLAMA_BUILD_TESTS", "OFF")
        .define("LLAMA_BUILD_TOOLS", "OFF")
        .define("LLAMA_BUILD_EXAMPLES", "OFF")
        .define("LLAMA_BUILD_SERVER", "OFF")
        .define("LLAMA_BUILD_APP", "OFF")
        .define("LLAMA_OPENSSL", "OFF")
        // 実行する機械の命令セットに依存させず、どこでも同じバイナリにする。
        .define("GGML_NATIVE", "OFF")
        .define("GGML_OPENMP", "OFF")
        .build();

    for dir in ["lib", "lib64"] {
        println!("cargo:rustc-link-search=native={}", dst.join(dir).display());
    }

    cc::Build::new()
        .file("csrc/shim.c")
        .include(root.join("include"))
        .include(root.join("ggml/include"))
        .compile("kotori_lm_shim");

    for lib in ["llama", "ggml", "ggml-cpu", "ggml-base"] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    let target = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match target.as_str() {
        "linux" => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
            println!("cargo:rustc-link-lib=dylib=m");
        }
        "macos" => println!("cargo:rustc-link-lib=dylib=c++"),
        _ => {}
    }
}
