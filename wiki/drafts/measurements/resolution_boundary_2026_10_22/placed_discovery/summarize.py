"""Verify preserved mode and report the bypass's semantic input differences."""
from pathlib import Path
from collections import Counter
import hashlib
import json
import sys

sys.path.insert(0,str(Path('tools/benchmark').resolve()))
from benchmark import TIMING_LINE

output=Path('build/placed-discovery')
observations=json.loads((output/'observations.json').read_text())
records=observations['records']
assert len(records)==42 and all(observations['metadata']['checks'].values())

def clean(text):
    return '\n'.join(line for line in text.splitlines()
                     if not line.startswith(('RESMEASURE\t','PLACEDMEASURE\t')) and not TIMING_LINE.match(line))

def statistics(row):
    fields={}
    for line in row['placement_trace']:
        kind=line.split('\t')[1]
        if kind in ('input','output','applications'):
            fields[kind]=dict((key,int(value)) for key,value in
                              (field.split('=') for field in line.split('\t')[2].split()))
    fields['invocation_counts']=[line.split('\t')[-1].count('SourceSpan {')
                                for line in row['placement_trace'] if line.startswith('PLACEDMEASURE\tapplication\t')]
    if 'trace' in row:
        events=row['trace']['events']
        fields['resolution_counts']=dict(Counter(event['label'] for event in events
                                                  if event['label'].startswith('resolver.resolve')))
        fields['expression_table_clones']=sum(event['label']=='resolved.expressions.clone' for event in events)
        fields['live_rows_cloned']=sum(event['fields']['expressions'] for event in events
                                      if event['label']=='resolved.expressions.clone')
    return fields

summary={}
for name in dict.fromkeys(row['name'] for row in records):
    selected=[row for row in records if row['name']==name]
    baseline=next(row for row in selected if row['mode']=='baseline')
    full=next(row for row in selected if row['mode']=='full')
    skip=next(row for row in selected if row['mode']=='skip')
    assert baseline['exit']==full['exit'],name
    if baseline['kind']=='positive':
        assert all(row['exit']==0 and row['native_exit']==0 for row in selected),name
        assert len({row['native_sha256'] for row in selected})==1,name
    else:
        assert baseline['exit']>0
        assert clean(baseline['output'])==clean(full['output']),name
    if baseline['kind']=='invalid':
        fragments=[line for line in (Path(baseline['subject']).parent/'expected.txt').read_text().splitlines() if line]
        assert all(fragment in baseline['output'] for fragment in fragments),(name,fragments)
        assert clean(baseline['output'])==clean(skip['output']),name
    repeatable={}
    for mode in ('full','skip'):
        repetitions=[row for row in selected if row['mode']==mode]
        if len(repetitions)==2:
            counts=[{label:(entry['calls'],entry['fields']) for label,entry in row['trace']['summary'].items()}
                    for row in repetitions]
            assert counts[0]==counts[1],(name,mode)
            repeatable[mode]=True
    summary[name]=dict(kind=baseline['kind'],exits={row['mode']:row['exit'] for row in selected},
                      full=statistics(full),skip=statistics(skip),counts_repeat=repeatable,
                      failure_diagnostic_preserved_by_skip=(None if baseline['exit']==0 else clean(baseline['output'])==clean(skip['output'])),
                      wall_ms={mode:[row['wall_ms'] for row in selected if row['mode']==mode] for mode in ('baseline','full','skip')})

for source,digest in observations['metadata']['inputs'].items():
    assert hashlib.sha256(Path(source).read_bytes()).hexdigest()==digest,source
controls=json.loads((output/'controls.json').read_text())
assert len(controls)==6 and all(row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'] for row in controls)
summary['checks']=dict(observations=42,native_executions=21,native_images_identical_per_subject=True,
                       preserved_mode_matches_baseline=True,intended_invalid_diagnostics_preserved=True,
                       original_workload_counts_repeat=True,both_engine_observations=6,
                       inputs_unchanged=True)
(output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
