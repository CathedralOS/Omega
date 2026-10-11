from pathlib import Path
from fractions import Fraction
import json,hashlib
p=Path(__file__).resolve().parents[1]
cases={
'branched':([(100,10),(0,5),(0,2)],[(0,1,40,50),(0,2,20,25),(1,2,10,50)]),
'cycle':([(30,2),(0,1),(0,1)],[(0,1,20,80),(1,2,10,60),(2,0,4,50)]),
'infeasible':([(10,2),(0,8),(0,5)],[(0,1,20,50),(1,2,15,40),(2,0,2,50)]),
'fractional':([(15,1),(0,1),(0,0)],[(0,1,7,33),(0,2,3,67),(1,2,1,25)]),
'zero':([(0,0)]*3,[]),
'boundary':([(1000,0)]*3,[(0,1,1000,0),(0,2,1000,100),(1,0,1000,50),(1,2,1000,25),(2,0,1000,75),(2,1,1000,100)])}
def number(v):
 assert v.denominator==1
 i=int(v)
 return ('-' if i<0 else '')+f'{abs(i):07d}\n'
manifest={}
for name,(nodes,edges) in cases.items():
 text=''.join(f'{a} {b}\n' for a,b in nodes)+str(len(edges))+'\n'+''.join(' '.join(map(str,e))+'\n' for e in edges)
 (p/'data'/f'{name}.stdin').write_text(text)
 received=[Fraction(0)]*3; assimilated=received.copy(); outgoing=received.copy(); loss=Fraction(0)
 output='trophic reconciliation v1; hundredths\n'
 for s,d,raw,percent in edges:
  converted=Fraction(raw*percent,100)
  received[d]+=raw; assimilated[d]+=converted; outgoing[s]+=raw; loss+=raw-converted
  output+='flow source destination raw assimilated loss\n'
  for v in [Fraction(s,100),Fraction(d,100),Fraction(raw),converted,Fraction(raw)-converted]: output+=number(v*100)
 balances=[Fraction(e)+a-m-o for (e,m),a,o in zip(nodes,assimilated,outgoing)]
 deficits=[max(-b,0) for b in balances]
 for i in range(3):
  output+='node intake assimilated outgoing residual deficit surplus\n'
  for v in [Fraction(i,100),received[i],assimilated[i],outgoing[i],balances[i],deficits[i],max(balances[i],0)]: output+=number(Fraction(v)*100)
 error=sum(Fraction(e-m) for e,m in nodes)-loss-sum(balances)
 assert error==0
 output+='web loss deficit balance_error\n'+number(loss*100)+number(sum(deficits)*100)+number(error*100)+('FEASIBLE\n' if sum(deficits)==0 else 'INFEASIBLE\n')
 (p/'data'/f'{name}.stdout').write_text(output)
 manifest[name]={'exit':0 if sum(deficits)==0 else 3,'residual_energy':[str(b) for b in balances],'loss_energy':str(loss),'deficit_energy':str(sum(deficits)),'stdout_sha256':hashlib.sha256(output.encode()).hexdigest()}
invalid={'missing':'','negative':'-1 0\n','oversized':'1001 0\n','too_many_digits':'00000 0\n','nondigit':'x\n','bad_node':'0 0 0 0 0 0 1 3 1 1 50\n','self_edge':'0 0 0 0 0 0 1 1 1 1 50\n','bad_efficiency':'0 0 0 0 0 0 1 0 1 1 101\n','too_many_edges':'0 0 0 0 0 0 7\n','truncated_edge':'0 0 0 0 0 0 1 0 1\n','trailing':'0 0 0 0 0 0 0 EXTRA\n'}
for n,t in invalid.items():
 (p/'data'/f'invalid_{n}.stdin').write_text(t)
manifest['invalid']={'exit':64,'cases':list(invalid)}
(p/'data/oracle.json').write_text(json.dumps({'model':'simultaneous bounded recorded flows; rational percent conversion','tolerance':'0 exact hundredths; stdout byte exact','cases':manifest},indent=2)+'\n')
