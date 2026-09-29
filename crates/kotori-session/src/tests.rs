#![allow(clippy::unwrap_used)]

use super::keymap::{parse_key, vk, MS_IME_KEYMAP};
use super::*;

fn key(s: &str) -> Key {
    parse_key(s).unwrap()
}

#[test]
fn keys_parse_with_modifiers() {
    assert_eq!(key("Space"), Key::plain(vk::SPACE));
    assert_eq!(
        key("Shift+Left"),
        Key {
            vk: vk::LEFT,
            shift: true,
            ctrl: false,
            alt: false
        }
    );
    let k = key("Ctrl+Alt+Delete");
    assert!(k.ctrl && k.alt && !k.shift && k.vk == vk::DELETE);
    assert_eq!(key("7").vk, 0x37);
    assert_eq!(key("F10").vk, 0x79);
    for bad in [
        "",
        "Shift+",
        "Hyper+Space",
        "Shift+Shift+Space",
        "F13",
        "F0",
        "a",
        "12",
    ] {
        assert_eq!(parse_key(bad), None, "{bad:?}");
    }
}

/// 同梱の MS-IME 互換キーマップが 11.2 の表どおりに引ける。
#[test]
fn ms_ime_preset_follows_spec_table() {
    use Command::*;
    use State::*;
    let km = Keymap::parse_tsv(MS_IME_KEYMAP).unwrap();
    let cases: &[(State, &str, Option<Command>)] = &[
        (Composing, "Space", Some(Convert)),
        (Converting, "Space", Some(NextCandidate)),
        (Selecting, "Convert", Some(NextCandidate)),
        (Composing, "Shift+Space", Some(ConvertPrev)),
        (Selecting, "Shift+Space", Some(PrevCandidate)),
        (Composing, "Enter", Some(Commit)),
        (Selecting, "Enter", Some(Commit)),
        (Composing, "Escape", Some(CancelInput)),
        (Converting, "Escape", Some(BackToComposing)),
        (Selecting, "Escape", Some(BackToConverting)),
        (Composing, "Backspace", Some(DeleteBack)),
        (Converting, "Backspace", Some(BackToComposing)),
        (Selecting, "Backspace", Some(BackToComposing)),
        (Composing, "Left", Some(CursorLeft)),
        (Converting, "Right", Some(FocusNext)),
        (Composing, "Shift+Left", None),
        (Converting, "Shift+Left", Some(ShrinkSegment)),
        (Selecting, "Shift+Right", Some(ExpandSegment)),
        (Composing, "Down", Some(Predict)),
        (Composing, "Up", None),
        (Converting, "Up", Some(PrevCandidate)),
        (Selecting, "Down", Some(NextCandidate)),
        (Selecting, "1", Some(Select(1))),
        (Selecting, "9", Some(Select(9))),
        (Converting, "1", None),
        (Composing, "F6", Some(ToHiragana)),
        (Converting, "F8", Some(ToHalfKatakana)),
        (Selecting, "F10", Some(ToHalfAscii)),
        (Composing, "Tab", Some(Predict)),
        (Converting, "Tab", None),
        (Selecting, "Tab", Some(NextPage)),
        (Selecting, "Ctrl+Delete", Some(ForgetCandidate)),
        (Idle, "Ctrl+Backspace", Some(UndoCommit)),
        (Idle, "Space", None),
    ];
    for &(state, k, want) in cases {
        assert_eq!(km.command(state, key(k)), want, "{state:?} {k}");
    }
    assert_eq!(
        Keymap::ms_ime().command(Composing, key("Space")),
        Some(Convert)
    );
}

#[test]
fn keymap_errors_name_the_line() {
    let ok = "# コメント\n\ncomposing\tSpace\tconvert\n";
    assert!(Keymap::parse_tsv(ok).is_ok());
    let cases = [
        ("composing\tSpace", KeymapError::Columns { line: 1 }),
        (
            "typing\tSpace\tconvert",
            KeymapError::UnknownState {
                line: 1,
                name: "typing".into(),
            },
        ),
        (
            "composing\tHyper\tconvert",
            KeymapError::UnknownKey {
                line: 1,
                name: "Hyper".into(),
            },
        ),
        (
            "composing\tSpace\tselect-10",
            KeymapError::UnknownCommand {
                line: 1,
                name: "select-10".into(),
            },
        ),
        (
            "composing\tSpace\tconvert\ncomposing\tSpace\tcommit",
            KeymapError::Duplicate { line: 2 },
        ),
    ];
    for (src, want) in cases {
        assert_eq!(Keymap::parse_tsv(src).unwrap_err(), want, "{src:?}");
    }
}
