from pathlib import Path
path=Path('build/remaining-first-checking/quality.py')
source=path.read_text()
gate='''timing_rows=json.loads((base/'timings.json').read_text())
times={variant:[row['seconds'] for row in timing_rows if row['variant']==variant and row['command'][1:3]==['update','--project']] for variant in ['A','B']}
assert all(len(values)==2 for values in times.values()),times
means={variant:sum(values)/len(values) for variant,values in times.items()}
reduction=1-means['B']/means['A']
screen_passed=reduction>=0.10 and max(times['B'])<min(times['A'])
(base/'performance-screen.json').write_text(json.dumps(dict(times=times,means=means,reduction=reduction,separated_ranges=max(times['B'])<min(times['A']),passed=screen_passed),indent=2))
if not screen_passed:
    print('Performance screen failed: only run the focused formation pair before removing the candidate; broader suite skipped.',flush=True)
    fixtures=['pass/calls/specification_result_without_guarantees_requires_formation','fail/calls/specification_result_without_guarantees_requires_formation']
'''
source=source.replace('for fixture in fixtures:',gate+'for fixture in fixtures:')
path.write_text(source,encoding='utf-8',newline='\n')
