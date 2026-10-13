"""Independent exact 27-image enumeration, separate Decimal shell integration.
This program never reads application source or observed output. --freeze creates
pre-execution expectations; validation only reads the frozen values.
"""
from decimal import Decimal, localcontext
from itertools import product, combinations
from pathlib import Path
import json, hashlib, argparse, re
ROOT=Path(__file__).resolve().parent
PI=Decimal("3.1415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679")

def shells(edges):
    with localcontext() as context:
        context.prec=90
        previous=Decimal(0); volumes=[]
        for edge in edges:
            radius=Decimal(edge)
            volumes.append(4*PI*(radius**3-previous**3)/3)
            previous=radius
        return volumes

def analyze(text):
    if len(text.encode())>32768: return {"exit":2}
    tokens=text.split()
    if any(not re.fullmatch(r"-?[0-9]{1,7}", token) or abs(int(token))>1000000 for token in tokens): return {"exit":2}
    numbers=list(map(int,tokens))
    if len(numbers)<4: return {"exit":2}
    box,particles,snapshots,bins=numbers[:4]
    if not (2<=box<=1000 and 2<=particles<=32 and 1<=snapshots<=8 and 1<=bins<=16): return {"exit":2}
    if len(numbers)!=4+bins+3*particles*snapshots: return {"exit":2}
    edges=numbers[4:4+bins]
    if any(a>=b for a,b in zip([0]+edges,edges)) or 2*edges[-1]>box: return {"exit":2}
    coordinates=numbers[4+bins:]
    counts=[0]*bins
    for snapshot in range(snapshots):
        positions=[tuple(Decimal(coordinates[3*(snapshot*particles+i)+axis]%box) for axis in range(3)) for i in range(particles)]
        # Decimal's remainder keeps the sign; canonicalize to [0,L) first.
        positions=[tuple((coordinate+box)%box for coordinate in position) for position in positions]
        for left,right in combinations(positions,2):
            distances=[sum((left[axis]-right[axis]+Decimal(image[axis]*box))**2 for axis in range(3)) for image in product((-1,0,1),repeat=3)]
            minimum=min(distances)
            for index,edge in enumerate(edges):
                if minimum<edge**2 or (index==bins-1 and minimum==edge**2):
                    counts[index]+=1; break
    with localcontext() as context:
        context.prec=80
        volumes=shells(edges); total=0; rows=[]
        for count,volume in zip(counts,volumes):
            total+=count
            expected=Decimal(snapshots*particles*(particles-1))*volume/(2*Decimal(box)**3)
            rows.append({"count":count,"g":str(Decimal(count)/expected),"coordination":str(Decimal(2*total)/Decimal(snapshots*particles)),"shell":str(volume)})
    return {"exit":0,"rows":rows}

def freeze():
    cases={file.stem:analyze(file.read_text()) for file in sorted((ROOT/"data").glob("*.stdin"))}
    (ROOT/"data/oracles.json").write_text(json.dumps(cases,indent=2)+"\n")
    paths=[ROOT/"reference.py",ROOT/"data/oracles.json",*sorted((ROOT/"data").glob("*.stdin"))]
    (ROOT/"data/FROZEN.sha256").write_text("".join(hashlib.sha256(path.read_bytes()).hexdigest()+"  "+str(path.relative_to(ROOT))+"\n" for path in paths))
if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("--freeze",action="store_true");options=parser.parse_args()
    if options.freeze:freeze()
