#[cfg(test)]
mod tests {
    use super::*;
    use crate::unicode_norm::UnicodeNorm;

    // --- PdfError tests ---

    #[test]
    fn pdf_error_parse_error_creation() {
        let err = PdfError::ParseError("invalid xref".to_string());
        assert_eq!(err.to_string(), "parse error: invalid xref");
    }

    #[test]
    fn pdf_error_io_error_creation() {
        let err = PdfError::IoError("file not found".to_string());
        assert_eq!(err.to_string(), "I/O error: file not found");
    }

    #[test]
    fn pdf_error_font_error_creation() {
        let err = PdfError::FontError("missing glyph widths".to_string());
        assert_eq!(err.to_string(), "font error: missing glyph widths");
    }

    #[test]
    fn pdf_error_interpreter_error_creation() {
        let err = PdfError::InterpreterError("unknown operator".to_string());
        assert_eq!(err.to_string(), "interpreter error: unknown operator");
    }

    #[test]
    fn pdf_error_resource_limit_exceeded() {
        let err = PdfError::ResourceLimitExceeded {
            limit_name: "max_input_bytes".to_string(),
            limit_value: 1024,
            actual_value: 2048,
        };
        assert_eq!(
            err.to_string(),
            "resource limit exceeded: max_input_bytes (limit: 1024, actual: 2048)"
        );
    }

    #[test]
    fn pdf_error_resource_limit_exceeded_structured_fields() {
        let err = PdfError::ResourceLimitExceeded {
            limit_name: "max_pages".to_string(),
            limit_value: 10,
            actual_value: 25,
        };
        if let PdfError::ResourceLimitExceeded {
            limit_name,
            limit_value,
            actual_value,
        } = &err
        {
            assert_eq!(limit_name, "max_pages");
            assert_eq!(*limit_value, 10);
            assert_eq!(*actual_value, 25);
        } else {
            panic!("expected ResourceLimitExceeded");
        }
    }

    #[test]
    fn pdf_error_password_required() {
        let err = PdfError::PasswordRequired;
        assert_eq!(err.to_string(), "PDF is encrypted and requires a password");
    }

    #[test]
    fn pdf_error_invalid_password() {
        let err = PdfError::InvalidPassword;
        assert_eq!(err.to_string(), "the supplied password is incorrect");
    }

