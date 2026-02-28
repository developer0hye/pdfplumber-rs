//! Content stream tokenizer for PDF operator/operand parsing.
//!
//! Parses raw PDF content stream bytes into a sequence of [`Operator`]s,
//! each carrying its [`Operand`] arguments. This is the foundation for
//! the content stream interpreter.

use crate::error::BackendError;

/// A PDF content stream operand value.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    /// Integer number (e.g., `42`, `-7`).
    Integer(i64),
    /// Real (floating-point) number (e.g., `3.14`, `.5`).
    Real(f64),
    /// Name object (e.g., `/F1`, `/DeviceRGB`). Stored without the leading `/`.
    Name(String),
    /// Literal string delimited by parentheses, stored as raw bytes.
    LiteralString(Vec<u8>),
    /// Hexadecimal string delimited by angle brackets, stored as decoded bytes.
    HexString(Vec<u8>),
    /// Array of operands (e.g., `[1 2 3]`).
    Array(Vec<Operand>),
    /// Boolean value (`true` or `false`).
    Boolean(bool),
    /// The null object.
    Null,
    /// Dictionary object (`<< /Key value ... >>`).
    Dictionary(Vec<(String, Operand)>),
}

/// A PDF content stream operator with its preceding operands.
#[derive(Debug, Clone, PartialEq)]
pub struct Operator {
    /// Operator name (e.g., `"BT"`, `"Tf"`, `"Tj"`, `"m"`).
    pub name: String,
    /// Operands that preceded this operator on the operand stack.
    pub operands: Vec<Operand>,
}

/// Inline image data captured from BI...ID...EI sequences.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineImageData {
    /// Dictionary entries between BI and ID as key-value pairs.
    pub dict: Vec<(String, Operand)>,
    /// Raw image data bytes between ID and EI.
    pub data: Vec<u8>,
}

/// Parse PDF content stream bytes into a sequence of operators.
///
/// Each operator collects the operands that preceded it on the operand stack.
/// Comments (% to end of line) are stripped. Inline images (BI/ID/EI) are
/// handled as a special case.
///
/// # Errors
///
/// Returns [`BackendError::Interpreter`] for malformed content streams.
pub fn tokenize(input: &[u8]) -> Result<Vec<Operator>, BackendError> {
    let mut ops = Vec::new();
    let mut operand_stack: Vec<Operand> = Vec::new();
    let mut pos = 0;

    while pos < input.len() {
        skip_whitespace_and_comments(input, &mut pos);
        if pos >= input.len() {
            break;
        }

        let b = input[pos];

        match b {
            // Literal string
            b'(' => {
                let s = parse_literal_string(input, &mut pos)?;
                operand_stack.push(Operand::LiteralString(s));
            }
            // Hex string
            b'<' => {
                if pos + 1 < input.len() && input[pos + 1] == b'<' {
                    // << is a dictionary start — used by BDC and other operators
                    let dict = parse_dictionary(input, &mut pos)?;
                    operand_stack.push(Operand::Dictionary(dict));
                } else {
                    let s = parse_hex_string(input, &mut pos)?;
                    operand_stack.push(Operand::HexString(s));
                }
            }
            // Array start
            b'[' => {
                pos += 1; // skip '['
                let arr = parse_array(input, &mut pos)?;
                operand_stack.push(Operand::Array(arr));
            }
            // Name
            b'/' => {
                let name = parse_name(input, &mut pos);
                operand_stack.push(Operand::Name(name));
            }
            // Number (digit, sign, or decimal point)
            b'0'..=b'9' | b'+' | b'-' | b'.' => {
                let num = parse_number(input, &mut pos)?;
                operand_stack.push(num);
            }
            // Keyword (operator, boolean, null)
            b'a'..=b'z' | b'A'..=b'Z' | b'*' | b'\'' | b'"' => {
                let keyword = parse_keyword(input, &mut pos);
                match keyword.as_str() {
                    "true" => operand_stack.push(Operand::Boolean(true)),
                    "false" => operand_stack.push(Operand::Boolean(false)),
                    "null" => operand_stack.push(Operand::Null),
                    "BI" => {
                        // Inline image: parse BI <dict> ID <data> EI
                        let (dict, data) = parse_inline_image(input, &mut pos)?;
                        ops.push(Operator {
                            name: "BI".to_string(),
                            operands: vec![
                                Operand::Array(
                                    dict.into_iter()
                                        .flat_map(|(k, v)| vec![Operand::Name(k), v])
                                        .collect(),
                                ),
                                Operand::LiteralString(data),
                            ],
                        });
                    }
                    _ => {
                        // It's an operator
                        ops.push(Operator {
                            name: keyword,
                            operands: std::mem::take(&mut operand_stack),
                        });
                    }
                }
            }
            // Array end — shouldn't appear at top level
            b']' => {
                return Err(BackendError::Interpreter(
                    "unexpected ']' outside array".to_string(),
                ));
            }
            _ => {
                // Skip unknown bytes
                pos += 1;
            }
        }
    }

    Ok(ops)
}

