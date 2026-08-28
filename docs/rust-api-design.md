# Rust facade API-design review

This guide is the required design-review contract for additions or changes to
the stable `pdfplumber` Rust facade. It turns broad API preferences into
questions that a pull request must answer with source and test evidence. It
does not promote `pdfplumber-core`, `pdfplumber-parse`, the Command-Line
Interface, Python, or WebAssembly adapters into the same compatibility surface.

Use this review before implementing a public facade change. Repeat it when an
implementation changes the proposed signature, ownership, materialization,
ordering, error, or extensibility behavior. A documentation-only correction
that does not change the contract may mark a dimension not applicable, but it
must give a reason rather than deleting the dimension.

## Required review record

Every pull request that adds or changes a stable facade item must carry this
record before merge. Put it in the pull-request description or a linked design
document and keep the answers consistent with the final diff.

| Field | Required answer |
|---|---|
| Before and after signature | Show the existing public declaration, or `new`, and the proposed declaration including lifetimes, bounds, features, and return type. |
| Observable contract | State indexing, ordering, laziness, failure, partial-consumption, resource-accounting, and repeatability behavior that callers may rely on. |
| Ownership and allocation impact | Identify each borrowed, consumed, cloned, buffered, cached, or newly allocated value and when it is released. |
| Compatibility classification | Classify source, behavioral, feature, and serialized-schema effects; name the required SemVer and deprecation path. |
| Validation evidence | Give focused compile, runtime, determinism, feature-combination, rustdoc, and SemVer commands appropriate to the change. |

For each of the seven dimensions below, record `accept`, `revise`, or `not
applicable: reason`. An unreasoned `N/A`, a generic claim such as “no impact,”
or a green `cargo-semver-checks` result without behavioral review is not a
complete record.

## Ownership

The caller should be able to tell from the signature whether a value is
borrowed for a call, borrowed from another value, or transferred. A caller
decides whether and where to clone; the facade must not take a shared borrow
and silently clone it merely because the implementation later wants ownership.

The current facade provides concrete reference points:

- [`Pdf::open_bytes`](../crates/pdfplumber/src/pdf.rs) borrows input bytes only
  for the call and returns an owned `Pdf` that does not borrow the slice.
- `Pdf::open_reader` consumes an `R: Read` value, buffers it, and returns an
  owned document. A caller that wants to retain the reader may pass `&mut R`.
- `Pdf::pages` returns the borrowed `Pages<'_>` view. That borrow does not
  outlive its source `Pdf`, while each successfully extracted `Page` is owned
  and may outlive the document.
- [`Page::chars`](../crates/pdfplumber/src/page.rs) borrows a slice stored in the
  page. Methods taking extraction settings borrow them for the call.

For a proposed API, answer all of these questions:

1. Does the implementation actually need ownership? Prefer `&T`, `&mut T`, or
   a bounded generic input when it does not. Take `T` when it does.
2. Can a returned borrow remain valid under every future internal cache or
   representation change? If not, return an owned value or an explicit owned
   view.
3. Does a lifetime express one real relationship, or does it unnecessarily
   tie unrelated inputs and outputs together?
4. Does retaining an input also retain a whole document, page, allocation, or
   lock longer than the caller can see?
5. Are `Send` and `Sync` effects still consistent with the
   [concurrency contract](rust-concurrency.md)?

## Allocations

Allocation is part of an API decision even when it is not visible in the Rust
type. Document whether the call is zero-allocation, amortized, proportional to
input, proportional to output, or intentionally eager.

The present facade distinguishes borrowed inspection from materialized work:
`Page::chars` returns a borrowed slice without creating a result collection;
`Page::edges` derives and allocates a new `Vec` on every call; and
`Page::extract_text` materializes a new `String`. `Pdf::open_reader` buffers the
complete input before parsing. These are contracts to make visible, not a rule
that every allocation is wrong.

Review eager versus lazy materialization explicitly. Do not replace an
iterator with a collection merely for implementation convenience, and do not
add an iterator when the algorithm must retain the full result and doing so
would disguise that cost. Expose an intermediate result when it prevents
duplicate expensive work without leaking parser internals.

