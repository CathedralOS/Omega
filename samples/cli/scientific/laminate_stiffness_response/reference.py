#!/usr/bin/env python3
"""Independent 70-digit fourth-order tensor rotation and symmetric LDL solve.
Freeze produces checked-in expectations; verify never regenerates them.
"""
from decimal import Decimal as D, getcontext
from pathlib import Path
import hashlib, json, re, sys
getcontext().prec = 70
ROOT = Path(__file__).resolve().parent
ORACLE_SHA256 = '9b7c5c43f6ba24db04517f11a0341d7f4ad28880a515043d63ed70bfd23648eb'

def assert_frozen_oracle():
    actual = hashlib.sha256((ROOT/'data/expected.json').read_bytes()).hexdigest()
    assert actual == ORACLE_SHA256, ('frozen oracle changed', actual)
    return actual

LOAD = list(map(D, ['1','.25','-.125','.1','-.05','.025']))

def rotated(m,n):
    # Rotate the elasticity tensor, rather than copying Omega's Qbar formula.
    rotation = [[m,-n],[n,m]]
    def original(i,j,k,l):
        if i==j==k==l: return D(12 if i==0 else 4)
        if i==j and k==l: return D(1)
        if i!=j and k!=l: return D(2)
        return D(0)
    def component(a,b,c,d):
        return sum(rotation[a][i]*rotation[b][j]*rotation[c][k]*rotation[d][l]*original(i,j,k,l)
                   for i in range(2) for j in range(2) for k in range(2) for l in range(2))
    pairs=[(0,0),(1,1),(0,1)]
    return [[component(*a,*b) for b in pairs] for a in pairs]

def ldlt(matrix,load):
    lower=[[D(int(i==j)) for j in range(6)] for i in range(6)]
    diagonal=[D(0)]*6
    for j in range(6):
        diagonal[j]=matrix[j][j]-sum(lower[j][k]**2*diagonal[k] for k in range(j))
        assert diagonal[j]>D('1e-12')
        for i in range(j+1,6):
            lower[i][j]=(matrix[i][j]-sum(lower[i][k]*lower[j][k]*diagonal[k] for k in range(j)))/diagonal[j]
    forward=[]
    for i in range(6): forward.append(load[i]-sum(lower[i][j]*forward[j] for j in range(i)))
    middle=[forward[i]/diagonal[i] for i in range(6)]
    result=[D(0)]*6
    for i in reversed(range(6)): result[i]=middle[i]-sum(lower[j][i]*result[j] for j in range(i+1,6))
    return result

def calculate(mode):
    half=D('.5').sqrt()
    directions={'0':(D(1),D(0)), '90':(D(0),D(1)), '+45':(half,half), '-45':(half,-half), 'oblique':(D('.6'),D('.8'))}
    specifications=[(['0','90'],['.5','.5']),(['90','0'],['.5','.5']),
      (['0','90','90','0'],['.25']*4),(['+45','-45','-45','+45'],['.25']*4),
      (['oblique','-45','90'],['.2','.3','.5']),(['0','90'],['.5','.5'])]
    angles,thicknesses=specifications[mode]
    thicknesses=list(map(D,thicknesses))
    q=[rotated(*directions[a]) for a in angles]
    if mode==5: q=[[[D(0)]*3 for _ in range(3)] for _ in angles]
    z=[-sum(thicknesses)/2]
    for thickness in thicknesses: z.append(z[-1]+thickness)
    matrices=[]
    for exponent in (1,2,3):
        matrices.append([[sum(q[k][i][j]*(z[k+1]**exponent-z[k]**exponent)/exponent for k in range(len(q))) for j in range(3)] for i in range(3)])
    a,b,d=matrices
    output=[('LAMINATE micro-units v1',[])]+[('Q',sum(ply,[])) for ply in q]+[(name,sum(matrix,[])) for name,matrix in zip(('A','B','D'),matrices)]
    response=[]; stress=[]
    if mode==5: output.append(('SINGULAR',[]))
    else:
        system=[a[i]+b[i] for i in range(3)]+[b[i]+d[i] for i in range(3)]
        response=ldlt(system,LOAD)
        for i in range(6):
            assert abs(sum(system[i][j]*response[j] for j in range(6))-LOAD[i])<D('1e-60')
        stress=[[sum(q[k][i][j]*(response[j]+(z[k]+z[k+1])/2*response[j+3]) for j in range(3)) for i in range(3)] for k in range(len(q))]
        output += [('RESPONSE',response)]+[('STRESS',row) for row in stress]+[('OK',[])]
    return {'angles':angles,'thicknesses':list(map(str,thicknesses)), 'q':q,'a':a,'b':b,'d':d,'response':response,'stress':stress,'sections':output,'exit':3 if mode==5 else 0}

