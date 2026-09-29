//! Windows の候補ウィンドウ(docs/SPEC.md 4.1、REQ-10-7、11.3、docs/adr/0010)。
//!
//! TIP から `\\.\pipe\kotori-renderer-<SID>` で候補と位置を受け取って描く。ユーザーごとに
//! 1つだけ動く(パイプの最初のインスタンスを作れなければ、ほかが動いているので終わる)。
//! パイプの ACL と接続元の確認はサーバーと同じ(REQ-10-4、docs/adr/0011)。
//! 今は受けた内容をログに出すだけで、ウィンドウは作らない(作業カード 14)。

// 位置の計算はウィンドウを作る変更(作業カード 14 の後半)で使う。
#[allow(dead_code)]
mod placement;

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use anyhow::Context;
    use kotori_proto::renderer::RendererMessage;
    use kotori_proto::{read_message, PROTOCOL_MAJOR};

    let name = kotori_client::default_renderer_pipe_name().context("パイプ名を決められない")?;
    let mut listener = kotori_client::PipeListener::bind(&name)
        .with_context(|| format!("{name} で待ち受けできない(ほかの renderer が動いている)"))?;
    eprintln!("kotori-renderer: {name} で待ち受け");
    loop {
        let mut stream = listener.accept()?;
        std::thread::spawn(move || {
            while let Ok(Some(msg)) = read_message::<_, RendererMessage>(&mut stream) {
                // メジャーバージョンの違う TIP からの内容は描かない。
                if msg.protocol_major == PROTOCOL_MAJOR {
                    eprintln!("kotori-renderer: {:?}", msg.body);
                }
            }
        });
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("kotori-renderer は Windows 専用(REQ-4-3)");
}
