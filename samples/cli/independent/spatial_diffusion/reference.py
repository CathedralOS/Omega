"""Independent dense energy assembly and pivoted Gaussian elimination oracle.

No CSR, iterative recurrence, Omega source parsing, or observed outputs are used.
Run before native execution; validate.py checks the frozen file's SHA-256.
"""
from pathlib import Path
import hashlib
import json
ROOT = Path(__file__).resolve().parent

def problem(n, diffusion=1., reaction=0., mixed=False):
    count=n*n
    matrix=[[0.]*count for _ in range(count)]
    coordinates=[((column+.5)/n,(row+.5)/n) for row in range(n) for column in range(n)]
    exact=[x+2*y if mixed else x*x+y*y for x,y in coordinates]
    right=[reaction*u+(0 if mixed else -4*diffusion) for u in exact]
    coefficient=diffusion*n*n
    # Each undirected interior edge contributes k/h^2 * (u_i-u_j)^2.
    for row in range(n):
        for column in range(n):
            index=row*n+column
            matrix[index][index]+=reaction
            for neighbor in ([index+1] if column+1<n else [])+([index+n] if row+1<n else []):
                matrix[index][index]+=coefficient
                matrix[neighbor][neighbor]+=coefficient
                matrix[index][neighbor]-=coefficient
                matrix[neighbor][index]-=coefficient
            x,y=coordinates[index]
            for side,on_face,face in [(0,column==0,(0,y)),(1,column==n-1,(1,y)),(2,row==0,(x,0)),(3,row==n-1,(x,1))]:
                if not on_face: continue
                if mixed and side!=0:
                    right[index]+=diffusion*n*{1:1,2:-2,3:2}[side]
                else:
                    bx,by=face
                    boundary=bx+2*by if mixed else bx*bx+by*by
                    matrix[index][index]+=2*coefficient
                    right[index]+=2*coefficient*boundary
    return matrix,right,exact

def gaussian(matrix,right):
    count=len(right)
    rows=[row[:]+[value] for row,value in zip(matrix,right)]
    for column in range(count):
        pivot=max(range(column,count),key=lambda row:abs(rows[row][column]))
        rows[column],rows[pivot]=rows[pivot],rows[column]
        assert abs(rows[column][column])>1e-12
        for row in range(column+1,count):
            ratio=rows[row][column]/rows[column][column]
            for index in range(column+1,count+1): rows[row][index]-=ratio*rows[column][index]
    solution=[0.]*count
    for row in reversed(range(count)):
        solution[row]=(rows[row][-1]-sum(rows[row][column]*solution[column] for column in range(row+1,count)))/rows[row][row]
    return solution

def freeze():
    cases=[]
    for name,code,n,k,reaction,mixed,limit in [
        ('coarse','0',4,1.,0.,False,400),('medium','1',8,1.,0.,False,400),
        ('fine','2',12,1.,0.,False,400),('mixed','3',8,1.,0.,True,400),
        ('reaction','4',8,2.,3.,False,400),('one_iteration','9',4,1.,0.,False,1),
        ('zero_iterations','a',4,1.,0.,False,0)]:
        matrix,right,exact=problem(n,k,reaction,mixed)
        direct=gaussian(matrix,right)
        if reaction==0:
            manufactured=[u if mixed else u-1/(4*n*n) for u in exact]
            assert max(abs(a-b) for a,b in zip(direct,manufactured))<1e-12
        solution=direct
        if limit==0: solution=[0.]*len(right)
        if limit==1:
            image=[sum(a*b for a,b in zip(row,right)) for row in matrix]
            alpha=sum(b*b for b in right)/sum(a*b for a,b in zip(right,image))
            solution=[alpha*b for b in right]
        residual=max(abs(b-sum(a*u for a,u in zip(row,solution))) for row,b in zip(matrix,right))
        cases.append(dict(name=name,input=code,exit=0 if limit>1 else 2,n=n,
                          nonzeros=sum(value!=0 for row in matrix for value in row),
                          solution=solution,exact=exact,error=max(abs(a-b) for a,b in zip(solution,exact)),
                          residual=residual,threshold=1e-9+1e-10*max(abs(b) for b in right),limit=limit))
    rejection='REJECT grid: positive finite diffusion, nonnegative reaction, four valid anchored boundaries, bounded mesh and controls required\n'
    for name,code in [('zero_coefficient','5'),('negative_coefficient','6'),('bad_boundary','7'),('singular_neumann','8'),('negative_reaction','b'),('missing_boundary','c'),('nan_coefficient','d'),('infinite_coefficient','e'),('zero_tolerance','f')]:
        cases.append(dict(name=name,input=code,exit=65,stdout=rejection))
    for name,code in [('empty',''),('unknown','z'),('trailing','0\n'),('extra','00')]:
        cases.append(dict(name=name,input=code,exit=64,stdout='REJECT input: expected exactly one byte 0..9 or a..f\n'))
    encoded=(json.dumps(dict(method='dense undirected-edge energy assembly; pivoted Gaussian elimination; analytic manufactured solution cross-check',numeric_tolerance=4e-8,cases=cases),indent=2)+'\n').encode()
    (ROOT/'data/oracle.json').write_bytes(encoded)
    (ROOT/'data/freeze.sha256').write_text(hashlib.sha256(encoded).hexdigest()+'  oracle.json\n')
    print('frozen',len(cases),'cases',hashlib.sha256(encoded).hexdigest())
if __name__=='__main__':freeze()
