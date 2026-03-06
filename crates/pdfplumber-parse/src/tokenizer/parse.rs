//! Private PDF token parsing primitives.
//!
//! All functions here are `pub(super)` — used only by the tokenizer.

use crate::error::BackendError;
use super::{InlineImageDict, Operand};

/// Returns `true` if `b` is a PDF whitespace character.
pub(super) fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0x0C | 0x00)
}

/// Returns `true` if `b` is a PDF delimiter character.
pub(super) fn is_delimiter(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

/// Skip whitespace and comments.
pub(super) fn skip_whitespace_and_comments(input: &[u8], pos: &mut usize) {
    while *pos < input.len() {
        if is_whitespace(input[*pos]) {
            *pos += 1;
        } else if input[*pos] == b'%' {
            // Comment — skip to end of line
            while *pos < input.len() && input[*pos] != b'\n' && input[*pos] != b'\r' {
                *pos += 1;
            }
        } else {
            break;
        }
    }
}

/// Parse a literal string `(...)` with balanced parentheses and escape sequences.
pub(super) fn parse_literal_string(input: &[u8], pos: &mut usize) -> Result<Vec<u8>, BackendError> {
    debug_assert_eq!(input[*pos], b'(');
    *pos += 1; // skip opening '('

    let mut result = Vec::new();
    let mut depth = 1u32;

    while *pos < input.len() {
        let b = input[*pos];
        match b {
            b'(' => {
                depth += 1;
                result.push(b'(');
                *pos += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    *pos += 1; // skip closing ')'
                    return Ok(result);
                }
                result.push(b')');
                *pos += 1;
            }
            b'\\' => {
                *pos += 1;
                if *pos >= input.len() {
                    return Err(BackendError::Interpreter(
                        "unterminated escape in literal string".to_string(),
                    ));
                }
                let escaped = input[*pos];
                match escaped {
                    b'n' => result.push(b'\n'),
                    b'r' => result.push(b'\r'),
                    b't' => result.push(b'\t'),
                    b'b' => result.push(0x08),
                    b'f' => result.push(0x0C),
                    b'(' => result.push(b'('),
                    b')' => result.push(b')'),
                    b'\\' => result.push(b'\\'),
                    b'\r' => {
                        // Backslash + CR (or CR+LF) = line continuation
                        *pos += 1;
                        if *pos < input.len() && input[*pos] == b'\n' {
                            *pos += 1;
                        }
                        continue;
                    }
                    b'\n' => {
                        // Backslash + LF = line continuation
                        *pos += 1;
                        continue;
                    }
                    b'0'..=b'7' => {
                        // Octal escape (1-3 digits)
                        let mut val = escaped - b'0';
                        for _ in 0..2 {
                            if *pos + 1 < input.len()
                                && input[*pos + 1] >= b'0'
                                && input[*pos + 1] <= b'7'
                            {
                                *pos += 1;
                                val = val * 8 + (input[*pos] - b'0');
                            } else {
                                break;
                            }
                        }
                        result.push(val);
                        *pos += 1;
                        continue;
                    }
                    _ => {
                        // Unknown escape — just include the character
                        result.push(escaped);
                    }
                }
                *pos += 1;
            }
            _ => {
                result.push(b);
                *pos += 1;
            }
        }
    }

    Err(BackendError::Interpreter(
        "unterminated literal string".to_string(),
    ))
}

/// Parse a hex string `<...>`.
pub(super) fn parse_hex_string(input: &[u8], pos: &mut usize) -> Result<Vec<u8>, BackendError> {
    debug_assert_eq!(input[*pos], b'<');
    *pos += 1; // skip '<'

    let mut hex_chars = Vec::new();
    while *pos < input.len() {
        let b = input[*pos];
        if b == b'>' {
            *pos += 1; // skip '>'
            break;
        }
        if is_whitespace(b) {
            *pos += 1;
            continue;
        }
        hex_chars.push(b);
        *pos += 1;
    }

    // If odd number of hex digits, append a trailing 0
    if hex_chars.len() % 2 != 0 {
        hex_chars.push(b'0');
    }

    let mut result = Vec::with_capacity(hex_chars.len() / 2);
    for chunk in hex_chars.chunks(2) {
        let hi = hex_digit(chunk[0])?;
        let lo = hex_digit(chunk[1])?;
        result.push((hi << 4) | lo);
    }

    Ok(result)
}

