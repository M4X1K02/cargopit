#!/usr/bin/env python3
"""Fail if CI Linux package targets are missing from GitHub environments."""

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
DEPLOYMENT_ENVIRONMENTS_FILE = REPO_ROOT / ".github" / "deployment-environments.json"
DEPLOYMENT_ENVIRONMENTS_KEY = "environments"
CI_WORKFLOW_FILE = REPO_ROOT / ".github" / "workflows" / "ci.yaml"
PRODUCTION_WORKFLOW_FILE = REPO_ROOT / ".github" / "workflows" / "production.yml"
UMBRELLA_ENVIRONMENT_NAME = "production"
CI_OS_NAME_PATTERN = re.compile(r"^\s+- os: ([A-Za-z0-9._-]+)\s*$", re.MULTILINE)
CI_ENVIRONMENT_NAME_PATTERN = re.compile(
    r"^\s+environment:\s*\n\s+name: ([A-Za-z0-9._-]+)\s*$",
    re.MULTILINE,
)


def load_environment_names():
    if not DEPLOYMENT_ENVIRONMENTS_FILE.is_file():
        raise SystemExit(f"missing {DEPLOYMENT_ENVIRONMENTS_FILE}")
    spec = json.loads(DEPLOYMENT_ENVIRONMENTS_FILE.read_text(encoding="utf-8"))
    names = spec.get(DEPLOYMENT_ENVIRONMENTS_KEY)
    if not isinstance(names, list) or not names:
        raise SystemExit(
            f"{DEPLOYMENT_ENVIRONMENTS_FILE} has no {DEPLOYMENT_ENVIRONMENTS_KEY} list"
        )
    return names


def main():
    environment_names = load_environment_names()
    if UMBRELLA_ENVIRONMENT_NAME not in environment_names:
        raise SystemExit(
            f"{DEPLOYMENT_ENVIRONMENTS_FILE} must include {UMBRELLA_ENVIRONMENT_NAME}"
        )

    if not CI_WORKFLOW_FILE.is_file():
        raise SystemExit(f"missing {CI_WORKFLOW_FILE}")
    if not PRODUCTION_WORKFLOW_FILE.is_file():
        raise SystemExit(f"missing {PRODUCTION_WORKFLOW_FILE}")

    ci_text = CI_WORKFLOW_FILE.read_text(encoding="utf-8")
    production_text = PRODUCTION_WORKFLOW_FILE.read_text(encoding="utf-8")
    if DEPLOYMENT_ENVIRONMENTS_FILE.name not in production_text:
        raise SystemExit(
            f"{PRODUCTION_WORKFLOW_FILE} does not read {DEPLOYMENT_ENVIRONMENTS_FILE.name}"
        )

    ci_os_names = set(CI_OS_NAME_PATTERN.findall(ci_text))
    ci_environment_literals = set(CI_ENVIRONMENT_NAME_PATTERN.findall(ci_text))
    linux_environment_names = [
        name for name in environment_names if name != UMBRELLA_ENVIRONMENT_NAME
    ]
    missing = []
    for name in linux_environment_names:
        if name in ci_os_names or name in ci_environment_literals:
            continue
        missing.append(name)
    if missing:
        raise SystemExit(
            "Linux GitHub environments missing from ci.yaml: " + ", ".join(missing)
        )

    extra_os_names = sorted(ci_os_names.difference(environment_names))
    if extra_os_names:
        raise SystemExit(
            "ci.yaml package os values missing from "
            f"{DEPLOYMENT_ENVIRONMENTS_FILE.name}: " + ", ".join(extra_os_names)
        )

    print("deployment environments match CI Linux package targets")
    return 0


if __name__ == "__main__":
    sys.exit(main())
