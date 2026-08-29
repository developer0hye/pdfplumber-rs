# Python package support metadata references

## Sources

- [Pyproject metadata specification](https://packaging.python.org/en/latest/specifications/pyproject-toml/)
  - `requires-python` maps to the core `Requires-Python` field.
  - classifiers are static project metadata.
- [Writing `pyproject.toml`](https://packaging.python.org/en/latest/guides/writing-pyproject-toml/)
  - Python classifiers describe supported versions for discovery.
  - `requires-python` is the field installers use to restrict versions.
- [PyO3 changelog](https://pyo3.rs/main/changelog.html)
  - PyO3 0.24.1 added the CPython 3.13 ABI feature.
  - PyO3 0.25.0 first added Python 3.14 beta support.
- [Python 3.14 release notes](https://docs.python.org/3.14/whatsnew/3.14.html)
  - Python 3.14 became a stable release on 2025-10-07.

## Applied boundary

The repository treats successful installation and execution of both a wheel
and source distribution as the support predicate. Static package metadata and
release wheel interpreters must be derived from that required matrix. An
untested interpreter, implementation, build mode, or forward-compatibility
override remains unsupported even if a builder can emit an artifact.