Any new allocation on an extraction hot path requires measurement with a
representative fixture and before/after evidence. Record bytes, allocation
count, peak memory, or a justified measurable proxy. If the cost is accepted,
document its scale and lifetime; if it is avoided by caching, also document
cache ownership, invalidation, and resource-budget interaction.

## Iterator behavior

An iterator review states item ownership, fallibility, order, laziness, size
knowledge, behavior after exhaustion, and what happens when iteration stops
early. Names should follow `iter`, `iter_mut`, and `into_iter` conventions, and
a public iterator should use a stable named type rather than expose an
incidental adapter stack.

The current page contract is the baseline:

- `Pages` is a cheap borrowed collection view, not an iterator and not an
  extracted page cache.
- `PagesIter` is lazy: each successful `next` or `next_back` interprets one page
  and yields an independently owned value through
  `Iterator<Item = Result<Page, PdfError>>`.
- `PagesIter` implements `DoubleEndedIterator`, `ExactSizeIterator`, and
  `FusedIterator`. Its exact length shrinks from both ends, the front and back
  never yield the same page, and exhaustion remains exhaustion.
- A page error consumes that page index, but it does not poison the iterator;
  the caller can continue after an iterator error. By contrast,
  `collect::<Result<Vec<_>, _>>()` intentionally stops at the first error.
- Partial consumption then drop stops future page interpretation. Already
  yielded pages remain owned by the caller; document-wide resource accounting
  already incurred by attempted pages is not rolled back.
- `Pdf::pages_parallel` is intentionally eager and returns a `Vec` containing
  one result per page in page-index order. Parallel execution must not reorder
  the public result.

Do not implement `ExactSizeIterator` unless the length is always exact, or
`FusedIterator` unless returning `None` is permanent. If a new iterator borrows
the document or page, show the lifetime in the before and after signature.

## Determinism

Every public sequence needs an ordering contract. Preserve source order when
it is meaningful; otherwise define a complete ordering with stable tie-breaks
that retain duplicates. Do not rely on thread completion, address, filesystem,
or hash iteration order.

`HashMap` is a lookup structure, not output order. For example,
`Page::chars_by_mcid` returns a `HashMap` for keyed lookup; callers must not use
map iteration as semantic character order. If map contents feed a `Vec`, text,
table, diagnostic list, or serialized output, sort them using documented keys
and deterministic tie-breaks first. Serialized sequence order is observable
even when a deserializer might ignore it.

Tests for a changed ordered API must repeat the same input, options, and fresh
resource state and require the same output. When no shared cumulative budget
can fail, require sequential and parallel forms to produce the same items in
page-index order. With a shared total-object or image-byte limit, worker
scheduling may decide which page first receives the resource error; only the
one-slot-per-page vector order remains deterministic. Include adversarial equal
primary keys so an unstable or incomplete sort cannot pass accidentally. A
documented unordered lookup container may compare by key/value content, but
its iteration must never be snapshotted as canonical output.

## Error composition

Anticipated runtime failure belongs in `Result`; panics are reserved for bugs
or documented precondition violations. A new fallible facade operation uses
the stable `PdfError`, selects an appropriate `PdfErrorKind` (adding a category
only when the existing kinds cannot describe it), attaches safe
`PdfErrorContext` where known, and preserves a lower-level cause through
`std::error::Error::source`.

Wrap a cause exactly once. Render an underlying error in the outer `Display`
or expose it through the source chain, not both. The facade keeps
source text out of ordinary `Display` and `Debug`, so protected diagnostics may
opt into the source chain without duplicating or accidentally publishing
document-derived data. See the complete [Rust error contract](rust-errors.md).

Error review must also cover partial results. State whether an error discards
all work, returns an intermediate result, or appears as one item in a fallible
iterator. As `PagesIter` demonstrates, an iterator error may consume one index
and allow the caller to continue; a collection-returning method must not
silently drop failed items or return a partial collection as if it were
complete. Preserve the most specific stable kind and contextual page/object
coordinates while keeping parser-specific types out of the facade signature.

