import json
import os
from pathlib import Path
import shutil
import signal
import tempfile
import unittest
from unittest.mock import patch

import test_evidence
import test_sensitivity as sensitivity


class SensitivityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.fixture = self.root / 'fixture'
        shutil.copytree(sensitivity.FIXTURE, self.fixture)
        head = test_evidence.git('rev-parse', 'HEAD')
        self.environment = patch.dict(os.environ, {'RUNNER_TEMP': str(self.root), 'GITHUB_REPOSITORY': 'local/meisei',
            'GITHUB_WORKFLOW_REF': 'local/test', 'GITHUB_RUN_ID': 'sensitivity-test', 'GITHUB_RUN_ATTEMPT': '1',
            'GITHUB_SHA': head, 'CARGO_NET_OFFLINE': 'true'})
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def execute(self, **kwargs):
        with patch.multiple(sensitivity, FIXTURE=self.fixture, **kwargs):
            code = sensitivity.run()
        outputs = list(self.root.glob('sensitivity-*/sensitivity.json'))
        self.assertEqual(len(outputs), 1)
        result = json.loads(outputs[0].read_text())
        if 'command' in result:
            self.assertFalse(Path(result['command'][4]).exists(), 'temporary fixture survived')
        return code, result

    def test_real_green_red_green(self):
        code, result = self.execute()
        self.assertEqual(code, 0)
        self.assertEqual([p['counts']['failed'] for p in result['phases']], [0, 1, 0])
        self.assertEqual(result['runs'], 3)

    def test_missed_and_compile_failure(self):
        for fault in ('    executed != 0\n', '    this is not Rust\n'):
            with self.subTest(fault=fault):
                code, result = self.execute(FAULT=fault)
                self.assertEqual((code, result['status']), (1, 'not_proven'))
                shutil.rmtree(next(self.root.glob('sensitivity-*')))

    def test_zero_tests(self):
        code, result = self.execute(TEST='nonexistent_canary_test')
        self.assertEqual((code, result['status']), (1, 'not_proven'))

    def test_interruption_after_real_baseline(self):
        main = test_evidence.main
        def interrupt():
            code = main()
            signal.raise_signal(signal.SIGTERM)
            return code
        with patch.object(test_evidence, 'main', side_effect=interrupt):
            code, result = self.execute()
        self.assertEqual((code, result['status']), (1, 'not_proven'))
        self.assertIn('interrupted by signal', result['error'])

    def test_tampered_artifact(self):
        main = test_evidence.main
        def tamper():
            code = main()
            path = Path(os.environ['RUNNER_TEMP']) / 'meisei-test-evidence' / 'test-output.log'
            path.write_bytes(path.read_bytes() + b'changed')
            return code
        with patch.object(test_evidence, 'main', side_effect=tamper):
            code, result = self.execute()
        self.assertEqual((code, result['status']), (1, 'not_proven'))


if __name__ == '__main__':
    unittest.main()
