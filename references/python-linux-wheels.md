# Python Linux wheel compatibility references

Observed: 2026-08-29.

## Primary sources

- [PyPA auditwheel](https://github.com/pypa/auditwheel) documents `auditwheel
  show` as the command for identifying external shared libraries and symbol
  versions that constrain a Linux wheel's manylinux tag.
- [PEP 600](https://peps.python.org/pep-0600/) defines a
  `manylinux_${GLIBCMAJOR}_${GLIBCMINOR}_${ARCH}` tag as a promise about the
  minimum glibc version and architecture, while allowing only a small set of
  core system libraries to be assumed.
- [PyPA platform compatibility tags](https://packaging.python.org/en/latest/specifications/platform-compatibility-tags/)
  specifies the Python, ABI, and platform components in wheel compatibility
  tags and records the manylinux tag families.

## Applied pattern

The release job uses auditwheel 6.8.1's JSON output instead of matching prose.
It requires the exact policy tag, an empty `external_libs` map, and
`unsupported_isa: false`, then retains the complete result with digests of the
wheel and policy. The check is artifact inspection, not a substitute for
installation tests on supported operating systems.
