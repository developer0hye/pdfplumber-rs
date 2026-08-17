"""Provenance stamped into every golden artifact (PARITY-002).

Golden data is the evidence behind every parity claim in PRD.md. Evidence that
does not say what produced it cannot be re-checked, so each artifact records the
upstream release, the dependency lock it was installed from, the source fixture,
and the machine that ran the generator.

Deliberately absent: a timestamp. Regenerating unchanged input must produce a
byte-identical file, otherwise every regeneration is a diff and a real change
hides among hundreds of clock updates. The generating machine is still recorded,
so the platform-varying fields live here rather than beside the extracted data —
PARITY-028 compares the data, not this block.
"""

from __future__ import annotations

import hashlib
import platform
from pathlib import Path

from compat.harness import lockfile, upstream

# The command a reader should run to reproduce the artifact this block describes.
GENERATION_COMMAND: str = "scripts/generate_golden.py"

_READ_CHUNK_BYTES: int = 1024 * 1024


def file_sha256(target: Path) -> str:
    digest = hashlib.sha256()
    with target.open("rb") as handle:
        while chunk := handle.read(_READ_CHUNK_BYTES):
            digest.update(chunk)
    return digest.hexdigest()


def _repository_relative(target: Path, repo_root: Path) -> str:
    """Repository-relative when possible; never an absolute machine path.

    Golden generation always reads fixtures from inside the repository. A path
    from elsewhere (a scratch file in a test, say) falls back to its bare name
    rather than leaking the generating machine's directory layout.
    """
    try:
        return target.resolve().relative_to(repo_root).as_posix()
    except ValueError:
        return target.name


def build(fixture_path: Path, repo_root: Path | None = None) -> dict[str, str]:
    """Describe the environment and input behind one golden artifact."""
    root: Path = repo_root if repo_root is not None else upstream.REPO_ROOT
    target: upstream.Target = upstream.load_target()
    return {
        "upstream_project": target.project,
        "upstream_version": target.version,
        "upstream_tag": target.tag,
        "upstream_commit": target.commit,
        "upstream_repository": target.repository,
        "lockfile_sha256": lockfile.digest(),
        "fixture_path": _repository_relative(Path(fixture_path), root),
        "fixture_sha256": file_sha256(Path(fixture_path)),
        "generated_by": GENERATION_COMMAND,
        "python_version": platform.python_version(),
        "platform_system": platform.system(),
        "platform_machine": platform.machine(),
    }
