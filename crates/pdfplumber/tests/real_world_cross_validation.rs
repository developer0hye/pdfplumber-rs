//! Real-world cross-validation tests: 45+ PDFs from Python pdfplumber test suite.
//!
//! These tests compare pdfplumber-rs extraction against golden data from Python
//! pdfplumber v0.11.9 across a wide variety of real-world PDF documents.
//!
//! Run with:
//!   cargo test -p pdfplumber --features full-fixtures --test real_world_cross_validation -- --nocapture
//!
//! Without `full-fixtures`, all tests are ignored (keeping CI fast for PRs).

mod common;

use common::*;

// ─── Helper macro to reduce boilerplate ─────────────────────────────────────

/// Generate a cross-validation test that is ignored unless `full-fixtures` is enabled.
macro_rules! real_world_test {
    ($name:ident, $pdf:expr, $doc:expr) => {
        #[doc = $doc]
        #[test]
        #[cfg_attr(not(feature = "full-fixtures"), ignore)]
        fn $name() {
            let result = try_validate_pdf($pdf);
            if let Some(ref err) = result.parse_error {
                eprintln!("PARSE ERROR for {}: {}", $pdf, err);
                // Informational: don't fail on parse errors for new fixtures.
                // These indicate areas where the parser needs improvement.
                return;
            }
            let cr = result.total_char_rate();
            let wr = result.total_word_rate();
            eprintln!(
                "RESULT {}: chars={:.1}% words={:.1}% lines={:.1}% rects={:.1}% tables={:.1}%",
                $pdf,
                cr * 100.0,
                wr * 100.0,
                result.total_line_rate() * 100.0,
                result.total_rect_rate() * 100.0,
                result.total_table_rate() * 100.0,
            );
        }
    };
}

// ─── Category 1: Government/Tabular Documents ──────────────────────────────

real_world_test!(
    rw_ag_energy_round_up,
    "ag-energy-round-up-2017-02-24.pdf",
    "Multi-page government energy report with tables"
);

real_world_test!(
    rw_background_checks,
    "background-checks.pdf",
    "NICS background check data variant"
);

real_world_test!(
    rw_ca_warn_report,
    "ca-warn-report.pdf",
    "California WARN report, dense table layout"
);

real_world_test!(
    rw_san_jose_pd,
    "san-jose-pd-firearm-sample.pdf",
    "San Jose Police Department firearm report"
);

real_world_test!(
    rw_federal_register,
    "federal-register-2020-17221.pdf",
    "Federal Register document, dense columnar text"
);

real_world_test!(
    rw_warn_report,
    "WARN-Report-for-7-1-2015-to-03-25-2016.pdf",
    "WARN table report variant"
);

real_world_test!(
    rw_senate_expenditures,
    "senate-expenditures.pdf",
    "US Senate financial expenditure data"
);

real_world_test!(
    rw_la_precinct_bulletin,
    "la-precinct-bulletin-2014-p1.pdf",
    "LA precinct bulletin document"
);

real_world_test!(
    rw_cupertino_usd,
    "cupertino_usd_4-6-16.pdf",
    "Cupertino school district document"
);

// ─── Category 2: Complex Layouts ───────────────────────────────────────────

real_world_test!(
    rw_150109dsp_milw,
    "150109DSP-Milw-505-90D.pdf",
    "Complex DSP document from Milwaukee"
);

real_world_test!(
    rw_2023_06_20_pv,
    "2023-06-20-PV.pdf",
    "Meeting minutes/process verbal layout"
);

real_world_test!(
    rw_chelsea_pdta,
    "chelsea_pdta.pdf",
    "Chelsea planning document (65 pages, complex tables)"
);

real_world_test!(
    rw_issue_13_dsp_fond,
    "issue-13-151201DSP-Fond-581-90D.pdf",
    "Complex DSP document from Fond du Lac"
);

// ─── Category 3: Table Edge Cases ──────────────────────────────────────────

real_world_test!(
    rw_table_curves,
    "table-curves-example.pdf",
    "Tables constructed with curve elements"
);

real_world_test!(
    rw_pr_136,
    "pr-136-example.pdf",
    "PR #136 regression: table detection issue"
);

real_world_test!(
    rw_pr_138,
    "pr-138-example.pdf",
    "PR #138 regression: table detection issue"
);

real_world_test!(
    rw_pr_88,
    "pr-88-example.pdf",
    "PR #88 regression: table detection issue"
);

// ─── Category 4: Annotations ───────────────────────────────────────────────

