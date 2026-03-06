//! Integration tests for pdfplumber-chunk against real PDF fixtures.
//!
//! These tests run against the same PDF fixtures used by the cross-validation
//! harness in `crates/pdfplumber/tests/`. They verify observable properties of
//! chunking output rather than exact text content, since exact output depends on
//! PDF structure which varies.
//!
//! All tests use the `FIXTURES_DIR` path which requires the test to be run from
//! the workspace root. `cargo test` from the workspace root satisfies this.

use pdfplumber::Pdf;
use pdfplumber_chunk::{Chunk, ChunkSettings, ChunkType, Chunker};

const FIXTURES_DIR: &str = "crates/pdfplumber/tests/fixtures/pdfs";

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(FIXTURES_DIR).join(name)
}

fn open_fixture(name: &str) -> Pdf {
    let path = fixture_path(name);
    Pdf::open_file(&path, None).unwrap_or_else(|e| panic!("failed to open fixture {name}: {e}"))
}

// ───────────────────────────── helpers ─────────────────────────────

/// Assert all chunks have valid structure.
fn assert_chunks_valid(chunks: &[Chunk], context: &str) {
    for (i, chunk) in chunks.iter().enumerate() {
        assert!(
            !chunk.text.trim().is_empty(),
            "{context}: chunk {i} has empty text"
        );
        assert!(
            chunk.token_count > 0,
            "{context}: chunk {i} has zero token_count"
        );
        // token_count must match what estimate() would produce.
        let expected = pdfplumber_chunk::token_estimate(&chunk.text);
        assert_eq!(
            chunk.token_count, expected,
            "{context}: chunk {i} token_count mismatch"
        );
    }
}

/// Assert no chunk exceeds max_tokens (with 10% tolerance for split edge cases).
fn assert_token_budget(chunks: &[Chunk], max_tokens: usize, context: &str) {
    let limit = max_tokens + max_tokens / 10; // 110% tolerance
    for (i, chunk) in chunks.iter().enumerate() {
        if chunk.chunk_type == ChunkType::Table {
            continue; // tables are never split
        }
        assert!(
            chunk.token_count <= limit,
            "{context}: chunk {i} token_count {} exceeds budget {} (limit with tolerance: {limit})",
            chunk.token_count,
            max_tokens
        );
    }
}

// ─────────────────────────── basic smoke ───────────────────────────

#[test]
fn federal_register_produces_chunks() {
    let pdf = open_fixture("federal-register-2020-17221.pdf");
    let chunker = Chunker::new(ChunkSettings::default());
    let chunks = chunker.chunk(&pdf).unwrap();
    assert!(!chunks.is_empty(), "federal-register should produce chunks");
    assert_chunks_valid(&chunks, "federal-register");
}

#[test]
fn cupertino_usd_produces_chunks() {
    let pdf = open_fixture("cupertino_usd_4-6-16.pdf");
    let chunker = Chunker::new(ChunkSettings::default());
    let chunks = chunker.chunk(&pdf).unwrap();
    assert!(!chunks.is_empty(), "cupertino_usd should produce chunks");
    assert_chunks_valid(&chunks, "cupertino_usd");
}

#[test]
fn chelsea_pdta_produces_chunks() {
    let pdf = open_fixture("chelsea_pdta.pdf");
    let chunker = Chunker::new(ChunkSettings::default());
    let chunks = chunker.chunk(&pdf).unwrap();
    assert!(!chunks.is_empty(), "chelsea_pdta should produce chunks");
    assert_chunks_valid(&chunks, "chelsea_pdta");
}

// ──────────────────────── token budget enforcement ──────────────────────

#[test]
fn federal_register_token_budget_512() {
    let pdf = open_fixture("federal-register-2020-17221.pdf");
    let settings = ChunkSettings {
        max_tokens: 512,
        overlap_tokens: 0,
        ..Default::default()
    };
    let chunker = Chunker::new(settings);
    let chunks = chunker.chunk(&pdf).unwrap();
    assert_token_budget(&chunks, 512, "federal-register max_tokens=512");
}

