//! CXF vector font and shape parser.
//!
//! Handles both linetype shape files (ltypeshp.cxf) and full font files
//! (Standard.cxf, NormalLatin2.cxf, etc.) with a single parser.
//!
//! Supported CXF geometry commands:
//!   L   x0,y0,x1,y1         — single line segment
//!   PL  x,y,b,...            — open polyline  (x, y, bulge triplets)
//!   PLC x,y,b,...            — closed polyline
//!   A   cx,cy,r,start,end   — arc CCW, angles in degrees
//!   AR  cx,cy,r,start,end   — arc CW,  angles in degrees
//!
//! Entry type decision rule:
//!   [XXXX] LABEL
//!   - If LABEL is exactly one character AND equals char::from_u32(hex)
//!     -> stored as a Unicode glyph (font use-case, e.g. [0041] A)
//!   - Otherwise -> stored as a named shape (ltypeshp, e.g. [0042] BAT)
//!
//! Glyph coordinate space:
//!   Origin = left baseline corner.
//!   Cap height = 9 glyph units.
//!   Scale factor = text_height / 9.0.

use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::OnceLock;

// ── Embedded assets ───────────────────────────────────────────────────────

const SRC_LTYPESHP: &str = include_str!("../../assets/fonts/ltypeshp.cxf");
const SRC_STANDARD: &str = include_str!("../../assets/fonts/Standard.cxf");
const SRC_NORMAL: &str = include_str!("../../assets/fonts/NormalLatin2.cxf");
const SRC_COURIER: &str = include_str!("../../assets/fonts/CourierCad.cxf");
const SRC_SANS: &str = include_str!("../../assets/fonts/SansNS.cxf");
const SRC_ITALIC: &str = include_str!("../../assets/fonts/ItalicT.cxf");
const SRC_GOTHIC: &str = include_str!("../../assets/fonts/GothITT.cxf");
const SRC_CURSIVE: &str = include_str!("../../assets/fonts/Cursive.cxf");
const SRC_SCRIPTC: &str = include_str!("../../assets/fonts/ScriptS.cxf");

// ── Public types ──────────────────────────────────────────────────────────

/// One glyph or shape: a list of open 2-D polyline strokes in local units.
#[derive(Clone, Default)]
pub struct CxfGlyph {
    /// Each stroke is a list of [x, y] points forming a connected polyline.
    pub strokes: Vec<Vec<[f32; 2]>>,
    /// Advance width in glyph units (rightmost X of all strokes).
    pub advance: f32,
}

/// A parsed CXF file.
#[derive(Clone)]
pub struct CxfFile {
    /// Font/file name from the `# Name:` header.
    pub name: String,
    /// Extra space between characters in glyph units (`# LetterSpacing:`).
    pub letter_spacing: f32,
    /// Space character width in glyph units (`# WordSpacing:`).
    pub word_spacing: f32,
    /// Line height multiplier relative to text height (`# LineSpacingFactor:`).
    pub line_spacing: f32,
    /// Named shapes keyed by uppercase name — used for ltypeshp.
    shapes: HashMap<String, CxfGlyph>,
    /// Unicode glyphs keyed by character — used for font files.
    glyphs: HashMap<char, CxfGlyph>,
}

impl CxfFile {
    /// Look up a linetype shape by name (case-insensitive).
    pub fn shape(&self, name: &str) -> Option<&CxfGlyph> {
        self.shapes.get(&name.to_ascii_uppercase())
    }

    /// Look up a font glyph by Unicode character.
    pub fn glyph(&self, c: char) -> Option<&CxfGlyph> {
        self.glyphs.get(&c)
    }
}

// ── Static registries ─────────────────────────────────────────────────────

static SHAPES: OnceLock<CxfFile> = OnceLock::new();
static FONTS: OnceLock<HashMap<String, CxfFile>> = OnceLock::new();

fn shapes_file() -> &'static CxfFile {
    SHAPES.get_or_init(|| parse(SRC_LTYPESHP))
}

