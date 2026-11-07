"""Independent LP oracle: HiGHS and exact Fraction active-constraint vertices.
No Omega execution or candidate output enters this computation.
"""
from fractions import Fraction as Q
from itertools import combinations, product
import hashlib, json, pathlib
import scipy
from scipy.optimize import linprog


def solve_unique(rows, right, count):
    matrix = [[Q(value) for value in row] + [Q(value)] for row, value in zip(rows, right)]
    pivot = 0
    for column in range(count):
        selected = next((index for index in range(pivot, len(matrix)) if matrix[index][column]), None)
        if selected is None:
            return None
        matrix[pivot], matrix[selected] = matrix[selected], matrix[pivot]
        scale = matrix[pivot][column]
        matrix[pivot] = [value / scale for value in matrix[pivot]]
        for index in range(len(matrix)):
            if index != pivot:
                scale = matrix[index][column]
                matrix[index] = [left - scale * right for left, right in zip(matrix[index], matrix[pivot])]
        pivot += 1
    if any(all(value == 0 for value in row[:-1]) and row[-1] for row in matrix):
        return None
    return tuple(matrix[index][-1] for index in range(count))


def certificates(stoichiometry, bounds):
    count = len(bounds)
    vertices = set()
    # This deliberately enumerates all bound-equation subsets, independently
    # of the sample's ternary free/fixed elimination algorithm.
    for bound_count in range(count + 1):
        for active in combinations(range(count), bound_count):
            for ends in product((0, 1), repeat=bound_count):
                rows = list(stoichiometry)
                right = [Q(0)] * len(rows)
                for index, end in zip(active, ends):
                    rows.append([int(column == index) for column in range(count)])
                    right.append(Q(bounds[index][end]))
                vector = solve_unique(rows, right, count)
                if vector is not None and all(Q(low) <= value <= Q(high) for value, (low, high) in zip(vector, bounds)):
                    vertices.add(vector)
    return sorted(vertices)


def oracle(stoichiometry, bounds, biomass):
    vertices = certificates(stoichiometry, bounds)
    objective = [int(index == biomass) for index in range(len(bounds))]
    result = linprog([-value for value in objective], A_eq=stoichiometry, b_eq=[0]*len(stoichiometry), bounds=[(float(Q(low)),float(Q(high))) for low, high in bounds], method='highs')
    if not vertices:
        assert result.status == 2, result.message
        return dict(status='infeasible', highs_status=result.status, vertices=[])
    assert result.success, result.message
    optimum = max(vector[biomass] for vector in vertices)
    face = [vector for vector in vertices if vector[biomass] == optimum]
    minima = [min(vector[index] for vector in face) for index in range(len(bounds))]
    maxima = [max(vector[index] for vector in face) for index in range(len(bounds))]
    assert abs(-result.fun-float(optimum)) <= 1e-7
    face_rows = list(stoichiometry)+[objective]
    face_right = [0]*len(stoichiometry)+[float(optimum)]
    highs_ranges=[]
    for index in range(len(bounds)):
        selected=[int(column==index) for column in range(len(bounds))]
        low=linprog(selected,A_eq=face_rows,b_eq=face_right,bounds=[(float(Q(a)),float(Q(b))) for a,b in bounds],method='highs')
        high=linprog([-value for value in selected],A_eq=face_rows,b_eq=face_right,bounds=[(float(Q(a)),float(Q(b))) for a,b in bounds],method='highs')
        assert low.success and high.success
        assert abs(low.fun-float(minima[index])) <= 1e-7
        assert abs(-high.fun-float(maxima[index])) <= 1e-7
        highs_ranges.append([low.fun,-high.fun])
    return dict(status='optimal',optimum=str(optimum),minima=list(map(str,minima)),maxima=list(map(str,maxima)),vertices=[list(map(str,v)) for v in vertices],optimal_vertices=[list(map(str,v)) for v in face],highs_optimum=-result.fun,highs_ranges=highs_ranges)