## Extension traits

The current `pdfplumber` stable facade has no stable facade extension trait.
`PdfBackend` and `ContentHandler` are advanced `pdfplumber-parse` hooks; they do
not make backend injection or content callbacks a high-level compatibility
promise.

Prefer an inherent method first when the crate owns the receiver type and one
operation belongs naturally to it. Before proposing an extension trait, state:

1. Which useful foreign or generic receiver cannot receive an inherent method?
2. Is downstream implementation a supported contract, or only method
   availability? Seal the trait when external implementations are not intended.
3. Could a blanket implementation overlap now or after another crate adds an
   implementation? Check coherence and method-name collision risks.
4. Must callers import the trait for method resolution, and is that discoverable
   in rustdoc and examples?
5. Is dyn compatibility (historically object safety) required? If so, check
   generic methods, associated types, `Self`, and `Self: Sized` exclusions.
6. How can methods be added later without breaking downstream implementors?

An extension trait is not an escape hatch for a premature facade abstraction.
Keep parser customization in the parser crate until at least two concrete
high-level uses establish a stable semantic contract.

## Future compatibility

Review how the proposal can evolve, not only whether it compiles today.

- A public field is a compatibility commitment: callers may construct and
  destructure it. The curated data models deliberately expose stable fields,
  while new opaque handles should prefer private fields and accessors.
- Use `#[non_exhaustive]` when callers should expect future variants or fields.
  `PdfErrorKind` already requires a wildcard arm. Do not add the attribute to
  an existing exhaustive type and call that change free; construction and
  matching behavior change.
- `ExtractOptions`, `TextOptions`, `WordOptions`, and `TableSettings` are
  defaultable but expose stable public fields. Adding a field during `0.3.x`
  is breaking for callers that use an exhaustive struct literal; `Default`
  does not erase that commitment. A future option family should choose private
  fields plus builders, or `#[non_exhaustive]`, before stabilization when field
  growth is intended. For new behavior in the current line, prefer a new named
  method with a new options type over changing an existing method's arity.
  Adding an argument to an existing Rust function is breaking.
- A public trait defines contracts for both callers and implementors. Adding a
  non-defaulted trait item is breaking; adding a defaulted method can still
  create method-resolution collisions. Seal traits not meant for downstream
  implementation and avoid a facade trait until its implementor set is clear.
- Tightening generic bounds, changing owned output to borrowed output, changing
  order, making lazy work eager, or changing which failures are returned can be
  behavioral or source breaks even when a signature diff tool is green.
- Document behavior as a contract when downstream code may depend on it.
  Undocumented accident is not a design strategy, and a documented behavior
  change needs explicit compatibility classification.

Run the release `cargo-semver-checks` policy for public signature and trait
changes, then apply human review for its documented gaps. Ordinary removals
also follow the two-subsequent-minor-release deprecation policy. Serialized
model changes are reviewed separately against `serde-json-v1`.

## Decision and validation

The final record chooses one outcome:

- **Accept:** the public value exceeds its ownership, performance, complexity,
  and compatibility cost, and the evidence covers the observable contract.
- **Revise:** keep the use case but change the signature, placement,
  materialization, error, order, trait, or extensibility design before merge.
- **Keep advanced:** expose the capability only in `pdfplumber-core` or
  `pdfplumber-parse` until a stable high-level contract exists.

At minimum, run the focused contract for this guide, the facade doctests and
strict rustdoc gates, relevant runtime tests under affected feature
combinations, and the complete compatibility harness. Add compile-fail tests
for ownership or exhaustiveness traps, repeated-run and parallel comparisons
for ordering, and allocation measurements when the review identifies a hot
path cost.

The source mapping for these criteria is recorded in
[`references/rust-api-design.md`](../references/rust-api-design.md). The
machine-readable documentation contract is
[`compat/tests/test_rust_api_design.py`](../compat/tests/test_rust_api_design.py).