fn fonts_map() -> &'static HashMap<String, CxfFile> {
    FONTS.get_or_init(|| {
        let mut map = HashMap::new();
        let mut insert = |src: &str, aliases: &[&str]| {
            let f = parse(src);
            map.insert(f.name.to_ascii_uppercase(), f.clone());
            for alias in aliases {
                map.insert(alias.to_ascii_uppercase(), f.clone());
            }
        };

        insert(SRC_STANDARD, &["STANDARD", "TXT", "TXT.SHX", "SIMPLEX"]);
        insert(
            SRC_NORMAL,
            &["NORMAL", "NORMALLATIN2", "ROMANS", "ROMANS.SHX"],
        );
        insert(SRC_COURIER, &["COURIER", "COURIERCAD", "COURIERCAD"]);
        insert(SRC_SANS, &["SANS", "SANSNS"]);
        insert(SRC_ITALIC, &["ITALIC", "ITALICT"]);
        insert(SRC_GOTHIC, &["GOTHIC", "GOTHITT"]);
        insert(SRC_CURSIVE, &["CURSIVE"]);
        insert(SRC_SCRIPTC, &["SCRIPTS", "SCRIPTC", "SCRIPT"]);
        map
    })
}

// ── Public API — linetype shapes ──────────────────────────────────────────

/// Look up a linetype shape by name (case-insensitive). Used by `complex_lt.rs`.
pub fn get(name: &str) -> Option<&'static CxfGlyph> {
    shapes_file().shape(name)
}

// ── Public API — text tessellation ────────────────────────────────────────

/// Return a font by name (case-insensitive).
/// Falls back to Standard → Normal → any available font.
pub fn get_font(name: &str) -> &'static CxfFile {
    let map = fonts_map();
    let key = name.to_ascii_uppercase();
    map.get(&key)
        .or_else(|| map.get("STANDARD"))
        .or_else(|| map.get("NORMAL"))
        .or_else(|| map.values().next())
        .expect("at least one CXF font must be embedded")
}

/// Tessellate a text string into world-space 2-D strokes.
///
/// - `origin`    — insertion point [world_x, world_y]
/// - `height`    — text height in world units (cap height = 9 glyph units)
/// - `rotation`  — rotation angle in radians (CCW positive)
/// - `font_name` — e.g. "Standard", "Romans", "Courier" (case-insensitive)
/// - `text`      — the string to render
///
/// Returns one Vec<[f32;2]> per stroke. Strokes from different glyphs are
/// separate entries.
#[allow(dead_code)]
pub fn tessellate_text(
    origin: [f32; 2],
    height: f32,
    rotation: f32,
    font_name: &str,
    text: &str,
) -> Vec<Vec<[f32; 2]>> {
    if text.is_empty() || height <= 0.0 {
        return vec![];
    }

    let font = get_font(font_name);
    let scale = height / 9.0; // CXF cap height = 9 glyph units
    let (cos_r, sin_r) = (rotation.cos(), rotation.sin());

    let xform = |gx: f32, gy: f32, cursor_x: f32| -> [f32; 2] {
        let lx = (cursor_x + gx) * scale;
        let ly = gy * scale;
        [
            origin[0] + lx * cos_r - ly * sin_r,
            origin[1] + lx * sin_r + ly * cos_r,
        ]
    };

    let mut out: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut cursor_x: f32 = 0.0;

    for ch in text.chars() {
        if ch == ' ' {
            cursor_x += font.word_spacing;
            continue;
        }
        match font.glyph(ch) {
            Some(glyph) => {
                for stroke in &glyph.strokes {
                    if stroke.len() < 2 {
                        continue;
                    }
                    out.push(
                        stroke
                            .iter()
                            .map(|&[gx, gy]| xform(gx, gy, cursor_x))
                            .collect(),
                    );
                }
                cursor_x += glyph.advance + font.letter_spacing;
            }
            None => {
                cursor_x += 6.0 + font.letter_spacing;
            }
        }
    }

    out
}