def fixtures():
    simple=[[1,-1,0],[0,1,-1]]
    alternate=[[1,-1,-1,0],[0,1,1,-1]]
    result=[
        ('linear_chain',simple,[('0','10'),('0','6'),('0','10')],2),
        ('alternate_pathways',alternate,[('0','10')]*4,3),
        ('rank_deficient',[[1,-1,0],[2,-2,0],[-1,1,0]],[('0','10'),('0','7'),('0','4')],2),
        ('infeasible',simple,[('1','10'),('0','0'),('0','10')],2),
        ('fractional_split',[[1,-2,-3,0,0,0],[0,1,1,-1,-1,0],[0,0,0,1,0,-1],[0,0,0,0,1,-1]],[('0','10')]*6,5),
        ('fixed_flux',simple,[('0.125','0.125'),('0','10'),('0','10')],2),
        ('decimal_bounds',simple,[('0','1.234567891'),('0','10'),('0','10')],2),
        ('zero_rank',[[0,0,0],[0,0,0]],[('0','10'),('0','4.2'),('1','3')],1),
        ('zero_optimum',simple,[('0','0'),('0','10'),('0','10')],2),
        ('near_distinct_face',alternate,[('0','1'),('0','0.999999999'),('0','1'),('0','10')],3),
        ('negative_coefficient',[[3,-1,0],[0,2,-3]],[('0','10'),('0','10'),('0','10')],2),
    ]
    result.append(('row_permutation',[alternate[1],alternate[0]],[('0','10')]*4,3))
    result.append(('row_sign',[[-x for x in row] for row in alternate],[('0','10')]*4,3))
    order=[3,2,0,1]
    result.append(('reaction_permutation',[[row[index] for index in order] for row in alternate],[('0','10')]*4,0))
    return result


def freeze(directory):
    directory=pathlib.Path(directory);directory.mkdir(parents=True,exist_ok=True)
    cases=[]
    for name,matrix,bounds,biomass in fixtures():
        text=f'{len(matrix)} {len(bounds)} {biomass}\n'+'\n'.join(' '.join(map(str,row)) for row in matrix)+'\n'+'\n'.join(' '.join(bound) for bound in bounds)+'\n'
        observed=oracle(matrix,bounds,biomass)
        cases.append(dict(name=name,input=text,sha256=hashlib.sha256(text.encode()).hexdigest(),matrix=matrix,bounds=bounds,biomass=biomass,oracle=observed,exit=0 if observed['status']=='optimal' else 11))
    base=cases[0]['input']
    invalid={
        'empty':'', 'metabolite_dimension':'1 3 2\n','reaction_dimension':'2 7 2\n','fractional_dimension':'2.1 3 2\n','biomass_access':'2 3 3\n','negative_dimension':'-2 3 2\n',
        'coefficient_range':base.replace('1 -1 0','4 -1 0'), 'coefficient_fraction':base.replace('1 -1 0','1.1 -1 0'), 'negative_bound':base.replace('0 6','-1 6'), 'bound_range':base.replace('0 6','0 11'), 'reversed_bound':base.replace('0 6','7 6'), 'nan':base.replace('0 6','NaN 6'), 'infinite':base.replace('0 6','0 inf'), 'exponent':base.replace('0 6','0 1e0'), 'truncated':base.rsplit('0 10',1)[0], 'extra':base+'1\n','fraction_budget':base.replace('0 6','0 0.1234567890'),'byte_budget':base+' '*4096,'oversized_integer':base.replace('0 6','0 999999999999999999999'), 'minus_only':base.replace('0 6','- 6'), 'point_only':base.replace('0 6','. 6'), 'nul':base+'\0'
    }
    for name,raw in invalid.items():
        cases.append(dict(name='invalid_'+name,input=raw,sha256=hashlib.sha256(raw.encode()).hexdigest(),exit=10))
    cases.append({**cases[0], 'name':'no_final_newline','input':base.rstrip(),'sha256':hashlib.sha256(base.rstrip().encode()).hexdigest()})
    transformed=base.replace(' ','\t').replace('\n','\r\n')
    cases.append({**cases[0], 'name':'tabs_crlf','input':transformed,'sha256':hashlib.sha256(transformed.encode()).hexdigest()})
    payload=dict(format=1,scipy_version=scipy.__version__,method='HiGHS independent LP + exact rational active-constraint vertex certificates',objective_tolerance=1e-7,range_tolerance=1e-7,scaled_feasibility_tolerance=1e-8,cases=cases)
    (directory/'frozen_oracle.json').write_text(json.dumps(payload,indent=2)+'\n')
    print(len(cases),'cases frozen',hashlib.sha256((directory/'frozen_oracle.json').read_bytes()).hexdigest())

if __name__=='__main__':
    import sys
    freeze(sys.argv[1])
