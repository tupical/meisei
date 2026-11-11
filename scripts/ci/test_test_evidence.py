import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import test_evidence as evidence

GOOD = b'     Running unittests src/lib.rs (target/debug/deps/example)\n\nrunning 1 test\ntest example ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n'
IDENTITY = {"head": "a", "composition": " abc daruma", "repository": "a/b", "workflow": "a/b/.github/workflows/rust.yml@refs/heads/main", "run_id": "1", "run_attempt": "1", "github_sha": "a"}


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name)
        self.record = {"command": evidence.COMMAND, "exit_code": 0, "identity": IDENTITY.copy(),
                       "sha256": hashlib.sha256(GOOD).hexdigest(), "counts": evidence.counts(GOOD)}
        (self.path / "test-output.log").write_bytes(GOOD)
        self.save()

    def save(self):
        (self.path / "summary.json").write_text(json.dumps(self.record))

    def test_valid(self):
        self.assertEqual(evidence.validate(self.path, IDENTITY)["counts"]["executed"], 1)

    def test_zero_skipped_failure_and_truncation(self):
        for raw in (GOOD.replace(b'1 passed', b'0 passed'), GOOD.replace(b'1 passed; 0 failed; 0 ignored', b'0 passed; 0 failed; 1 ignored'), GOOD.replace(b'ok. 1 passed; 0 failed', b'FAILED. 0 passed; 1 failed'), GOOD[:100], GOOD + b'     Running unittests second (target/x)\n'):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                evidence.counts(raw)

    def test_artifact_tampering(self):
        for key, value in (("counts", {"executed": 42}), ("sha256", "bad"), ("exit_code", 1), ("command", ["other"])):
            old = self.record[key]
            self.record[key] = value
            self.save()
            with self.subTest(key=key), self.assertRaises(ValueError):
                evidence.validate(self.path, IDENTITY)
            self.record[key] = old
        self.save()
        (self.path / "test-output.log").write_bytes(GOOD + b'changed')
        with self.assertRaises(ValueError):
            evidence.validate(self.path, IDENTITY)
        (self.path / "test-output.log").unlink()
        with self.assertRaises(FileNotFoundError):
            evidence.validate(self.path, IDENTITY)

    def test_wrong_identity_and_composition(self):
        for key in IDENTITY:
            current = IDENTITY.copy()
            current[key] += 'wrong'
            with self.subTest(key=key), self.assertRaises(ValueError):
                evidence.validate(self.path, current)
        current = IDENTITY | {"composition": "+abc daruma"}
        self.record["identity"] = current
        self.save()
        with self.assertRaises(ValueError):
            evidence.validate(self.path, current)


if __name__ == '__main__':
    unittest.main()