/// Returns `true` if `b` is a PDF whitespace character.
fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0x0C | 0x00)
}

/// Returns `true` if `b` is a PDF delimiter character.
fn is_delimiter(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

/// Skip whitespace and comments.
fn skip_whitespace_and_comments(input: &[u8], pos: &mut usize) {
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
fn parse_literal_string(input: &[u8], pos: &mut usize) -> Result<Vec<u8>, BackendError> {
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
fn parse_hex_string(input: &[u8], pos: &mut usize) -> Result<Vec<u8>, BackendError> {
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
fn hex_digit(b: u8) -> Result<u8, BackendError> {
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
fn parse_array(input: &[u8], pos: &mut usize) -> Result<Vec<Operand>, BackendError> {
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
fn parse_dictionary(input: &[u8], pos: &mut usize) -> Result<Vec<(String, Operand)>, BackendError> {
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
fn parse_dictionary_value(input: &[u8], pos: &mut usize) -> Result<Operand, BackendError> {
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
fn parse_name(input: &[u8], pos: &mut usize) -> String {
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
fn parse_number(input: &[u8], pos: &mut usize) -> Result<Operand, BackendError> {
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
fn parse_keyword(input: &[u8], pos: &mut usize) -> String {
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

/// Inline image dictionary entries: key-value pairs.
type InlineImageDict = Vec<(String, Operand)>;

/// Parse inline image data: `BI <dict entries> ID <data> EI`.
/// Called after `BI` keyword has been consumed.
fn parse_inline_image(
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
fn parse_inline_image_value(input: &[u8], pos: &mut usize) -> Result<Operand, BackendError> {
    let b = input[*pos];
    match b {
        b'/' => Ok(Operand::Name(parse_name(input, pos))),
        b'(' => Ok(Operand::LiteralString(parse_literal_string(input, pos)?)),
        b'<' => Ok(Operand::HexString(parse_hex_string(input, pos)?)),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Operand parsing tests ----

    #[test]
    fn parse_integer() {
        let ops = tokenize(b"42 m").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "m");
        assert_eq!(ops[0].operands, vec![Operand::Integer(42)]);
    }

    #[test]
    fn parse_negative_integer() {
        let ops = tokenize(b"-7 Td").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operands, vec![Operand::Integer(-7)]);
    }

    #[test]
    fn parse_real_number() {
        let ops = tokenize(b"3.14 w").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operands, vec![Operand::Real(3.14)]);
    }

    #[test]
    fn parse_real_leading_dot() {
        let ops = tokenize(b".5 w").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operands, vec![Operand::Real(0.5)]);
    }

    #[test]
    fn parse_negative_real() {
        let ops = tokenize(b"-.002 w").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operands, vec![Operand::Real(-0.002)]);
    }

    #[test]
    fn parse_name_operand() {
        let ops = tokenize(b"/F1 12 Tf").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Tf");
        assert_eq!(
            ops[0].operands,
            vec![Operand::Name("F1".to_string()), Operand::Integer(12)]
        );
    }

    #[test]
    fn parse_name_with_hex_escape() {
        let ops = tokenize(b"/F#231 12 Tf").unwrap();
        assert_eq!(ops[0].operands[0], Operand::Name("F#1".to_string()));
    }

    #[test]
    fn parse_literal_string_simple() {
        let ops = tokenize(b"(Hello) Tj").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Tj");
        assert_eq!(
            ops[0].operands,
            vec![Operand::LiteralString(b"Hello".to_vec())]
        );
    }

    #[test]
    fn parse_literal_string_escaped_chars() {
        let ops = tokenize(b"(line1\\nline2) Tj").unwrap();
        assert_eq!(
            ops[0].operands,
            vec![Operand::LiteralString(b"line1\nline2".to_vec())]
        );
    }

    #[test]
    fn parse_literal_string_balanced_parens() {
        let ops = tokenize(b"(a(b)c) Tj").unwrap();
        assert_eq!(
            ops[0].operands,
            vec![Operand::LiteralString(b"a(b)c".to_vec())]
        );
    }

    #[test]
    fn parse_literal_string_octal_escape() {
        // \101 = 'A' (65)
        let ops = tokenize(b"(\\101) Tj").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::LiteralString(vec![65])]);
    }

    #[test]
    fn parse_hex_string() {
        let ops = tokenize(b"<48656C6C6F> Tj").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operands, vec![Operand::HexString(b"Hello".to_vec())]);
    }

    #[test]
    fn parse_hex_string_odd_digits() {
        // Odd number of hex digits: trailing 0 appended → <ABC> = <ABC0>
        let ops = tokenize(b"<ABC> Tj").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::HexString(vec![0xAB, 0xC0])]);
    }

    #[test]
    fn parse_hex_string_with_whitespace() {
        let ops = tokenize(b"<48 65 6C 6C 6F> Tj").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::HexString(b"Hello".to_vec())]);
    }

    #[test]
    fn parse_array_operand() {
        let ops = tokenize(b"[1 2 3] re").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0].operands,
            vec![Operand::Array(vec![
                Operand::Integer(1),
                Operand::Integer(2),
                Operand::Integer(3),
            ])]
        );
    }

    #[test]
    fn parse_boolean_true() {
        let ops = tokenize(b"true m").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::Boolean(true)]);
    }

    #[test]
    fn parse_boolean_false() {
        let ops = tokenize(b"false m").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::Boolean(false)]);
    }

    #[test]
    fn parse_null_operand() {
        let ops = tokenize(b"null m").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::Null]);
    }

    // ---- Operator parsing tests ----

    #[test]
    fn parse_bt_et() {
        let ops = tokenize(b"BT ET").unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].name, "BT");
        assert!(ops[0].operands.is_empty());
        assert_eq!(ops[1].name, "ET");
        assert!(ops[1].operands.is_empty());
    }

    #[test]
    fn parse_tf_operator() {
        let ops = tokenize(b"/F1 12 Tf").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Tf");
        assert_eq!(
            ops[0].operands,
            vec![Operand::Name("F1".to_string()), Operand::Integer(12)]
        );
    }

    #[test]
    fn parse_td_operator() {
        let ops = tokenize(b"72 700 Td").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Td");
        assert_eq!(
            ops[0].operands,
            vec![Operand::Integer(72), Operand::Integer(700)]
        );
    }

    #[test]
    fn parse_tj_operator() {
        let ops = tokenize(b"(Hello World) Tj").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Tj");
        assert_eq!(
            ops[0].operands,
            vec![Operand::LiteralString(b"Hello World".to_vec())]
        );
    }

    #[test]
    fn parse_tj_array_with_kerning() {
        let ops = tokenize(b"[(H) -20 (ello)] TJ").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "TJ");
        assert_eq!(
            ops[0].operands,
            vec![Operand::Array(vec![
                Operand::LiteralString(b"H".to_vec()),
                Operand::Integer(-20),
                Operand::LiteralString(b"ello".to_vec()),
            ])]
        );
    }

    #[test]
    fn parse_path_operators() {
        let ops = tokenize(b"100 200 m 300 400 l S").unwrap();
        assert_eq!(ops.len(), 3);
        assert_eq!(ops[0].name, "m");
        assert_eq!(
            ops[0].operands,
            vec![Operand::Integer(100), Operand::Integer(200)]
        );
        assert_eq!(ops[1].name, "l");
        assert_eq!(
            ops[1].operands,
            vec![Operand::Integer(300), Operand::Integer(400)]
        );
        assert_eq!(ops[2].name, "S");
        assert!(ops[2].operands.is_empty());
    }

    #[test]
    fn parse_re_operator() {
        let ops = tokenize(b"10 20 100 50 re f").unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].name, "re");
        assert_eq!(
            ops[0].operands,
            vec![
                Operand::Integer(10),
                Operand::Integer(20),
                Operand::Integer(100),
                Operand::Integer(50),
            ]
        );
        assert_eq!(ops[1].name, "f");
    }

    #[test]
    fn parse_f_star_operator() {
        let ops = tokenize(b"f*").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "f*");
    }

    // ---- Comment handling ----

    #[test]
    fn skip_comments() {
        let ops = tokenize(b"% this is a comment\nBT ET").unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].name, "BT");
        assert_eq!(ops[1].name, "ET");
    }

    #[test]
    fn inline_comment_between_operators() {
        let ops = tokenize(b"BT % begin text\n/F1 12 Tf\nET").unwrap();
        assert_eq!(ops.len(), 3);
        assert_eq!(ops[0].name, "BT");
        assert_eq!(ops[1].name, "Tf");
        assert_eq!(ops[2].name, "ET");
    }

    // ---- Mixed content stream tests ----

    #[test]
    fn parse_typical_text_stream() {
        let stream = b"BT\n/F1 12 Tf\n72 700 Td\n(Hello World) Tj\nET";
        let ops = tokenize(stream).unwrap();
        assert_eq!(ops.len(), 5);
        assert_eq!(ops[0].name, "BT");
        assert_eq!(ops[1].name, "Tf");
        assert_eq!(ops[1].operands.len(), 2);
        assert_eq!(ops[2].name, "Td");
        assert_eq!(ops[2].operands.len(), 2);
        assert_eq!(ops[3].name, "Tj");
        assert_eq!(ops[3].operands.len(), 1);
        assert_eq!(ops[4].name, "ET");
    }

    #[test]
    fn parse_mixed_text_and_graphics() {
        let stream = b"q\n1 0 0 1 72 720 cm\nBT\n/F1 12 Tf\n(Test) Tj\nET\n100 200 300 400 re S\nQ";
        let ops = tokenize(stream).unwrap();
        assert_eq!(ops[0].name, "q");
        assert_eq!(ops[1].name, "cm");
        assert_eq!(ops[1].operands.len(), 6);
        assert_eq!(ops[2].name, "BT");
        assert_eq!(ops[3].name, "Tf");
        assert_eq!(ops[4].name, "Tj");
        assert_eq!(ops[5].name, "ET");
        assert_eq!(ops[6].name, "re");
        assert_eq!(ops[7].name, "S");
        assert_eq!(ops[8].name, "Q");
    }

    #[test]
    fn parse_color_operators() {
        let ops = tokenize(b"0.5 g\n1 0 0 RG").unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].name, "g");
        assert_eq!(ops[0].operands, vec![Operand::Real(0.5)]);
        assert_eq!(ops[1].name, "RG");
        assert_eq!(
            ops[1].operands,
            vec![
                Operand::Integer(1),
                Operand::Integer(0),
                Operand::Integer(0),
            ]
        );
    }

    #[test]
    fn parse_quote_operator() {
        let ops = tokenize(b"(text) '").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "'");
        assert_eq!(
            ops[0].operands,
            vec![Operand::LiteralString(b"text".to_vec())]
        );
    }

    #[test]
    fn parse_double_quote_operator() {
        let ops = tokenize(b"1 2 (text) \"").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "\"");
        assert_eq!(
            ops[0].operands,
            vec![
                Operand::Integer(1),
                Operand::Integer(2),
                Operand::LiteralString(b"text".to_vec()),
            ]
        );
    }

    #[test]
    fn parse_empty_stream() {
        let ops = tokenize(b"").unwrap();
        assert!(ops.is_empty());
    }

    #[test]
    fn parse_whitespace_only() {
        let ops = tokenize(b"   \t\n\r  ").unwrap();
        assert!(ops.is_empty());
    }

    #[test]
    fn parse_inline_image() {
        // Space after ID is the mandatory single whitespace separator (per PDF spec)
        let stream = b"BI\n/W 2 /H 2 /CS /G /BPC 8\nID \x00\xFF\x00\xFF\nEI";
        let ops = tokenize(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "BI");
        // First operand is array of key-value pairs flattened
        if let Operand::Array(ref entries) = ops[0].operands[0] {
            // W=2, H=2, CS=G, BPC=8 → 8 elements (4 pairs)
            assert_eq!(entries.len(), 8);
            assert_eq!(entries[0], Operand::Name("W".to_string()));
            assert_eq!(entries[1], Operand::Integer(2));
        } else {
            panic!("expected array operand for BI dict");
        }
        // Second operand is the raw data
        if let Operand::LiteralString(ref data) = ops[0].operands[1] {
            assert_eq!(data, &[0x00, 0xFF, 0x00, 0xFF]);
        } else {
            panic!("expected literal string operand for BI data");
        }
    }

    // ---- Edge cases ----

    #[test]
    fn parse_positive_sign_number() {
        let ops = tokenize(b"+5 m").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::Integer(5)]);
    }

    #[test]
    fn parse_zero() {
        let ops = tokenize(b"0 m").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::Integer(0)]);
    }

    #[test]
    fn parse_zero_real() {
        let ops = tokenize(b"0.0 m").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::Real(0.0)]);
    }

    #[test]
    fn parse_multiple_operators_no_operands() {
        let ops = tokenize(b"q Q n W").unwrap();
        assert_eq!(ops.len(), 4);
        assert_eq!(ops[0].name, "q");
        assert_eq!(ops[1].name, "Q");
        assert_eq!(ops[2].name, "n");
        assert_eq!(ops[3].name, "W");
    }

    #[test]
    fn parse_text_matrix() {
        let ops = tokenize(b"1 0 0 1 72 700 Tm").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Tm");
        assert_eq!(ops[0].operands.len(), 6);
    }

    #[test]
    fn unterminated_literal_string_error() {
        let result = tokenize(b"(unclosed");
        assert!(result.is_err());
    }

    #[test]
    fn unterminated_array_error() {
        let result = tokenize(b"[1 2 3");
        assert!(result.is_err());
    }

    #[test]
    fn unexpected_array_close_error() {
        let result = tokenize(b"]");
        assert!(result.is_err());
    }

    #[test]
    fn parse_do_operator() {
        let ops = tokenize(b"/Im0 Do").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Do");
        assert_eq!(ops[0].operands, vec![Operand::Name("Im0".to_string())]);
    }

    #[test]
    fn parse_scn_operator() {
        let ops = tokenize(b"0.5 0.2 0.8 scn").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "scn");
        assert_eq!(ops[0].operands.len(), 3);
    }

    #[test]
    fn parse_dash_pattern() {
        let ops = tokenize(b"[3 5] 6 d").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "d");
        assert_eq!(
            ops[0].operands,
            vec![
                Operand::Array(vec![Operand::Integer(3), Operand::Integer(5)]),
                Operand::Integer(6),
            ]
        );
    }

    #[test]
    fn parse_consecutive_strings() {
        let ops = tokenize(b"(abc) (def) Tj").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "Tj");
        assert_eq!(ops[0].operands.len(), 2);
    }

    // ---- Dictionary parsing tests ----

    #[test]
    fn parse_dictionary_operand() {
        let ops = tokenize(b"<< /Type /Foo >> pop").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "pop");
        assert_eq!(
            ops[0].operands,
            vec![Operand::Dictionary(vec![(
                "Type".to_string(),
                Operand::Name("Foo".to_string())
            )])]
        );
    }

    #[test]
    fn parse_bdc_with_inline_dict() {
        let ops = tokenize(b"/Tag << /MCID 0 >> BDC").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "BDC");
        assert_eq!(ops[0].operands.len(), 2);
        assert_eq!(ops[0].operands[0], Operand::Name("Tag".to_string()));
        assert_eq!(
            ops[0].operands[1],
            Operand::Dictionary(vec![(
                "MCID".to_string(),
                Operand::Integer(0)
            )])
        );
    }

    #[test]
    fn parse_nested_dictionary() {
        let ops = tokenize(b"<< /Outer << /Inner 42 >> >> pop").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0].operands,
            vec![Operand::Dictionary(vec![(
                "Outer".to_string(),
                Operand::Dictionary(vec![(
                    "Inner".to_string(),
                    Operand::Integer(42)
                )])
            )])]
        );
    }

    #[test]
    fn parse_bdc_with_dict_in_content_stream() {
        // Real-world pattern from SCOTUS PDFs
        let stream = b"/P << /MCID 0 >> BDC\nBT\n/F1 12 Tf\nET\nEMC";
        let ops = tokenize(stream).unwrap();
        assert_eq!(ops.len(), 5);
        assert_eq!(ops[0].name, "BDC");
        assert_eq!(ops[1].name, "BT");
        assert_eq!(ops[2].name, "Tf");
        assert_eq!(ops[3].name, "ET");
        assert_eq!(ops[4].name, "EMC");
    }
}
