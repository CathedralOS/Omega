"""Independent unit-square enumeration oracle; does not use rectangle overlap."""
from pathlib import Path
import json, hashlib
ROOT = Path(__file__).resolve().parent
CONFIGS = [(0,0,2,2,2,2), (1,1,2,2,2,2), (0,0,4,4,1,1),
           (6,0,2,2,2,2), (0,0,3,3,2,2), (0,0,2,2,2,2)]
HEADER = 'LANDCOVER exact planar area; enter fixture 1..6 (7..9 invalid)\n'
LABELS = 'CELL row col forest water urban nodata outside dominant (fractions of full cell)\n'
fixtures=[]
for mode, (x,y,w,h,columns,rows) in enumerate(CONFIGS,1):
    classes = [1,2,3,0] if mode != 6 else [0]*4
    cells=[]; totals=[0]*5; output=HEADER+LABELS
    for row in range(rows):
        for column in range(columns):
            counts=[0]*5
            for yy in range(y+row*h,y+(row+1)*h):
                for xx in range(x+column*w,x+(column+1)*w):
                    bucket = 4 if not (0<=xx<4 and 0<=yy<4) else {1:0,2:1,3:2,0:3}[classes[(yy//2)*2+xx//2]]
                    counts[bucket]+=1
            valid=counts[:3]; winner=valid.index(max(valid))+1 if max(valid)>0 else 0
            cells.append(dict(row=row,column=column,numerators=counts,denominator=w*h,dominant=winner))
            totals=[a+b for a,b in zip(totals,counts)]
            output+=f'CELL {row:03} {column:03} '+''.join(f'{n:03}/{w*h:03} ' for n in counts)+f'{winner:03}\n'
    area=w*h*columns*rows
    assert sum(totals)==area
    output+='TOTAL forest water urban nodata outside area '+' '.join(f'{n:03}' for n in totals+[area])+'\n'
    output+='AUDIT PASS: class conservation and full-area closure; tolerance=0\n'
    fixtures.append(dict(mode=mode,grid=[x,y,w,h,columns,rows],classes=classes,cells=cells,totals=totals,area=area))
    (ROOT/f'expected_{mode}.txt').write_text(output)
    (ROOT/f'input_{mode}.txt').write_text(str(mode)+'\n')
(ROOT/'fixtures.json').write_text(json.dumps(dict(model='planar integer metre ticks; half-open cells; no-data=0; full-cell denominator; lowest valid class wins ties',absolute_tolerance=0,relative_tolerance=0,fixtures=fixtures),indent=2)+'\n')
for name in ['fixtures.json']+[f'expected_{i}.txt' for i in range(1,7)]:
    print(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),name)
