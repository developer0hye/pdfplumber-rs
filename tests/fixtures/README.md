# PDF Test Fixtures

Real-world and generated PDF files for integration testing of pdfplumber-rs.

## Directory Structure

- `generated/` - PDFs created by `generate_fixtures.py` using fpdf2
- `downloaded/` - Publicly available PDFs copied from a pinned upstream revision
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

- Generated PDFs: Created by this project and recorded as Apache-2.0, matching
  the workspace manifest.
- Downloaded PDFs: See `downloaded/README.md` for attribution.
- Every committed PDF in the repository has a SHA-256 digest, immutable source
  revision (for external files), SPDX license, license evidence, public-source
  assertion, and redistribution status in `compat/fixture-provenance.toml`.

Run `python3 scripts/check_fixture_licenses.py` from the repository root after
adding or regenerating any PDF. The audit rejects unregistered, stale, changed,
private-source, or redistribution-restricted fixtures.