/// Convert a hex digit character to its value (0-15).
pub(super) fn hex_digit(b: u8) -> Result<u8, BackendError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(BackendError::Interpreter(format!(
            "invalid hex digit: {:?}",
            b as char
        ))),
    }
}

/// Parse an array until `]`. Assumes `[` already consumed.
pub(super) fn parse_array(input: &[u8], pos: &mut usize) -> Result<Vec<Operand>, BackendError> {
    let mut elements = Vec::new();

    loop {
        skip_whitespace_and_comments(input, pos);
        if *pos >= input.len() {
            return Err(BackendError::Interpreter("unterminated array".to_string()));
        }

        if input[*pos] == b']' {
            *pos += 1; // skip ']'
            return Ok(elements);
        }

        let b = input[*pos];
        match b {
            b'(' => {
                let s = parse_literal_string(input, pos)?;
                elements.push(Operand::LiteralString(s));
            }
            b'<' => {
                let s = parse_hex_string(input, pos)?;
                elements.push(Operand::HexString(s));
            }
            b'[' => {
                *pos += 1;
                let arr = parse_array(input, pos)?;
                elements.push(Operand::Array(arr));
            }
            b'/' => {
                let name = parse_name(input, pos);
                elements.push(Operand::Name(name));
            }
            b'0'..=b'9' | b'+' | b'-' | b'.' => {
                let num = parse_number(input, pos)?;
                elements.push(num);
            }
            b'a'..=b'z' | b'A'..=b'Z' => {
                let keyword = parse_keyword(input, pos);
                match keyword.as_str() {
                    "true" => elements.push(Operand::Boolean(true)),
                    "false" => elements.push(Operand::Boolean(false)),
                    "null" => elements.push(Operand::Null),
                    _ => {
                        // In TJ arrays, operators don't appear — treat as name-like
                        elements.push(Operand::Name(keyword));
                    }
                }
            }
            _ => {
                return Err(BackendError::Interpreter(format!(
                    "unexpected byte in array: 0x{:02X}",
                    b
                )));
            }
        }
    }
}

/// Parse a dictionary `<< /Key value ... >>`. Assumes current bytes are `<<`.
pub(super) fn parse_dictionary(input: &[u8], pos: &mut usize) -> Result<Vec<(String, Operand)>, BackendError> {
    // Skip '<<'
    *pos += 2;

    let mut entries = Vec::new();

    loop {
        skip_whitespace_and_comments(input, pos);
        if *pos >= input.len() {
            return Err(BackendError::Interpreter(
                "unterminated dictionary".to_string(),
            ));
        }

        // Check for '>>'
        if *pos + 1 < input.len() && input[*pos] == b'>' && input[*pos + 1] == b'>' {
            *pos += 2; // skip '>>'
            return Ok(entries);
        }

        // Parse key (must be a name)
        if input[*pos] != b'/' {
            return Err(BackendError::Interpreter(
                "expected name key in dictionary".to_string(),
            ));
        }
        let key = parse_name(input, pos);

        // Parse value
        skip_whitespace_and_comments(input, pos);
        if *pos >= input.len() {
            return Err(BackendError::Interpreter(
                "unterminated dictionary value".to_string(),
            ));
        }

        let value = parse_dictionary_value(input, pos)?;
        entries.push((key, value));
    }
}

