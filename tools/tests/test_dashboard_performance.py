"""Historical sizes stay bound to source; incomplete evidence never becomes speedup."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import dashboard_performance as performance


def record(revision='a' * 40, milliseconds=10):
    return {'schema': 'omega-benchmark-record/1', 'recorded_utc': '2026-01-01T00:00:00Z',
            'subject': {'name': 'probe', 'root': 'bench/probe/main.omg', 'source_revision': revision},
            'key': {'target': 'windows_x86_64', 'selection': {'enabled': [], 'disabled': []}},
            'host': {'os': 'windows', 'machine': 'x86_64', 'cpu': 'CPU', 'logical_cpus': 8,
                     'rustc': 'rustc1', 'omega_profile': 'release', 'omega_binary': 'omega.exe',
                     'omega_binary_sha256': 'b' * 64},
            'metrics': {'compile_time_ms': {'status': 'measured', 'samples': [20, 30]},
                        'runtime_ms': {'status': 'measured', 'samples': [milliseconds, milliseconds],
                                       'exit_codes': [0, 0], 'exit_code_expected': 0, 'exit_code_match': True},
                        'peak_memory_bytes': {'status': 'measured', 'compile_max_rss': 1024, 'run_max_rss': 512},
                        'code_size_bytes': {'status': 'measured', 'value': 256, 'stable': True}},
            'measurement': {'protocol': 'omega-benchmark-process-v1/fresh-build-dir',
                            'source_clean': True, 'dependency_lock_stable': True, 'input_sha256': 'c' * 64, 'dependency_lock_sha256': 'c' * 64, 'machine_id': 'd' * 64,
                            'runtime_scope': 'whole-process-including-startup', 'work_units': 100,
                            'compiler_profile': {'schema': 'omega-compiler-build-profile-v1', 'opt_level': '3'}}}


class PerformanceTests(unittest.TestCase):
    def row(self, source, identity):
        return performance.flatten(source, identity, 'record.json', 'r' * 40,
                                   {'sloc': 100, 'sha256': 'e' * 64})

    def test_code_lines_ignore_comments_and_preserve_literal_comment_markers(self):
        text = '// ignored\n\nlet a = "// code"; // tail\n/* outside\n /* nested */\n*/ let b = 1;\n'
        self.assertEqual(performance.source_lines(text), 2)
        self.assertEqual(performance.source_lines('/* only */'), 0)
        self.assertEqual(performance.source_lines('value // no final newline'), 1)

    def test_duration_uses_all_samples_and_refuses_invalid_measurements(self):
        self.assertEqual(performance.duration({'status': 'measured', 'samples': [8, 2, 5]})['median'], 5)
        for value in (float('nan'), float('inf'), -1, 0, True):
            with self.subTest(value=value), self.assertRaises(ValueError):
                performance.duration({'status': 'measured', 'samples': [value]})
        self.assertIsNone(performance.duration({'status': 'unavailable'}))

    def test_missing_size_keeps_raw_times_and_unknown_memory_is_not_zero(self):
        source = record()
        source['observation_warnings'] = ['Preparation timed out']
        source['metrics']['package_preparation_ms'] = {'status': 'measured', 'samples': [50]}
        source['metrics']['peak_memory_bytes'] = {'status': 'unavailable', 'reason': 'unsupported'}
        row = performance.flatten(source, 'id', 'record.json', 'r', {'sloc': None, 'error': 'missing'})
        self.assertEqual(row['compile']['median'], 25)
        self.assertEqual(row['preparation']['median'], 50)
        self.assertIsNone(row['compile_ms_kloc'])
        self.assertIsNone(row['compile_memory'])
        self.assertEqual(row['observation_warnings'], ['Preparation timed out'])

    def test_ratios_require_same_source_host_input_profile_and_verified_exit(self):
        base_source, current_source = record(milliseconds=20), record(milliseconds=10)
        current_source['recorded_utc'] = '2026-01-02T00:00:00Z'
        base, current = self.row(base_source, 'base'), self.row(current_source, 'current')
        performance.pair_baselines([base, current])
        self.assertEqual(current['baseline_ratio'], 2)
        self.assertEqual(current['baseline_id'], 'base')
        self.assertIsNone(base['baseline_ratio'])
        for change in ('host', 'source', 'input', 'profile', 'dirty', 'oracle', 'legacy', 'units', 'lock'):
            candidate = copy.deepcopy(current_source)
            if change == 'host': candidate['measurement']['machine_id'] = 'f' * 64
            elif change == 'source': candidate['subject']['source_revision'] = 'f' * 40
            elif change == 'input': candidate['measurement']['input_sha256'] = 'f' * 64
            elif change == 'profile': candidate['measurement']['compiler_profile']['opt_level'] = '1'
            elif change == 'dirty': candidate['measurement']['source_clean'] = False
            elif change == 'oracle': candidate['metrics']['runtime_ms']['exit_code_expected'] = None
            elif change == 'legacy': candidate.pop('measurement')
            elif change == 'lock': candidate['measurement']['dependency_lock_sha256'] = 'f' * 64
            elif change == 'units': candidate['measurement']['work_units'] = 200
            row = self.row(candidate, 'candidate')
            performance.pair_baselines([self.row(base_source, 'base'), row])
            self.assertIsNone(row['baseline_ratio'], change)

    def test_git_history_preserves_rows_and_uses_recorded_source_not_current_checkout(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            def git(*args):
                return subprocess.check_output(['git', '-C', str(root), *args], stderr=subprocess.DEVNULL).decode().strip()
            git('init')
            project = root / 'bench/probe'
            project.mkdir(parents=True)
            (project / 'main.omg').write_text('// comment\nmachine main() {}\n')
            (project / 'build.omg').write_text('machine build() {}\n')
            library = root / 'source/library'
            library.mkdir(parents=True)
            (library / 'other.omg').write_text('machine excluded() {}\n')
            git('add', '.')
            git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '-m', 'sources')
            revision = git('rev-parse', 'HEAD')
            directory = root / 'tools/benchmark/records'
            directory.mkdir(parents=True)
            path = directory / 'probe.json'
            first = record(revision, 20)
            path.write_text(json.dumps(first))
            git('add', '.'); git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '-m', 'baseline')
            second = record(revision, 10)
            second['recorded_utc'] = '2026-01-02T00:00:00Z'
            path.write_text(json.dumps(second))
            git('add', '.'); git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '-m', 'candidate')
            (project / 'main.omg').write_text('\n'.join(['machine changed() {}'] * 20))
            warnings = []
            data = performance.collect_performance(root, warnings)
            self.assertEqual(warnings, [])
            self.assertEqual(len(data['records']), 2)
            self.assertTrue(all(row['project']['sloc'] == 2 for row in data['records']))
            self.assertEqual(data['records'][-1]['baseline_ratio'], 2)
            self.assertEqual(len(data['records'][-1]['project']['files']), 2)
            second['measurement']['source_clean'] = False
            path.write_text(json.dumps(second))
            pending = performance.collect_performance(root, [])
            dirty = next(row for row in pending['records'] if row['measurement']['source_clean'] is False)
            self.assertIsNone(dirty['record_revision'])
            self.assertIsNone(dirty['compile_ms_kloc'])
            self.assertEqual(dirty['compile']['median'], 25)
            path.write_text('{bad json')
            warnings = []
            recovered = performance.collect_performance(root, warnings)
            self.assertEqual(len(recovered['records']), 2)
            self.assertTrue(warnings)
            self.assertIn('working-tree', recovered['errors'][0])


if __name__ == '__main__':
    unittest.main()
