"""Freeze independent 80-digit Decimal block solves before application runs.
Gaussian forward elimination/back substitution, independent of Omega's GJ.
"""
from decimal import Decimal, localcontext
from pathlib import Path
import hashlib,json
ROOT=Path(__file__).resolve().parents[1]
def reference(tokens):
 with localcontext() as context:
  context.prec=80
  n,m=map(int,tokens[:2]);variance,range_,noise=map(Decimal,tokens[2:5])
  points=[list(map(Decimal,tokens[5+3*i:8+3*i])) for i in range(n)]
  queries=[list(map(Decimal,tokens[5+3*n+2*i:7+3*n+2*i])) for i in range(m)]
  def cov(a,b):return variance*(-sum((a[k]-b[k])**2 for k in (0,1)).sqrt()/range_).exp()
  covariance=[[cov(a,b)+(noise if i==j else 0) for j,b in enumerate(points)] for i,a in enumerate(points)]
  block=[row+[Decimal(1)] for row in covariance]+[[Decimal(1)]*n+[Decimal(0)]]
  results=[]
  for query in queries:
   cross=[cov(p,query) for p in points];matrix=[row[:] for row in block];rhs=cross+[Decimal(1)]
   for k in range(n+1):
    pivot=max(range(k,n+1),key=lambda i:abs(matrix[i][k]))
    if not matrix[pivot][k]:return None
    matrix[k],matrix[pivot]=matrix[pivot],matrix[k];rhs[k],rhs[pivot]=rhs[pivot],rhs[k]
    for i in range(k+1,n+1):
     ratio=matrix[i][k]/matrix[k][k]
     for j in range(k,n+1):matrix[i][j]-=ratio*matrix[k][j]
     rhs[i]-=ratio*rhs[k]
   answer=[Decimal(0)]*(n+1)
   for i in reversed(range(n+1)):answer[i]=(rhs[i]-sum(matrix[i][j]*answer[j] for j in range(i+1,n+1)))/matrix[i][i]
   weights=answer[:n];prediction=sum(w*p[2] for w,p in zip(weights,points));latent=variance-sum(w*c for w,c in zip(weights,cross))-answer[n]
   assert abs(sum(weights)-1)<Decimal('1e-70')
   if n==2:
    a=covariance[0][0];b=covariance[0][1]
    analytic=(1+(cross[0]-cross[1])/(a-b))/2
    assert abs(weights[0]-analytic)<Decimal('1e-70')
   if len(set(p[2] for p in points))==1:assert abs(prediction-points[0][2])<Decimal('1e-70')
   results.append({'weights':[str(w) for w in weights],'prediction':str(prediction),'variance':str(latent),'multiplier':str(answer[n]),'cross':[str(c) for c in cross]})
  return {'queries':results,'covariance':[[str(c) for c in row] for row in covariance]}
def main():
 ROOT.joinpath('data').mkdir(exist_ok=True)
 base=['4','3','2.5','1.7','0.15','0','0','2','1','0','4','0','2','-1','3','1','3','0.5','0.5','2','2','-1','1']
 fixtures={
  'planar':base,
  'two_point':['2','3','2','1','0','0','0','2','2','0','6','1','0','0','0','3','1'],
  'two_point_noisy':['2','2','2','1','0.5','0','0','2','2','0','6','1','0','0','0'],
  'constant':['3','2','1.3','2.2','0.4','0','0','7','1','0','7','0','1','7','0.3','0.2','3','3'],
  'interpolation':['3','3','3','2','0','0','0','2','1','0','4','0','1','-1','0','0','1','0','0','1'],
  'noisy_duplicates':['3','2','2','1','0.2','0','0','1','0','0','3','1','0','5','0','0','0.5','0'],
  'singular_duplicates':['3','1','2','1','0','0','0','1','0','0','3','1','0','5','0','0'],
  'maximum':['8','8','1','2','0.01']+sum(([str(i%4),str(i//4),str(i-3)] for i in range(8)),[])+sum(([str(i/4),str(i/5)] for i in range(8)),[]),
  'far_field':['2','1','1','0.01','0','0','0','2','1','1','4','100','100'],
 }
 permutation=[2,0,3,1]
 fixtures['permuted']=base[:5]+sum((base[5+3*i:8+3*i] for i in permutation),[])+base[17:]
 translated=base[:]
 for index in list(range(5,17,3))+list(range(17,23,2)):
  translated[index]=str(Decimal(base[index])+11);translated[index+1]=str(Decimal(base[index+1])-7)
 fixtures['translated']=translated
 cases={}
 for name,tokens in fixtures.items():
  data=(' '.join(tokens)+'\n').encode();ROOT.joinpath('data',name+'.stdin').write_bytes(data)
  result=reference(tokens)
  cases[name]={'exit':0 if result else 2,'input_sha256':hashlib.sha256(data).hexdigest(),'expected':result}
 invalid={'empty':b'','nan':b'2 1 NaN 1 0 0 0 1 1 0 2 0 0\n','infinity':b'2 1 1 inf 0 0 0 1 1 0 2 0 0\n','truncated':b'2 1 1 1 0\n','too_long':b' '*2048,'long_number':b'1234567890123\n','junk':b'2 1 1 1 0 !\n'}
 basic=fixtures['two_point'][:]
 for name,index,value in [('zero_variance',2,'0'),('negative_variance',2,'-1'),('zero_range',3,'0'),('negative_range',3,'-1'),('negative_noise',4,'-0.1'),('fraction_count',0,'2.5'),('few_observations',0,'1'),('many_observations',0,'9'),('no_queries',1,'0'),('many_queries',1,'9')]:
  changed=basic[:];changed[index]=value;invalid[name]=(' '.join(changed)+'\n').encode()
 invalid['extra']=(' '.join(basic)+' 1\n').encode()
 invalid['misplaced_sign']=b'2 1 1 1 0 - 0 1 1 0 2 0 0\n'
 for name,payload in invalid.items():
  ROOT.joinpath('data',name+'.stdin').write_bytes(payload);cases[name]={'exit':64,'input_sha256':hashlib.sha256(payload).hexdigest()}
 # Grammar variants are frozen inputs too, not learned from runtime output.
 payload=ROOT.joinpath('data','planar.stdin').read_bytes()
 for name,data in [('eof',payload.rstrip()),('tabs',payload.replace(b' ',b'\t')),('crlf',payload.replace(b'\n',b'\r\n'))]:
  ROOT.joinpath('data',name+'.stdin').write_bytes(data);cases[name]={**cases['planar'],'input_sha256':hashlib.sha256(data).hexdigest()}
 result={'method':'80-digit Decimal covariance and independent forward elimination/back substitution; analytic two-point and constant-field assertions','precision':80,'cases':cases,'tolerances':{'values_absolute':1e-7,'weight_sum':1e-9,'scaled_residual':1e-9}}
 raw=(json.dumps(result,indent=2)+'\n').encode();ROOT.joinpath('validation','frozen.json').write_bytes(raw)
 digest=hashlib.sha256(raw).hexdigest();ROOT.joinpath('validation','FROZEN.sha256').write_text(digest+'  frozen.json\n');print(digest)
if __name__=='__main__':main()
