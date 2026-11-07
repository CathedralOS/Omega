"""Independent exact rational oracle: marginal means, residuals, and ANOVA.
Freeze command runs BEFORE any Omega check or execution. Never reads Omega output.
"""
from fractions import Fraction as F
from pathlib import Path
import json, hashlib, sys
ROOT=Path(__file__).resolve().parent
BANNER='crossed_gauge_study: exact crossed random-effects sample; all numbers /32\n'
def oracle(text):
    tokens=''.join(c for c in text if c not in ' \r\n')
    if tokens.startswith('1'): return 5,BANNER+'not identifiable: one repeat cannot separate interaction from repeatability\n'
    invalid=(2,BANNER+'invalid input: expected header 2 and exactly eight digit/m slots\n')
    if len(tokens)!=9 or tokens[0]!='2' or any(c not in '0123456789m' for c in tokens[1:]): return invalid
    slots=tokens[1:]
    counts=[sum(c!='m' for c in slots[2*i:2*i+2]) for i in range(4)]
    if 0 in counts: return 3,BANNER+'not identifiable: missing part/operator cell; balanced ANOVA refused\n'
    if counts==[1]*4: return 5,BANNER+'not identifiable: one repeat cannot separate interaction from repeatability\n'
    if counts!=[2]*4: return 4,BANNER+'not identifiable: unequal replication; balanced ANOVA refused\n'
    y=[[[F(slots[(i*2+j)*2+k]) for k in range(2)] for j in range(2)] for i in range(2)]
    grand=sum(v for row in y for cell in row for v in cell)/8
    parts=[sum(v for cell in row for v in cell)/4 for row in y]
    operators=[sum(y[i][j][k] for i in range(2) for k in range(2))/4 for j in range(2)]
    cells=[[sum(cell)/2 for cell in row] for row in y]
    part=4*sum((v-grand)**2 for v in parts)
    operator=4*sum((v-grand)**2 for v in operators)
    interaction=2*sum((cells[i][j]-parts[i]-operators[j]+grand)**2 for i in range(2) for j in range(2))
    error=sum((y[i][j][k]-cells[i][j])**2 for i in range(2) for j in range(2) for k in range(2))
    total=sum((v-grand)**2 for row in y for cell in row for v in cell)
    assert total==part+operator+interaction+error
    repeat=error/4
    raw_part=(part-interaction)/4
    raw_operator=(operator-interaction)/4
    raw_interaction=(interaction-repeat)/2
    values=dict(ss_part=part,ss_operator=operator,ss_interaction=interaction,ss_error=error,ss_total=total,ms_error=repeat,raw_part=raw_part,raw_operator=raw_operator,raw_interaction=raw_interaction,repeatability=repeat,part_variance=max(0,raw_part),operator_variance=max(0,raw_operator),interaction_variance=max(0,raw_interaction))
    values['gauge_variance']=repeat+values['operator_variance']+values['interaction_variance']
    output=BANNER+'identifiable: parts=2 operators=2 repeats=2; df=1,1,1,4 total=7\n'
    for name,value in values.items():
        numerator=value*32
        assert numerator.denominator==1
        output+=f'{name}={int(numerator)}\n'
    return 0,output+'interaction: retained; negative components clamped individually; no pooling\n'
def freeze():
    records={}
    for path in sorted((ROOT/'data').glob('*.stdin')):
        status,output=oracle(path.read_text())
        records[path.stem]={'exit':status,'stdout':output,'input_sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
    document={'method':'Fraction marginal-mean residual ANOVA, independent of Omega contrast algorithm','tolerance':'exact stdout, exact rational numerators; absolute=0 relative=0','cases':records}
    (ROOT/'data/expected.json').write_text(json.dumps(document,indent=2)+'\n')
    print(hashlib.sha256((ROOT/'data/expected.json').read_bytes()).hexdigest())
if __name__=='__main__':
    if sys.argv[1:]!=['--freeze']: raise SystemExit('usage: reference.py --freeze')
    freeze()
