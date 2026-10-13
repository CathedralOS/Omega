"""Independent Decimal logarithmic least squares, frozen before Omega runs."""
from decimal import Decimal, localcontext
import json
from pathlib import Path

def oracle(text):
    with localcontext() as context:
        context.prec = 80
        values = list(map(Decimal, text.split()))
        count, unknowns = map(int, values[:2])
        pairs = list(zip(values[2:2+2*count:2], values[3:2+2*count:2]))
        x = [copies.log10() for copies, cq in pairs]
        y = [cq for copies, cq in pairs]
        meanx, meany = sum(x)/count, sum(y)/count
        sxx = sum((v-meanx)**2 for v in x)
        slope = sum((a-meanx)*(b-meany) for a,b in zip(x,y))/sxx
        intercept = meany-slope*meanx
        residuals = [b-intercept-slope*a for a,b in zip(x,y)]
        r2 = 1-sum(v*v for v in residuals)/sum((v-meany)**2 for v in y)
        output = {'slope':[slope], 'intercept':[intercept], 'r2':[r2], 'efficiency':[Decimal(10)**(-1/slope)-1], 'residual':residuals, 'copies':[], 'lower':[], 'upper':[], 'extrapolated':[], 'interval_extrapolated':[]}
        for cq, margin in zip(values[2+2*count::2],values[3+2*count::2]):
            central = (cq-intercept)/slope
            low = (cq+margin-intercept)/slope
            high = (cq-margin-intercept)/slope
            for key,arg in [('copies',central),('lower',low),('upper',high)]:
                output[key].append(Decimal(10)**arg)
            output['extrapolated'].append(Decimal(int(central < min(x) or central > max(x))))
            output['interval_extrapolated'].append(Decimal(int(low < min(x) or high > max(x))))
        return {key:list(map(str,values)) for key,values in output.items()}

CASES = [
 ('doubling','4 4\n1 40 2 39 4 38 8 37\n38.5 0.5 40 0 36 1 41 0.25\n'),
 ('decimal_noisy','5 3\n1 41.2 13 37.4 250 32.9 7000 27.8 1000000 20.1\n35.7 0.3 22.6 1.2 45 0\n'),
 ('minimum','3 0\n1 40 100 33 10000 26\n'),
 ('maximum','8 8\n1 42 10 38.6 100 35.2 1000 31.8 10000 28.4 100000 25 500000 22.6235019853 1000000 21.6\n42 0 38.6 0.1 35.2 0.2 31.8 0.3 28.4 0.4 25 0.5 21.6 1 0 0\n'),
 ('replicates','4 2\n10 38 10 38.2 1000 31.3 1000 31.1\n34.5 0.25 38.1 0\n'),
 ('fractional','3 2\n1.5 40 120.25 33 990000.75 19\n29 0.125 45 0.75\n'),
]
if __name__ == '__main__':
    root=Path(__file__).parent
    cases=[dict(name=name,input=text,expected=oracle(text)) for name,text in CASES]
    # Analytic doubling: slope=-log_2(10), efficiency=1, sqrt(8), [2,4].
    with localcontext() as context:
        context.prec=80
        expected=cases[0]['expected']
        assert abs(Decimal(expected['slope'][0])+Decimal(10).ln()/Decimal(2).ln()) < Decimal('1e-75')
        assert abs(Decimal(expected['efficiency'][0])-1)<Decimal('1e-75')
        assert abs(Decimal(expected['copies'][0])-Decimal(8).sqrt())<Decimal('1e-75')
        assert abs(Decimal(expected['lower'][0])-2)<Decimal('1e-75')
        assert abs(Decimal(expected['upper'][0])-4)<Decimal('1e-75')
    (root/'oracles.json').write_text(json.dumps(cases,indent=2)+'\n')
