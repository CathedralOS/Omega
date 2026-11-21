import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from lowering_origins import SOURCE
from lowering_sites import HEADER, LIMITS, METHODS, finish, parse, summary, verify

PATH = SOURCE + 'machines/fixed_arrays.rs'
LITERAL = f'{PATH}\t57\t24\tconstructor\tLoweringError::new\tliteral\tbad%09value'
FORWARD = f'{PATH}\t60\t5\tforwarder-body\tLoweringError::new\ttemplate\t{{reason}}'
WRAPPER = f'{PATH}\t62\t9\twrapper\tinner::unsupported_node\tliteral\tbad'
CALL = f'{PATH}\t11\t47\trefuse'
# integration/scanner-parent: `Steps::refuse(&self)` constructs at 10:41, but rustc sends the
# `self.refuse()` at 11:47 in a `&mut self` method to an imported blanket trait impl instead.
PARENT = SOURCE + 'main.rs'
PARENT_BODY = f'{PARENT}\t10\t41\tconstructor\tLoweringError::new\tliteral\tinherent'
PARENT_CALL = f'{PARENT}\t11\t47\trefuse'


def tsv(*rows, calls=(), footer=None):
    footer = footer or f'#end\tsites={len(rows)}\tfiles=2\tself_method_calls={len(calls)}'
    return '\n'.join([*HEADER, *rows, *METHODS, *calls, footer]) + '\n'


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

    def test_a_self_method_call_is_neither_a_candidate_nor_a_forwarder(self):
        # Nothing refused, so the listed call earns nothing and the inherent constructor stays counted.
        inv = parse(tsv(PARENT_BODY, calls=[PARENT_CALL]))
        report = {'measurement_errors': [], 'package_fixtures': []}
        verify(inv, report)
        self.assertEqual([r['role'] for r in inv['rows']], ['constructor'])
        self.assertEqual((inv['counts']['origin_candidates'], inv['counts']['self_method_calls']), (1, 1))
        self.assertEqual((inv['observed_candidates'], inv['runtime_confirmed_method_calls']), (0, 0))
        self.assertNotIn('observations', inv['self_method_calls'][0])
        self.assertEqual(report['measurement_errors'], [])
        self.assertIn('0 of 1** candidate', '\n'.join(summary(report)))
        self.assertIn('**0** of the 1 self method calls', '\n'.join(summary(report)))

    def test_only_an_observed_refusal_at_the_listed_position_confirms_a_method_call(self):
        sibling = f'{PATH}\t11\t60\trefuse'  # another call on the same line
        unrelated = f'{PATH}\t40\t14\trefuse'  # same spelling, never refused
        other_file = f'{SOURCE}other.rs\t11\t47\trefuse'
        inv = parse(tsv(LITERAL, calls=[CALL, sibling, unrelated, other_file]))
        report = {'measurement_errors': [], 'package_fixtures': [
            fixture(11, 47, message='dynamic reason'), fixture(11, 47, message='second witness'),
            fixture(11, 47, state='not a unique passing capture'), fixture(40, 14, state='indistinguishable repeated receipt'),
            fixture(11, 48), fixture(12, 47), fixture(11, 47, path=SOURCE + 'third.rs')]}
        verify(inv, report)
        self.assertEqual([len(c.get('observations', [])) for c in inv['self_method_calls']], [2, 0, 0, 0])
        self.assertEqual(inv['runtime_confirmed_method_calls'], 1)
        # Static coverage is untouched: the literal constructor is still the only candidate and unobserved.
        self.assertEqual((inv['observed_candidates'], inv['counts']['origin_candidates']), (0, 1))
        self.assertEqual([r['role'] for r in inv['rows']], ['constructor'])
        # A moved column, a moved line and an unlisted file are still unmatched origins.
        self.assertEqual(len(report['measurement_errors']), 3)
        self.assertTrue(all('no candidate at' in e for e in report['measurement_errors']))
        self.assertIn('**1** of the 4 self method calls', '\n'.join(summary(report)))

    def test_a_missing_or_moved_call_position_does_not_confirm_the_origin(self):
        for calls in ([], [CALL.replace('11\t47', '12\t47')], [CALL.replace('11\t47', '11\t46')],
                      [CALL.replace(PATH, SOURCE + 'sibling.rs')]):
            with self.subTest(calls=calls):
                inv = parse(tsv(LITERAL, calls=calls))
                report = {'measurement_errors': [], 'package_fixtures': [fixture(11, 47)]}
                verify(inv, report)
                self.assertEqual(inv['runtime_confirmed_method_calls'], 0)
                self.assertEqual(len(report['measurement_errors']), 1)
                self.assertIn('no candidate at ' + PATH + ':11:47', report['measurement_errors'][0])

    def test_corrupt_scan_does_not_publish_counts_or_credit(self):
        for text in [tsv(FORWARD), tsv(WRAPPER), tsv(FORWARD, FORWARD.replace('60\t5', '61\t5'), WRAPPER),
                     tsv(LITERAL, LITERAL), tsv(WRAPPER, LITERAL), tsv(LITERAL, footer='#end\tsites=3\tfiles=2\tself_method_calls=0'),
                     tsv(LITERAL.replace('57\t24', '57\t0')), tsv(LITERAL.replace('machines/', '../src/')),
                     tsv(LITERAL.replace('%09', '%zz')), tsv(FORWARD.replace('LoweringError::new', 'bad'), WRAPPER),
                     tsv(LITERAL).replace('files=2', 'files=0'), tsv(LITERAL).replace('v2', 'v1'),
                     tsv(LITERAL.replace('literal', 'pending')), '\n'.join([*HEADER, LITERAL]),
                     # The method section is required, counted and validated like the candidates.
                     tsv(LITERAL).replace(METHODS[0] + '\n', ''), tsv(LITERAL).replace('\tself_method_calls=0', ''),
                     tsv(LITERAL, calls=[CALL], footer='#end\tsites=1\tfiles=2\tself_method_calls=0'),
                     tsv(LITERAL, calls=[CALL.replace('refuse', 'self.refuse')]), tsv(LITERAL, calls=[CALL, CALL]),
                     tsv(LITERAL, calls=[CALL.replace('11\t47', '12\t9'), CALL]), tsv(LITERAL, calls=[CALL.replace('11\t47', '0\t47')]),
                     tsv(LITERAL, calls=[CALL.replace(PATH, SOURCE + '../outside.rs')]),
                     tsv(LITERAL, calls=[CALL.replace('11\t47', '57\t24')])]:
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
