//! Font files for DXF text styles.
//!
//! Jw_cad names the font of a text by its Windows face (`ＭＳ ゴシック`). A DXF
//! text style names the font *file* (`msgothic.ttc`) and keeps the TrueType
//! family in the style's `ACAD` xdata. AutoCAD reads a face name in the file
//! field as a missing SHX font, substitutes one and shows Japanese text as `?`.
//!
//! The table and the rules follow `cad2d_ir.fonts`, so a JWW drawing gets the
//! same text styles whether it reaches DXF through this converter or the IR.

/// How a DXF text style names its font.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyleFont {
    /// The style's font file (group 3).
    pub file: String,
    /// TrueType family for the style's `ACAD` xdata; `None` for SHX fonts.
    pub family: Option<String>,
    /// The xdata `1071` value (charset << 8 | pitch and family).
    pub flags: i32,
}

struct TrueTypeFont {
    /// English family name, the one AutoCAD keeps in the style xdata.
    family: &'static str,
    /// File in `%WINDIR%\Fonts`; the first family of a `.ttc` comes first.
    file: &'static str,
    /// Face name as Windows (and Jw_cad) spells it.
    face: &'static str,
    flags: i32,
}

const SHIFTJIS: i32 = 128 << 8;
const HANGUL: i32 = 129 << 8;
const GB2312: i32 = 134 << 8;
const FIXED_MODERN: i32 = 0x31;
const VARIABLE_MODERN: i32 = 0x32;
const FIXED_ROMAN: i32 = 0x11;
const VARIABLE_ROMAN: i32 = 0x12;
const VARIABLE_SWISS: i32 = 0x22;

macro_rules! font {
    ($family:literal, $file:literal, $face:literal, $flags:expr) => {
        TrueTypeFont {
            family: $family,
            file: $file,
            face: $face,
            flags: $flags,
        }
    };
}

