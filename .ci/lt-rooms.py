#!/usr/bin/env python3
"""Manual heavy-suite driver boundary. Missing real integration is exit 69."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
PLAN = ROOT / 'docs/integration/lt-rooms.json'


def blocked(reason):
    print(json.dumps({'suite': 'LT-ROOMS', 'status': 'blocked', 'reason': reason}))
    return 69


def main():
    plan = json.loads(PLAN.read_text())
    if sys.argv[1:] == ['--validate-contract']:
        assert [case['id'] for case in plan['cases']] == list(range(1, 7))
        assert plan['transport'] == 'real-tor' and plan['executor'] == 'crow'
        assert plan['production_group_cap'] == 100
        print('LT-ROOMS contract is well formed; no workload was executed')
        return 0
    if sys.argv[1:]:
        return blocked('unknown runner arguments')
    if os.environ.get('LT_ROOMS_EXECUTOR') != 'crow':
        return blocked('expensive room workloads require Crow scheduling')
    driver = os.environ.get('LT_ROOMS_DRIVER')
    expected = os.environ.get('LT_ROOMS_DRIVER_SHA256')
    output = os.environ.get('LT_ROOMS_OUTPUT')
    if not driver or not expected or not output:
        return blocked('real cmsg/OpenMLS/Link/Tor/Valkey driver artifact and evidence directory are required')
    driver = Path(driver)
    if not driver.is_absolute() or not driver.is_file():
        return blocked('driver artifact is not an existing absolute file')
    if len(expected) != 64 or hashlib.sha256(driver.read_bytes()).hexdigest() != expected:
        return blocked('driver artifact identity mismatch')
    output = Path(output)
    if not output.is_absolute() or output.exists():
        return blocked('evidence must use a new absolute job-owned directory')
    output.mkdir(parents=True)
    # Driver owns actual cmsg registry calls, proofs, network, measurements and
    # assertions. This launcher cannot turn JSON formatting into acceptance.
    result = subprocess.run([str(driver), '--plan', str(PLAN), '--output', str(output)],
                            check=False, timeout=3300)
    if result.returncode:
        return result.returncode
    receipt_path = output / 'acceptance.json'
    if not receipt_path.is_file():
        return blocked('driver supplied no conjunctive acceptance receipt')
    receipt = json.loads(receipt_path.read_text())
    if receipt.get('suite') != 'LT-ROOMS' or receipt.get('status') != 'passed':
        return blocked('driver did not pass the complete suite')
    if receipt.get('cases') != [1, 2, 3, 4, 5, 6]:
        return blocked('all six real cases are required without skips')
    artifacts = receipt.get('artifacts', {})
    for name in plan['required_artifacts']:
        evidence = artifacts.get(name, {})
        path = Path(evidence.get('path', ''))
        if not path.is_absolute() or not path.is_relative_to(output) or not path.is_file():
            return blocked('a required driver evidence artifact is absent')
        if hashlib.sha256(path.read_bytes()).hexdigest() != evidence.get('sha256'):
            return blocked('driver evidence identity mismatch')
    print(json.dumps({'suite': 'LT-ROOMS', 'status': 'driver-passed',
                      'receipt': str(receipt_path), 'review_required': True}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
