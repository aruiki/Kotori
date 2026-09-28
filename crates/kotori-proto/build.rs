//! `proto/kotori.proto` から Rust の型を生成する。
//! 純 Rust の `protox` でコンパイルするため、`protoc` のインストールは不要。

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto");
    let fds = protox::compile(["kotori.proto"], ["proto"])?;
    prost_build::Config::new().compile_fds(fds)?;
    Ok(())
}
