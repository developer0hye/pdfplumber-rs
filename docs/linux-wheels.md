# Linux wheel compatibility

Every release-candidate Linux wheel is inspected after build and before its
Software Bill of Materials, provenance, and retained artifact group are
created. The machine policy is [`python-wheel-targets.toml`](../python-wheel-targets.toml),
and [`scripts/check_linux_wheel.py`](../scripts/check_linux_wheel.py) enforces it.

## Current policy

| Target | Python / ABI | Required filename platforms | Required auditwheel tag |
|---|---|---|---|
| x86-64 | `cp313` / `cp313` | `manylinux_2_17_x86_64.manylinux2014_x86_64` | `manylinux_2_17_x86_64` |
| AArch64 | `cp313` / `cp313` | `manylinux_2_17_aarch64.manylinux2014_aarch64` | `manylinux_2_17_aarch64` |

The build uses the explicit Maturin `manylinux2014` policy rather than `auto`.
The dual filename tag keeps the PEP 600 `manylinux_2_17` spelling and the
legacy `manylinux2014` spelling for installers that recognize either form.

## Enforced inspection

The release matrix installs the versions of `auditwheel`, `packaging`, and
`pyelftools` pinned in the policy. For each target it runs `auditwheel show
--json`, then rejects the candidate unless all of these statements are true:

- the filename carries the exact CPython, ABI, and dual platform tags;
- `overall_tag` and `sym_tag` equal the target's `manylinux_2_17` tag;
- `external_libs` is empty, so the wheel has no shared-library dependency
  outside the libraries supplied by the manylinux policy;
- `unsupported_isa` is false and the artifact is a native wheel.

References to `libc`, `libdl`, `libpthread`, or `libgcc_s` in
`versioned_symbols` describe policy-provided system libraries. They are not
unbundled project-specific dependencies; auditwheel reports any dependency
outside the policy in `external_libs`, which is required to remain empty.

The checker writes a deterministic JSON record containing the complete
auditwheel report plus SHA-256 digests for the wheel and policy. That record is
retained beside the wheel's integrity material.

## Scope boundary

This proves that the built artifact's tags, versioned symbols, external
libraries, and instruction-set requirements satisfy the declared Linux wheel
policy. It does not prove installation or runtime behavior on a particular
Linux distribution, and it does not cover macOS or Windows. Those remain
separate installed-artifact and platform tasks.

Primary-source context is recorded in the
[Linux wheel reference note](../references/python-linux-wheels.md).