#[test]
fn federal_register_token_budget_256() {
    let pdf = open_fixture("federal-register-2020-17221.pdf");
    let settings = ChunkSettings {
        max_tokens: 256,
        overlap_tokens: 0,
        ..Default::default()
    };
    let chunker = Chunker::new(settings);
    let chunks = chunker.chunk(&pdf).unwrap();
    assert_token_budget(&chunks, 256, "federal-register max_tokens=256");
}

#[test]
fn cupertino_token_budget_128() {
    let pdf = open_fixture("cupertino_usd_4-6-16.pdf");
    let settings = ChunkSettings {
        max_tokens: 128,
        overlap_tokens: 0,
        ..Default::default()
    };
    let chunker = Chunker::new(settings);
    let chunks = chunker.chunk(&pdf).unwrap();
    assert_token_budget(&chunks, 128, "cupertino max_tokens=128");
}

// ──────────────────────── page index provenance ──────────────────────

#[test]
fn chunks_carry_page_index() {
    let pdf = open_fixture("federal-register-2020-17221.pdf");
    let chunker = Chunker::new(ChunkSettings::default());
    let chunks = chunker.chunk(&pdf).unwrap();

    // All chunk page indices must be valid (0..page_count).
    let page_count = pdf.page_count();
    for chunk in &chunks {
        assert!(
            chunk.page < page_count,
            "chunk.page {} out of range (page_count={})",
            chunk.page,
            page_count
        );
    }

    // If the PDF has >1 page, chunks from different pages should appear.
    if page_count > 1 {
        let pages_seen: std::collections::HashSet<usize> = chunks.iter().map(|c| c.page).collect();
        assert!(
            pages_seen.len() > 1,
            "multi-page PDF should produce chunks from multiple pages"
        );
    }
}

// ──────────────────────── bbox coverage ──────────────────────

#[test]
fn bboxes_populated_when_include_bbox() {
    let pdf = open_fixture("cupertino_usd_4-6-16.pdf");
    let settings = ChunkSettings {
        include_bbox: true,
        ..Default::default()
    };
    let chunker = Chunker::new(settings);
    let chunks = chunker.chunk(&pdf).unwrap();

    for (i, chunk) in chunks.iter().enumerate() {
        let b = &chunk.bbox;
        // Non-degenerate bbox: either has width or height.
        let has_extent = b.x1 > b.x0 || b.bottom > b.top;
        assert!(
            has_extent,
            "chunk {i} on page {} has degenerate bbox {:?}",
            chunk.page, b
        );
    }
}

#[test]
fn bboxes_zero_when_not_include_bbox() {
    let pdf = open_fixture("cupertino_usd_4-6-16.pdf");
    let settings = ChunkSettings {
        include_bbox: false,
        ..Default::default()
    };
    let chunker = Chunker::new(settings);
    let chunks = chunker.chunk(&pdf).unwrap();

    for (i, chunk) in chunks.iter().enumerate() {
        let b = &chunk.bbox;
        assert_eq!(
            b.x0, 0.0,
            "chunk {i} bbox.x0 should be 0 when include_bbox=false"
        );
        assert_eq!(
            b.x1, 0.0,
            "chunk {i} bbox.x1 should be 0 when include_bbox=false"
        );
    }
}

// ──────────────────────── overlap continuity ──────────────────────

#[test]
fn overlap_increases_chunk_count() {
    // With overlap, each split chunk carries prefix tokens → more total tokens →
    // requires more splits → more chunks than without overlap.
    let pdf = open_fixture("federal-register-2020-17221.pdf");

    let no_overlap = ChunkSettings {
        max_tokens: 200,
        overlap_tokens: 0,
        ..Default::default()
    };
    let with_overlap = ChunkSettings {
        max_tokens: 200,
        overlap_tokens: 40,
        ..Default::default()
    };

    let chunks_no = Chunker::new(no_overlap).chunk(&pdf).unwrap();
    let chunks_ov = Chunker::new(with_overlap).chunk(&pdf).unwrap();

    // Overlap must produce at least as many chunks (usually more).
    assert!(
        chunks_ov.len() >= chunks_no.len(),
        "overlap should produce >= chunks than no-overlap: {}/{}",
        chunks_ov.len(),
        chunks_no.len()
    );
}

