#!/usr/bin/env python3
"""Stage exact sample sources for the repository's native+Terminal stream gate.

Only this script's temporary fixture group is created/removed. Checked-tree
interpretation is never requested. The corpus runner must be built from the
same compiler source revision and use this checkout's bundled library.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent
REPOSITORY = next(parent for parent in ROOT.parents if (parent / 'AGENTS.md').exists())
GROUP = REPOSITORY / 'tests/omega/run/ind19_spectrometer_review'
SOURCE_GROUP = REPOSITORY / 'tests/omega/fail/ind19_spectrometer_review'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--runner', required=True)
    parser.add_argument('--case', action='append', help='Run only these frozen runtime cases')
    parser.add_argument('--source-control-only', action='store_true',
                        help='Check the separate complete intended-invalid project')
    arguments = parser.parse_args()
    oracle_bytes = (ROOT / 'data/oracle.json').read_bytes()
    oracle_digest = hashlib.sha256(oracle_bytes).hexdigest()
    assert oracle_digest == (ROOT / 'data/oracle.sha256').read_text().split()[0], 'oracle changed'
    oracle = json.loads(oracle_bytes)
    boundary_bytes = (ROOT / 'data/boundary_oracle.json').read_bytes()
    boundary_digest = hashlib.sha256(boundary_bytes).hexdigest()
    assert boundary_digest == (ROOT / 'data/boundary_oracle.sha256').read_text().split()[0], 'boundary oracle changed'
    boundaries = json.loads(boundary_bytes)['cases']
    assert len(boundaries) == 3 and not (oracle['cases'].keys() & boundaries.keys())
    oracle['cases'] = {**oracle['cases'], **boundaries}
    assert not (arguments.case and arguments.source_control_only), 'Choose runtime cases or the source control'
    cases = oracle['cases'] if arguments.case is None else {name: oracle['cases'][name] for name in arguments.case}
    group = SOURCE_GROUP if arguments.source_control_only else GROUP
    assert not group.exists(), f'Preserve existing fixture group: {group}'
    evidence = ROOT / 'build' / ('source_control' if arguments.source_control_only else 'corpus')
    evidence.mkdir(parents=True, exist_ok=True)
    try:
        for name, expected in ([] if arguments.source_control_only else cases.items()):
            for suffix, key in [('input', 'input_sha256'), ('expected', 'output_sha256')]:
                assert hashlib.sha256((ROOT / 'data' / f'{name}.{suffix}').read_bytes()).hexdigest() == expected[key]
            fixture = group / name
            fixture.mkdir(parents=True)
            shutil.copyfile(ROOT / 'main.omg', fixture / 'main.omg')
            shutil.copytree(ROOT / 'spectrometer', fixture / 'spectrometer')
            build = (ROOT / 'build.omg').read_text().replace('../../../../source/library/std', '../../../../../source/library/std')
            (fixture / 'build.omg').write_text(build)
            shutil.copyfile(ROOT / 'data' / f'{name}.input', fixture / 'input.txt')
            shutil.copyfile(ROOT / 'data' / f'{name}.expected', fixture / 'expected_stdout.txt')
            (fixture / 'expected_stderr.txt').write_text('')
            (fixture / 'expected_native_exit.txt').write_text(str(expected['exit'])+'\n')
        if arguments.source_control_only:
            fixture = group / 'intended_overflow'
            shutil.copytree(ROOT / 'intended_invalid', fixture)
        environment = os.environ.copy()
        environment.update(OMEGA_CORPUS_JOBS='1', OMEGA_CORPUS_FIXTURE_SECS='600', OMEGA_CORPUS_NATIVE='1',
                           OMEGA_CORPUS_NATIVE_TARGET='linux_x86_64', OMEGA_CORPUS_INTERPRET='1',
                           OMEGA_CORPUS_FIXTURE_FILTER=('fail/' if arguments.source_control_only else 'run/')+'ind19_spectrometer_review/',
                           OMEGA_LIBRARY_ROOT=str(REPOSITORY / 'source/library'))
        if arguments.source_control_only:
            for key in ('OMEGA_CORPUS_NATIVE', 'OMEGA_CORPUS_NATIVE_TARGET', 'OMEGA_CORPUS_INTERPRET'):
                environment.pop(key, None)
        environment.pop('OMEGA_CORPUS_CHECKED_INTERPRETER', None)
        with (evidence / 'records.txt').open('wb') as output, (evidence / 'stderr.txt').open('wb') as errors:
            process = subprocess.run([str(Path(arguments.runner).resolve())], cwd=REPOSITORY,
                                     env=environment, stdout=output, stderr=errors, timeout=600 * (1 if arguments.source_control_only else len(cases)) + 120)
        records = (evidence / 'records.txt').read_text()
        rows = [row for row in records.splitlines() if row.startswith(('fail/' if arguments.source_control_only else 'run/')+'ind19_spectrometer_review/')]
        assert process.returncode == 0, (evidence / 'stderr.txt').read_text()
        assert len(rows) == (1 if arguments.source_control_only else len(cases)), (len(rows), records)
        for row in rows:
            if arguments.source_control_only:
                assert 'rejected' in row.split() and 'expected' in row.split(), row
                assert (ROOT / 'intended_invalid/expected.txt').read_text().strip() in records, records
                continue
            for fact in ('stdout:match', 'stderr:match', 'interpreter-stdout:match', 'interpreter-stderr:match', 'interpreter:agrees'):
                assert fact in row.split(), (fact, row)
            assert 'interpreter-engine:checked-tree' not in row
        summary = dict(cases=len(rows), runner=str(Path(arguments.runner).resolve()),
                       oracle_sha256=oracle_digest,
                       boundary_oracle_sha256=boundary_digest,
                       runner_sha256=hashlib.sha256(Path(arguments.runner).read_bytes()).hexdigest(),
                       jobs=1, mode='source-rejection' if arguments.source_control_only else 'native-and-Terminal',
                       records_sha256=hashlib.sha256(records.encode()).hexdigest())
        (evidence / 'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
        print(json.dumps(summary, indent=2))
    finally:
        if group.exists():
            shutil.rmtree(group)

if __name__ == '__main__':
    main()
