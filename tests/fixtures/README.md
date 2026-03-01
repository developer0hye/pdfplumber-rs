# PDF Test Fixtures

Real-world and generated PDF files for integration testing of pdfplumber-rs.

## Directory Structure

- `generated/` - PDFs created by `generate_fixtures.py` using fpdf2
- `downloaded/` - Real-world PDFs from public domain / MIT sources
- `generate_fixtures.py` - Script to regenerate the generated fixtures
- `download_fixtures.sh` - Script to re-download the external fixtures
- `checksums.sha256` - SHA-256 checksums for all fixture files

## Regenerating Fixtures

```bash
pip install fpdf2
python3 tests/fixtures/generate_fixtures.py
```

## Re-downloading Fixtures

```bash
bash tests/fixtures/download_fixtures.sh
```

## Extended Real-World Fixtures

45+ real-world PDFs from the Python pdfplumber test suite are available in
`crates/pdfplumber/tests/fixtures/pdfs/`. These are used by cross-validation
tests that compare Rust extraction against Python pdfplumber golden data.

```bash
# Download all real-world fixtures
bash tests/fixtures/download_all_fixtures.sh

# Generate golden reference data (requires pdfplumber==0.11.9)
pip install pdfplumber==0.11.9
python3 tests/fixtures/generate_cross_validation_golden.py

# Run extended tests
cargo test -p pdfplumber --features full-fixtures --test real_world_cross_validation -- --nocapture
```

## Licensing

- Generated PDFs: Created by this project, same license as the repository
- Downloaded PDFs: See `downloaded/README.md` for per-file attribution
- Extended fixtures: From [pdfplumber](https://github.com/jsvine/pdfplumber) (MIT License)
