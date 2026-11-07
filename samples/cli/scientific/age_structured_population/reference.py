"""Independent Fraction/matrix oracle. Run --freeze only before Omega execution."""
from fractions import Fraction
from pathlib import Path
import hashlib
import json
import sys
ROOT = Path(__file__).resolve().parent
HEADER = 'year,newborn,juvenile,adult,total,births,deaths,migration\n'
CASES = {
    'births': [4, 80, 40, 20, 500, 750, 500, 0, 250, 500, 0, 0, 0],
    'net_emigration': [2, 10, 10, 10, 1000, 1000, 1000, 0, 0, 0, 0, -2, -1],
    'final_births': [2, 0, 0, 10, 0, 0, 0, 0, 0, 500, 0, 0, 0],
    'large_integer': [9999999999, 1, 1, 1, 1000, 1000, 1000, 0, 0, 0, 0, 0, 0],
    'extinction': [3, 17, 11, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    'migration': [3, 20, 10, 8, 1000, 1000, 1000, 0, 0, 0, 3, -2, 1],
    'fractional': [4, 7, 5, 3, 333, 667, 500, 100, 250, 333, 1, 0, 0],
    'zero_steps': [0, 0, 0, 0, 1000, 1000, 1000, 1000, 1000, 1000, 0, 0, 0],
    'terminal_age': [3, 0, 0, 15, 1000, 1000, 1000, 0, 0, 0, 0, 0, 0],
    'maximum_horizon': [32, 0, 0, 1, 1000, 1000, 1000, 0, 0, 0, 0, 0, 0],
    'invalid_survival': [1, 1, 1, 1, 1001, 500, 500, 0, 0, 0, 0, 0, 0],
    'negative_rate': [1, 1, 1, 1, 500, -1, 500, 0, 0, 0, 0, 0, 0],
    'invalid_fertility': [1, 1, 1, 1, 500, 500, 500, 0, 1001, 0, 0, 0, 0],
    'invalid_horizon': [33, 1, 1, 1, 500, 500, 500, 0, 0, 0, 0, 0, 0],
    'negative_count': [1, -1, 1, 1, 500, 500, 500, 0, 0, 0, 0, 0, 0],
    'invalid_migration': [1, 1, 1, 1, 500, 500, 500, 0, 0, 0, 1000001, 0, 0],
    'infeasible_emigration': [2, 2, 2, 2, 1000, 1000, 1000, 0, 0, 0, 0, -3, 0],
    'capacity': [1, 1000000, 1000000, 1000000, 1000, 1000, 1000, 1000, 1000, 1000, 0, 0, 0],
}
def oracle(values):
    horizon, *rest = values
    population, survival, fertility, migration = [rest[i:i+3] for i in range(0, 12, 3)]
    if not (0 <= horizon <= 32 and all(0 <= n <= 1000000 for n in population)
            and all(0 <= n <= 1000 for n in survival + fertility)
            and all(-1000000 <= n <= 1000000 for n in migration)):
        return 2, 'ERROR invalid input\n'
    matrix = [[Fraction(n, 1000) for n in fertility],
              [Fraction(survival[0], 1000), 0, 0],
              [0, Fraction(survival[1], 1000), Fraction(survival[2], 1000)]]
    def floor(value): return value.numerator // value.denominator
    def row(year, births, deaths, net):
        return ','.join(map(str, [year, *population, sum(population), births, deaths, net])) + '\n'
    output = HEADER + row(0, 0, 0, 0)
    for year in range(1, horizon + 1):
        old_total = sum(population)
        births = floor(sum(a*b for a,b in zip(matrix[0], population)))
        survivors = [floor(Fraction(n*r, 1000)) for n,r in zip(population, survival)]
        # Oldest survivor components floor independently, before addition.
        next_population = [births, survivors[0], survivors[1]+survivors[2]]
        population = [a+b for a,b in zip(next_population, migration)]
        deaths = old_total - sum(survivors)
        assert sum(population) == old_total + births - deaths + sum(migration)
        if not all(0 <= n <= 1000000 for n in population):
            return 3, output + 'ERROR infeasible cohort\n'
        output += row(year, births, deaths, sum(migration))
    return 0, output

def freeze():
    manifest = {'tolerance': 0, 'arithmetic': 'integer census; exact Fraction reference; component survival floors; aggregate birth floor', 'cases': {}}
    for name, values in CASES.items():
        content = ''.join(f'{value}\n' for value in values)
        status, output = oracle(values)
        (ROOT/'fixtures'/f'{name}.input').write_text(content)
        (ROOT/'fixtures'/f'{name}.stdout').write_text(output)
        manifest['cases'][name] = {'exit':status, 'input_sha256':hashlib.sha256(content.encode()).hexdigest(), 'stdout_sha256':hashlib.sha256(output.encode()).hexdigest()}
    for name, content in {'malformed':'1\n2x\n', 'missing':'1\n', 'overlong':'12345678901\n', 'empty':'\n', 'trailing':''.join(f'{v}\n' for v in CASES['births'])+'0\n'}.items():
        output='ERROR invalid input\n'
        (ROOT/'fixtures'/f'{name}.input').write_text(content)
        (ROOT/'fixtures'/f'{name}.stdout').write_text(output)
        manifest['cases'][name]={'exit':2,'input_sha256':hashlib.sha256(content.encode()).hexdigest(),'stdout_sha256':hashlib.sha256(output.encode()).hexdigest()}
    (ROOT/'fixtures/oracle.json').write_text(json.dumps(manifest, indent=2)+'\n')
if __name__ == '__main__':
    if sys.argv[1:] != ['--freeze']: raise SystemExit('usage: reference.py --freeze (before execution only)')
    freeze()
