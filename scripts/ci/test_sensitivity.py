#!/usr/bin/env python3
"""Opt-in fixture sensitivity check using CM-1 capture; it is not CI attestation.

Set RUNNER_TEMP and the CM-1 GITHUB_* identity variables before invoking this file.
Local identity values are synthetic. Each invocation saves raw evidence under RUNNER_TEMP.
"""
import difflib
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import tempfile
import time

import test_evidence as evidence

FIXTURE = Path(__file__).resolve().parent / 'canary_fixture'
ORIGINAL = '    executed > 0\n'
FAULT = '    executed >= 0\n'
TEST = 'rejects_zero_executed'


def interrupted(signum, frame):
    raise InterruptedError(f'interrupted by signal {signum}')


def run():
    output = Path(tempfile.mkdtemp(prefix='sensitivity-', dir=os.environ['RUNNER_TEMP']))
    started = time.monotonic()
    old_command = evidence.COMMAND
    old_temp = os.environ['RUNNER_TEMP']
    old_target = os.environ.get('CARGO_TARGET_DIR')
    old_handlers = {sig: signal.signal(sig, interrupted) for sig in (signal.SIGINT, signal.SIGTERM)}
    result = {'status': 'not_proven', 'head': evidence.git('rev-parse', 'HEAD'), 'phases': [], 'runs': 0,
              'environment': {key: os.environ.get(key) for key in ('CARGO_NET_OFFLINE', 'RUSTFLAGS', 'RUSTUP_TOOLCHAIN')}}
    try:
        with tempfile.TemporaryDirectory(prefix='sensitivity-fixture-') as temporary:
            fixture = Path(temporary) / 'fixture'
            shutil.copytree(FIXTURE, fixture)
            source = fixture / 'src/lib.rs'
            original = source.read_text()
            if original.count(ORIGINAL) != 1:
                raise ValueError('fixture mutation point is not unique')
            mutated = original.replace(ORIGINAL, FAULT)
            result['temporary_diff'] = ''.join(difflib.unified_diff(original.splitlines(True), mutated.splitlines(True), fromfile='src/lib.rs', tofile='src/lib.rs'))
            evidence.COMMAND = ['cargo', 'test', '--no-fail-fast', '--manifest-path', str(fixture / 'Cargo.toml'), '--lib', TEST]
            result['command'] = evidence.COMMAND
            os.environ['CARGO_TARGET_DIR'] = str(Path(temporary) / 'target')
            for phase in ('baseline', 'fault', 'restored'):
                source.write_text(mutated if phase == 'fault' else original)
                phase_dir = output / phase
                phase_dir.mkdir()
                os.environ['RUNNER_TEMP'] = str(phase_dir)
                result['runs'] += 1
                code = evidence.main()
                directory = phase_dir / 'meisei-test-evidence'
                raw = (directory / 'test-output.log').read_bytes()
                record = json.loads((directory / 'summary.json').read_text())
                actual = evidence.counts(raw)
                if record.get('command') != evidence.COMMAND or record.get('identity') != evidence.identity() or record.get('sha256') != hashlib.sha256(raw).hexdigest() or record.get('counts') != actual:
                    raise ValueError(f'{phase}: invalid or stale evidence')
                if phase == 'fault':
                    if code != 1 or record['exit_code'] != 101 or actual['executed'] != 1 or actual['failed'] != 1 or f'test {TEST} ... FAILED'.encode() not in raw or b'assertion failed: !accepts_evidence(0)' not in raw:
                        raise ValueError('fault: expected assertion failure was not observed')
                elif code != 0 or actual['executed'] != 1 or actual['passed'] != 1 or f'test {TEST} ... ok'.encode() not in raw:
                    raise ValueError(f'{phase}: expected baseline pass was not observed')
                result['phases'].append({'name': phase, 'counts': actual, 'sha256': record['sha256'], 'exit_code': record['exit_code']})
        result['status'] = 'proven'
    except (OSError, KeyError, ValueError, InterruptedError) as error:
        result['error'] = str(error)
    finally:
        result['seconds'] = round(time.monotonic() - started, 3)
        (output / 'sensitivity.json').write_text(json.dumps(result, indent=2) + '\n')
        evidence.COMMAND = old_command
        os.environ['RUNNER_TEMP'] = old_temp
        if old_target is None:
            os.environ.pop('CARGO_TARGET_DIR', None)
        else:
            os.environ['CARGO_TARGET_DIR'] = old_target
        for sig, handler in old_handlers.items():
            signal.signal(sig, handler)
    print(f'sensitivity: {result["status"]}; evidence: {output}')
    return 0 if result['status'] == 'proven' else 1


if __name__ == '__main__':
    raise SystemExit(run())
