//! 特殊変換(docs/SPEC.md 5.3)。読みから動的に候補を作る。

/// 日付と時刻。タイムゾーンの扱いは呼び出し側が決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

impl DateTime {
    /// 1970-01-01 からの日数と、その日の秒から作る(UNIX 時刻に時差を足して渡す)。
    pub fn from_unix(secs: i64) -> Self {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        Self {
            year,
            month,
            day,
            hour: (rem / 3600) as u32,
            minute: (rem % 3600 / 60) as u32,
        }
    }

    fn days(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    fn add_days(&self, n: i64) -> Self {
        let (year, month, day) = civil_from_days(self.days() + n);
        Self {
            year,
            month,
            day,
            ..*self
        }
    }

    fn weekday(&self) -> &'static str {
        // 1970-01-01 は木曜日。
        ["木", "金", "土", "日", "月", "火", "水"][self.days().rem_euclid(7) as usize]
    }
}

// Howard Hinnant の days_from_civil / civil_from_days。
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((yoe + era * 400 + i64::from(m <= 2)) as i32, m, d)
}

/// 和暦(令和以降のみ)。
fn wareki(t: &DateTime) -> Option<String> {
    let reiwa_start = days_from_civil(2019, 5, 1);
    if t.days() < reiwa_start {
        return None;
    }
    let n = t.year - 2018;
    let year = if n == 1 {
        "元".to_owned()
    } else {
        n.to_string()
    };
    Some(format!("令和{year}年{}月{}日", t.month, t.day))
}

fn date_candidates(t: &DateTime) -> Vec<String> {
    let mut out = vec![
        format!("{}/{:02}/{:02}", t.year, t.month, t.day),
        format!("{}-{:02}-{:02}", t.year, t.month, t.day),
        format!("{}年{}月{}日", t.year, t.month, t.day),
    ];
    out.extend(wareki(t));
    out.push(format!("{}月{}日({})", t.month, t.day, t.weekday()));
    out
}