pub fn tessellate_text_ex(
    origin: [f32; 2],
    height: f32,
    rotation: f32,
    width_factor: f32,
    oblique_angle: f32,
    font_name: &str,
    text: &str,
) -> Vec<Vec<[f32; 2]>> {
    if text.is_empty() || height <= 0.0 {
        return vec![];
    }

    let font = get_font(font_name);
    let scale = height / 9.0;
    // Negative width_factor produces mirrored (backward) text.
    let wf = if width_factor < 0.0 {
        width_factor.clamp(-100.0, -0.01)
    } else {
        width_factor.clamp(0.01, 100.0)
    };
    let ob = oblique_angle.tan();
    let (cos_r, sin_r) = (rotation.cos(), rotation.sin());

    // Transform from glyph-space (cursor_x + gx, gy) to world space.
    let xform = |gx: f32, gy: f32, cx: f32| -> [f32; 2] {
        let sx = (cx + gx) * scale * wf + gy * scale * ob;
        let sy = gy * scale;
        [
            origin[0] + sx * cos_r - sy * sin_r,
            origin[1] + sx * sin_r + sy * cos_r,
        ]
    };

    let mut out: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut cursor_x: f32 = 0.0;
    // Decoration toggle state: Some(start_cursor_x) when active.
    // Y positions in 9-unit em space (baseline=0, cap=9).
    let mut underline: Option<f32> = None;
    let mut overline: Option<f32> = None;
    let mut strikethrough: Option<f32> = None;
    const UNDER_Y: f32 = -1.5;
    const OVER_Y: f32 = 10.5;
    const STRIKE_Y: f32 = 4.5;

    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        // MText decoration toggles: \L on, \l off, \O on, \o off, \K on, \k off.
        if ch == '\\' {
            match chars.peek().copied() {
                Some('L') => {
                    chars.next();
                    if underline.is_none() { underline = Some(cursor_x); }
                    continue;
                }
                Some('l') => {
                    chars.next();
                    if let Some(s) = underline.take() {
                        out.push(vec![xform(s, UNDER_Y, 0.0), xform(cursor_x, UNDER_Y, 0.0)]);
                    }
                    continue;
                }
                Some('O') => {
                    chars.next();
                    if overline.is_none() { overline = Some(cursor_x); }
                    continue;
                }
                Some('o') => {
                    chars.next();
                    if let Some(s) = overline.take() {
                        out.push(vec![xform(s, OVER_Y, 0.0), xform(cursor_x, OVER_Y, 0.0)]);
                    }
                    continue;
                }
                Some('K') => {
                    chars.next();
                    if strikethrough.is_none() { strikethrough = Some(cursor_x); }
                    continue;
                }
                Some('k') => {
                    chars.next();
                    if let Some(s) = strikethrough.take() {
                        out.push(vec![xform(s, STRIKE_Y, 0.0), xform(cursor_x, STRIKE_Y, 0.0)]);
                    }
                    continue;
                }
                _ => {} // not a decoration code — fall through and render as backslash glyph
            }
        }

        // Resolve DXF %%x special-character sequences inline.
        let render_ch: char = if ch == '%' && chars.peek() == Some(&'%') {
            chars.next(); // consume second '%'
            match chars.peek().map(|c| c.to_ascii_lowercase()) {
                Some('d') => { chars.next(); '°' }
                Some('p') => { chars.next(); '±' }
                Some('c') => { chars.next(); '⌀' }
                Some('%') => { chars.next(); '%' }
                Some('u') => {
                    chars.next();
                    underline = match underline.take() {
                        Some(start) => {
                            out.push(vec![xform(start, UNDER_Y, 0.0), xform(cursor_x, UNDER_Y, 0.0)]);
                            None
                        }
                        None => Some(cursor_x),
                    };
                    continue;
                }
                Some('o') => {
                    chars.next();
                    overline = match overline.take() {
                        Some(start) => {
                            out.push(vec![xform(start, OVER_Y, 0.0), xform(cursor_x, OVER_Y, 0.0)]);
                            None
                        }
                        None => Some(cursor_x),
                    };
                    continue;
                }
                Some(d) if d.is_ascii_digit() => {
                    // %%nnn — 3-digit decimal Unicode scalar
                    let mut digits = String::with_capacity(3);
                    for _ in 0..3 {
                        match chars.peek() {
                            Some(&c) if c.is_ascii_digit() => { digits.push(chars.next().unwrap()); }
                            _ => break,
                        }
                    }
                    if digits.len() == 3 {
                        if let Ok(n) = digits.parse::<u32>() {
                            if let Some(c) = char::from_u32(n) {
                                c
                            } else { continue; }
                        } else { continue; }
                    } else {
                        // Partial digit sequence — advance as unknown glyph and move on
                        cursor_x += (6.0 + font.letter_spacing) * wf;
                        continue;
                    }
                }
                _ => { continue; } // unknown %%x — skip silently
            }
        } else {
            ch
        };

        if render_ch == ' ' {
            cursor_x += font.word_spacing;
            continue;
        }
        match font.glyph(render_ch) {
            Some(glyph) => {
                for stroke in &glyph.strokes {
                    if stroke.len() < 2 {
                        continue;
                    }
                    out.push(
                        stroke
                            .iter()
                            .map(|&[gx, gy]| xform(gx, gy, cursor_x))
                            .collect(),
                    );
                }
                cursor_x += (glyph.advance + font.letter_spacing) * wf;
            }
            None => {
                cursor_x += (6.0 + font.letter_spacing) * wf;
            }
        }
    }

    // Close any decoration spans that weren't explicitly closed.
    if let Some(start) = underline {
        out.push(vec![xform(start, UNDER_Y, 0.0), xform(cursor_x, UNDER_Y, 0.0)]);
    }
    if let Some(start) = overline {
        out.push(vec![xform(start, OVER_Y, 0.0), xform(cursor_x, OVER_Y, 0.0)]);
    }
    if let Some(start) = strikethrough {
        out.push(vec![xform(start, STRIKE_Y, 0.0), xform(cursor_x, STRIKE_Y, 0.0)]);
    }

    out
}