    #[test]
    fn pdf_error_password_required_clone_and_eq() {
        let err1 = PdfError::PasswordRequired;
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn pdf_error_invalid_password_clone_and_eq() {
        let err1 = PdfError::InvalidPassword;
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn pdf_error_other() {
        let err = PdfError::Other("something went wrong".to_string());
        assert_eq!(err.to_string(), "something went wrong");
    }

    #[test]
    fn pdf_error_implements_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(PdfError::ParseError("test".to_string()));
        assert_eq!(err.to_string(), "parse error: test");
    }

    #[test]
    fn pdf_error_clone_and_eq() {
        let err1 = PdfError::ParseError("test".to_string());
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn pdf_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing file");
        let pdf_err: PdfError = io_err.into();
        assert!(matches!(pdf_err, PdfError::IoError(_)));
        assert!(pdf_err.to_string().contains("missing file"));
    }

    // --- ExtractWarning tests ---

    #[test]
    fn warning_new_with_description_only() {
        let w = ExtractWarning::new("missing font metrics");
        assert_eq!(w.description, "missing font metrics");
        assert!(matches!(w.code, ExtractWarningCode::Other(_)));
        assert_eq!(w.page, None);
        assert_eq!(w.element, None);
        assert_eq!(w.operator_index, None);
        assert_eq!(w.font_name, None);
        assert_eq!(w.to_string(), "[OTHER] missing font metrics");
    }

    #[test]
    fn warning_on_page() {
        let w = ExtractWarning::on_page("unknown operator", 3);
        assert_eq!(w.description, "unknown operator");
        assert_eq!(w.page, Some(3));
        assert_eq!(w.element, None);
        assert_eq!(w.operator_index, None);
        assert_eq!(w.font_name, None);
        assert_eq!(w.to_string(), "[OTHER] unknown operator (page 3)");
    }

    #[test]
    fn warning_with_full_context() {
        let w = ExtractWarning::with_context("missing width", 1, "char at offset 42");
        assert_eq!(w.description, "missing width");
        assert_eq!(w.page, Some(1));
        assert_eq!(w.element, Some("char at offset 42".to_string()));
        assert_eq!(w.operator_index, None);
        assert_eq!(w.font_name, None);
        assert_eq!(
            w.to_string(),
            "[OTHER] missing width (page 1) [char at offset 42]"
        );
    }

    #[test]
    fn warning_with_operator_context() {
        let w =
            ExtractWarning::with_operator_context("font not found in resources", 5, "Helvetica");
        assert_eq!(w.description, "font not found in resources");
        assert_eq!(w.page, None);
        assert_eq!(w.element, None);
        assert_eq!(w.operator_index, Some(5));
        assert_eq!(w.font_name, Some("Helvetica".to_string()));
        assert_eq!(
            w.to_string(),
            "[OTHER] font not found in resources [font Helvetica] [operator #5]"
        );
    }

    #[test]
    fn warning_display_with_all_fields() {
        let w = ExtractWarning {
            code: ExtractWarningCode::MissingFont,
            description: "test warning".to_string(),
            page: Some(2),
            element: Some("extra context".to_string()),
            operator_index: Some(10),
            font_name: Some("Arial".to_string()),
        };
        assert_eq!(
            w.to_string(),
            "[MISSING_FONT] test warning (page 2) [font Arial] [operator #10] [extra context]"
        );
    }

    #[test]
    fn warning_clone_and_eq() {
        let w1 = ExtractWarning::on_page("test warning", 0);
        let w2 = w1.clone();
        assert_eq!(w1, w2);
    }

    #[test]
    fn warning_with_operator_context_clone_and_eq() {
        let w1 = ExtractWarning::with_operator_context("test", 3, "Times");
        let w2 = w1.clone();
        assert_eq!(w1, w2);
    }

    // --- ExtractResult tests ---

    #[test]
    fn extract_result_ok_no_warnings() {
        let result = ExtractResult::ok(42);
        assert_eq!(result.value, 42);
        assert!(result.warnings.is_empty());
        assert!(result.is_clean());
    }

    #[test]
    fn extract_result_with_warnings() {
        let warnings = vec![
            ExtractWarning::new("warn 1"),
            ExtractWarning::on_page("warn 2", 0),
        ];
        let result = ExtractResult::with_warnings("hello", warnings);
        assert_eq!(result.value, "hello");
        assert_eq!(result.warnings.len(), 2);
        assert!(!result.is_clean());
    }

    #[test]
    fn extract_result_map_preserves_warnings() {
        let warnings = vec![ExtractWarning::new("test")];
        let result = ExtractResult::with_warnings(10, warnings);
        let mapped = result.map(|v| v * 2);
        assert_eq!(mapped.value, 20);
        assert_eq!(mapped.warnings.len(), 1);
        assert_eq!(mapped.warnings[0].description, "test");
    }

    #[test]
    fn extract_result_collect_multiple_warnings() {
        let mut result = ExtractResult::ok(Vec::<String>::new());
        result.warnings.push(ExtractWarning::new("first"));
        result.warnings.push(ExtractWarning::on_page("second", 1));
        result
            .warnings
            .push(ExtractWarning::with_context("third", 2, "char 'A'"));
        assert_eq!(result.warnings.len(), 3);
    }

    // --- ExtractOptions tests ---

    #[test]
    fn extract_options_default_values() {
        let opts = ExtractOptions::default();
        assert_eq!(opts.max_recursion_depth, 10);
        assert_eq!(opts.max_objects_per_page, 100_000);
        assert_eq!(opts.max_stream_bytes, 100 * 1024 * 1024);
        assert!(opts.collect_warnings);
        assert_eq!(opts.unicode_norm, UnicodeNorm::Nfc);
        assert!(!opts.extract_image_data);
        assert!(opts.max_input_bytes.is_none());
        assert!(opts.max_pages.is_none());
        assert!(opts.max_total_image_bytes.is_none());
        assert!(opts.max_total_objects.is_none());
    }

    #[test]
    fn extract_options_for_llm() {
        let opts = ExtractOptions::for_llm();
        assert_eq!(opts.unicode_norm, UnicodeNorm::Nfc);
        assert_eq!(opts.max_recursion_depth, 10);
        assert_eq!(opts.max_objects_per_page, 100_000);
        assert_eq!(opts.max_stream_bytes, 100 * 1024 * 1024);
        assert!(opts.collect_warnings);
    }

    #[test]
    fn extract_options_custom_values() {
        let opts = ExtractOptions {
            max_recursion_depth: 5,
            max_objects_per_page: 50_000,
            max_stream_bytes: 10 * 1024 * 1024,
            collect_warnings: false,
            unicode_norm: UnicodeNorm::None,
            extract_image_data: true,
            strict_mode: true,
            max_input_bytes: Some(1024),
            max_pages: Some(10),
            max_total_image_bytes: Some(5 * 1024 * 1024),
            max_total_objects: Some(100_000),
            dedupe: None,
        };
        assert_eq!(opts.max_recursion_depth, 5);
        assert_eq!(opts.max_objects_per_page, 50_000);
        assert_eq!(opts.max_stream_bytes, 10 * 1024 * 1024);
        assert!(!opts.collect_warnings);
        assert!(opts.extract_image_data);
        assert!(opts.strict_mode);
        assert_eq!(opts.max_input_bytes, Some(1024));
        assert_eq!(opts.max_pages, Some(10));
        assert_eq!(opts.max_total_image_bytes, Some(5 * 1024 * 1024));
        assert_eq!(opts.max_total_objects, Some(100_000));
    }

    #[test]
    fn extract_options_clone() {
        let opts1 = ExtractOptions::default();
        let opts2 = opts1.clone();
        assert_eq!(opts2.max_recursion_depth, opts1.max_recursion_depth);
        assert_eq!(opts2.collect_warnings, opts1.collect_warnings);
    }

    // --- US-096: ExtractWarningCode tests ---

    #[test]
    fn warning_code_missing_font() {
        let code = ExtractWarningCode::MissingFont;
        assert_eq!(code.as_str(), "MISSING_FONT");
    }

    #[test]
    fn warning_code_unsupported_operator() {
        let code = ExtractWarningCode::UnsupportedOperator;
        assert_eq!(code.as_str(), "UNSUPPORTED_OPERATOR");
    }

    #[test]
    fn warning_code_malformed_object() {
        let code = ExtractWarningCode::MalformedObject;
        assert_eq!(code.as_str(), "MALFORMED_OBJECT");
    }

    #[test]
    fn warning_code_resource_limit_reached() {
        let code = ExtractWarningCode::ResourceLimitReached;
        assert_eq!(code.as_str(), "RESOURCE_LIMIT_REACHED");
    }

    #[test]
    fn warning_code_encoding_fallback() {
        let code = ExtractWarningCode::EncodingFallback;
        assert_eq!(code.as_str(), "ENCODING_FALLBACK");
    }

    #[test]
    fn warning_code_other_preserves_custom_message() {
        let code = ExtractWarningCode::Other("custom issue".to_string());
        assert_eq!(code.as_str(), "OTHER");
        if let ExtractWarningCode::Other(msg) = &code {
            assert_eq!(msg, "custom issue");
        } else {
            panic!("expected Other variant");
        }
    }

    #[test]
    fn warning_code_clone_and_eq() {
        let code1 = ExtractWarningCode::MissingFont;
        let code2 = code1.clone();
        assert_eq!(code1, code2);

        let code3 = ExtractWarningCode::Other("test".to_string());
        let code4 = code3.clone();
        assert_eq!(code3, code4);
    }

    #[test]
    fn warning_with_code_field() {
        let w = ExtractWarning::new("missing font metrics");
        // new() should default to Other code
        assert!(matches!(w.code, ExtractWarningCode::Other(_)));
    }

    #[test]
    fn warning_with_explicit_code() {
        let w = ExtractWarning {
            code: ExtractWarningCode::MissingFont,
            description: "font not found".to_string(),
            page: Some(0),
            element: None,
            operator_index: None,
            font_name: None,
        };
        assert_eq!(w.code, ExtractWarningCode::MissingFont);
        assert_eq!(w.page, Some(0));
    }

    #[test]
    fn warning_display_format_with_code() {
        let w = ExtractWarning {
            code: ExtractWarningCode::MissingFont,
            description: "font not found".to_string(),
            page: Some(2),
            element: None,
            operator_index: None,
            font_name: None,
        };
        assert_eq!(w.to_string(), "[MISSING_FONT] font not found (page 2)");
    }

    #[test]
    fn warning_display_format_with_code_no_page() {
        let w = ExtractWarning {
            code: ExtractWarningCode::UnsupportedOperator,
            description: "unknown op".to_string(),
            page: None,
            element: None,
            operator_index: None,
            font_name: None,
        };
        assert_eq!(w.to_string(), "[UNSUPPORTED_OPERATOR] unknown op");
    }

    #[test]
    fn warning_display_format_other_code() {
        let w = ExtractWarning {
            code: ExtractWarningCode::Other("custom".to_string()),
            description: "something happened".to_string(),
            page: Some(5),
            element: None,
            operator_index: None,
            font_name: None,
        };
        assert_eq!(w.to_string(), "[OTHER] something happened (page 5)");
    }

    #[test]
    fn strict_mode_default_false() {
        let opts = ExtractOptions::default();
        assert!(!opts.strict_mode);
    }

    #[test]
    fn strict_mode_converts_warning_to_error() {
        let warning = ExtractWarning {
            code: ExtractWarningCode::MissingFont,
            description: "font not found".to_string(),
            page: Some(0),
            element: None,
            operator_index: None,
            font_name: None,
        };
        let err: PdfError = warning.to_error();
        assert!(matches!(err, PdfError::Other(_)));
        assert!(err.to_string().contains("font not found"));
    }

    #[test]
    fn warning_code_display() {
        assert_eq!(
            format!("{}", ExtractWarningCode::MissingFont),
            "MISSING_FONT"
        );
        assert_eq!(
            format!("{}", ExtractWarningCode::Other("x".into())),
            "OTHER"
        );
    }

    // --- US-097: Document-level resource budgets ---

    #[test]
    fn resource_budget_defaults_none() {
        let opts = ExtractOptions::default();
        assert!(opts.max_input_bytes.is_none());
        assert!(opts.max_pages.is_none());
        assert!(opts.max_total_image_bytes.is_none());
        assert!(opts.max_total_objects.is_none());
    }

    #[test]
    fn resource_budget_custom_values() {
        let opts = ExtractOptions {
            max_input_bytes: Some(1024 * 1024),
            max_pages: Some(50),
            max_total_image_bytes: Some(10 * 1024 * 1024),
            max_total_objects: Some(500_000),
            ..ExtractOptions::default()
        };
        assert_eq!(opts.max_input_bytes, Some(1024 * 1024));
        assert_eq!(opts.max_pages, Some(50));
        assert_eq!(opts.max_total_image_bytes, Some(10 * 1024 * 1024));
        assert_eq!(opts.max_total_objects, Some(500_000));
    }

    #[test]
    fn resource_limit_exceeded_clone_and_eq() {
        let err1 = PdfError::ResourceLimitExceeded {
            limit_name: "max_input_bytes".to_string(),
            limit_value: 100,
            actual_value: 200,
        };
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }
}
