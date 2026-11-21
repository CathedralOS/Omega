"""Prepared-site frames go through the real L5 fixture reader, not a second log parser."""
import unittest
from zlib import adler32

import lowering_evidence as evidence
from test_lowering_origins import NAME, SITE, block, replay


LOCAL = 'app\tmain.omg\t2\t3\t1\t3\tx'
IMPORTED = 'dep\tmain.omg\t2\t3\t\t\t'
EXACT = ('Move', 'app', 'main.omg', '2', '3')
OTHER_FILE = ('Move', 'app', 'other.omg', '2', '3')
OTHER_OWNER = ('Move', 'dep', 'main.omg', '2', '3')


def receipt(*sites, counts, position=LOCAL, entry='prepared+sites-v1', origin=True):
    prefix = f'OMEGA_FIXTURE\t{NAME}\tother.omg,main.omg\t{counts}\trefused: stopped at main.omg\t{position}'
    if origin:
        prefix += f'\t{SITE}\t57\t24'
    selected = position.split('\t')[-1]
    frame = '\t'.join([entry, 'stopped', selected, 'root', 'stopped',
                       'other.omg', 'πx', 'main.omg', 'πx', *(field for site in sites for field in site)])
    if entry.endswith('+sites-v1'):
        frame += f'\t{adler32((NAME + chr(9) + position + chr(9) + frame).encode()):08x}'
    return prefix + '\treplay-v1\t' + frame


def classify(log, *, required=False, spec_count=0):
    report = replay(log, required=required)
    report['obligations'] = {'Move': {}, 'Borrow': {}}
    if spec_count:
        report['obligations']['Move']['a.md:1'] = spec_count
        report['cases']['spec/a.md:1'] = {'stage': 'L5', 'status': 'open gap'}
        target, test = evidence.SPEC_AGGREGATE
        report['suites']['L5']['tests'].append(dict(target=target, test=test, status='FAILED'))
    errors = report['measurement_errors']
    inputs = evidence.obligation_inputs(report, evidence.test_results(report), errors)
    kinds = evidence.classify_obligations(report, inputs, {}, [], errors)
    return kinds, errors


class PreparedSiteTests(unittest.TestCase):
    def test_repeated_calls_and_repeated_rows_have_different_denominators(self):
        row = receipt(EXACT, EXACT, OTHER_FILE, OTHER_OWNER, ('Borrow', 'app', 'main.omg', '0', '3'),
                      counts='Borrow=1,Move=4')
        elsewhere = receipt(OTHER_FILE, counts='Move=1', origin=False, entry='source+sites-v1')
        kinds, errors = classify(block(row, row, elsewhere), spec_count=2)
        self.assertEqual(errors, [])
        sites = kinds['Move']['refusal_sites']
        self.assertEqual(sites['inputs'], {'exact': 2, 'elsewhere': 1, 'unknown': 1})
        self.assertEqual(sites['occurrences'], {'exact': 4, 'elsewhere': 5, 'unknown': 2})
        self.assertEqual([w['fixture_index'] for w in sites['witnesses']], [None, 0, 1, 2])
        self.assertEqual(sites['witnesses'][0]['site'], 'spec/a.md:1')
        table = '\n'.join(evidence.refusal_site_table(kinds))
        self.assertIn('| Borrow | 0 / 2 / 0 | 0 / 2 / 0 |', table)  # enclosing span isn't exact
        self.assertIn('| Move | 2 / 1 / 1 | 4 / 5 / 2 |', table)
        self.assertIn('does not establish', table)

    def test_imported_rows_use_their_owners_coordinates_not_local_bytes(self):
        row = receipt(EXACT, OTHER_OWNER, ('Move', 'dep', 'absent.omg', '88', '99'),
                      counts='Move=3', position=IMPORTED)
        kinds, errors = classify(block(row))
        self.assertEqual(errors, [])
        self.assertEqual(kinds['Move']['refusal_sites']['occurrences'], {'exact': 1, 'elsewhere': 2, 'unknown': 0})

    def test_empty_frame_is_not_missing_and_keeps_replay_entry_and_sources(self):
        report = replay(block(receipt(counts='', entry='source+sites-v1')))
        self.assertEqual(report['measurement_errors'], [])
        f, = report['package_fixtures']
        self.assertTrue(f['obligation_sites_observed'])
        self.assertEqual(f['replay']['obligation_sites'], [])
        self.assertEqual(f['replay']['entry'], 'source')
        self.assertEqual(f['replay']['files'], [['other.omg', 'πx'], ['main.omg', 'πx']])

    def test_inconsistent_even_if_checksum_matches(self):
        for sites, counts in [([], 'Move=1'), ([EXACT, EXACT], 'Move=1'), ([EXACT], 'Borrow=1'),
                              ([('Move', 'app', 'absent.omg', '0', '0')], 'Move=1'),
                              ([('Move', '', 'main.omg', '2', '3')], 'Move=1'),
                              ([('Move', 'dep', 'invalid%FF', '0', '0')], 'Move=1'),
                              *[([('Move', 'app', 'main.omg', start, end)], 'Move=1')
                                for start, end in [('0', '1'), ('1', '1'), ('2', '4'), ('2', '0'), ('02', '3')]]]:
            with self.subTest(sites=sites, counts=counts):
                kinds, errors = classify(block(receipt(*sites, counts=counts)))
                self.assertTrue(errors)
                self.assertEqual(kinds[counts.split('=')[0]]['refusal_sites']['inputs']['unknown'], 1)

    def test_altered_or_truncated_frame_invalidates_capture(self):
        good = receipt(EXACT, counts='Move=1')
        for bad in (good.rsplit('\t', 1)[0], good.split('Move\tapp')[0].rstrip('\t'),
                    good.replace('Move\tapp', 'Move\tdep'), good.replace('Move\tapp\tmain', 'Move\tapp\tother'),
                    good.replace(LOCAL, IMPORTED), good.replace('πx', 'πy')):
            with self.subTest(bad=bad):
                kinds, errors = classify(block(bad, good))
                self.assertTrue(errors)
                self.assertEqual(kinds['Move']['refusal_sites']['inputs']['unknown'], 2)

    def test_absent_frame_in_old_log_unknown_and_live_reader_flags_absence(self):
        old = receipt(counts='Move=1', entry='source')
        for required in (False, True):
            kinds, errors = classify(block(old), required=required)
            self.assertEqual(bool(errors), required)
            self.assertEqual(kinds['Move']['refusal_sites']['occurrences'], {'exact': 0, 'elsewhere': 0, 'unknown': 1})
        fresh = receipt(EXACT, counts='Move=1')
        for log in (block(fresh, status='FAILED'), block(fresh) + block(fresh)):
            kinds, _ = classify(log)
            self.assertEqual(kinds['Move']['refusal_sites']['inputs']['exact'], 0)


if __name__ == '__main__':
    unittest.main()