def freeze():
    cases=[calculate(i) for i in range(6)]
    # Exact independently known cardinal-angle stiffness and reversal invariants.
    assert cases[0]['q'][0]==[[D(12),D(1),D(0)],[D(1),D(4),D(0)],[D(0),D(0),D(2)]]
    assert cases[0]['q'][1]==[[D(4),D(1),D(0)],[D(1),D(12),D(0)],[D(0),D(0),D(2)]]
    for i in range(3):
        for j in range(3):
            assert cases[0]['a'][i][j]==cases[1]['a'][i][j]
            assert cases[0]['d'][i][j]==cases[1]['d'][i][j]
            assert cases[0]['b'][i][j]==-cases[1]['b'][i][j]
            assert abs(cases[2]['b'][i][j])<D('1e-60')
            assert abs(cases[3]['b'][i][j])<D('1e-60')
    assert abs(cases[3]['a'][0][2])<D('1e-60')
    assert abs(cases[3]['d'][0][2])>D('.01') # Balanced A does not imply balanced D.
    record={'oracle':'Decimal(70), tensor rotation + LDL^T, frozen before Omega execution',
      'numeric_absolute_tolerance':'0.000002', 'numeric_relative_tolerance':'0',
      'pivot_absolute_floor':'0.000000000001','units':'synthetic Q and unit total thickness; printed integers / 1000000',
      'cases':cases,'invalid_inputs':{'6':'INVALID MODEL','7':'INVALID MODEL','8':'INVALID MODEL','9':'INVALID INPUT','x':'INVALID INPUT','empty':'INVALID INPUT','00':'INVALID INPUT','overlong':'INVALID INPUT'}}
    (ROOT/'data/expected.json').write_text(json.dumps(record,indent=2,default=str)+'\n')
    for mode in range(9): (ROOT/f'data/mode_{mode}.stdin').write_text(str(mode)+'\n')
    print('frozen: exact cardinal checks, reversal, cancellation and six Decimal cases')

def verify(mode,path,code):
    assert_frozen_oracle()
    expected=json.loads((ROOT/'data/expected.json').read_text())
    lines=Path(path).read_text().splitlines()
    if mode in ('6','7','8','9','x','empty','00','overlong','crlf'):
        assert code==2,(mode,code)
        assert lines==[('INVALID INPUT' if mode=='crlf' else expected['invalid_inputs'][mode])],lines
    else:
        case=expected['cases'][int(mode)]; assert code==case['exit'],(mode,code)
        cursor=0
        for label,values in case['sections']:
            assert lines[cursor]==label,(cursor,label,lines[cursor]); cursor+=1
            for value in values:
                assert re.fullmatch(r"-?[0-9]+", lines[cursor], flags=re.ASCII), ("integer format", lines[cursor])
                actual=D(lines[cursor])/D(1000000); cursor+=1
                assert abs(actual-D(value))<=D(expected['numeric_absolute_tolerance']),(mode,label,actual,value)
        assert cursor==len(lines),(cursor,len(lines))
    print(f'verified mode={mode} exit={code} artifact={path}')

if __name__=='__main__':
    if sys.argv[1:]==['freeze']: freeze()
    elif len(sys.argv)==5 and sys.argv[1]=='verify': verify(sys.argv[2],sys.argv[3],int(sys.argv[4]))
    else: raise SystemExit('reference.py freeze | verify MODE STDOUT EXIT')