const KANJI_DIGITS: [char; 10] = ['〇', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
const DAIJI_DIGITS: [&str; 10] = ["零", "壱", "弐", "参", "四", "五", "六", "七", "八", "九"];

/// 4 桁以下の数を「千百十」の位取りで書く。
fn kanji_under_10000(n: u64, daiji: bool) -> String {
    let units: [&str; 4] = if daiji {
        ["阡", "百", "拾", ""]
    } else {
        ["千", "百", "十", ""]
    };
    let mut out = String::new();
    for (i, unit) in units.iter().enumerate() {
        let d = (n / 10u64.pow(3 - i as u32) % 10) as usize;
        if d == 0 {
            continue;
        }
        // 「一千」「一百」「一十」は「千」「百」「十」と書く(大字は壱を省かない)。
        if d != 1 || unit.is_empty() || daiji {
            if daiji {
                out.push_str(DAIJI_DIGITS[d]);
            } else {
                out.push(KANJI_DIGITS[d]);
            }
        }
        out.push_str(unit);
    }
    out
}

/// 万・億・兆・京で区切った漢数字(`daiji` なら大字)。
fn kanji_number(n: u64, daiji: bool) -> String {
    if n == 0 {
        return if daiji { "零" } else { "〇" }.to_owned();
    }
    let big = ["", if daiji { "萬" } else { "万" }, "億", "兆", "京"];
    let mut out = String::new();
    for (i, unit) in big.iter().enumerate().rev() {
        let part = n / 10_000u64.pow(i as u32) % 10_000;
        if part > 0 {
            out.push_str(&kanji_under_10000(part, daiji));
            out.push_str(unit);
        }
    }
    out
}

/// 「1万2345」のような位取り表記。
fn mixed_number(n: u64) -> String {
    let big = ["", "万", "億", "兆", "京"];
    let mut out = String::new();
    for (i, unit) in big.iter().enumerate().rev() {
        let part = n / 10_000u64.pow(i as u32) % 10_000;
        if part > 0 {
            out.push_str(&format!("{part}{unit}"));
        }
    }
    if out.is_empty() {
        out.push('0');
    }
    out
}

fn with_commas(digits: &str) -> String {
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn number_candidates(digits: &str) -> Vec<String> {
    let full: String = digits
        .chars()
        .map(|c| char::from_u32(c as u32 - '0' as u32 + '０' as u32).unwrap_or(c))
        .collect();
    let per_digit: String = digits
        .chars()
        .map(|c| KANJI_DIGITS[c as usize - '0' as usize])
        .collect();
    let mut out = vec![digits.to_owned(), full, with_commas(digits)];
    // 先頭が 0 の列(電話番号など)は数値として読まない。
    if let (Ok(n), false) = (
        digits.parse::<u64>(),
        digits.len() > 1 && digits.starts_with('0'),
    ) {
        out.push(mixed_number(n));
        out.push(kanji_number(n, false));
        out.push(kanji_number(n, true));
    }
    out.push(per_digit);
    out
}

/// 単位と記号の読み。
const UNITS: &[(&str, &[&str])] = &[
    ("ヘイホウメートル", &["㎡", "m²", "平方メートル"]),
    ("リッポウメートル", &["㎥", "m³", "立方メートル"]),
    ("ヘイホウセンチメートル", &["㎠", "cm²"]),
    ("キロメートル", &["㎞", "km", "キロメートル"]),
    ("センチメートル", &["㎝", "cm", "センチメートル"]),
    ("ミリメートル", &["㎜", "mm", "ミリメートル"]),
    ("キログラム", &["㎏", "kg", "キログラム"]),
    ("リットル", &["ℓ", "L", "リットル"]),
    ("ド", &["℃", "°", "度"]),
    ("パーセント", &["%", "％", "パーセント"]),
];

/// 読み全体に対する特殊変換の候補。該当しなければ空。
pub fn special_candidates(reading: &str, now: &DateTime) -> Vec<String> {
    let ascii: String = reading
        .chars()
        .map(|c| match c {
            '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c),
            _ => c,
        })
        .collect();
    if !ascii.is_empty() && ascii.chars().all(|c| c.is_ascii_digit()) {
        return number_candidates(&ascii);
    }
    match reading {
        "キョウ" | "ホンジツ" => return date_candidates(now),
        "アシタ" | "アス" | "ミョウニチ" => return date_candidates(&now.add_days(1)),
        "キノウ" | "サクジツ" => return date_candidates(&now.add_days(-1)),
        "イマ" => {
            return vec![
                format!("{:02}:{:02}", now.hour, now.minute),
                format!("{}時{}分", now.hour, now.minute),
            ]
        }
        _ => {}
    }
    UNITS
        .iter()
        .find(|(r, _)| *r == reading)
        .map(|(_, s)| s.iter().map(|x| (*x).to_owned()).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime {
        DateTime {
            year: 2026,
            month: 9,
            day: 28,
            hour: 9,
            minute: 5,
        }
    }

    #[test]
    fn numbers() {
        assert_eq!(
            special_candidates("12345", &now()),
            [
                "12345",
                "１２３４５",
                "12,345",
                "1万2345",
                "一万二千三百四十五",
                "壱萬弐阡参百四拾五",
                "一二三四五"
            ]
        );
        let c = special_candidates("１００００００１", &now());
        assert!(c.contains(&"1000万1".to_owned()), "{c:?}");
        assert!(
            c.contains(&"一千万一".to_owned()) || c.contains(&"千万一".to_owned()),
            "{c:?}"
        );
        // 0 始まりは数値の読みを出さない。
        assert_eq!(
            special_candidates("090", &now()),
            ["090", "０９０", "090", "〇九〇"]
        );
        assert_eq!(special_candidates("0", &now())[3..6], ["0", "〇", "零"]);
        assert!(special_candidates(&"9".repeat(19), &now()).len() == 7);
        assert!(
            special_candidates(&"9".repeat(25), &now()).len() == 4,
            "u64 を超える桁"
        );
    }

    #[test]
    fn dates_and_time() {
        assert_eq!(
            special_candidates("キョウ", &now()),
            [
                "2026/09/28",
                "2026-09-28",
                "2026年9月28日",
                "令和8年9月28日",
                "9月28日(月)"
            ]
        );
        assert_eq!(special_candidates("アシタ", &now())[2], "2026年9月29日");
        let eom = DateTime {
            month: 12,
            day: 31,
            ..now()
        };
        assert_eq!(special_candidates("アス", &eom)[0], "2027/01/01");
        assert_eq!(special_candidates("キノウ", &now())[4], "9月27日(日)");
        assert_eq!(special_candidates("イマ", &now()), ["09:05", "9時5分"]);
        let first = DateTime {
            year: 2019,
            month: 5,
            day: 1,
            ..now()
        };
        assert_eq!(special_candidates("キョウ", &first)[3], "令和元年5月1日");
        let old = DateTime {
            year: 2019,
            month: 4,
            day: 30,
            ..now()
        };
        assert_eq!(
            special_candidates("キョウ", &old).len(),
            4,
            "令和より前は和暦を出さない"
        );
    }

    #[test]
    fn unix_time_to_date() {
        assert_eq!(
            DateTime::from_unix(0),
            DateTime {
                year: 1970,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0
            }
        );
        // 2026-09-28T09:05:00Z
        assert_eq!(DateTime::from_unix(1_790_586_300), now());
    }

    #[test]
    fn units_and_others() {
        assert_eq!(
            special_candidates("ヘイホウメートル", &now())[..2],
            ["㎡", "m²"]
        );
        assert!(special_candidates("ネコ", &now()).is_empty());
        assert!(special_candidates("", &now()).is_empty());
    }
}
