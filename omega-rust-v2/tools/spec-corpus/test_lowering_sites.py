import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from lowering_origins import SOURCE
from lowering_sites import HEADER, LIMITS, finish, parse, summary, verify

PATH = SOURCE + 'machines/fixed_arrays.rs'
LITERAL = f'{PATH}\t57\t24\tconstructor\tLoweringError::new\tliteral\tbad%09value'
FORWARD = f'{PATH}\t60\t5\tforwarder-body\tLoweringError::new\ttemplate\t{{reason}}'
WRAPPER = f'{PATH}\t62\t9\twrapper\tinner::unsupported_node\tliteral\tbad'


def tsv(*rows, footer=None):
    return '\n'.join([*HEADER, *rows, footer or f'#end\tsites={len(rows)}\tfiles=2']) + '\n'


def fixture(line, col, state='observed', message='bad value', path=PATH):
    return {'target': 'tests/owned_arrays.rs', 'test': 'refusal', 'origin_status': state,
            'reported_compiler_site': {'path': path, 'line': line, 'column': col},
            'location': {'file': 'arrays.omg'}, 'outcome': f'refused: {message} at arrays.omg'}


class LoweringSiteTests(unittest.TestCase):
    def test_exact_runtime_origin_not_nearest_line_message_or_wrapper_body(self):
        inv = parse(tsv(LITERAL, FORWARD, WRAPPER))
        report = {'measurement_errors': [], 'package_fixtures': [
            fixture(57, 24), fixture(62, 9, message='bad: Statement'),
            fixture(57, 25), fixture(57, 24, message='another literal'), fixture(60, 5),
            fixture(57, 24, path=SOURCE + 'other.rs'),
            fixture(57, 24, state='not a unique passing capture'), fixture(62, 9, state='malformed capture')]}
        verify(inv, report)
        self.assertEqual(inv['observed_candidates'], 2)
        self.assertEqual(inv['counts']['origin_candidates'], 2)
        self.assertEqual(inv['counts']['explicit_constructors'], 2)
        self.assertEqual(inv['counts']['wrapper_calls'], 1)
        self.assertEqual(inv['counts']['forwarder_bodies'], 1)
        self.assertEqual(inv['counts']['constructor_arguments'], {'literal': 1, 'template': 1, 'dynamic': 0})
        self.assertEqual([len(r.get('observations', [])) for r in inv['rows']], [1, 0, 1])
        self.assertEqual(len(report['measurement_errors']), 4)
        self.assertIn('2 of 2', '\n'.join(summary(report)))
        self.assertIn(LIMITS, summary(report))

    def test_corrupt_scan_does_not_publish_counts_or_credit(self):
        for text in [tsv(FORWARD), tsv(WRAPPER), tsv(FORWARD, FORWARD.replace('60\t5', '61\t5'), WRAPPER),
                     tsv(LITERAL, LITERAL), tsv(WRAPPER, LITERAL), tsv(LITERAL, footer='#end\tsites=3\tfiles=2'),
                     tsv(LITERAL.replace('57\t24', '57\t0')), tsv(LITERAL.replace('machines/', '../src/')),
                     tsv(LITERAL.replace('%09', '%zz')), tsv(FORWARD.replace('LoweringError::new', 'bad'), WRAPPER),
                     tsv(LITERAL).replace('files=2', 'files=0'), tsv(LITERAL).replace('v1', 'v0'),
                     tsv(LITERAL.replace('literal', 'pending')), '\n'.join([*HEADER, LITERAL])]:
            with self.subTest(text=text):
                inv = parse(text)
                report = {'measurement_errors': [], 'package_fixtures': [fixture(57, 24)]}
                verify(inv, report)
                self.assertIsNone(inv['counts'])
                self.assertNotIn('observed_candidates', inv)
                self.assertTrue(report['measurement_errors'])

    def test_live_artifacts_retained_and_unavailable_not_reported_as_zero(self):
        report = {'last_layer': 'L4', 'measurement_errors': []}
        def run(command, env, stdin, stdout, stderr):
            stdout.write(tsv(LITERAL).encode())
            return subprocess.CompletedProcess(command, 0)
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as name:
            directory = Path(name)
            with patch('lowering_sites.subprocess.run', side_effect=run) as invoke:
                finish(report, directory, {})
                invoke.assert_not_called()
                self.assertEqual(summary(report), [])
                report['last_layer'] = 'L5'
                finish(report, directory, {})
            self.assertEqual((directory / 'lowering-sites.tsv').read_text(), tsv(LITERAL))
            self.assertEqual(report['lowering_sites']['observed_candidates'], 0)
            with patch('lowering_sites.subprocess.run', side_effect=OSError('no scanner')):
                finish(report, directory, {})
            self.assertIsNone(report['lowering_sites']['counts'])
            self.assertIn('unmeasured', '\n'.join(summary(report)))
            self.assertIn('no scanner', report['measurement_errors'][0])


if __name__ == '__main__':
    unittest.main()
