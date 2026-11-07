"""Check independently executed DESeq2 output against frozen SciPy quantities."""
import csv,json,math
from pathlib import Path
root=Path(__file__).resolve().parent
oracle=json.loads((root/'oracle.json').read_text())
maximum=[0.]*9
files=sorted((root/'deseq2').glob('*.csv'))
expected_cases={name for name,case in oracle.items() if case['exit']==0 and any(row['reason']=='tested' for row in case['rows'])}
assert {file.stem for file in files}==expected_cases, 'incomplete DESeq2 outputs'
for file in files:
    expected=oracle[file.stem]
    factors=list(map(float,(file.with_suffix('.factors')).read_text().split()))
    assert len(factors)==6
    assert all(math.isclose(x,y,rel_tol=1e-12) for x,y in zip(factors,expected['factors']))
    rows=list(csv.DictReader(file.open()))
    assert {int(row['gene']) for row in rows}=={i for i,row in enumerate(expected['rows']) if row['reason']=='tested'}
    assert len(rows)==sum(row['reason']=='tested' for row in expected['rows'])
    for row in rows:
        values=expected['rows'][int(row['gene'])]['values']
        fields=[('baseMean',0),('dispersion',1),('mean0',2),('mean1',3),('logLikelihood',6),('log2FoldChange',4),('lfcSE',5),('pvalue',7),('padj',8)]
        for field,index in fields:
            actual=float(row[field]); difference=abs(actual-values[index]); maximum[index]=max(maximum[index],difference)
            assert math.isclose(actual,values[index],rel_tol=2e-5,abs_tol=2e-6),(file.stem,field,actual,values[index])
    print('DESeq2 verified',file.stem)
print('maximum absolute discrepancies',maximum)