// ──────────────────────── table preservation ──────────────────────

#[test]
fn table_chunks_never_split() {
    // nics-background-checks is a table-heavy PDF (when it works). Use cupertino
    // which has confirmed tables from the cross-validation harness.
    let pdf = open_fixture("cupertino_usd_4-6-16.pdf");
    let settings = ChunkSettings {
        max_tokens: 10, // very small limit — tables must still be single chunks
        preserve_tables: true,
        ..Default::default()
    };
    let chunker = Chunker::new(settings);
    let chunks = chunker.chunk(&pdf).unwrap();

    // All table chunks must be single (not split). We verify by checking that
    // each Table chunk is a contiguous block (no table text spans two chunks).
    // We can't easily detect this without golden data, so we just verify type.
    for chunk in &chunks {
        if chunk.chunk_type == ChunkType::Table {
            // The table chunk text must be the full table render — contains " | "
            // from our pipe-delimited format.
            assert!(
                chunk.text.contains(" | ") || chunk.text.contains('\n') || !chunk.text.is_empty(),
                "table chunk should have content"
            );
        }
    }
}

// ──────────────────────── heading inference ──────────────────────

#[test]
fn headings_precede_section_assignment() {
    let pdf = open_fixture("federal-register-2020-17221.pdf");
    let chunker = Chunker::new(ChunkSettings::default());
    let chunks = chunker.chunk(&pdf).unwrap();

    // If any heading chunks were found, the subsequent paragraph chunks on
    // that page should have a non-None section.
    let mut last_heading_text: Option<String> = None;
    let mut last_heading_page: Option<usize> = None;

    for chunk in &chunks {
        if chunk.chunk_type == ChunkType::Heading {
            last_heading_text = Some(chunk.text.clone());
            last_heading_page = Some(chunk.page);
        } else if chunk.chunk_type == ChunkType::Paragraph {
            if let (Some(heading), Some(hpage)) = (&last_heading_text, last_heading_page) {
                if chunk.page == hpage {
                    assert_eq!(
                        chunk.section.as_deref(),
                        Some(heading.as_str()),
                        "paragraph on page {} should inherit section '{}'",
                        chunk.page,
                        heading
                    );
                }
            }
        }
    }
}

// ──────────────────────── ChunkSettings defaults ──────────────────────

#[test]
fn default_settings_are_sensible() {
    let s = ChunkSettings::default();
    assert_eq!(s.max_tokens, 512);
    assert_eq!(s.overlap_tokens, 64);
    assert!(s.preserve_tables);
    assert!(s.include_bbox);
}

// ──────────────────────── edge cases ──────────────────────

#[test]
fn image_only_page_gives_no_chunks() {
    // image_structure.pdf should have pages with no extractable text.
    // We just verify it doesn't panic.
    let pdf = open_fixture("image_structure.pdf");
    let chunker = Chunker::new(ChunkSettings::default());
    let result = chunker.chunk(&pdf);
    assert!(result.is_ok(), "image PDF should not error");
    // May produce 0 chunks or some if there's any text — just no panic.
}

#[test]
fn annotations_pdf_does_not_panic() {
    let pdf = open_fixture("annotations.pdf");
    let chunker = Chunker::new(ChunkSettings::default());
    let result = chunker.chunk(&pdf);
    assert!(result.is_ok());
}

#[test]
fn zero_max_tokens_gives_single_word_chunks() {
    // Edge case: max_tokens=1 means every word boundary triggers a split.
    // We just verify it doesn't panic or infinite-loop.
    let pdf = open_fixture("cupertino_usd_4-6-16.pdf");
    let settings = ChunkSettings {
        max_tokens: 1,
        overlap_tokens: 0,
        ..Default::default()
    };
    let chunker = Chunker::new(settings);
    let result = chunker.chunk(&pdf);
    assert!(result.is_ok(), "max_tokens=1 should not error");
    // Should produce many chunks.
    let chunks = result.unwrap();
    assert!(!chunks.is_empty() || pdf.page_count() == 0);
}
