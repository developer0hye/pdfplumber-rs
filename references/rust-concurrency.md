# Rust concurrency design sources

Checked on 2026-08-27 for DX-008. The compile-time and runtime repository tests
are the evidence for the concrete `pdfplumber` types; these sources define the
language and dependency semantics used to describe them.

## Rust thread-safety traits

- [`std::marker::Send`](https://doc.rust-lang.org/std/marker/trait.Send.html)
  identifies values that can be transferred across thread boundaries. It is an
  auto trait, so the compiler derives the answer from every field of a type.
- [`std::marker::Sync`](https://doc.rust-lang.org/std/marker/trait.Sync.html)
  identifies types whose shared references can safely cross threads; formally,
  `T: Sync` when `&T: Send`.

The local test uses ordinary `T: Send + Sync` bounds for `Pdf`, `Pages`,
`PagesIter`, `Page`, and `CroppedPage`, rather than maintaining an unchecked
manual list.

## Rayon

- [Rayon `IndexedParallelIterator`](https://docs.rs/rayon/1.11.0/rayon/iter/trait.IndexedParallelIterator.html)
  defines indexed splitting and collection. Its examples collect indexed input
  into the corresponding sequential order.
- [Rayon `ThreadPool`](https://docs.rs/rayon/1.11.0/rayon/struct.ThreadPool.html)
  states that top-level Rayon work executes in the current pool and that
  parallel iterators called inside `ThreadPool::install` use that pool.

`Pdf::pages_parallel` maps the indexed Rust range directly into a `Vec`; the
local feature-gated test compares the resulting page-number/text sequence with
sequential extraction. Because it collects `Result` as an ordinary item rather
than using a short-circuiting `try_*` consumer, its API returns every per-page
result.

## PyO3

- [PyO3 0.24.2 parallelism](https://pyo3.rs/v0.24.2/parallelism) explains that
  `Python::allow_threads` temporarily releases the Global Interpreter Lock for
  Rust work and that holding the lock prevents CPU-bound Python threads from
  executing that work in parallel.
- [PyO3 free-threaded Python](https://pyo3.rs/v0.24.2/free-threading) describes
  the experimental CPython 3.13 build and the explicit module declaration
  needed to attest that an extension supports running without the Global
  Interpreter Lock.

The current binding uses PyO3 0.24.2 and neither releases the lock around
extraction nor declares free-threaded support. Its `Arc` and `Mutex` fields are
therefore documented only as memory-safety and cache-coordination mechanisms,
not as evidence of parallel Python execution.
