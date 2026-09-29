//! 候補ウィンドウの位置(docs/SPEC.md 11.3、REQ-11-3)。OS に依存しない。
//!
//! 座標は物理ピクセルで、マルチモニタでは負になりうる。

/// 画面上の矩形。`right` と `bottom` は含まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// ウィンドウの大きさ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

/// ウィンドウの左上の位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// 候補ウィンドウの左上の位置を決める。
///
/// - 縦はキャレットの下に置く。作業領域の下にはみ出すならキャレットの上に置く。上にも
///   収まらなければ、はみ出しの少ないほうに置き、作業領域の中へ押し込む。
///   入力中の文字(キャレットの矩形)には重ならない。ただし上下どちらにも収まらないときは、
///   画面の外に出さないことを優先する。
/// - 横はキャレットの左端にそろえ、右にはみ出すなら左へ寄せる。左端より左には出さない。
pub fn place(work: Rect, caret: Rect, size: Size) -> Point {
    let below = caret.bottom;
    let above = caret.top - size.height;
    let fits_below = below + size.height <= work.bottom;
    let fits_above = above >= work.top;
    let y = if fits_below {
        below
    } else if fits_above {
        above
    } else {
        // どちらにも収まらない。広いほうに置いて作業領域の中へ押し込む。
        let room_below = work.bottom - caret.bottom;
        let room_above = caret.top - work.top;
        let y = if room_below >= room_above {
            below
        } else {
            above
        };
        clamp(y, work.top, work.bottom - size.height)
    };
    let x = clamp(caret.left, work.left, work.right - size.width);
    Point { x, y }
}

/// `v` を `lo..=hi` に収める。`hi < lo`(ウィンドウが作業領域より大きい)なら `lo`。
fn clamp(v: i32, lo: i32, hi: i32) -> i32 {
    v.min(hi).max(lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1040,
    };
    const SIZE: Size = Size {
        width: 300,
        height: 200,
    };

    fn caret(left: i32, top: i32) -> Rect {
        Rect {
            left,
            top,
            right: left + 2,
            bottom: top + 20,
        }
    }

    fn overlaps(p: Point, size: Size, r: Rect) -> bool {
        p.x < r.right && r.left < p.x + size.width && p.y < r.bottom && r.top < p.y + size.height
    }

    #[test]
    fn below_the_caret_by_default() {
        let c = caret(100, 100);
        let p = place(WORK, c, SIZE);
        assert_eq!(p, Point { x: 100, y: 120 });
        assert!(!overlaps(p, SIZE, c));
    }

    #[test]
    fn above_the_caret_at_the_bottom_edge() {
        let c = caret(100, 1000);
        let p = place(WORK, c, SIZE);
        assert_eq!(p, Point { x: 100, y: 800 });
        assert!(!overlaps(p, SIZE, c));
    }

    #[test]
    fn shifted_left_at_the_right_edge() {
        let p = place(WORK, caret(1800, 100), SIZE);
        assert_eq!(p.x, 1620);
        assert!(p.x + SIZE.width <= WORK.right);
    }

    #[test]
    fn negative_coordinates_on_a_left_monitor() {
        let work = Rect {
            left: -2560,
            top: -200,
            right: 0,
            bottom: 1240,
        };
        let c = caret(-2600, 1220);
        let p = place(work, c, SIZE);
        // 左端より左には出さず、下にはみ出すので上に置く。
        assert_eq!(p, Point { x: -2560, y: 1020 });
        assert!(!overlaps(p, SIZE, c));
    }

    #[test]
    fn too_tall_for_either_side_stays_on_screen() {
        let work = Rect {
            left: 0,
            top: 0,
            right: 800,
            bottom: 300,
        };
        let p = place(work, caret(10, 140), SIZE);
        assert!(p.y >= work.top && p.y + SIZE.height <= work.bottom);
        // ウィンドウが作業領域より大きければ左上にそろえる。
        let big = Size {
            width: 1000,
            height: 400,
        };
        assert_eq!(place(work, caret(10, 140), big), Point { x: 0, y: 0 });
    }
}
