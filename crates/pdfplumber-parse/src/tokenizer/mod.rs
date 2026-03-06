//! Content stream tokenizer for PDF operator/operand parsing.
//!
//! Parses raw PDF content stream bytes into a sequence of [`Operator`]s,
//! each carrying its [`Operand`] arguments. This is the foundation for
//! the content stream interpreter.
//!
//! # Module layout
//!
//! | Module | Contents |
//! |--------|----------|
//! | (this) | Public types and `tokenize` / `tokenize_lenient` |
//! | [`parse`] | Private byte-level parsing primitives |

use crate::error::BackendError;

mod parse;

use parse::{
    parse_array, parse_dictionary, parse_hex_string, parse_inline_image,
    parse_keyword, parse_literal_string, parse_name, parse_number, skip_whitespace_and_comments,
};

// Type alias used by parse.rs helpers — must be visible to that submodule.
pub(crate) type InlineImageDict = Vec<(String, Operand)>;

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
/// Comments (`%` to end of line) are stripped. Inline images (`BI`/`ID`/`EI`)
/// are handled as a special case, emitted as a single `"BI"` operator with two
/// operands: a flattened key-value array and a `LiteralString` of the raw data.
///
/// # Errors
///
/// Returns [`BackendError::Interpreter`] for malformed content streams:
/// unterminated strings, invalid hex digits, mismatched brackets, etc.
/// Use [`tokenize_lenient`] when graceful recovery is preferred over hard failure.
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
            b'(' => {
                let s = parse_literal_string(input, &mut pos)?;
                operand_stack.push(Operand::LiteralString(s));
            }
            b'<' => {
                if pos + 1 < input.len() && input[pos + 1] == b'<' {
                    let dict = parse_dictionary(input, &mut pos)?;
                    operand_stack.push(Operand::Dictionary(dict));
                } else {
                    let s = parse_hex_string(input, &mut pos)?;
                    operand_stack.push(Operand::HexString(s));
                }
            }
            b'[' => {
                pos += 1;
                let arr = parse_array(input, &mut pos)?;
                operand_stack.push(Operand::Array(arr));
            }
            b'/' => {
                let name = parse_name(input, &mut pos);
                operand_stack.push(Operand::Name(name));
            }
            b'0'..=b'9' | b'+' | b'-' | b'.' => {
                let num = parse_number(input, &mut pos)?;
                operand_stack.push(num);
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'*' | b'\'' | b'"' => {
                let keyword = parse_keyword(input, &mut pos);
                match keyword.as_str() {
                    "true" => operand_stack.push(Operand::Boolean(true)),
                    "false" => operand_stack.push(Operand::Boolean(false)),
                    "null" => operand_stack.push(Operand::Null),
                    "BI" => {
                        let (dict, data) = parse_inline_image(input, &mut pos)?;
                        ops.push(Operator {
                            name: "BI".to_string(),
                            operands: vec![
                                Operand::Array(
                                    dict.into_iter()
                                        .flat_map(|(k, v)| [Operand::Name(k), v])
                                        .collect(),
                                ),
                                Operand::LiteralString(data),
                            ],
                        });
                    }
                    _ => ops.push(Operator {
                        name: keyword,
                        operands: std::mem::take(&mut operand_stack),
                    }),
                }
            }
            b']' => {
                return Err(BackendError::Interpreter(
                    "unexpected ']' outside array".to_string(),
                ));
            }
            _ => {
                pos += 1;
            }
        }
    }

    Ok(ops)
}

