"""Freeze before executing Omega. No recurrence shared with the application.

Tiny distributions enumerate subsets of labeled individuals. Larger ones use
species-presence inclusion/exclusion over subsets of species. Moments use exact
inclusion and covariance probabilities, independently of either distribution.
"""
from fractions import Fraction
from itertools import combinations
from math import comb
from pathlib import Path
import hashlib, json
ROOT = Path(__file__).resolve().parent

def choose(n, k):
    return comb(n, k) if 0 <= k <= n else 0

def moments(counts, depth):
    total = sum(counts)
    denominator = comb(total, depth)
    absent = [Fraction(choose(total-c, depth), denominator) for c in counts]
    inclusion = [1-a for a in absent]
    expectation = sum(inclusion, Fraction())
    variance = sum((p*(1-p) for p in inclusion), Fraction())
    covariance = []
    for left, right in combinations(range(len(counts)), 2):
        joint = 1-absent[left]-absent[right]+Fraction(choose(total-counts[left]-counts[right], depth), denominator)
        value = joint-inclusion[left]*inclusion[right]
        covariance.append([left, right, str(value)])
        variance += 2*value
    return expectation, variance, inclusion, covariance

def distribution(counts, depth):
    total = sum(counts)
    weights = [0]*(len(counts)+1)
    if total <= 10:
        labels = [species for species, count in enumerate(counts) for _ in range(count)]
        for subset in combinations(range(total), depth):
            weights[len({labels[index] for index in subset})] += 1
        method = 'labeled individual subsets'
    else:
        species = range(len(counts))
        for richness in range(len(counts)+1):
            for present in combinations(species, richness):
                # Number of depth-subsets confined to present and meeting every species.
                for missing_size in range(richness+1):
                    for missing in combinations(present, missing_size):
                        available = sum(counts[index] for index in present if index not in missing)
                        weights[richness] += (-1)**missing_size * choose(available, depth)
        method = 'species inclusion exclusion'
    return weights, method

def positive(counts, depths):
    rows = []
    for depth in depths:
        denominator = comb(sum(counts), depth)
        weights, method = distribution(counts, depth)
        expectation, variance, inclusion, covariance = moments(counts, depth)
        assert sum(weights) == denominator and min(weights) >= 0
        first = sum(r*w for r,w in enumerate(weights))
        second = sum(r*r*w for r,w in enumerate(weights))
        assert Fraction(first, denominator) == expectation
        assert Fraction(second, denominator)-expectation**2 == variance
        center = first // denominator
        centered_second = second+center*center*denominator-2*center*first
        numerator = centered_second*denominator-(first-center*denominator)**2
        assert Fraction(numerator, denominator**2) == variance
        rows.append(dict(depth=depth, denominator=denominator, weights=weights,
                         expectation_numerator=first, variance_numerator=numerator,
                         variance_denominator=denominator**2, expectation=str(expectation),
                         variance=str(variance), inclusion=list(map(str,inclusion)),
                         covariance=covariance, method=method))
    return rows

def freeze():
    fixtures = {
        'unequal': ([1,2,3], list(range(7))),
        'permuted': ([3,1,2], list(range(7))),
        'absent': ([0,1,0,2,3], list(range(7))),
        'one_individual': ([1], [0,1]),
        'one_species': ([8], list(range(9))),
        'singletons': ([1]*8, list(range(9))),
        'maximum_total': ([2]*11+[10], [0,1,2,8,16,24,31,32]),
        'maximum_depths': ([1,2,3,4,5,1], list(range(16))),
        'duplicates': ([2,3], [0,2,2,5]),
        'maximum_capacity': ([0]*11+[32], list(range(16))),
        'maximum_capacity_eof': ([0]*11+[32], list(range(16))),
        'maximum_single_species': ([32], [0,16,32]),
        'line_capacity': ([1], [0,1]),
        'exact_eof': ([2,1], [0,1,2,3]),
        'whitespace': ([1,2,3], [0,2,6]),
        'zero_depths': ([1], []),
    }
    cases = {}
    for name, (counts, depths) in fixtures.items():
        values = [len(counts),*counts,len(depths),*depths]
        text = ' '.join(map(str,values)) + ('' if name in ('exact_eof','maximum_capacity_eof') else '\n')
        if name == 'line_capacity': text = text.rstrip('\n').ljust(255)+'\n'
        if name == 'whitespace': text = '\t'+'\t '.join(map(str,values))+'\r\n'
        cases[name] = dict(exit=0, counts=counts, depths=depths, rows=positive(counts, depths))
        (ROOT/'data'/f'{name}.stdin').write_text(text)
    refusals = {
        'negative_count':'2 -1 2 1 0\n', 'excess_total':'2 20 20 1 0\n',
        'zero_total':'2 0 0 1 0\n', 'count_33':'1 33 1 0\n', 'count_99':'1 99 1 0\n',
        'full_eof':'1 1 1 0'.ljust(256), 'zero_species':'0 1 0\n',
        'excess_species':'13 '+'1 '*13+'1 0\n',
        'negative_depth':'1 3 1 -1\n', 'excess_depth':'1 3 1 4\n',
        'excess_depths':'1 32 17 '+'0 '*17+'\n',
        'missing_query_count':'1 1', 'extra_after_zero_queries':'1 1 0 0\n', 'missing':'2 1 2 2 0\n',
        'extra':'1 1 1 0 0\n', 'bad_token':'1 x 1 0\n',
        'large_token':'1 999 1 0\n', 'empty':'', 'second_line':'1 1 1 0\n0\n',
        'long_line':' '*256+'\n', 'too_many_tokens':'1 '*32+'\n',
    }
    for name, text in refusals.items():
        (ROOT/'data'/f'{name}.stdin').write_text(text)
        cases[name] = dict(exit=64, stdout='ERROR invalid_input\n')
    for name, case in cases.items():
        case['input_sha256'] = hashlib.sha256((ROOT/'data'/f'{name}.stdin').read_bytes()).hexdigest()
    document = dict(schema=2, tolerance=1e-10, cases=cases)
    (ROOT/'data/oracle.json').write_text(json.dumps(document,indent=2)+'\n')
    digest = hashlib.sha256((ROOT/'data/oracle.json').read_bytes()).hexdigest()
    (ROOT/'ORACLE_SHA256').write_text(digest+'\n')
    print(digest, len(cases), 'cases frozen')
if __name__ == '__main__': freeze()