/// Parse a single value inside a dictionary.
pub(super) fn parse_dictionary_value(input: &[u8], pos: &mut usize) -> Result<Operand, BackendError> {
    let b = input[*pos];
    match b {
        b'/' => Ok(Operand::Name(parse_name(input, pos))),
        b'(' => Ok(Operand::LiteralString(parse_literal_string(input, pos)?)),
        b'<' => {
            if *pos + 1 < input.len() && input[*pos + 1] == b'<' {
                // Nested dictionary
                Ok(Operand::Dictionary(parse_dictionary(input, pos)?))
            } else {
                Ok(Operand::HexString(parse_hex_string(input, pos)?))
            }
        }
        b'[' => {
            *pos += 1;
            Ok(Operand::Array(parse_array(input, pos)?))
        }
        b'0'..=b'9' | b'+' | b'-' | b'.' => parse_number(input, pos),
        b'a'..=b'z' | b'A'..=b'Z' => {
            let kw = parse_keyword(input, pos);
            match kw.as_str() {
                "true" => Ok(Operand::Boolean(true)),
                "false" => Ok(Operand::Boolean(false)),
                "null" => Ok(Operand::Null),
                _ => Ok(Operand::Name(kw)),
            }
        }
        _ => Err(BackendError::Interpreter(format!(
            "unexpected byte in dictionary value: 0x{:02X}",
            b
        ))),
    }
}

