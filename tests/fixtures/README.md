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

## Licensing

- Generated PDFs: Created by this project, same license as the repository
- Downloaded PDFs: See `downloaded/README.md` for per-file attribution