/// Measure the width of a rendered text string in font glyph units × scale.
/// Returns the total advance width in world units (height-independent: divide by height for
/// a normalised ratio, or use directly when `height` is already the desired em size).
pub fn measure_text(text: &str, height: f32, width_factor: f32, font_name: &str) -> f32 {
    if text.is_empty() || height <= 0.0 {
        return 0.0;
    }
    let font = get_font(font_name);
    let scale = height / 9.0;
    let wf = width_factor.clamp(0.01, 100.0);
    let mut cursor_x: f32 = 0.0;
    for ch in text.chars() {
        if ch == ' ' {
            cursor_x += font.word_spacing;
        } else if let Some(glyph) = font.glyph(ch) {
            cursor_x += glyph.advance + font.letter_spacing;
        } else {
            cursor_x += 6.0 + font.letter_spacing;
        }
    }
    cursor_x * scale * wf
}

// ── Parser ────────────────────────────────────────────────────────────────

fn parse(src: &str) -> CxfFile {
    let mut file = CxfFile {
        name: String::from("Unknown"),
        letter_spacing: 3.0,
        word_spacing: 6.75,
        line_spacing: 1.0,
        shapes: HashMap::new(),
        glyphs: HashMap::new(),
    };

    let mut cur_char: Option<char> = None;
    let mut cur_name: Option<String> = None;
    let mut cur_strokes: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut cur_max_x: f32 = 0.0;

    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        // ── Header ────────────────────────────────────────────────────────
        if line.starts_with('#') {
            if let Some(rest) = line.strip_prefix("# Name:") {
                file.name = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("# LetterSpacing:") {
                if let Ok(v) = rest.trim().parse::<f32>() {
                    file.letter_spacing = v;
                }
            } else if let Some(rest) = line.strip_prefix("# WordSpacing:") {
                if let Ok(v) = rest.trim().parse::<f32>() {
                    file.word_spacing = v;
                }
            } else if let Some(rest) = line.strip_prefix("# LineSpacingFactor:") {
                if let Ok(v) = rest.trim().parse::<f32>() {
                    file.line_spacing = v;
                }
            }
            continue;
        }

        // ── Entry header: `[XXXX] LABEL` ─────────────────────────────────
        if line.starts_with('[') {
            flush_entry(
                &mut file,
                &mut cur_char,
                &mut cur_name,
                &mut cur_strokes,
                &mut cur_max_x,
            );

            let hex_end = match line.find(']') {
                Some(i) => i,
                None => continue,
            };
            let hex_str = line[1..hex_end].trim();
            let label = line[hex_end + 1..].trim();

            if let Ok(cp) = u32::from_str_radix(hex_str, 16) {
                let cp_char = char::from_u32(cp);

                // Glyph: label is exactly the one character that the codepoint maps to.
                // Shape: label is a word (different from the codepoint char) or empty
                //        with a non-printable / non-matching codepoint.
                let is_glyph = match cp_char {
                    Some(ch) => label.chars().count() == 1 && label.chars().next() == Some(ch),
                    None => false,
                };

                if is_glyph {
                    cur_char = cp_char;
                    cur_name = None;
                } else if !label.is_empty() {
                    cur_name = Some(label.to_string());
                    cur_char = None;
                } else if let Some(ch) = cp_char {
                    // No label but valid codepoint — treat as glyph.
                    cur_char = Some(ch);
                    cur_name = None;
                }
            }
            continue;
        }

        // ── Geometry (only inside a valid entry) ──────────────────────────
        if cur_char.is_none() && cur_name.is_none() {
            continue;
        }

        let up = line.to_ascii_uppercase();

        if up.starts_with("PLC ") {
            if let Some(s) = parse_triplet_poly(&line[4..], true) {
                update_max_x(&s, &mut cur_max_x);
                if !s.is_empty() {
                    cur_strokes.push(s);
                }
            }
        } else if up.starts_with("PL ") {
            if let Some(s) = parse_triplet_poly(&line[3..], false) {
                update_max_x(&s, &mut cur_max_x);
                if !s.is_empty() {
                    cur_strokes.push(s);
                }
            }
        } else if up.starts_with("AR ") {
            if let Some(s) = parse_arc(&line[3..], true) {
                update_max_x(&s, &mut cur_max_x);
                cur_strokes.push(s);
            }
        } else if up.starts_with("A ") {
            if let Some(s) = parse_arc(&line[2..], false) {
                update_max_x(&s, &mut cur_max_x);
                cur_strokes.push(s);
            }
        } else if up.starts_with("L ") {
            let vals: Vec<f32> = line[2..]
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if vals.len() >= 4 {
                let s = vec![[vals[0], vals[1]], [vals[2], vals[3]]];
                update_max_x(&s, &mut cur_max_x);
                cur_strokes.push(s);
            }
        }
    }

    flush_entry(
        &mut file,
        &mut cur_char,
        &mut cur_name,
        &mut cur_strokes,
        &mut cur_max_x,
    );
    file
}

