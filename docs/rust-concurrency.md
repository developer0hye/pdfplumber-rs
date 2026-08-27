# Rust concurrency and thread safety

The Rust facade supports concurrent, immutable extraction. This page defines
what can be moved or shared between threads, which state is shared, and what
the optional Rayon and Python surfaces do differently.

## Public Rust values

| Value | `Send` | `Sync` | Concurrency contract |
|---|---:|---:|---|
| `Pdf` | Yes | Yes | Share one opened document as `Arc<Pdf>` and call read-only methods from multiple threads. |
| `Pages<'_>` / `PagesIter<'_>` | Yes | Yes | Each value borrows its `Pdf`; it cannot outlive that document. Iteration calls `Pdf::page` on demand. |
| `Page` | Yes | Yes | This is an owned extraction result. Its public query methods are read-only, so an owned `Page` may be moved or shared. |
| `CroppedPage` | Yes | Yes | This is an owned, filtered result with the same read-only sharing contract as `Page`. |

These traits are compile-time guarantees. The repository also opens one
fixture behind an `Arc<Pdf>`, extracts pages concurrently, and shares owned
page values across threads in `crates/pdfplumber/tests/concurrency.rs`.

The document's geometry, metadata, bookmarks, structure tree, and related
caches are built when the `Pdf` is opened and are immutable after open.
`Pdf::page` does not populate a mutable shared page cache: every call extracts
a new owned `Page`. Returned slices such as `Page::chars` borrow immutable data
from that owned page.

## Shared resource budgets

Two resource budgets are intentionally shared mutable state inside one `Pdf`:
`max_total_objects` and `max_total_image_bytes`. Their counters use
`AtomicUsize` and are updated with `Ordering::Relaxed`, which makes the totals
race-free without imposing an ordering between otherwise independent page
results.

Every extraction attempt that reaches the resource-accounting step adds its
page's objects, then its image bytes. Extracting a repeated page counts that
page again; an attempt which pushes a counter past its limit returns
`PdfErrorKind::ResourceLimit`, but does not roll the counter back. If the object
limit fails first, that attempt does not reach the later image-byte update.
Concurrent callers therefore compete for one document-wide budget, and which
caller receives the first over-limit error is scheduling-dependent. Use
separate `Pdf` values when each worker needs an independent budget.

Opening limits such as `max_input_bytes` and `max_pages` are checked once while
constructing the document and do not change afterward.

## Parallel page processing

Enable the optional `parallel` Cargo feature to use
`Pdf::pages_parallel()`. It maps the indexed range `0..page_count` in parallel
and returns `Vec<Result<Page, PdfError>>` in page-index order, regardless of
the order in which workers finish. It collects every per-page result and does
not cancel remaining page work when one result is an error. Convert the vector
to `Result<Vec<_>, _>` afterward if the caller wants aggregate success/failure.

The method does not create or configure a thread pool. When called normally it
uses the global Rayon thread pool; when called inside
`rayon::ThreadPool::install`, it uses that current pool. Thread count,
scheduling, and completion order are not API guarantees. The sequential
`Pdf::pages` view remains available when callers need explicit work ordering or
bounded page retention.

The `parallel` feature is optional, is not enabled by default, and is not
available on the supported WebAssembly target. Continuous Integration runs the
public concurrency test with and without that feature.

## Python bindings

The Python wrapper stores the Rust document in `Arc<Pdf>` and protects Python
object caches with `Mutex`. Those mutexes prevent cache races; a Mutex around a
cache is not a promise that Python extraction calls execute in parallel.

The current PyO3 binding receives a `Python<'_>` token and does not release the
Python GIL with `Python::allow_threads` (or its later `Python::detach` name)
around extraction. On the supported GIL-enabled CPython builds, calls through
Python threads are therefore serialized while they execute binding code. Do
not infer Python CPU parallelism from the Rust `Send`/`Sync` traits or from
`Pdf::pages_parallel`; use the Rust API directly or process-level parallelism
for CPU-bound batches.

Free-threaded CPython is not supported and not verified by the current package
matrix. The extension does not attest a free-threaded module contract. Adding
empirical Python tests for concurrent reads from separate documents and pages
remains open under `PYAPI-023`; this Rust documentation task does not close or
prejudge it.

## Caller responsibilities

- Treat extracted values as immutable while shared. The current public methods
  enforce this with shared references, but caller-owned synchronization is
  still required around any separate mutable application state.
- Keep the borrowed `Pages<'_>` and `PagesIter<'_>` within the lifetime of the
  source `Pdf`; use owned `Page` values to move results beyond that scope.
- Size document-wide budgets for all attempted page extractions, including
  retries and parallel duplicates.
- Preserve page indices when applying additional caller-side parallelism if
  deterministic output order matters.
