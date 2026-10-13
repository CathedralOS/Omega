"""Independent analytic beta integral at 200 Decimal digits, exact prediction.

The CDF integrates the expanded density, using Horner evaluation. It does not
use the application's binomial-tail recurrence. Bisection is performed at high
precision for 120 steps. Predictive masses use factorials and rational arithmetic.
"""
from decimal import Decimal, localcontext
from fractions import Fraction
from math import comb, factorial
from pathlib import Path
import hashlib, json
ROOT = Path(__file__).resolve().parents[1]

def cdf(alpha, beta, value):
    with localcontext() as context:
        context.prec = 200
        value = Decimal(value)
        polynomial = Decimal(0)
        for position in range(beta - 1, -1, -1):
            coefficient = Decimal((-1)**position * comb(beta-1, position)) / Decimal(alpha+position)
            polynomial = polynomial * value + coefficient
        normalization = Decimal(factorial(alpha+beta-1)) / Decimal(factorial(alpha-1)*factorial(beta-1))
        return +(value**alpha * polynomial * normalization)

def quantile(alpha, beta, target):
    with localcontext() as context:
        context.prec = 200
        lower, upper = Decimal(0), Decimal(1)
        for _ in range(120):
            midpoint = (lower + upper) / 2
            if cdf(alpha, beta, midpoint) < target: lower = midpoint
            else: upper = midpoint
        return +(lower + upper) / 2

def row(alpha, beta):
    with localcontext() as context:
        context.prec = 200
        total = Decimal(alpha+beta)
        return { 'alpha': alpha, 'beta': beta,
            'mean': str(Decimal(alpha)/total),
            'variance': str(Decimal(alpha*beta)/(total*total*(total+1))),
            'lower': str(quantile(alpha,beta,Decimal('.025'))),
            'upper': str(quantile(alpha,beta,Decimal('.975'))),
            'above_half': str(1-cdf(alpha,beta,Decimal('.5'))) }

def predictive(alpha, beta, future):
    denominator = Fraction(factorial(alpha-1)*factorial(beta-1),factorial(alpha+beta-1))
    return [Fraction(comb(future,position)*factorial(alpha+position-1)*factorial(beta+future-position-1),factorial(alpha+beta+future-1))/denominator for position in range(future+1)]

def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()

def freeze():
    models = {
        'sequential': (2,3,[(7,10),(12,20),(0,0)],16),
        'all_success': (1,1,[(50,50)]*8,32),
        'all_failure': (1,1,[(0,50)]*8,32),
        'symmetry': (20,20,[(7,50),(31,50),(12,50)],32),
        'symmetry_complement': (20,20,[(43,50),(19,50),(38,50)],32),
        'order': (2,3,[(0,0),(12,20),(7,10)],16),
        'regroup': (2,3,[(19,30)],16),
        'uninformative': (1,1,[(0,0)],0),
        'skew_prior': (1,20,[(0,0)],32),
        'skew_complement': (20,1,[(0,0)],32),
        'maximum_balanced': (20,20,[(25,50)]*8,32),
        'no_lf': (3,4,[(1,2),(2,5)],1),
        'whitespace': (2,3,[(7,10),(12,20),(0,0)],16),
    }
    cases = {}
    for name,(alpha,beta,batches,future) in models.items():
        tokens=[alpha,beta,len(batches),future,*[item for pair in batches for item in pair]]
        text=' '.join(map(str,tokens))+('\n' if name!='no_lf' else '')
        if name=='whitespace': text='\t\r\n'.join(map(str,tokens))+'\t \n'
        path=ROOT/'data'/f'{name}.txt';path.write_text(text)
        rows=[]
        for successes,trials in batches:
            alpha+=successes;beta+=trials-successes;rows.append(row(alpha,beta))
        masses=predictive(alpha,beta,future)
        assert sum(masses)==1
        cases[name]={'input_sha256':digest(path),'rows':rows,'future':future,
                     'predictive':[{'numerator':str(mass.numerator),'denominator':str(mass.denominator)} for mass in masses]}
    invalid = {
        'prior_zero':'0 2 1 2 1 2\n', 'prior_large':'21 2 1 2 1 2\n',
        'prior_negative':'-1 2 1 2 1 2\n', 'prior_fraction':'1.5 2 1 2 1 2\n',
        'batches_zero':'1 1 0 0\n','batches_large':'1 1 9 0\n',
        'future_large':'1 1 1 33 1 2\n','trials_large':'1 1 1 2 1 51\n',
        'successes_large':'1 1 1 2 3 2\n','negative_count':'1 1 1 2 -1 2\n',
        'extra':'1 1 1 2 1 2 0\n','extra_record':'1 1 1 2 1 2\n1 1 1 0 0 0\n',
        'missing':'1 1 1 2 1\n','malformed':'1 1 1 2 1 x\n',
        'oversized_token':'1 1 1 2 0001 2\n','over_budget':'1 1 1 2 1 2'+ ' '*4096,
        'empty':'', 'future_negative':'1 1 1 -1 1 2\n',
    }
    for name,text in invalid.items():
        path=ROOT/'data'/f'{name}.txt';path.write_text(text)
        cases[name]={'input_sha256':digest(path),'exit':64,'stdout':'ERROR invalid_input\n'}
    oracle={'precision':200,'quantile_iterations':120,'cases':cases}
    path=ROOT/'validation/oracle.json';path.write_text(json.dumps(oracle,indent=2)+'\n')
    (ROOT/'validation/FREEZE.json').write_text(json.dumps({'reference_sha256':digest(Path(__file__)), 'oracle_sha256':digest(path),'timing':'frozen before first application check or run'},indent=2)+'\n')
    (ROOT/'input.txt').write_bytes((ROOT/'data/sequential.txt').read_bytes())
    print('Frozen',len(models),'models and',len(invalid),'refusals;',digest(path))
if __name__=='__main__':freeze()
