# Emits the proof-library prefix for STATIC CONTRACT DISCHARGE: lemmas the epsilon compiler
# cites (via `(use N)`) to discharge `ensures` obligations that hold only up to a theorem,
# not by reduction alone. Isolated from gen-lib2d.py (the convergence's library) so the two
# can't perturb each other's def ids. A linker concatenates this with the compiler's per-
# contract citation; the trust anchor checks library + citation together.
import sys, os, re
HERE=os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
from delab import decompile
from elab import elaborate
txt=open(os.path.join(HERE,'test.sh')).read()
C={m.group(1):m.group(2) for m in re.finditer(r'chk "([^"]*)"\s+"([^"]*)"\s+(accept|reject)', txt, re.DOTALL)}
def groups(s):
    res=[];d=0;g=None
    for i,ch in enumerate(s):
        if ch=='(':
            if d==0:g=i
            d+=1
        elif ch==')':
            d-=1
            if d==0:res.append((g,i+1))
    return res
def cof(n):
    c=C[n]
    while c.count('(')<c.count(')'): c=c.rstrip()[:-1]
    return decompile(c)
def strip_decls(raw):
    return ''.join(raw[g[0]:g[1]] for g in groups(raw) if not (raw[g[0]:g[1]].startswith('(data') or raw[g[0]:g[1]].startswith('(fun')))
def shift(raw, base):
    def su(s):
        for mm in sorted(set(int(x) for x in re.findall(r'\(use (\d+)\)',s)),reverse=True):
            s=s.replace('(use %d)'%mm,'(useT%d)'%(mm+base))
        return s.replace('(useT','(use ')
    items=[raw[g[0]:g[1]] for g in groups(raw)]; out=[]; idx=base; i=0
    while i<len(items):
        it=items[i]
        if it.startswith('(def'):
            body=it[it.index(' ',5)+1:-1]; bg=groups(body)
            out.append("(def %d %s %s)"%(idx,body[bg[0][0]:bg[0][1]],su(body[bg[1][0]:bg[1][1]]))); idx+=1; i+=1
        else:
            out.append("(def %d %s %s)"%(idx,su(items[i]),su(items[i+1]))); idx+=1; i+=2
    return ''.join(out), idx-1
idx=0; blk=""; ids={}
def emb(name):
    global blk, idx
    b,last=shift(strip_decls(cof(name)),idx); blk+=b; idx=last+1; return last
ids['add-zero-right'] = emb('add zero on the right (a+z = a)')   # discharge.rs: ADD_ZERO_RIGHT
ids['add-commutes']   = emb('+ commutative')                     # discharge.rs: ADD_COMMUTES (pulls deps into 1..4)
ids['le-trans']       = emb('le trans')                          # discharge.rs: LE_TRANS (pulls deps into 6..8)
ids['mult-commutes']  = emb('mult commutes')                     # discharge.rs: MULT_COMMUTES (pulls deps into 10..19)
assert ids['add-zero-right']==0 and ids['add-commutes']==5 and ids['le-trans']==9 and ids['mult-commutes']==20, \
    "contract library def ids drifted: %r (discharge.rs pins add-zero-right=0, add-commutes=5, le-trans=9, mult-commutes=20)"%ids
full=elaborate(blk+" (= z z) (refl z)")
sys.stdout.write(full.rsplit("(= z z) (refl z)",1)[0].rstrip())
