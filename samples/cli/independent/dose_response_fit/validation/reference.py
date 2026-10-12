"""Independent full four-variable bounded TRF fits and nuisance refits.
No application code or output is imported. Known Gaussian sigma sets the
95% likelihood-ratio cutoff; these are bounded, pointwise profile intervals.
"""
import numpy as np
from scipy.optimize import least_squares, brentq
from scipy.special import expit
from scipy.stats import chi2
import scipy,json,pathlib,hashlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[1]/'data'; ROOT.mkdir(exist_ok=True)
FREEZE = sys.argv[1:] == ["--freeze"]
if sys.argv[1:] and not FREEZE: raise SystemExit("usage: reference.py [--freeze]")
if FREEZE and (ROOT.parent/"validation"/"oracle.json").exists(): raise SystemExit("refusing to overwrite frozen oracle")
DOSES=[1,3,9,27,81]
def fixture(name,direction,a,b,t,h,noisy=False):
 rng=np.random.default_rng(314159 if direction==1 else 271828)
 rows=[]
 for dose in DOSES:
  for replicate in (1,2):
   response=a+(b-a)*expit(h*(np.log(dose)-t))
   if noisy: response+=rng.normal(0,.7)
   rows.append([dose,replicate,0,int(round(response*1000))])
 for role,response in ((1,a),(2,b)):
  for replicate in (1,2):
   rows.append([0,replicate,role,int(round((response+(rng.normal(0,.7) if noisy else 0))*1000))])
 text=f'{direction} {len(rows)} 1000\n'+''.join(' '.join(map(str,r))+'\n' for r in rows)
 if FREEZE:
  with (ROOT/(name+'.txt')).open('x') as output: output.write(text)
 else: assert (ROOT/(name+'.txt')).read_text()==text, 'frozen fixture changed'
 return rows

def solve(rows,direction,sigma=1):
 x=np.array([np.log(r[0]) if r[0] else 0 for r in rows]); y=np.array([r[3]/1000 for r in rows]); roles=np.array([r[2] for r in rows]); samples=roles==0
 lower=np.array([0,0,np.log(min(r[0] for r in rows if r[2]==0)/10),.2]); upper=np.array([100,100,np.log(max(r[0] for r in rows if r[2]==0)*10),5.])
 def residual(p):
  a,b,t,h=p; fractions=np.where(roles==1,0,np.where(roles==2,1,expit(h*(x-t))))
  return a+(b-a)*fractions-y
 starts=[np.array([20 if direction==1 else 80,80 if direction==1 else 20,t,h]) for t in np.linspace(lower[2]+.05,upper[2]-.05,5) for h in (.3,1.,3.)]
 def optimize(fixed=None,value=None,start=None):
  free=[j for j in range(4) if j!=fixed]
  def unpack(q):
   p=np.zeros(4); p[free]=q
   if fixed is not None: p[fixed]=value
   return p
  candidates=[]
  for initial in ([start]+starts if start is not None else starts):
   fitted=least_squares(lambda q:residual(unpack(q)),np.clip(initial[free],lower[free]+1e-10,upper[free]-1e-10),bounds=(lower[free],upper[free]),method='trf',ftol=1e-12,xtol=1e-12,gtol=1e-12,max_nfev=300)
   candidates.append((float(np.dot(fitted.fun,fitted.fun)),unpack(fitted.x),fitted))
  return min(candidates,key=lambda r:r[0])
 sse,p,result=optimize(); cutoff=sse+float(chi2.ppf(.95,1))*sigma*sigma
 intervals={}; profiles={}
 for j,key in enumerate(('lowdose_asymptote','highdose_asymptote','log_half_dose','slope')):
  def profile(value): return optimize(j,float(value),p)[0]-cutoff
  left=profile(lower[j]); right=profile(upper[j]); ends=[]
  for endpoint,sign in ((lower[j],left),(upper[j],right)):
   ends.append(float(brentq(profile,min(endpoint,p[j]),max(endpoint,p[j]),xtol=2e-9)) if sign>0 else None)
  intervals[key]=ends
  profiles[key]=[{ 'value':float(v),'sse':float(optimize(j,float(v),p)[0])} for v in (p[j],*(v for v in ends if v is not None))]
 residuals=residual(p); jac=result.jac; singular=np.linalg.svd(jac,compute_uv=False)
 return {'parameters':dict(zip(('lowdose_asymptote','highdose_asymptote','log_half_dose','slope'),map(float,p))),'half_dose':float(np.exp(p[2])),'sse':sse,'residuals':list(map(float,-residuals)),'predictions':list(map(float,y+residuals)),'intervals':intervals,'profiles':profiles,'singular_values':list(map(float,singular)),'independent_success':bool(result.success),'cutoff':cutoff}
if __name__=='__main__':
 cases={}
 for name,direction,a,b,t,h,noisy in [('exact_activation',1,10,82,np.log(9),1,False),('exact_inhibition',-1,85,13,np.log(9),1,False),('noisy_activation',1,12,86,np.log(12),1.4,True),('noisy_inhibition',-1,87,14,np.log(7),.7,True)]:
  rows=fixture(name,direction,a,b,t,h,noisy); cases[name]=solve(rows,direction); print(name,cases[name]['half_dose'],cases[name]['sse'],flush=True)
 record={'scientific_contract':'unweighted 4PL; roles 1/2 observe low/high dose limits; known sigma=1; 95% pointwise chi-square(1) profile likelihood; A/B [0,100], slope [.2,5], log-half-dose [log(min-dose/10),log(max-dose*10)]','reference':'SciPy direct full 4-variable bounded trust-region reflective least squares, 15 deterministic starts; generic 3-variable nuisance refits and Brent roots','scipy_version':scipy.__version__,'chi_square_95_1':float(chi2.ppf(.95,1)),'cases':cases,'tolerances':{'parameters_absolute':2e-4,'intervals_absolute':2e-3,'sse_absolute':1e-5,'residual_absolute':2e-4}}
 output=ROOT.parent/'validation'/'oracle.json'
 if FREEZE:
  with output.open('x') as destination: destination.write(json.dumps(record,indent=2)+'\n')
  print('frozen',hashlib.sha256(output.read_bytes()).hexdigest(),flush=True)
 else:
  expected=json.loads(output.read_text())
  for name,result in cases.items():
   for key,value in result['parameters'].items(): assert abs(value-expected['cases'][name]['parameters'][key])<1e-6
   for key,values in result['intervals'].items():
    for actual,wanted in zip(values,expected['cases'][name]['intervals'][key]): assert actual is not None and wanted is not None and abs(actual-wanted)<1e-6
  print('independent reproduction: all fits and profile endpoints match frozen oracle')
