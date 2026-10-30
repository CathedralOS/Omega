"""Check counter accounting, including the constructor's first-check trap."""
from pathlib import Path
from tempfile import TemporaryDirectory
from cache_diagnostics_run import parse

def fixture(folder,metrics):
    rows=[]
    for index in range(418):
        rows.append('SOURCE_QUERY_BEGIN\tThreadId(1)')
        if index<173:
            suffix=''.join(f'\t{key}={value}' for key,value in metrics.items()) if index==0 else ''
            rows.append('REBIND\tquery\tThreadId(1)\t0'+suffix)
        rows.append('SOURCE_QUERY_END\tThreadId(1)')
    (folder/'stderr.txt').write_text('\n'.join(rows))

with TemporaryDirectory() as directory:
    folder=Path(directory)
    metrics=dict(requests=16,cold_entries=4,same_allocation_checks=10,
        changed_allocation_misses=6,transform_calls=10,equal_contents_misses=3,
        different_contents_misses=3,cold_input_rows=8,same_allocation_input_rows=20)
    fixture(folder,metrics);result=parse(folder,'on','counts')
    assert (result['true_hits'],result['misses'],result['avoided_input_rows'])==(6,10,12)
    metrics=dict(requests=1,cold_entries=1,same_allocation_checks=1,transform_calls=1,
        cold_input_rows=2,same_allocation_input_rows=2)
    fixture(folder,metrics);result=parse(folder,'on','counts')
    assert result['true_hits']==0 and result['misses']==1
    metrics['transform_calls']=0;fixture(folder,metrics)
    try:parse(folder,'on','counts')
    except AssertionError:pass
    else:raise AssertionError('inconsistent transform accounting was accepted')
print('Counter parser rejects inconsistent accounting and does not count cold checks as reuse.')
