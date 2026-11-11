#!/usr/bin/env python3
"""Capture and validate the existing Rust CI test command."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

COMMAND = ["cargo", "test", "--workspace", "--no-fail-fast"]
FOOTER = re.compile(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in \d+(?:\.\d+)?s")
START = re.compile(r"\s*(?:Running (?:unittests|tests/|.*\(.*\))|Doc-tests ).*")
TEST = re.compile(r"test (.+) \.\.\. (ok|FAILED|ignored)(?:, (.+))?")


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def identity():
    return {
        "head": git("rev-parse", "HEAD"),
        "composition": git("submodule", "status", "--recursive"),
        "repository": os.environ.get("GITHUB_REPOSITORY", ""),
        "workflow": os.environ.get("GITHUB_WORKFLOW_REF", ""),
        "run_id": os.environ.get("GITHUB_RUN_ID", ""),
        "run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT", ""),
        "github_sha": os.environ.get("GITHUB_SHA", ""),
    }


def counts(raw):
    result = {"executed": 0, "passed": 0, "failed": 0, "skipped": 0, "skip_reasons": []}
    section = None
    completed = 0
    for line in raw.decode("utf-8", errors="strict").splitlines():
        if START.fullmatch(line):
            if section is not None:
                raise ValueError("incomplete test section")
            section = {"running": None, "ok": 0, "FAILED": 0, "ignored": 0}
        elif section is not None and re.fullmatch(r"running \d+ tests?", line):
            if section["running"] is not None:
                raise ValueError("duplicate test count")
            section["running"] = int(line.split()[1])
        elif section is not None and (test := TEST.fullmatch(line)):
            section[test[2]] += 1
            if test[2] == "ignored":
                result["skip_reasons"].append({"test": test[1], "reason": test[3]})
        elif footer := FOOTER.fullmatch(line):
            if section is None or section["running"] is None:
                raise ValueError("missing test section")
            passed, failed, skipped, measured = map(int, footer.group(2, 3, 4, 5))
            if (measured or section["running"] != passed + failed + skipped
                    or (passed, failed, skipped) != (section["ok"], section["FAILED"], section["ignored"])
                    or (footer[1] == "ok") != (failed == 0)):
                raise ValueError("inconsistent test output")
            result["passed"] += passed
            result["failed"] += failed
            result["skipped"] += skipped
            completed += 1
            section = None
    if not completed or section is not None:
        raise ValueError("missing or incomplete test output")
    result["executed"] = result["passed"] + result["failed"]
    return result


def validate(directory, current):
    raw = (directory / "test-output.log").read_bytes()
    record = json.loads((directory / "summary.json").read_text())
    if record["command"] != COMMAND or record["exit_code"] != 0:
        raise ValueError("wrong command or nonzero exit")
    if record["identity"] != current or not all(current.values()):
        raise ValueError("wrong or missing run identity")
    if current["head"] != current["github_sha"]:
        raise ValueError("checkout does not match CI SHA")
    if any(line.startswith(("-", "+", "U")) for line in current["composition"].splitlines()):
        raise ValueError("submodule composition mismatch")
    if record["sha256"] != hashlib.sha256(raw).hexdigest():
        raise ValueError("raw output digest mismatch")
    if record["counts"] != counts(raw):
        raise ValueError("counts do not match raw output")
    if record["counts"]["failed"] or not record["counts"]["executed"]:
        raise ValueError("test run failed or executed zero tests")
    return record


def main():
    directory = Path(os.environ["RUNNER_TEMP"]) / "meisei-test-evidence"
    directory.mkdir(parents=True, exist_ok=True)
    before = identity()
    with (directory / "test-output.log").open("wb") as output:
        result = subprocess.run(COMMAND, stdout=output, stderr=subprocess.STDOUT, check=False)
    raw = (directory / "test-output.log").read_bytes()
    sys.stdout.buffer.write(raw)
    record = {"command": COMMAND, "exit_code": result.returncode, "identity": before,
              "sha256": hashlib.sha256(raw).hexdigest()}
    try:
        record["counts"] = counts(raw)
    except (ValueError, UnicodeError) as error:
        record["parse_error"] = str(error)
    (directory / "summary.json").write_text(json.dumps(record, indent=2) + "\n")
    try:
        if git("status", "--porcelain", "--untracked-files=no"):
            raise ValueError("tracked files changed during test")
        validate(directory, identity())
    except (OSError, KeyError, ValueError, UnicodeError) as error:
        print(f"test evidence rejected: {error}", file=sys.stderr)
        return 1
    print("test evidence verified:", json.dumps(record["counts"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