/// Leniently parse PDF content stream bytes, recovering from malformed tokens.
///
/// Unlike [`tokenize`], this function does not abort on parse errors. When a
/// malformed construct is encountered (unterminated string, invalid hex digit,
/// unexpected delimiter, etc.), the parser skips forward to the next
/// recognizable token and continues. Successfully parsed operators before and
/// after the error are preserved.
///
/// # Returns
///
/// A tuple of `(operators, warnings)`. Each warning is a human-readable
/// description of a skipped malformed token with its byte offset.
pub fn tokenize_lenient(input: &[u8]) -> (Vec<Operator>, Vec<String>) {
    let mut ops = Vec::new();
    let mut operand_stack: Vec<Operand> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut pos = 0;

    while pos < input.len() {
        skip_whitespace_and_comments(input, &mut pos);
        if pos >= input.len() {
            break;
        }

        let saved_pos = pos;
        let b = input[pos];

        let result: Result<(), BackendError> = (|| {
            match b {
                b'(' => {
                    let s = parse_literal_string(input, &mut pos)?;
                    operand_stack.push(Operand::LiteralString(s));
                }
                b'<' => {
                    if pos + 1 < input.len() && input[pos + 1] == b'<' {
                        let dict = parse_dictionary(input, &mut pos)?;
                        operand_stack.push(Operand::Dictionary(dict));
                    } else {
                        let s = parse_hex_string(input, &mut pos)?;
                        operand_stack.push(Operand::HexString(s));
                    }
                }
                b'[' => {
                    pos += 1;
                    let arr = parse_array(input, &mut pos)?;
                    operand_stack.push(Operand::Array(arr));
                }
                b'/' => {
                    let name = parse_name(input, &mut pos);
                    operand_stack.push(Operand::Name(name));
                }
                b'0'..=b'9' | b'+' | b'-' | b'.' => {
                    let num = parse_number(input, &mut pos)?;
                    operand_stack.push(num);
                }
                b'a'..=b'z' | b'A'..=b'Z' | b'*' | b'\'' | b'"' => {
                    let keyword = parse_keyword(input, &mut pos);
                    match keyword.as_str() {
                        "true" => operand_stack.push(Operand::Boolean(true)),
                        "false" => operand_stack.push(Operand::Boolean(false)),
                        "null" => operand_stack.push(Operand::Null),
                        "BI" => {
                            let (dict, data) = parse_inline_image(input, &mut pos)?;
                            ops.push(Operator {
                                name: "BI".to_string(),
                                operands: vec![
                                    Operand::Array(
                                        dict.into_iter()
                                            .flat_map(|(k, v)| [Operand::Name(k), v])
                                            .collect(),
                                    ),
                                    Operand::LiteralString(data),
                                ],
                            });
                        }
                        _ => ops.push(Operator {
                            name: keyword,
                            operands: std::mem::take(&mut operand_stack),
                        }),
                    }
                }
                b']' => {
                    return Err(BackendError::Interpreter(
                        "unexpected ']' outside array".to_string(),
                    ));
                }
                _ => {
                    pos += 1;
                }
            }
            Ok(())
        })();

        if let Err(e) = result {
            warnings.push(format!(
                "skipped malformed token at byte offset {saved_pos}: {e}",
            ));
            // Reset to one past the error start so subsequent bytes are re-examined.
            pos = saved_pos + 1;
            operand_stack.clear();
        }
    }

    (ops, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(ops[0].operands, vec![Operand::Integer(-7)]);
    }

    #[test]
    fn parse_real_number() {
        let ops = tokenize(b"3.14 w").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::Real(3.14)]);
    }

    #[test]
    fn parse_real_leading_dot() {
        let ops = tokenize(b".5 w").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::Real(0.5)]);
    }

    #[test]
    fn parse_negative_real() {
        let ops = tokenize(b"-.002 w").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::Real(-0.002)]);
    }

    #[test]
    fn parse_name_operand() {
        let ops = tokenize(b"/F1 12 Tf").unwrap();
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
        let ops = tokenize(b"(\\101) Tj").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::LiteralString(vec![65])]);
    }

    #[test]
    fn parse_hex_string() {
        let ops = tokenize(b"<48656C6C6F> Tj").unwrap();
        assert_eq!(ops[0].operands, vec![Operand::HexString(b"Hello".to_vec())]);
    }

    #[test]
    fn parse_hex_string_odd_digits() {
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

    #[test]
    fn parse_bt_et() {
        let ops = tokenize(b"BT ET").unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].name, "BT");
        assert_eq!(ops[1].name, "ET");
    }

    #[test]
    fn parse_tf_operator() {
        let ops = tokenize(b"/F1 12 Tf").unwrap();
        assert_eq!(ops[0].name, "Tf");
        assert_eq!(
            ops[0].operands,
            vec![Operand::Name("F1".to_string()), Operand::Integer(12)]
        );
    }

    #[test]
    fn parse_td_operator() {
        let ops = tokenize(b"72 700 Td").unwrap();
        assert_eq!(ops[0].name, "Td");
        assert_eq!(
            ops[0].operands,
            vec![Operand::Integer(72), Operand::Integer(700)]
        );
    }

    #[test]
    fn parse_tj_operator() {
        let ops = tokenize(b"(Hello World) Tj").unwrap();
        assert_eq!(ops[0].name, "Tj");
        assert_eq!(
            ops[0].operands,
            vec![Operand::LiteralString(b"Hello World".to_vec())]
        );
    }

    #[test]
    fn parse_tj_array_with_kerning() {
        let ops = tokenize(b"[(H) -20 (ello)] TJ").unwrap();
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
        assert_eq!(ops[1].name, "l");
        assert_eq!(ops[2].name, "S");
    }

    #[test]
    fn parse_re_operator() {
        let ops = tokenize(b"10 20 100 50 re f").unwrap();
        assert_eq!(ops[0].name, "re");
        assert_eq!(ops[0].operands.len(), 4);
        assert_eq!(ops[1].name, "f");
    }

    #[test]
    fn parse_f_star_operator() {
        let ops = tokenize(b"f*").unwrap();
        assert_eq!(ops[0].name, "f*");
    }

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
    }

    #[test]
    fn parse_typical_text_stream() {
        let stream = b"BT\n/F1 12 Tf\n72 700 Td\n(Hello World) Tj\nET";
        let ops = tokenize(stream).unwrap();
        assert_eq!(ops.len(), 5);
        assert_eq!(ops[0].name, "BT");
        assert_eq!(ops[4].name, "ET");
    }

    #[test]
    fn parse_mixed_text_and_graphics() {
        let stream = b"q\n1 0 0 1 72 720 cm\nBT\n/F1 12 Tf\n(Test) Tj\nET\n100 200 300 400 re S\nQ";
        let ops = tokenize(stream).unwrap();
        assert_eq!(ops[0].name, "q");
        assert_eq!(ops[1].name, "cm");
        assert_eq!(ops[1].operands.len(), 6);
        assert_eq!(ops[8].name, "Q");
    }

    #[test]
    fn parse_color_operators() {
        let ops = tokenize(b"0.5 g\n1 0 0 RG").unwrap();
        assert_eq!(ops[0].name, "g");
        assert_eq!(ops[1].name, "RG");
    }

    #[test]
    fn parse_quote_operator() {
        let ops = tokenize(b"(text) '").unwrap();
        assert_eq!(ops[0].name, "'");
    }

    #[test]
    fn parse_double_quote_operator() {
        let ops = tokenize(b"1 2 (text) \"").unwrap();
        assert_eq!(ops[0].name, "\"");
        assert_eq!(ops[0].operands.len(), 3);
    }

    #[test]
    fn parse_empty_stream() {
        assert!(tokenize(b"").unwrap().is_empty());
    }

    #[test]
    fn parse_whitespace_only() {
        assert!(tokenize(b"   \t\n\r  ").unwrap().is_empty());
    }

    #[test]
    fn parse_inline_image() {
        let stream = b"BI\n/W 2 /H 2 /CS /G /BPC 8\nID \x00\xFF\x00\xFF\nEI";
        let ops = tokenize(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "BI");
        let Operand::Array(ref entries) = ops[0].operands[0] else {
            panic!("expected array operand for BI dict");
        };
        assert_eq!(entries.len(), 8);
        assert_eq!(entries[0], Operand::Name("W".to_string()));
        let Operand::LiteralString(ref data) = ops[0].operands[1] else {
            panic!("expected literal string operand for BI data");
        };
        assert_eq!(data, &[0x00, 0xFF, 0x00, 0xFF]);
    }

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
    }

    #[test]
    fn parse_text_matrix() {
        let ops = tokenize(b"1 0 0 1 72 700 Tm").unwrap();
        assert_eq!(ops[0].name, "Tm");
        assert_eq!(ops[0].operands.len(), 6);
    }

    #[test]
    fn unterminated_literal_string_error() {
        assert!(tokenize(b"(unclosed").is_err());
    }

    #[test]
    fn unterminated_array_error() {
        assert!(tokenize(b"[1 2 3").is_err());
    }

    #[test]
    fn unexpected_array_close_error() {
        assert!(tokenize(b"]").is_err());
    }

    #[test]
    fn parse_do_operator() {
        let ops = tokenize(b"/Im0 Do").unwrap();
        assert_eq!(ops[0].name, "Do");
        assert_eq!(ops[0].operands, vec![Operand::Name("Im0".to_string())]);
    }

    #[test]
    fn parse_scn_operator() {
        let ops = tokenize(b"0.5 0.2 0.8 scn").unwrap();
        assert_eq!(ops[0].name, "scn");
        assert_eq!(ops[0].operands.len(), 3);
    }

    #[test]
    fn parse_dash_pattern() {
        let ops = tokenize(b"[3 5] 6 d").unwrap();
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
        assert_eq!(ops[0].operands.len(), 2);
    }

    #[test]
    fn parse_dictionary_operand() {
        let ops = tokenize(b"<< /Type /Foo >> pop").unwrap();
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
        assert_eq!(ops[0].name, "BDC");
        assert_eq!(ops[0].operands.len(), 2);
        assert_eq!(ops[0].operands[0], Operand::Name("Tag".to_string()));
        assert_eq!(
            ops[0].operands[1],
            Operand::Dictionary(vec![("MCID".to_string(), Operand::Integer(0))])
        );
    }

    #[test]
    fn parse_nested_dictionary() {
        let ops = tokenize(b"<< /Outer << /Inner 42 >> >> pop").unwrap();
        assert_eq!(
            ops[0].operands,
            vec![Operand::Dictionary(vec![(
                "Outer".to_string(),
                Operand::Dictionary(vec![("Inner".to_string(), Operand::Integer(42))])
            )])]
        );
    }

    #[test]
    fn parse_bdc_with_dict_in_content_stream() {
        let stream = b"/P << /MCID 0 >> BDC\nBT\n/F1 12 Tf\nET\nEMC";
        let ops = tokenize(stream).unwrap();
        assert_eq!(ops.len(), 5);
        assert_eq!(ops[0].name, "BDC");
        assert_eq!(ops[4].name, "EMC");
    }

    #[test]
    fn inline_image_with_dict_value() {
        let stream = b"BI\n/W 2 /H 2 /CS /G /BPC 8 /DP << /Columns 2 >>\nID \x00\xFF\x00\xFF\nEI";
        let ops = tokenize(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].name, "BI");
    }

    #[test]
    fn lenient_wellformed_stream_matches_strict() {
        let stream = b"BT /F1 12 Tf (Hello) Tj ET";
        let strict = tokenize(stream).unwrap();
        let (lenient, warnings) = tokenize_lenient(stream);
        assert_eq!(lenient.len(), strict.len());
        assert!(warnings.is_empty());
    }

    #[test]
    fn lenient_unexpected_close_bracket_recovers() {
        assert!(tokenize(b"BT ] ET").is_err());
        let (ops, warnings) = tokenize_lenient(b"BT ] ET");
        let names: Vec<&str> = ops.iter().map(|o| o.name.as_str()).collect();
        assert!(names.contains(&"BT") && names.contains(&"ET"));
        assert!(!warnings.is_empty());
    }

    #[test]
    fn lenient_unterminated_string_recovers() {
        assert!(tokenize(b"BT (unclosed").is_err());
        let (ops, warnings) = tokenize_lenient(b"BT (unclosed");
        let names: Vec<&str> = ops.iter().map(|o| o.name.as_str()).collect();
        assert!(names.contains(&"BT"));
        assert!(!warnings.is_empty());
    }

    #[test]
    fn lenient_unterminated_string_followed_by_valid_ops() {
        let stream = b"BT (unterminated ET 100 200 Td (Hello) Tj ET";
        let (ops, warnings) = tokenize_lenient(stream);
        let names: Vec<&str> = ops.iter().map(|o| o.name.as_str()).collect();
        assert!(names.contains(&"BT"));
        assert!(names.contains(&"Tj") || names.contains(&"ET") || names.contains(&"Td"));
        assert!(!warnings.is_empty());
    }

    #[test]
    fn lenient_unterminated_array_recovers() {
        assert!(tokenize(b"BT [1 2 3 ET").is_err());
        let (ops, warnings) = tokenize_lenient(b"BT [1 2 3 ET");
        let names: Vec<&str> = ops.iter().map(|o| o.name.as_str()).collect();
        assert!(names.contains(&"BT"));
        assert!(!warnings.is_empty());
    }

    #[test]
    fn lenient_invalid_hex_digit_recovers() {
        assert!(tokenize(b"BT <ZZZZ> Tj ET").is_err());
        let (ops, warnings) = tokenize_lenient(b"BT <ZZZZ> Tj ET");
        let names: Vec<&str> = ops.iter().map(|o| o.name.as_str()).collect();
        assert!(names.contains(&"BT") && names.contains(&"ET"));
        assert!(!warnings.is_empty());
    }

    #[test]
    fn lenient_unterminated_dictionary_recovers() {
        assert!(tokenize(b"BT << /Key 42 ET").is_err());
        let (ops, warnings) = tokenize_lenient(b"BT << /Key 42 ET");
        let names: Vec<&str> = ops.iter().map(|o| o.name.as_str()).collect();
        assert!(names.contains(&"BT"));
        assert!(!warnings.is_empty());
    }

    #[test]
    fn lenient_operators_before_and_after_error_preserved() {
        let stream = b"q 1 0 0 1 0 0 cm BT ] /F1 12 Tf (Hello) Tj ET Q";
        let (ops, warnings) = tokenize_lenient(stream);
        let names: Vec<&str> = ops.iter().map(|o| o.name.as_str()).collect();
        assert!(names.contains(&"q") && names.contains(&"cm") && names.contains(&"BT"));
        assert!(names.contains(&"Tf") && names.contains(&"Tj") && names.contains(&"Q"));
        assert!(!warnings.is_empty());
    }

    #[test]
    fn lenient_empty_stream_no_warnings() {
        let (ops, warnings) = tokenize_lenient(b"");
        assert!(ops.is_empty() && warnings.is_empty());
    }

    #[test]
    fn lenient_multiple_errors_all_recovered() {
        let stream = b"BT ] /F1 12 Tf <ZZ> Tj ET";
        let (ops, warnings) = tokenize_lenient(stream);
        let names: Vec<&str> = ops.iter().map(|o| o.name.as_str()).collect();
        assert!(names.contains(&"BT") && names.contains(&"Tf") && names.contains(&"ET"));
        assert!(warnings.len() >= 2, "expected ≥2 warnings, got {}", warnings.len());
    }
}
