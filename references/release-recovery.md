# Release Recovery References

Official registry and credential-provider documentation informs the runbook.

## Cargo and crates.io

- [cargo yank](https://doc.rust-lang.org/cargo/commands/cargo-yank.html) removes a
  version from normal dependency selection but does not delete its data; an
  existing lockfile or direct download can still use it, and `--undo` restores
  index selection.

## PyPI

- [Yanking](https://docs.pypi.org/project-management/yanking/) is the documented
  non-destructive alternative to deletion. PyPI yanks whole releases, and exact
  `==` or `===` pins can still select one.

## npm

- [Unpublish policy](https://docs.npmjs.com/policies/unpublish/) makes published
  versions immutable and non-reusable; unpublish is constrained and cannot be
  undone, while deprecation preserves dependent builds with a warning.
- [Token revocation](https://docs.npmjs.com/revoking-access-tokens/) requires
  deletion at npm and warns that revocation can be delayed.

## GitHub

- [Leaked-secret remediation](https://docs.github.com/en/code-security/tutorials/remediate-leaked-secrets/remediating-a-leaked-secret)
  requires revocation with the provider; removing the stored secret is not
  sufficient.
- [Immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases)
  lock published assets and tags, so corrections may need a new release.
- [Release endpoints](https://docs.github.com/en/rest/releases) distinguish
  releases and assets from package registries; changing one does not retract
  external registry artifacts.
