# pdfium-render 0.9.3

- **Project:** https://github.com/ajrcarey/pdfium-render
- **Observed API:** https://docs.rs/pdfium-render/0.9.3/pdfium_render/prelude/struct.PdfPages.html
- **Observed source:** https://docs.rs/pdfium-render/0.9.3/src/pdfium_render/pdf/document/pages.rs.html
- **License:** MIT/Apache-2.0

## Page-collection pattern

The observed `PdfPages<'a>` type is a collection view tied to its document. It
exposes `len`, `is_empty`, direct zero-based `PdfPages::get`, and lazy
`PdfPages::iter` operations instead of forcing callers to materialize every page.
The direct geometry helper also explicitly distinguishes retrieving page size
from loading the complete page.

The Rust standard library documents `std::iter::IntoIterator` as the conventional
way for collection-like types to work with `for` loops. It documents
`ExactSizeIterator` for iterators whose remaining length is known exactly:

- https://doc.rust-lang.org/std/iter/trait.IntoIterator.html
- https://doc.rust-lang.org/std/iter/trait.ExactSizeIterator.html

## Relevance to pdfplumber-rs

`Pdf::pages()` follows the collection-view boundary while retaining this crate's
owned extracted `Page` model. `Pages::get` calls the existing direct page path,
and iteration processes one content stream per `next` or `next_back` call. The
view stores only `&Pdf`; it neither clones the parsed document nor retains yielded
pages. Existing `Pdf::page` and `Pdf::pages_iter` entry points remain compatibility
shortcuts during the alpha line.