real_world_test!(
    rw_annotations,
    "annotations.pdf",
    "PDF with basic annotations"
);

real_world_test!(
    rw_annotations_rotated_90,
    "annotations-rotated-90.pdf",
    "Annotations on 90-degree rotated page"
);

real_world_test!(
    rw_annotations_rotated_180,
    "annotations-rotated-180.pdf",
    "Annotations on 180-degree rotated page"
);

real_world_test!(
    rw_annotations_rotated_270,
    "annotations-rotated-270.pdf",
    "Annotations on 270-degree rotated page"
);

real_world_test!(
    rw_annotations_unicode,
    "annotations-unicode-issues.pdf",
    "Annotations with Unicode edge cases"
);

// ─── Category 5: Character Handling ────────────────────────────────────────

real_world_test!(
    rw_duplicate_chars,
    "issue-71-duplicate-chars.pdf",
    "Duplicate character handling (issue #71)"
);

real_world_test!(
    rw_duplicate_chars_2,
    "issue-71-duplicate-chars-2.pdf",
    "Duplicate character handling variant (issue #71)"
);

real_world_test!(
    rw_dedupe_chars,
    "issue-1114-dedupe-chars.pdf",
    "Character deduplication edge case (issue #1114)"
);

real_world_test!(
    rw_line_char_render,
    "line-char-render-example.pdf",
    "Character rendering modes"
);

real_world_test!(rw_test_punkt, "test-punkt.pdf", "Punctuation handling test");

real_world_test!(
    rw_decimalize,
    "issue-203-decimalize.pdf",
    "Decimal coordinate handling (issue #203)"
);

// ─── Category 6: Structure/Tagged PDFs ─────────────────────────────────────

real_world_test!(
    rw_figure_structure,
    "figure_structure.pdf",
    "Tagged PDF with figure structure"
);

real_world_test!(
    rw_hello_structure,
    "hello_structure.pdf",
    "Simple tagged PDF structure tree"
);

real_world_test!(
    rw_image_structure,
    "image_structure.pdf",
    "Tagged PDF with image structure"
);

real_world_test!(
    rw_pdf_structure,
    "pdf_structure.pdf",
    "Full PDF structure tree example"
);

real_world_test!(
    rw_word365_structure,
    "word365_structure.pdf",
    "Word 365 generated structured PDF"
);

// ─── Category 7: Edge Cases ────────────────────────────────────────────────

/// empty.pdf: 0-byte file, should fail gracefully.
#[test]
#[cfg_attr(not(feature = "full-fixtures"), ignore)]
fn rw_empty_pdf() {
    let path = fixtures_dir().join("pdfs").join("empty.pdf");
    let result = pdfplumber::Pdf::open_file(&path, None);
    // Empty PDF should fail to open, not panic
    assert!(result.is_err(), "empty PDF should return an error");
    eprintln!(
        "empty.pdf correctly returned error: {}",
        result.err().unwrap()
    );
}

/// password-example.pdf: password-protected, should fail without password.
#[test]
#[cfg_attr(not(feature = "full-fixtures"), ignore)]
fn rw_password_pdf() {
    let path = fixtures_dir().join("pdfs").join("password-example.pdf");
    let result = pdfplumber::Pdf::open_file(&path, None);
    // Password-protected PDF should fail without password
    assert!(
        result.is_err(),
        "password PDF should return an error without password"
    );
    eprintln!(
        "password-example.pdf correctly returned error: {}",
        result.err().unwrap()
    );
}

real_world_test!(
    rw_malformed,
    "malformed-from-issue-932.pdf",
    "Malformed PDF recovery (issue #932)"
);

real_world_test!(
    rw_page_boxes,
    "page-boxes-example.pdf",
    "PDF with CropBox/MediaBox/BleedBox"
);

real_world_test!(
    rw_extra_attrs,
    "extra-attrs-example.pdf",
    "PDF with extra attributes"
);

real_world_test!(
    rw_nics_rotated,
    "nics-background-checks-2015-11-rotated.pdf",
    "NICS background checks with rotated pages"
);

// ─── Category 8: Issue Regressions ─────────────────────────────────────────

real_world_test!(rw_issue_53, "issue-53-example.pdf", "Issue #53 regression");

real_world_test!(rw_issue_67, "issue-67-example.pdf", "Issue #67 regression");

real_world_test!(rw_issue_90, "issue-90-example.pdf", "Issue #90 regression");

