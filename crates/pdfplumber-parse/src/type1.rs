//! Encoding recovery from embedded Type 1 font programs.
//!
//! A Type 1 font carries its own code-to-glyph mapping in the cleartext header
//! of its font program, as PostScript of the form:
//!
//! ```text
//! /Encoding 256 array
//! 0 1 255 {1 index exch /.notdef put} for
//! dup 39 /phi1 put
//! dup 69 /E put
//! readonly def
//! ```
//!
//! PDFs produced by TeX rely on this: their fonts have neither an `/Encoding`
//! entry nor a `/ToUnicode` map, and without reading the program a paper's
//! symbols decode as unrelated ASCII.

use std::collections::HashMap;

/// Read the `dup <code> /<glyph> put` entries of a Type 1 font program.
///
/// Only the cleartext header is inspected: scanning stops at `eexec`, where the
/// encrypted portion begins. Returns the code-to-glyph-name pairs found, which
/// is empty for a font that leaves its encoding to the PDF.
pub fn parse_builtin_encoding(font_program: &[u8]) -> HashMap<u8, String> {
    let header_end = find(font_program, b"eexec").unwrap_or(font_program.len());
    let header = &font_program[..header_end];

    let mut encoding = HashMap::new();
    let mut position = 0;

    while let Some(offset) = find(&header[position..], b"dup ") {
        position += offset + b"dup ".len();

        let Some((code, after_code)) = read_number(header, position) else {
            continue;
        };
        let Some(after_slash) = expect_slash(header, after_code) else {
            continue;
        };
        let (name, after_name) = read_name(header, after_slash);
        if name.is_empty() || !follows_with_put(header, after_name) {
            continue;
        }

        if let Ok(code) = u8::try_from(code) {
            encoding.insert(code, name);
        }
        position = after_name;
    }

    encoding
}

/// Index of the first occurrence of `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Read a decimal number at `position`, returning it and the offset after it.
fn read_number(data: &[u8], position: usize) -> Option<(u32, usize)> {
    let mut end = position;
    while end < data.len() && data[end].is_ascii_digit() {
        end += 1;
    }
    if end == position {
        return None;
    }

    let digits = std::str::from_utf8(&data[position..end]).ok()?;
    Some((digits.parse().ok()?, end))
}

/// Skip spaces and the `/` that introduces a glyph name.
fn expect_slash(data: &[u8], position: usize) -> Option<usize> {
    let mut position = position;
    while position < data.len() && data[position] == b' ' {
        position += 1;
    }
    (data.get(position) == Some(&b'/')).then_some(position + 1)
}

/// Read a PostScript name, which runs until whitespace or a delimiter.
fn read_name(data: &[u8], position: usize) -> (String, usize) {
    let mut end = position;
    while end < data.len() && !is_name_terminator(data[end]) {
        end += 1;
    }
    (
        String::from_utf8_lossy(&data[position..end]).into_owned(),
        end,
    )
}

/// Whether `byte` ends a PostScript name.
fn is_name_terminator(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b'/' | b'(' | b')' | b'[' | b']' | b'{' | b'}')
}

/// Whether the next token is `put`, which completes an encoding entry.
fn follows_with_put(data: &[u8], position: usize) -> bool {
    let mut position = position;
    while position < data.len() && data[position].is_ascii_whitespace() {
        position += 1;
    }
    data[position..].starts_with(b"put")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CM_HEADER: &[u8] = b"%!PS-AdobeFont-1.0: CMMI10\n\
/Encoding 256 array\n\
0 1 255 {1 index exch /.notdef put} for\n\
dup 39 /phi1 put\n\
dup 69 /E put\n\
dup 59 /comma put\n\
readonly def\n\
currentdict end\n\
currentfile eexec\n\
\xcd\xcd\xcd\xcd binary junk dup 200 /shouldbeignored put\n";

    #[test]
    fn reads_the_encoding_entries() {
        let encoding = parse_builtin_encoding(CM_HEADER);

        assert_eq!(encoding.get(&39).map(String::as_str), Some("phi1"));
        assert_eq!(encoding.get(&69).map(String::as_str), Some("E"));
        assert_eq!(encoding.get(&59).map(String::as_str), Some("comma"));
    }

    #[test]
    fn stops_at_the_encrypted_portion() {
        // The binary section can contain anything, including bytes that read as
        // an encoding entry; it is not PostScript and must not be scanned.
        let encoding = parse_builtin_encoding(CM_HEADER);

        assert!(!encoding.contains_key(&200));
    }

    #[test]
    fn ignores_the_notdef_initialiser() {
        // `{1 index exch /.notdef put}` is a loop body, not a `dup` entry.
        let encoding = parse_builtin_encoding(CM_HEADER);

        assert!(!encoding.values().any(|name| name == ".notdef"));
    }

    #[test]
    fn a_font_without_an_encoding_yields_nothing() {
        let program = b"%!PS-AdobeFont-1.0: Plain\n/FontName /Plain def\ncurrentfile eexec\n";

        assert!(parse_builtin_encoding(program).is_empty());
    }

    #[test]
    fn a_code_beyond_a_byte_is_skipped() {
        let program = b"/Encoding 256 array\ndup 300 /toobig put\ndup 7 /bell put\nreadonly def\n";
        let encoding = parse_builtin_encoding(program);

        assert_eq!(encoding.get(&7).map(String::as_str), Some("bell"));
        assert_eq!(encoding.len(), 1);
    }
}
