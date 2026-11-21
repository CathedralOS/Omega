"""Only a located fixture return in the capture of its passing L5 test is origin evidence."""
import unittest

from check_layer_readiness import collect, readiness_exit
from lowering_origins import SOURCE, summary
from refusal_evidence import observed_refusals
from test_l5_records import libtest

NAME = 'an_ordinal_outside_the_declared_extent_rejects'
TARGET = r'tests\owned_arrays.rs'
OLD = (f'OMEGA_FIXTURE\t{NAME}\tarrays.omg\tMove=1,VerifyLibraryImplementation=1\t'
       'refused: owned element transfer selects outside its fixed array at arrays.omg\tapp\tarrays.omg\t73\t74\t1\t74\t2')
SITE = SOURCE + 'machines/fixed_arrays.rs'
ROW = OLD + f'\t{SITE}\t57\t24'
PSI = OLD.split('refused: ')[0] + 'produced Psi'


def block(*lines, status='ok', target=TARGET, name=NAME):
    return libtest(target, (name, status, lines))


def replay(log, stage='L5', required=False):
    report = {'cases': {}, 'package_fixtures': [], 'suites': {}, 'measurement_errors': []}
    tests, counts = collect(log, stage, report, require_lowering_origins=required)
    report['suites'][stage] = {'tests': tests, 'counts': counts, 'exit': int(bool(counts.get('failed')))}
    return report


class LoweringOriginTests(unittest.TestCase):
    def test_subset_is_observational_not_a_rule_roster(self):
        # Two possible branch origins and one repeated origin at a different occurrence. Same public message.
        report = replay(block(ROW, ROW.replace('57\t24', '58\t24'), ROW.replace('73\t74\t1\t74', '74\t75\t1\t75')), required=True)
        ledger = report['lowering_origins']
        self.assertEqual(ledger['dispositions'], {'observed': 3})
        self.assertEqual(ledger['distinct_observed_sites'], 2)
        self.assertIsNone(ledger['compiler_site_denominator'])
        self.assertEqual(readiness_exit(report), 0)
        fixture = report['package_fixtures'][0]
        self.assertEqual(fixture['reported_compiler_site'], {'path': SITE, 'line': 57, 'column': 24})
        self.assertEqual((fixture['location']['file'], fixture['location']['byte_start']), ('arrays.omg', 73))
        self.assertIn('unknown', '\n'.join(summary(report)))

    def test_real_historical_grammars_and_empty_measurement(self):
        for rows, expected in [([OLD], 1), ([OLD.split('\tapp\t')[0]], 1), ([PSI], 0), ([], 0)]:
            with self.subTest(rows=rows):
                report = replay(block(*rows))
                self.assertEqual(report['measurement_errors'], [])
                ledger = report['lowering_origins']
                self.assertEqual(ledger['helper_refusals'], expected)
                self.assertEqual(ledger['dispositions'], {'no origin reported': 1} if expected else {})
                self.assertIsNone(ledger['compiler_site_denominator'])
                self.assertEqual(ledger['distinct_observed_sites'], 0)
        self.assertIn('missing compiler origin', replay(block(OLD), required=True)['measurement_errors'][0])
        self.assertEqual(replay(block(PSI), required=True)['measurement_errors'], [])

    def test_bad_attempt_invalidates_its_capture_not_the_next_one(self):
        for bad in [OLD+'\t', OLD+'\t'+SITE, ROW+'\textra', ROW.replace('/src/', '/tests/'),
                    ROW.replace('machines/', '../src/'), ROW.replace(SITE, 'C:/src/'+SITE),
                    ROW.replace('57\t24', '1\t0'), ROW.replace('57\t24', '０１\t1'),
                    ROW.replace('73\t74\t1\t74', '73\t70\t1\t74'), PSI+'\t'+SITE+'\t1\t1',
                    ROW.replace(NAME, 'borrowed'), 'OMEGA_FIXTURE_ATTEMPT\t'+NAME]:
            with self.subTest(bad=bad):
                report = replay(block(ROW, bad)+block(ROW.replace(NAME, 'another'), name='another'))
                self.assertTrue(report['measurement_errors'])
                self.assertEqual(report['lowering_origins']['malformed_fixture_records'], 1)
                self.assertEqual([f['test'] for f in report['package_fixtures'] if f.get('origin_status') == 'observed'], ['another'])

    def test_duplicate_receipts_are_ambiguous_not_independent_origin_witnesses(self):
        report = replay(block(ROW, ROW))
        self.assertEqual(report['lowering_origins']['dispositions'], {'indistinguishable repeated receipt': 2})
        self.assertEqual(report['lowering_origins']['distinct_observed_sites'], 0)
        # The preexisting registry comparison must still see both, not a silently coalesced count.
        self.assertEqual(list(observed_refusals(report)[0].values()), [2])

    def test_capture_must_have_one_named_pass(self):
        for log in [block(ROW, status='FAILED'), block(ROW).replace(' ... ok', ' ... ignored'),
                    block(ROW).replace(' ... ok', ' ... filtered'), block(ROW)+block(),
                    block(ROW).replace(' ... ok', ' ... filtered').replace(ROW, f'test {NAME} ... ok\n'+ROW),
                    block(ROW).replace(ROW, ROW+f'\n---- {NAME} stdout ----')]:
            with self.subTest(log=log):
                self.assertEqual(replay(log)['lowering_origins']['dispositions'], {'not a unique passing capture': 1})

    def test_only_the_owner_capture_can_report_an_origin(self):
        for stage in ['L0', 'L1', 'L2', 'L3', 'L4', 'CLI']:
            report = replay(block(ROW), stage)
            self.assertEqual(report['package_fixtures'], [])
            self.assertNotIn('lowering_origins', report)
            self.assertTrue(report['measurement_errors'])
        for log in [ROW+'\n'+block(), block(ROW, target='src/lib.rs'), block(ROW, name='other'),
                    block(ROW).replace(' stdout ----', ' stderr ----'), block('---- another stderr ----', ROW),
                    block('arrays.omg:1:74: lowering: owned element transfer selects outside its fixed array (bytes 73..74)')]:
            report = replay(log)
            self.assertEqual(report['lowering_origins']['helper_refusals'], 0)


if __name__ == '__main__':
    unittest.main()