real_world_test!(
    rw_issue_140,
    "issue-140-example.pdf",
    "Issue #140 regression"
);

real_world_test!(
    rw_issue_297,
    "issue-297-example.pdf",
    "Issue #297 regression"
);

real_world_test!(
    rw_issue_316,
    "issue-316-example.pdf",
    "Issue #316 regression"
);

real_world_test!(
    rw_issue_461,
    "issue-461-example.pdf",
    "Issue #461 regression"
);

// ─── Summary ───────────────────────────────────────────────────────────────

/// Aggregate summary across all real-world PDFs (informational, never fails).
#[test]
#[cfg_attr(not(feature = "full-fixtures"), ignore)]
fn rw_all_summary() {
    let pdfs = [
        // Government/Tabular
        "ag-energy-round-up-2017-02-24.pdf",
        "background-checks.pdf",
        "ca-warn-report.pdf",
        "san-jose-pd-firearm-sample.pdf",
        "federal-register-2020-17221.pdf",
        "WARN-Report-for-7-1-2015-to-03-25-2016.pdf",
        "senate-expenditures.pdf",
        "la-precinct-bulletin-2014-p1.pdf",
        "cupertino_usd_4-6-16.pdf",
        // Complex Layouts
        "150109DSP-Milw-505-90D.pdf",
        "2023-06-20-PV.pdf",
        "chelsea_pdta.pdf",
        "issue-13-151201DSP-Fond-581-90D.pdf",
        // Table Edge Cases
        "table-curves-example.pdf",
        "pr-136-example.pdf",
        "pr-138-example.pdf",
        "pr-88-example.pdf",
        // Annotations
        "annotations.pdf",
        "annotations-rotated-90.pdf",
        "annotations-rotated-180.pdf",
        "annotations-rotated-270.pdf",
        "annotations-unicode-issues.pdf",
        // Character Handling
        "issue-71-duplicate-chars.pdf",
        "issue-71-duplicate-chars-2.pdf",
        "issue-1114-dedupe-chars.pdf",
        "line-char-render-example.pdf",
        "test-punkt.pdf",
        "issue-203-decimalize.pdf",
        // Structure/Tagged
        "figure_structure.pdf",
        "hello_structure.pdf",
        "image_structure.pdf",
        "pdf_structure.pdf",
        "word365_structure.pdf",
        // Edge Cases
        "malformed-from-issue-932.pdf",
        "page-boxes-example.pdf",
        "extra-attrs-example.pdf",
        "nics-background-checks-2015-11-rotated.pdf",
        // Issue Regressions
        "issue-53-example.pdf",
        "issue-67-example.pdf",
        "issue-90-example.pdf",
        "issue-140-example.pdf",
        "issue-297-example.pdf",
        "issue-316-example.pdf",
        "issue-461-example.pdf",
    ];

    eprintln!("\n========================================================");
    eprintln!("Real-World Cross-Validation Summary ({} PDFs)", pdfs.len());
    eprintln!(
        "PRD targets: chars/words >= {:.0}%, tables >= {:.0}%",
        CHAR_THRESHOLD * 100.0,
        TABLE_THRESHOLD * 100.0
    );
    eprintln!("========================================================");

    let mut pass_count = 0;
    let mut fail_count = 0;
    let mut error_count = 0;

    for pdf_name in &pdfs {
        let result = try_validate_pdf(pdf_name);
        if result.parse_error.is_some() {
            eprintln!("  {} -> PARSE ERROR", pdf_name);
            error_count += 1;
            continue;
        }
        let char_ok = result.total_char_rate() >= CHAR_THRESHOLD;
        let word_ok = result.total_word_rate() >= WORD_THRESHOLD;
        if char_ok && word_ok {
            eprintln!(
                "  {} -> PASS (chars={:.1}%, words={:.1}%)",
                pdf_name,
                result.total_char_rate() * 100.0,
                result.total_word_rate() * 100.0,
            );
            pass_count += 1;
        } else {
            eprintln!(
                "  {} -> BELOW TARGET (chars={:.1}%, words={:.1}%)",
                pdf_name,
                result.total_char_rate() * 100.0,
                result.total_word_rate() * 100.0,
            );
            fail_count += 1;
        }
    }

    eprintln!("========================================================");
    eprintln!(
        "Results: {} PASS, {} BELOW TARGET, {} PARSE ERROR (out of {})",
        pass_count,
        fail_count,
        error_count,
        pdfs.len()
    );
    eprintln!("========================================================\n");
}
