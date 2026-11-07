import sys,json,copy
from pathlib import Path
r=Path(__file__).resolve().parents[1]
assert not (r/'validation/oracle.json').exists(), 'refuse to overwrite frozen qualification inputs'
sys.path.insert(0,str(r/'validation'))
from reference import reference
cases={}
def add(name,nodes,bars,analytic=None):
    expected=reference(nodes,bars)
    if analytic:
        for key,values in analytic.items():
            actual=expected[key]
            if isinstance(values,list): assert all(abs(a-b)<1e-12 for a,b in zip(actual,values))
            else: assert abs(actual-values)<1e-12
    cases[name]={'nodes':nodes,'bars':bars,'exit':0,'expected':expected,'analytic':analytic}
    (r/'data'/f'{name}.txt').write_text(f'{len(nodes)} {len(bars)}\n'+''.join(' '.join(map(str,row))+'\n' for row in nodes+bars))
bar=[[0,0,1,1,0,0],[2,0,0,1,10,0]]; bars=[[0,1,100]]
add('bar',bar,bars,{'displacement':[0,0,.2,0],'reaction':[-10,0,0,0],'extension':[.2],'force':[10],'energy':1,'work':2})
add('compression',[[0,0,1,1,0,0],[2,0,0,1,-10,0]],bars)
# 3-4-5 right triangle with pin and roller: horizontal member zero,
# vertical member compression -10; diagonal zero under vertical tip load.
triangle=[[0,0,1,1,0,0],[4,0,0,1,0,0],[4,3,0,0,0,-10]]; tri_bars=[[0,1,400],[1,2,300],[0,2,500]]
add('triangle',triangle,tri_bars,{'displacement':[0,0,0,0,.075,-.1],'force':[0,-10,0],'energy':.5,'work':1})
# Equivalent 90 degree rotation exchanges support flags and load axes.
rotated=[[-y,x,fy,fx,-ly,lx] for x,y,fx,fy,lx,ly in triangle]
add('rotated',rotated,tri_bars)
order=[2,0,1]; reverse={old:new for new,old in enumerate(order)}
add('reordered',[triangle[i] for i in order],[[reverse[b],reverse[a],ea] for a,b,ea in reversed(tri_bars)])
add('unloaded',[[*n[:4],0,0] for n in triangle],tri_bars)
add('support_load',[[0,0,1,1,7,3],[2,0,0,1,10,-4]],bars)
add('stiff_small_displacement',[[0,0,1,1,0,0],[1,0,0,1,1,0]],[[0,1,1000000]])
add('compliant',[[0,0,1,1,0,0],[1000,0,0,1,1000,0]],[[0,1,1]])
add('all_fixed',[[0,0,1,1,3,-2],[2,1,1,1,-7,4]],[[0,1,100]])
# eight nodes, sixteen axial bars, asymmetric rigidity/load distribution
nodes=[[0,0,1,1,0,0],[2,0,0,1,0,0],[4,0,0,0,3,-2],[6,0,0,0,-1,2],[0,2,0,0,1,-3],[2,2,0,0,-2,-4],[4,2,0,0,2,-5],[6,2,0,0,4,-6]]
edges=[(0,1),(1,2),(2,3),(4,5),(5,6),(6,7),(0,4),(1,5),(2,6),(3,7),(0,5),(1,4),(1,6),(2,5),(2,7),(3,6)]
add('dense8',nodes,[[a,b,100+17*i] for i,(a,b) in enumerate(edges)])
add('triangle_horizontal',[[0,0,1,1,0,0],[4,0,0,1,0,0],[4,3,0,0,12,-10]],tri_bars)
# Generate refusal fixtures from known valid source, but no expected solve.
def refuse(name,text,exit=64):
    (r/'data'/f'{name}.txt').write_text(text);cases[name]={'exit':exit}
def text(ns,bs):return f'{len(ns)} {len(bs)}\n'+''.join(' '.join(map(str,row))+'\n' for row in ns+bs)
refuse('mechanism',text([[0,0,1,1,0,0],[2,0,0,0,10,0]],bars),65)
refuse('unloaded_mechanism',text([[0,0,0,0,0,0],[2,0,0,0,0,0]],bars),65)
refuse('no_bars_mechanism',text(bar,[]),65)
refuse('zero_length',text([[0,0,1,1,0,0],[0,0,0,1,10,0]],bars))
refuse('self_bar',text(bar,[[0,0,100]]))
refuse('invalid_index',text(bar,[[0,2,100]]))
refuse('negative_index',text(bar,[[-1,1,100]]))
refuse('zero_ea',text(bar,[[0,1,0]]))
refuse('negative_ea',text(bar,[[0,1,-100]]))
refuse('bad_flag',text([[0,0,2,1,0,0],bar[1]],bars))
for name,token in [('nan','NaN'),('infinity','inf'),('overflow','1000001'),('fraction','1.5'),('exponent','1e2'),('lone_sign','-')]:
    refuse(name,text(bar,bars).replace('100\n',token+'\n'))
refuse('missing',text(bar,bars).rsplit('100',1)[0])
refuse('extra',text(bar,bars)+'1\n')
refuse('overlong',text(bar,bars)+' '*4096)
refuse('bad_dimensions','9 0\n');refuse('too_many_bars','2 17\n');refuse('empty','')
# all-fixed with zero bars is valid and has exact reactions
add('empty_fixed',[[0,0,1,1,3,4],[1,0,1,1,-2,-4]],[])
cases['crlf']=cases['bar']
(r/'data/crlf.txt').write_bytes((r/'data/bar.txt').read_bytes().replace(b'\n',b'\r\n'))
refuse('alphabetic_separator',(r/'data/bar.txt').read_text().replace('2 1\n','2b1\n'))
oracle=r/'validation/oracle.json';assert not oracle.exists()
oracle.write_text(json.dumps({'method':'70-digit Decimal endpoint scatter and partial-pivot dense free solve; analytic bar and 3-4-5 triangle certification','relative_tolerance':1e-7,'absolute_tolerance':2e-12,'equilibrium_relative':1e-8,'cases':cases},indent=2)+'\n')
(r/'input.txt').write_text((r/'data/triangle.txt').read_text())
