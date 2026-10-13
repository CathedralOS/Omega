"""Independent Decimal oracle, 90 digits. Solve cleared electroneutrality
quartic by bracketing, not the implementation's rational charge iteration.
Run only to intentionally regenerate expectations before product execution.
"""
from decimal import Decimal as D, localcontext
from pathlib import Path
import hashlib,json
ROOT=Path(__file__).resolve().parents[1]
CASES={
'curve':'0.05 0.1 1e-3 1e-7 1e-14 0.1 0 1e-8 0.025 0.049999 0.05 0.050001 0.075 0.099999 0.1 0.100001 0.15',
'volume_scaled':'5 0.1 1e-3 1e-7 1e-14 0.1 0 0.000001 2.5 4.9999 5 5.0001 7.5 9.9999 10 10.0001 15',
'diluted':'0.05 0.001 1e-3 1e-7 1e-14 0.001 0 0.025 0.05 0.1 0.15',
'equal_constants':'0.1 0.1 1e-5 1e-5 1e-14 0.1 0 0.1 0.2 0.3',
'extreme':'0.000001 0.000001 1e-12 1e-14 1e-16 1 0 0.000001 1000',
'upper':'1000 1 1 1 1e-12 1 0 500 1000',
'maximum_rows':'1 0.1 1e-3 1e-7 1e-14 0.1 '+' '.join(str(x/10) for x in range(16)),
'eof_decimal':'+.05 .1 0.001 0.0000001 0.00000000000001 +0.1 0. 0.05 0.1',
}
def oracle(text):
    with localcontext() as context:
        context.prec=90
        va,ca,k1,k2,kw,cb,*volumes=map(D,text.split());rows=[]
        for vb in volumes:
            ct=va*ca/(va+vb);na=vb*cb/(va+vb)
            # (H+Na) H(H²+K1 H+K1 K2) - Kw(H²+K1 H+K1 K2)
            # - Ct H(K1 H+2 K1 K2) = 0.
            def polynomial(h):
                den=h*h+k1*h+k1*k2
                return (h+na)*h*den-kw*den-ct*h*(k1*h+2*k1*k2)
            lower=D('1e-40');upper=D(10)
            assert polynomial(lower)<0<polynomial(upper)
            for _ in range(600):
                midpoint=(lower+upper)/2
                if polynomial(midpoint)>0:upper=midpoint
                else:lower=midpoint
            h=(lower+upper)/2;den=h*h+k1*h+k1*k2
            neutral=ct*h*h/den;mono=ct*k1*h/den;di=ct*k1*k2/den
            rows.append({k:str(v) for k,v in dict(volume=vb,pH=-h.log10(),H=h,OH=kw/h,H2A=neutral,HA=mono,A=di,total_acid=ct,counterion=na,mass_closure=neutral+mono+di-ct,charge_residual=h+na-kw/h-mono-2*di).items()})
        return rows
if __name__=='__main__':
    cases={}
    for name,text in CASES.items():
        path=ROOT/'data'/f'{name}.txt';path.write_text(text+('' if name=='eof_decimal' else '\n'))
        cases[name]={'input_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'rows':oracle(text)}
    output=ROOT/'validation/oracle.json';output.write_text(json.dumps({'precision':90,'method':'cleared quartic, Decimal 600 bisections','cases':cases},indent=2)+'\n')
    (ROOT/'validation/FREEZE.json').write_text(json.dumps({'oracle_sha256':hashlib.sha256(output.read_bytes()).hexdigest(),'reference_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'frozen_before_product_execution':True},indent=2)+'\n')
