#!/usr/bin/env python3
"""Reject compatibility percentage-threshold reductions without approval."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT))

from compat.harness import threshold_policy  # noqa: E402


SOURCE_ROOTS = ("compat", "crates/pdfplumber/tests", "scripts")
SOURCE_SUFFIXES = (".py", ".rs")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True, help="base Git revision")
    parser.add_argument("--head", required=True, help="head Git revision")
    parser.add_argument("--head-sha", required=True, help="exact reviewed head SHA")
    parser.add_argument("--repository", help="GitHub owner/repository")
    parser.add_argument("--pull-request", type=int, help="GitHub pull request number")
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    return parser.parse_args()


def _git(repo_root: Path, *arguments: str) -> str:
    completed = subprocess.run(
        ["git", "-C", str(repo_root), *arguments],
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise threshold_policy.ThresholdPolicyError(
            f"git {' '.join(arguments)} failed: {detail}"
        )
    return completed.stdout


def _source_paths(repo_root: Path, revision: str) -> set[str]:
    output = _git(repo_root, "ls-tree", "-r", "--name-only", revision, "--", *SOURCE_ROOTS)
    return {
        path
        for path in output.splitlines()
        if path.endswith(SOURCE_SUFFIXES) and "/fixtures/" not in path
    }


def thresholds_at_revision(repo_root: Path, revision: str) -> dict[str, float]:
    thresholds: dict[str, float] = {}
    for path in sorted(_source_paths(repo_root, revision)):
        source = _git(repo_root, "show", f"{revision}:{path}")
        extracted = threshold_policy.extract_thresholds(path, source)
        duplicate = thresholds.keys() & extracted.keys()
        if duplicate:
            raise threshold_policy.ThresholdPolicyError(
                f"duplicate threshold identities: {', '.join(sorted(duplicate))}"
            )
        thresholds.update(extracted)
    return dict(sorted(thresholds.items()))


def _github_json(repository: str, endpoint: str, token: str) -> Any:
    url = f"https://api.github.com/repos/{repository}/{endpoint.lstrip('/')}"
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "User-Agent": "pdfplumber-rs-threshold-policy",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    except (OSError, UnicodeError, json.JSONDecodeError, urllib.error.HTTPError) as error:
        raise threshold_policy.ThresholdPolicyError(
            f"GitHub approval lookup failed for {url}: {error}"
        ) from error


def _reviews_and_permissions(
    repository: str, pull_request: int, token: str
) -> tuple[list[dict[str, str]], dict[str, str]]:
    reviews: list[dict[str, str]] = []
    page = 1
    while True:
        payload = _github_json(
            repository,
            f"pulls/{pull_request}/reviews?per_page=100&page={page}",
            token,
        )
        if not isinstance(payload, list):
            raise threshold_policy.ThresholdPolicyError(
                "GitHub reviews response is not a list"
            )
        for item in payload:
            user = item.get("user") or {}
            reviews.append(
                {
                    "login": str(user.get("login") or ""),
                    "state": str(item.get("state") or ""),
                    "commit_id": str(item.get("commit_id") or ""),
                }
            )
        if len(payload) < 100:
            break
        page += 1

    permissions: dict[str, str] = {}
    for login in sorted({review["login"] for review in reviews if review["login"]}):
        quoted_login = urllib.parse.quote(login, safe="")
        payload = _github_json(
            repository, f"collaborators/{quoted_login}/permission", token
        )
        permissions[login] = str(payload.get("permission") or "")
    return reviews, permissions


def main() -> int:
    args = parse_args()
    try:
        before = thresholds_at_revision(args.repo_root, args.base)
        after = thresholds_at_revision(args.repo_root, args.head)
        reductions = threshold_policy.find_reductions(before, after)
        if not reductions:
            print(
                f"Threshold policy OK: {len(before)} guarded base thresholds, "
                "no reductions"
            )
            return 0

        if not args.repository or not args.pull_request:
            raise threshold_policy.ThresholdPolicyError(
                "reductions require --repository and --pull-request for maintainer approval"
            )
        token = os.environ.get("GITHUB_TOKEN", "")
        if not token:
            raise threshold_policy.ThresholdPolicyError(
                "reductions require GITHUB_TOKEN for maintainer approval"
            )

        prd_text = _git(args.repo_root, "show", f"{args.head}:PRD.md")
        reviews, permissions = _reviews_and_permissions(
            args.repository, args.pull_request, token
        )
        result = threshold_policy.enforce_policy(
            reductions,
            prd_text=prd_text,
            reviews=reviews,
            permissions=permissions,
            head_sha=args.head_sha,
        )
    except threshold_policy.ThresholdPolicyError as error:
        print(f"Threshold policy failed: {error}", file=sys.stderr)
        return 1

    print(
        f"Threshold policy OK: {len(reductions)} approved reduction(s), "
        f"reviewed by {result.approver}"
    )
    for reduction in reductions:
        print(f"  - {threshold_policy.format_evidence_marker(reduction)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
