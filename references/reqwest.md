# reqwest

- Repository: <https://github.com/seanmonstar/reqwest>
- Rust error-handling guidance: <https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html>
- Reviewed: 2026-08-26

## Relevant pattern

- The primary README example is a complete executable rather than a fragment.
- `main` returns `Result<(), Box<dyn std::error::Error>>`, fallible operations use
  `?`, and the successful path ends with `Ok(())`.
- The example prints the useful value produced by the library.
- The Rust Book documents this `main` return type as the standard way to propagate
  recoverable errors and produce a nonzero process exit status.

## Applied here

DX-001 applies that shape to the first Rust extraction example. The repository
contract additionally caps it at fifteen lines, rejects `unwrap()` and `expect()`,
executes it in a fresh consumer project, and requires known extracted fixture text
on standard output.
