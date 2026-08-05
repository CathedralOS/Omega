# Emits the proof-library prefix: bounds-2d as a referenceable proof-kernel `def` (id 30),
# preceded by the lemmas its proof needs. A linker concatenates this with a compiler's
# linkage output; the trust anchor checks library + linkage together. Regenerated from
# the banked bounds-2d theorem so it never drifts. (id 30 is the certify-linked contract)
import sys, os
HERE=os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import re
from delab import decompile
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
from elab import elaborate
# the proof library: theorems the certificate producers cite, each a referenceable def.
idx=0; blk=""; ids={}
def emb(name):
    global blk, idx
    b,last=shift(strip_decls(cof(name)),idx); blk+=b; idx=last+1; return last
ids['bounds-2d']    = emb('2D array bounds (i<m & j<n -> i*n+j < m*n)')   # certify-linked / certify-loop
ids['lt-le-trans']  = emb('lt le trans')                                 # certify-loop
ids['mult-overflow']= emb('mult overflow bound (a<B & b<C -> a*b < B*C)') # certify-mul
assert ids['bounds-2d']==30 and ids['lt-le-trans']==34 and ids['mult-overflow']==66, \
    "library def ids drifted: %r (certify-linked uses 30; certify-loop 30 & 34; certify-mul 66)"%ids
# emit RAW defs (what check.exe reads): elaborate with a throwaway goal/proof, then strip it
full=elaborate(blk+" (= z z) (refl z)")
sys.stdout.write(full.rsplit("(= z z) (refl z)",1)[0].rstrip())