fn flush_entry(
    file: &mut CxfFile,
    cur_char: &mut Option<char>,
    cur_name: &mut Option<String>,
    cur_strokes: &mut Vec<Vec<[f32; 2]>>,
    cur_max_x: &mut f32,
) {
    let strokes = std::mem::take(cur_strokes);
    let advance = *cur_max_x;
    *cur_max_x = 0.0;
    let glyph = CxfGlyph { strokes, advance };

    if let Some(ch) = cur_char.take() {
        file.glyphs.insert(ch, glyph);
    } else if let Some(name) = cur_name.take() {
        file.shapes.insert(name.to_ascii_uppercase(), glyph);
    }
}

fn update_max_x(stroke: &[[f32; 2]], max_x: &mut f32) {
    for &[x, _] in stroke {
        if x > *max_x {
            *max_x = x;
        }
    }
}

// ── Arc tessellation ──────────────────────────────────────────────────────

/// Parse `cx,cy,radius,start_deg,end_deg` and tessellate into a polyline.
/// `reversed = true` → CW sweep (AR), `false` → CCW (A).
fn parse_arc(data: &str, reversed: bool) -> Option<Vec<[f32; 2]>> {
    let vals: Vec<f32> = data
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if vals.len() < 5 {
        return None;
    }

    let (cx, cy, r) = (vals[0], vals[1], vals[2]);
    let sa = vals[3].to_radians();
    let ea = vals[4].to_radians();
    if r <= 0.0 {
        return None;
    }

    let sweep = if reversed {
        let mut s = ea - sa;
        if s > 0.0 {
            s -= TAU;
        }
        if s.abs() < 1e-6 {
            s = -TAU;
        }
        s
    } else {
        let mut s = ea - sa;
        if s < 0.0 {
            s += TAU;
        }
        if s.abs() < 1e-6 {
            s = TAU;
        }
        s
    };

    let segs = ((sweep.abs() / TAU) * 64.0).ceil().clamp(4.0, 64.0) as u32;
    let mut pts = Vec::with_capacity(segs as usize + 1);
    for i in 0..=segs {
        let t = sa + sweep * (i as f32 / segs as f32);
        pts.push([cx + r * t.cos(), cy + r * t.sin()]);
    }
    Some(pts)
}