/// Parse a `/Name` token. Assumes current byte is `/`.
pub(super) fn parse_name(input: &[u8], pos: &mut usize) -> String {
    debug_assert_eq!(input[*pos], b'/');
    *pos += 1; // skip '/'

    let start = *pos;
    while *pos < input.len() && !is_whitespace(input[*pos]) && !is_delimiter(input[*pos]) {
        *pos += 1;
    }

    // Handle #XX hex escapes in names
    let raw = &input[start..*pos];
    let mut name = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'#' && i + 2 < raw.len() {
            if let (Ok(hi), Ok(lo)) = (hex_digit(raw[i + 1]), hex_digit(raw[i + 2])) {
                name.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        name.push(raw[i]);
        i += 1;
    }

    String::from_utf8_lossy(&name).into_owned()
}

/// Parse a number (integer or real).
pub(super) fn parse_number(input: &[u8], pos: &mut usize) -> Result<Operand, BackendError> {
    let start = *pos;
    let mut has_dot = false;

    // Sign
    if *pos < input.len() && (input[*pos] == b'+' || input[*pos] == b'-') {
        *pos += 1;
    }

    // Digits and decimal point
    while *pos < input.len() {
        let b = input[*pos];
        if b == b'.' {
            if has_dot {
                break; // second dot — stop
            }
            has_dot = true;
            *pos += 1;
        } else if b.is_ascii_digit() {
            *pos += 1;
        } else {
            break;
        }
    }

    let token = &input[start..*pos];
    let s = std::str::from_utf8(token)
        .map_err(|_| BackendError::Interpreter("invalid UTF-8 in number token".to_string()))?;

    if has_dot {
        let val: f64 = s
            .parse()
            .map_err(|_| BackendError::Interpreter(format!("invalid real number: {s}")))?;
        Ok(Operand::Real(val))
    } else {
        let val: i64 = s
            .parse()
            .map_err(|_| BackendError::Interpreter(format!("invalid integer: {s}")))?;
        Ok(Operand::Integer(val))
    }
}

/// Parse a keyword (alphabetic + `*` + `'` + `"`).
pub(super) fn parse_keyword(input: &[u8], pos: &mut usize) -> String {
    let start = *pos;
    while *pos < input.len() {
        let b = input[*pos];
        if b.is_ascii_alphabetic() || b == b'*' || b == b'\'' || b == b'"' {
            *pos += 1;
        } else {
            break;
        }
    }
    String::from_utf8_lossy(&input[start..*pos]).into_owned()
}

/// Skip a `<< ... >>` dictionary block, handling nesting.
pub(super) fn skip_dict(input: &[u8], pos: &mut usize) {
    debug_assert!(input[*pos] == b'<' && *pos + 1 < input.len() && input[*pos + 1] == b'<');
    *pos += 2; // skip '<<'
    let mut depth = 1u32;

    while *pos < input.len() && depth > 0 {
        if *pos + 1 < input.len() && input[*pos] == b'<' && input[*pos + 1] == b'<' {
            depth += 1;
            *pos += 2;
        } else if *pos + 1 < input.len() && input[*pos] == b'>' && input[*pos + 1] == b'>' {
            depth -= 1;
            *pos += 2;
        } else {
            *pos += 1;
        }
    }
}

/// Parse inline image data: `BI <dict entries> ID <data> EI`.
/// Called after `BI` keyword has been consumed.
pub(super) fn parse_inline_image(
    input: &[u8],
    pos: &mut usize,
) -> Result<(InlineImageDict, Vec<u8>), BackendError> {
    // Parse dictionary entries until ID keyword
    let mut dict = Vec::new();

    loop {
        skip_whitespace_and_comments(input, pos);
        if *pos >= input.len() {
            return Err(BackendError::Interpreter(
                "unterminated inline image (missing ID)".to_string(),
            ));
        }

        // Check for ID keyword
        if *pos + 1 < input.len()
            && input[*pos] == b'I'
            && input[*pos + 1] == b'D'
            && (*pos + 2 >= input.len() || is_whitespace(input[*pos + 2]))
        {
            *pos += 2; // skip "ID"
            // Skip single whitespace byte after ID
            if *pos < input.len() && is_whitespace(input[*pos]) {
                *pos += 1;
            }
            break;
        }

        // Parse key (name)
        if input[*pos] != b'/' {
            return Err(BackendError::Interpreter(
                "expected name key in inline image dictionary".to_string(),
            ));
        }
        let key = parse_name(input, pos);

        // Parse value
        skip_whitespace_and_comments(input, pos);
        if *pos >= input.len() {
            return Err(BackendError::Interpreter(
                "unterminated inline image dictionary".to_string(),
            ));
        }

        let value = parse_inline_image_value(input, pos)?;
        dict.push((key, value));
    }

    // Read image data until EI
    let data_start = *pos;
    // Look for EI preceded by whitespace
    while *pos < input.len() {
        if *pos + 2 <= input.len()
            && (*pos == data_start || is_whitespace(input[*pos - 1]))
            && input[*pos] == b'E'
            && input[*pos + 1] == b'I'
            && (*pos + 2 >= input.len()
                || is_whitespace(input[*pos + 2])
                || is_delimiter(input[*pos + 2]))
        {
            let data = input[data_start..*pos].to_vec();
            // Trim trailing whitespace from data
            let data = if data.last().is_some_and(|&b| is_whitespace(b)) {
                data[..data.len() - 1].to_vec()
            } else {
                data
            };
            *pos += 2; // skip "EI"
            return Ok((dict, data));
        }
        *pos += 1;
    }

    Err(BackendError::Interpreter(
        "unterminated inline image (missing EI)".to_string(),
    ))
}

/// Parse a single value in an inline image dictionary.
pub(super) fn parse_inline_image_value(input: &[u8], pos: &mut usize) -> Result<Operand, BackendError> {
    let b = input[*pos];
    match b {
        b'/' => Ok(Operand::Name(parse_name(input, pos))),
        b'(' => Ok(Operand::LiteralString(parse_literal_string(input, pos)?)),
        b'<' => {
            if *pos + 1 < input.len() && input[*pos + 1] == b'<' {
                // Dictionary value — skip and return as Null
                skip_dict(input, pos);
                Ok(Operand::Null)
            } else {
                Ok(Operand::HexString(parse_hex_string(input, pos)?))
            }
        }
        b'[' => {
            *pos += 1;
            Ok(Operand::Array(parse_array(input, pos)?))
        }
        b'0'..=b'9' | b'+' | b'-' | b'.' => parse_number(input, pos),
        b'a'..=b'z' | b'A'..=b'Z' => {
            let kw = parse_keyword(input, pos);
            match kw.as_str() {
                "true" => Ok(Operand::Boolean(true)),
                "false" => Ok(Operand::Boolean(false)),
                "null" => Ok(Operand::Null),
                _ => Ok(Operand::Name(kw)),
            }
        }
        _ => Err(BackendError::Interpreter(format!(
            "unexpected byte in inline image value: 0x{:02X}",
            b
        ))),
    }
}