#[rustfmt::skip]
const FONTS: &[TrueTypeFont] = &[
    font!("MS Gothic", "msgothic.ttc", "ＭＳ ゴシック", SHIFTJIS | FIXED_MODERN),
    font!("MS PGothic", "msgothic.ttc", "ＭＳ Ｐゴシック", SHIFTJIS | VARIABLE_MODERN),
    font!("MS UI Gothic", "msgothic.ttc", "MS UI Gothic", SHIFTJIS | VARIABLE_MODERN),
    font!("MS Mincho", "msmincho.ttc", "ＭＳ 明朝", SHIFTJIS | FIXED_ROMAN),
    font!("MS PMincho", "msmincho.ttc", "ＭＳ Ｐ明朝", SHIFTJIS | VARIABLE_ROMAN),
    font!("Meiryo", "meiryo.ttc", "メイリオ", SHIFTJIS | VARIABLE_MODERN),
    font!("Meiryo UI", "meiryo.ttc", "Meiryo UI", SHIFTJIS | VARIABLE_MODERN),
    font!("Yu Gothic", "YuGothR.ttc", "游ゴシック", SHIFTJIS | VARIABLE_MODERN),
    font!("Yu Gothic UI", "YuGothR.ttc", "Yu Gothic UI", SHIFTJIS | VARIABLE_MODERN),
    font!("Yu Gothic Medium", "YuGothM.ttc", "游ゴシック Medium", SHIFTJIS | VARIABLE_MODERN),
    font!("Yu Mincho", "yumin.ttf", "游明朝", SHIFTJIS | VARIABLE_ROMAN),
    font!("BIZ UDGothic", "BIZ-UDGothicR.ttc", "BIZ UDゴシック", SHIFTJIS | FIXED_MODERN),
    font!("BIZ UDPGothic", "BIZ-UDGothicR.ttc", "BIZ UDPゴシック", SHIFTJIS | VARIABLE_MODERN),
    font!("BIZ UDMincho Medium", "BIZ-UDMinchoM.ttc", "BIZ UD明朝 Medium", SHIFTJIS | FIXED_ROMAN),
    font!("BIZ UDPMincho Medium", "BIZ-UDMinchoM.ttc", "BIZ UDP明朝 Medium", SHIFTJIS | VARIABLE_ROMAN),
    // Microsoft Office (common on Jw_cad users' machines).
    font!("HGGothicM", "HGRGM.TTC", "HGｺﾞｼｯｸM", SHIFTJIS | FIXED_MODERN),
    font!("HGPGothicM", "HGRGM.TTC", "HGPｺﾞｼｯｸM", SHIFTJIS | VARIABLE_MODERN),
    font!("HGSGothicM", "HGRGM.TTC", "HGSｺﾞｼｯｸM", SHIFTJIS | VARIABLE_MODERN),
    font!("HGGothicE", "HGRGE.TTC", "HGｺﾞｼｯｸE", SHIFTJIS | FIXED_MODERN),
    font!("HGPGothicE", "HGRGE.TTC", "HGPｺﾞｼｯｸE", SHIFTJIS | VARIABLE_MODERN),
    font!("HGSGothicE", "HGRGE.TTC", "HGSｺﾞｼｯｸE", SHIFTJIS | VARIABLE_MODERN),
    font!("HGMinchoE", "HGRME.TTC", "HG明朝E", SHIFTJIS | FIXED_ROMAN),
    font!("HGPMinchoE", "HGRME.TTC", "HGP明朝E", SHIFTJIS | VARIABLE_ROMAN),
    font!("HGSMinchoE", "HGRME.TTC", "HGS明朝E", SHIFTJIS | VARIABLE_ROMAN),
    font!("HGMinchoB", "HGRMB.TTC", "HG明朝B", SHIFTJIS | FIXED_ROMAN),
    font!("HGPMinchoB", "HGRMB.TTC", "HGP明朝B", SHIFTJIS | VARIABLE_ROMAN),
    font!("HGSMinchoB", "HGRMB.TTC", "HGS明朝B", SHIFTJIS | VARIABLE_ROMAN),
    font!("HGMaruGothicMPRO", "HGRSMP.TTF", "HG丸ｺﾞｼｯｸM-PRO", SHIFTJIS | VARIABLE_MODERN),
    font!("HGSoeiKakugothicUB", "HGRSGU.TTC", "HG創英角ｺﾞｼｯｸUB", SHIFTJIS | FIXED_MODERN),
    font!("Arial", "arial.ttf", "Arial", VARIABLE_SWISS),
    font!("Arial Narrow", "ARIALN.TTF", "Arial Narrow", VARIABLE_SWISS),
    font!("Arial Black", "ariblk.ttf", "Arial Black", VARIABLE_SWISS),
    font!("Times New Roman", "times.ttf", "Times New Roman", VARIABLE_ROMAN),
    font!("Courier New", "cour.ttf", "Courier New", FIXED_MODERN),
    font!("Century", "CENTURY.TTF", "Century", VARIABLE_ROMAN),
    font!("Century Gothic", "GOTHIC.TTF", "Century Gothic", VARIABLE_SWISS),
    font!("Tahoma", "tahoma.ttf", "Tahoma", VARIABLE_SWISS),
    font!("Verdana", "verdana.ttf", "Verdana", VARIABLE_SWISS),
    font!("Calibri", "calibri.ttf", "Calibri", VARIABLE_SWISS),
    font!("Segoe UI", "segoeui.ttf", "Segoe UI", VARIABLE_SWISS),
    font!("ISOCPEUR", "isocpeur.ttf", "ISOCPEUR", VARIABLE_SWISS),
    font!("SimSun", "simsun.ttc", "宋体", GB2312 | FIXED_MODERN),
    font!("NSimSun", "simsun.ttc", "新宋体", GB2312 | FIXED_MODERN),
    font!("SimHei", "simhei.ttf", "黑体", GB2312 | FIXED_MODERN),
    font!("FangSong", "simfang.ttf", "仿宋", GB2312 | FIXED_MODERN),
    font!("KaiTi", "simkai.ttf", "楷体", GB2312 | FIXED_MODERN),
    font!("Microsoft YaHei", "msyh.ttc", "微软雅黑", GB2312 | VARIABLE_SWISS),
    font!("Gulim", "gulim.ttc", "굴림", HANGUL | VARIABLE_SWISS),
    font!("Batang", "batang.ttc", "바탕", HANGUL | VARIABLE_ROMAN),
    font!("Malgun Gothic", "malgun.ttf", "맑은 고딕", HANGUL | VARIABLE_SWISS),
];

/// Lookup key: full-width ASCII folded, spaces, hyphens and underscores dropped, ASCII lowercased.
fn key(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch),
            _ => ch,
        })
        .filter(|ch| !ch.is_whitespace() && !matches!(ch, '-' | '_'))
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn has_suffix(name: &str, suffixes: &[&str]) -> bool {
    let lower = name.to_ascii_lowercase();
    suffixes.iter().any(|suffix| lower.ends_with(suffix))
}

const TRUETYPE_SUFFIXES: &[&str] = &[".ttf", ".ttc", ".otf"];

fn lookup(name: &str) -> Option<&'static TrueTypeFont> {
    let mut bare = name.trim();
    if let Some(rest) = bare.strip_prefix('@') {
        bare = rest.trim();
    }
    if bare.is_empty() {
        return None;
    }
    if has_suffix(bare, TRUETYPE_SUFFIXES) {
        if let Some(found) = FONTS
            .iter()
            .find(|font| font.file.eq_ignore_ascii_case(bare))
        {
            return Some(found);
        }
        bare = &bare[..bare.rfind('.').unwrap_or(bare.len())];
    }
    let wanted = key(bare);
    FONTS
        .iter()
        .find(|font| key(font.family) == wanted || key(font.face) == wanted)
}

