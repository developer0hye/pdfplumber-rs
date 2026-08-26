# Polars

Source: https://github.com/pola-rs/polars

## Relevant pattern

- The README places its broad performance statement directly beside a link to the
  published PDS-H benchmark results.
- Feature claims are grouped as short, reviewable bullets rather than buried in a
  long marketing paragraph.

## Applied here

- Keep each major README and release-note claim adjacent to a stable evidence link.
- Enforce that adjacency with a repository test, while accepting only local tests,
  scorecards, benchmark artifacts, and generated support entries as evidence.
