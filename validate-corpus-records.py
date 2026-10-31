import hashlib, json
from pathlib import Path
root = Path.cwd()
def load(name):
    return json.loads((root / 'build/call-rank' / (name + '.json')).read_text())
new = load('corpus-new-source')
assert new['counts']['fixtures'] == 24 and new['counts']['by_status'] == {'checked':11, 'rejected':13}
assert new['counts']['wrong_reason'] == new['counts']['fail_not_rejected'] == new['counts']['lost_successes'] == 0
for name, count in [('corpus-new',11), ('corpus-neighbors-both',31)]:
    record = load(name)
    assert record['counts']['fixtures'] == count and record['counts']['by_status'] == {'built':count}
    assert not record['oracle_failures'] and not record['infrastructure_error']
    for row in record['observations']:
        assert row['facts']['exit'] == row['facts']['interpreter-exit'] == '0'
        assert row['facts']['interpreter'] == 'agrees'
    assert record['counts']['lost_successes'] == 0
neighbors = load('corpus-neighbors-source')
assert neighbors['counts']['fixtures'] == 93 and neighbors['counts']['wrong_reason'] == 0
assert neighbors['counts']['fail_not_rejected_fixtures'] == ['fail/core/scoped_integer_call_guarantee_requires_published_equality']
baseline = load('neighbors-baseline-attribution')
assert len(baseline['results']) == 8
assert hashlib.sha256(Path(baseline['producer']['executable']).read_bytes()).hexdigest() == baseline['producer']['sha256']
current = {row['fixture']:row for row in neighbors['observations']}
for row in baseline['results']:
    source = root / 'tests/omega' / row['fixture'] / 'main.omg'
    assert hashlib.sha256(source.read_bytes()).hexdigest() == row['source_sha256']
    diagnostics = [line[len('error: '):].replace(str(root) + '/', '') for line in row['stderr'].splitlines() if line.startswith('error: ')]
    assert diagnostics == current[row['fixture']]['diagnostics']
    assert (row['exit'] == 0) == (current[row['fixture']]['status'] == 'checked')
print('24 new source outcomes; 11 new and 31 neighboring BOTH native0/interpreter0. Eight source exceptions exactly reproduce frozen C6 diagnostics/status. Performance-only movement is not runtime failure or a performance acceptance claim.')