// ── Polyline (bulge) tessellation ─────────────────────────────────────────

/// Parse `(x, y, bulge)` triplets from a PL / PLC command.
fn parse_triplet_poly(data: &str, closed: bool) -> Option<Vec<[f32; 2]>> {
    let vals: Vec<f32> = data
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let n = vals.len();
    if n < 6 || n % 3 != 0 {
        return None;
    }

    let triplets: Vec<([f32; 2], f32)> = (0..n)
        .step_by(3)
        .map(|i| ([vals[i], vals[i + 1]], vals[i + 2]))
        .collect();
    let tc = triplets.len();
    let mut pts: Vec<[f32; 2]> = Vec::with_capacity(tc * 6);

    for (idx, &(start, bulge)) in triplets.iter().enumerate() {
        let end = if idx + 1 < tc {
            triplets[idx + 1].0
        } else if closed && tc > 0 {
            triplets[0].0
        } else {
            if pts.last() != Some(&start) {
                pts.push(start);
            }
            break;
        };

        if pts.last() != Some(&start) {
            pts.push(start);
        }

        if bulge.abs() < 1e-7 {
            pts.push(end);
        } else {
            bulge_arc(&mut pts, start, end, bulge, 16);
        }
    }

    if closed {
        if let Some(&first) = pts.first() {
            if pts.last() != Some(&first) {
                pts.push(first);
            }
        }
    }
    Some(pts)
}

/// Approximate a bulge arc from `p0` to `p1`.
/// `bulge = tan(included_angle / 4)`: positive = CCW, negative = CW.
fn bulge_arc(out: &mut Vec<[f32; 2]>, p0: [f32; 2], p1: [f32; 2], bulge: f32, n: usize) {
    let (x1, y1) = (p0[0], p0[1]);
    let (x2, y2) = (p1[0], p1[1]);

    let theta = 4.0 * bulge.atan();
    let dx = x2 - x1;
    let dy = y2 - y1;
    let d = (dx * dx + dy * dy).sqrt();
    if d < 1e-10 {
        out.push(p1);
        return;
    }

    let half_theta = theta * 0.5;
    let r = (d * 0.5) / half_theta.sin().abs();
    let px = -dy / d;
    let py = dx / d;
    let d_mc = r * half_theta.cos();
    let sign = if bulge > 0.0 { 1.0_f32 } else { -1.0_f32 };
    // Center lies to the LEFT of chord for CCW (bulge>0) and to the RIGHT for CW (bulge<0).
    // px = -dy/d is the left-perpendicular unit vector of the chord.
    // CCW (sign=+1): center = mid + perp * d_mc  (left)
    // CW  (sign=-1): center = mid - perp * d_mc  (right)
    let cx = (x1 + x2) * 0.5 + sign * px * d_mc;
    let cy = (y1 + y2) * 0.5 + sign * py * d_mc;

    let a1 = (y1 - cy).atan2(x1 - cx);
    let a2 = (y2 - cy).atan2(x2 - cx);

    // With the corrected center, raw sweep (a2-a1) is already in the right
    // range for most arcs. A single clamp is sufficient — no loop needed.
    let mut sweep = a2 - a1;
    if bulge > 0.0 {
        // CCW: sweep must be in (0, TAU].
        if sweep <= 0.0 {
            sweep += TAU;
        }
    } else {
        // CW: sweep must be in [-TAU, 0).
        if sweep >= 0.0 {
            sweep -= TAU;
        }
    }
    if sweep.abs() < 1e-7 {
        sweep = if bulge > 0.0 { TAU } else { -TAU };
    }

    for i in 1..=n {
        let t = i as f32 / n as f32;
        let a = a1 + sweep * t;
        out.push([cx + r * a.cos(), cy + r * a.sin()]);
    }
}
