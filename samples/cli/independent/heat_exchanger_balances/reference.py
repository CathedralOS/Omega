from pathlib import Path
from fractions import Fraction as F
import json, hashlib
p=Path('samples/cli/independent/heat_exchanger_balances')
cases={
'zero_conductance':[1,3,2,5,3,4,100,20,0,0,0],
'equal_one':[1,1,2,5,2,5,100,20,5],
'equal_three':[1,3,2,5,2,5,100,20,2,5,8],
'unequal_hot_larger':[1,3,3,4,2,3,120,20,1,3,6],
'unequal_cold_larger':[1,4,1,6,3,4,150,10,0,2,5,6],
'conductance_limit':[1,2,1,1,1,1,100,0,1,1],
'equilibrium':[1,3,2,5,3,4,70,70,2,5,8],
'eight_stages':[1,8,2,5,3,4,180,15,0,1,2,3,4,5,6,7],
'capacity_limit':[1,1,100,100,100,100,200,0,200],
}
invalid={'invalid_direction':'0 1 2 5 2 5 100 20 5\n','zero_flow':'1 1 0 5 2 5 100 20 5\n','zero_capacity':'1 1 2 0 2 5 100 20 5\n','stage_overflow':'1 9 2 5 2 5 100 20 5\n','excess_conductance':'1 1 1 2 2 5 100 20 3\n','reversed_temperature':'1 1 2 5 2 5 20 100 5\n','missing':'1 1 2 5 2 5 100 20\n','extra':'1 1 2 5 2 5 100 20 5 0\n','negative':'1 1 -2 5 2 5 100 20 5\n','nondigit':'1 1 2 5 2 5 100 20 x\n','large_token':'1 1 22222222222222222222 5 2 5 100 20 5\n','empty':'','byte_limit':' '*256}
# Independent simultaneous enthalpy equations, exact rational Gaussian elimination.
def reference(v):
 _,n,hf,hp,cf,cp,hi,ci,*gs=v
 H,C=F(hf*hp),F(cf*cp)
 a=[]
 for j,g in enumerate(gs):
  a.append([F(j==k)+ F(g)*(F(1,H) if k<j else F(1,C) if k>j else (F(1,H)+F(1,C))/2) for k in range(n)]+[F(g)*(hi-ci)])
 for j in range(n):
  pivot=a[j][j]; assert pivot
  a[j]=[x/pivot for x in a[j]]
  for k in range(n):
   if k!=j:
    t=a[k][j]; a[k]=[x-t*y for x,y in zip(a[k],a[j])]
 q=[a[j][-1] for j in range(n)]
 hot=[F(hi)-sum(q[:j])/H for j in range(n+1)]
 cold=[F(ci)+sum(q[j:])/C for j in range(n+1)]
 for j,g in enumerate(gs):
  assert q[j]==F(g)*(hot[j]+hot[j+1]-cold[j]-cold[j+1])/2
  assert hot[j]>=hot[j+1] and cold[j]>=cold[j+1]
 assert H*(hot[0]-hot[-1])==C*(cold[0]-cold[-1])==sum(q)
 return hot,cold,q,sum(q)
def milli(x):
 y=x*1000+F(1,2); return y.numerator//y.denominator
def freeze():
 entries=[]
 for name,v in cases.items():
  data=(' '.join(map(str,v))+'\n').encode(); (p/'data'/f'{name}.txt').write_bytes(data)
  h,c,q,t=reference(v)
  rows=['heat_exchanger_balances midpoint counterflow; temperature=mdegC heat=mW']
  for j in range(len(h)):
   rows.append(f'node hot cold: {milli(h[j])} {milli(c[j])}')
   if j<len(q): rows.append(f'stage heat: {milli(q[j])}')
  rows += [f'total heat: {milli(t)}','PASS enthalpy and cold boundary tolerance=1e-7']
  (p/'data'/f'{name}.stdout').write_text('\n'.join(rows)+'\n')
  entries.append(dict(name=name,exit=0,input_sha256=hashlib.sha256(data).hexdigest(),hot=list(map(str,h)),cold=list(map(str,c)),heat=list(map(str,q)),total=str(t)))
 for name,data in invalid.items():
  data=data.encode(); (p/'data'/f'{name}.txt').write_bytes(data)
  (p/'data'/f'{name}.stdout').write_text('INVALID bounded counterflow input\n')
  entries.append(dict(name=name,exit=64,input_sha256=hashlib.sha256(data).hexdigest()))
 (p/'data/oracle.json').write_text(json.dumps(dict(model='midpoint finite-volume counterflow',freeze='before any Omega check or execution',absolute_balance_tolerance=1e-7,absolute_temperature_tolerance=1e-7,display_quantum=0.001,display_rounding='nearest half upward',cases=entries),indent=2)+'\n')

if __name__ == "__main__":
 freeze()
