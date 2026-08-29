# Trusted publishing sources

Official registry and GitHub documentation defines the identity contract.

- [crates.io trusted publishing](https://crates.io/docs/trusted-publishing)
  requires one configuration per existing crate and matches the GitHub owner,
  repository, workflow filename, and optional environment. The official
  [`crates-io-auth-action`](https://github.com/rust-lang/crates-io-auth-action)
  exchanges GitHub OpenID Connect identity for a temporary token and revokes it
  after the job.
- [PyPI publishing with a trusted publisher](https://docs.pypi.org/trusted-publishers/using-a-publisher/)
  requires `id-token: write`, no explicit password, and recommends a dedicated
  environment and publishing job.
- [npm trusted publishers](https://docs.npmjs.com/trusted-publishers/) require
  npm 11.5.1 or later, Node 22.14.0 or later, a GitHub-hosted runner,
  `id-token: write`, the exact workflow filename, and an allowed operation.
- [`npm trust`](https://docs.npmjs.com/cli/v11/commands/npm-trust/) manages
  publisher records from an authenticated npm 11.15.0-or-newer client and
  supports exact repository, workflow, environment, and operation flags.
- [Node.js 24.5.0](https://nodejs.org/en/blog/release/v24.5.0) bundles npm
  11.5.1, satisfying npm's documented client floor without a mutable install.
- [GitHub OpenID Connect](https://docs.github.com/en/actions/reference/security/oidc)
  says `id-token: write` enables requesting an identity token but grants no
  resource write by itself; job-scoped permission limits exposure.
- [`GITHUB_TOKEN`](https://docs.github.com/en/actions/concepts/security/github_token)
  is a per-job GitHub App installation token restricted to the repository and
  expires when the job ends or reaches its maximum lifetime.

The checked workflow gives registry identity only to publisher jobs, artifact
identity only to attestation callers, and GitHub content write only to the
GitHub Release job. Private registry bindings require separate verification.