/// A file that draws the script of an unknown face, and that file's charset bits.
fn fallback_file(face: &str) -> (String, i32) {
    let has = |range: std::ops::RangeInclusive<char>| face.chars().any(|ch| range.contains(&ch));
    if has('\u{3040}'..='\u{30FF}') || has('\u{FF01}'..='\u{FF9F}') {
        ("msgothic.ttc".to_string(), SHIFTJIS)
    } else if has('\u{AC00}'..='\u{D7AF}') {
        ("gulim.ttc".to_string(), HANGUL)
    } else if has('\u{3400}'..='\u{4DBF}') || has('\u{4E00}'..='\u{9FFF}') {
        ("simsun.ttc".to_string(), GB2312)
    } else {
        (format!("{face}.ttf"), 0)
    }
}

/// The font file and TrueType family a DXF text style needs for a Jw_cad font name.
///
/// Known faces get their real file and English family. An unknown face keeps
/// its name as the family and gets a file that at least draws its script.
/// SHX names (`txt`, `simplex`) pass through; an empty name or `STANDARD`
/// becomes `txt`.
pub(crate) fn style_font(font: &str) -> StyleFont {
    let name = font.trim();
    let shx = |file: &str| StyleFont {
        file: file.to_string(),
        family: None,
        flags: 0,
    };
    if name.is_empty() || name.eq_ignore_ascii_case("STANDARD") {
        return shx("txt");
    }
    if has_suffix(name, &[".shx"]) {
        return shx(name);
    }
    if let Some(known) = lookup(name) {
        let vertical = if name.starts_with('@') { "@" } else { "" };
        return StyleFont {
            file: known.file.to_string(),
            family: Some(format!("{vertical}{}", known.family)),
            flags: known.flags,
        };
    }
    if has_suffix(name, TRUETYPE_SUFFIXES) {
        return shx(name);
    }
    if name.is_ascii() && !name.contains(' ') {
        // A bare SHX name such as `txt` or `simplex`.
        return shx(name);
    }
    let bare = name.strip_prefix('@').unwrap_or(name).trim();
    let (file, flags) = fallback_file(bare);
    StyleFont {
        file,
        family: Some(name.to_string()),
        flags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn truetype(file: &str, family: &str, flags: i32) -> StyleFont {
        StyleFont {
            file: file.to_string(),
            family: Some(family.to_string()),
            flags,
        }
    }

    #[test]
    fn jw_cad_faces_become_the_files_autocad_looks_for() {
        assert_eq!(
            style_font("ＭＳ ゴシック"),
            truetype("msgothic.ttc", "MS Gothic", 0x8031)
        );
        assert_eq!(
            style_font("ＭＳ Ｐゴシック"),
            truetype("msgothic.ttc", "MS PGothic", 0x8032)
        );
        assert_eq!(
            style_font("ＭＳ 明朝"),
            truetype("msmincho.ttc", "MS Mincho", 0x8011)
        );
        assert_eq!(
            style_font("@ＭＳ ゴシック"),
            truetype("msgothic.ttc", "@MS Gothic", 0x8031)
        );
        assert_eq!(
            style_font("HGｺﾞｼｯｸM"),
            truetype("HGRGM.TTC", "HGGothicM", 0x8031)
        );
        assert_eq!(
            style_font("BIZ UDPゴシック"),
            truetype("BIZ-UDGothicR.ttc", "BIZ UDPGothic", 0x8032)
        );
        assert_eq!(style_font("Arial"), truetype("arial.ttf", "Arial", 0x22));
        assert_eq!(
            style_font("MS PGothic.ttf"),
            truetype("msgothic.ttc", "MS PGothic", 0x8032)
        );
    }

    #[test]
    fn unknown_faces_keep_their_name_and_get_a_file_for_their_script() {
        assert_eq!(
            style_font("FCフォント"),
            truetype("msgothic.ttc", "FCフォント", 0x8000)
        );
        assert_eq!(
            style_font("helvetica neue"),
            truetype("helvetica neue.ttf", "helvetica neue", 0)
        );
    }

    #[test]
    fn shx_names_pass_through() {
        let shx = |file: &str| StyleFont {
            file: file.to_string(),
            family: None,
            flags: 0,
        };
        assert_eq!(style_font(""), shx("txt"));
        assert_eq!(style_font("STANDARD"), shx("txt"));
        assert_eq!(style_font("txt"), shx("txt"));
        assert_eq!(style_font("romans.shx"), shx("romans.shx"));
    }
}
