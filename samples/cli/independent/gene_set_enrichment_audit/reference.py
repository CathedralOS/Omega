from pathlib import Path
import json,itertools,hashlib
from fractions import Fraction
p=Path('samples/cli/independent/gene_set_enrichment_audit')
cases={
 'overlap':['ABCDEFGHIJ','AABC','ABCD','BCEEF','GHIJ'],
 'rank_reversal':['ABCDEFGH','ABCD','EFGH','BCDE','ABCD'],
 'ties':['ABCDEF','AB','AB','AB','ABCDEF'],
 'empty_query':['ABCDE','','AB','','ABCDE'],
 'all_query':['ABCDE','ABCDE','AB','CDE',''],
 'sparse':['ACEGI','AAG','AC','G','EI'],
 'invalid_duplicate_universe':['AABC','A','AB','BC','C'],
 'invalid_empty_universe':['','','','',''],
 'invalid_outside_query':['ABC','D','AB','BC','C'],
 'invalid_outside_annotation':['ABC','A','AB','BD','C'],
 'invalid_byte':['ABC','A?','AB','BC','C'],
 'invalid_capacity':['ABC','A'*64,'AB','BC','C'],
 'invalid_eof':None,
 'invalid_trailing':None,
}
expected={}
for name,rows in cases.items():
 data='\n'.join(rows)+'\n' if rows else ('ABC\nA\nAB\nBC\n' if name=='invalid_eof' else 'ABC\nA\nAB\nBC\nC\nA\n')
 (p/'fixtures'/f'{name}.stdin').write_text(data)
 if name.startswith('invalid_'):
  expected[name]={'exit':65,'stdout':'INVALID_INPUT\n'}; continue
 universe,query,*annotations=map(set,rows)
 draws=list(itertools.combinations(sorted(universe),len(query)))
 probabilities=[]; results=[]
 for ident,annotation in enumerate(annotations,1):
  hits=query&annotation
  numerator=sum(len(set(draw)&annotation)>=len(hits) for draw in draws)
  probabilities.append(Fraction(numerator,len(draws)))
  results.append(dict(identifier=ident,K=len(annotation),hits=len(hits),p=str(probabilities[-1]),members=''.join(sorted(annotation)),overlap=''.join(sorted(hits))))
 order=sorted(range(3),key=lambda i:(probabilities[i],i))
 corrected=Fraction(1)
 for position in range(2,-1,-1):
  index=order[position]; rank=position+1
  corrected=min(corrected,probabilities[index]*3/rank)
  results[index].update(q=str(corrected),rank=rank)
 expected[name]={'exit':0,'N':len(universe),'n':len(query),'duplicate_query':len(rows[1])-len(query),'duplicate_annotations':sum(len(row)-len(set(row)) for row in rows[2:]),'results':results}
(p/'fixtures'/'expected.json').write_text(json.dumps(expected,indent=2)+'\n')
(p/'fixtures'/'oracle.sha256').write_text(hashlib.sha256((p/'fixtures'/'expected.json').read_bytes()).hexdigest()+'  expected.json\n')
