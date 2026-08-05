#!/usr/bin/env python3
"""
delab.py — the inverse of elab.py: decompile a raw proof certificate (de Bruijn) back into
the named-binder surface syntax. Two uses:

  1. Migration — convert the hand-written gate certificates into maintainable .elab sources.
  2. Round-trip validation — for any cert C, `elab.py < (delab.py < C)` should re-accept,
     and ideally reproduce C byte-for-byte. This cross-checks BOTH tools: a bug in either
     surfaces as a changed or rejected certificate.

Each de Bruijn binder is given a fresh, globally-unique name (so there is never shadowing
ambiguity); references resolve by depth exactly as the checker reads them.

Scope: handles CLOSED proofs (the entire normal corpus). A goal with a FREE individual
variable — `(v k)` with no enclosing binder, e.g. the eigenvariable edge-case cert — has no
name to resolve to and is out of scope; so are deliberately ill-scoped reject-certs (an
out-of-range `(v k)` is exactly the bug they pin, and is correctly un-nameable).

Usage:  delab.py < cert.txt              # print the named-binder .elab source
        delab.py --roundtrip ./check.exe < cert.txt   # delab, re-elab, diff + check
"""
import sys
from elab import tokenize, parse

class Ctx:
    def __init__(self): self.n = 0
    def fresh(self, p):
        self.n += 1
        return "%s%d" % (p, self.n)

def ref(stack, k):
    return stack[len(stack) - 1 - int(k)]

def dt(n, iv):  # decompile a term
    if isinstance(n, str):
        return n            # z, nil
    h = n[0]
    if h == 'v':                                  # ivar reference, or fun-rule pattern field (out of ctx)
        k = int(n[1])
        return ref(iv, n[1]) if k < len(iv) else "(v %s)" % n[1]
    if h == 's':   return "(s %s)" % dt(n[1], iv)
    if h == 'p':   return "(+ %s %s)" % (dt(n[1], iv), dt(n[2], iv))
    if h == 'm':   return "(* %s %s)" % (dt(n[1], iv), dt(n[2], iv))
    if h == 'cons':return "(cons %s %s)" % (dt(n[1], iv), dt(n[2], iv))
    if h == 'app': return "(++ %s %s)" % (dt(n[1], iv), dt(n[2], iv))
    if h == 'len': return "(len %s)" % dt(n[1], iv)
    if h == 'k':   return "(k %s)" % ' '.join([n[1]] + [dt(a, iv) for a in n[2:]])
    if h == 'f':   return "(f %s)" % ' '.join([n[1]] + [dt(a, iv) for a in n[2:]])
    if h == 'rec': return "(rec %s)" % n[1]
    if h == 'recx':return "(recx %s %s)" % (n[1], dt(n[2], iv))    # accumulator recursion: field i, extra E
    if h == 'y':   return "(y %s)" % n[1]
    raise SystemExit("delab: bad term %r" % (n,))

def dp(n, iv, c):  # decompile a prop
    if isinstance(n, str):
        return n            # atomic proposition (ATOM char)
    h = n[0]
    if h == 'All':    x = c.fresh('x'); return "(all %s %s)" % (x, dp(n[1], iv + [x], c))
    if h == 'Exists': x = c.fresh('x'); return "(ex %s %s)" % (x, dp(n[1], iv + [x], c))
    if h == '->':     return "(-> %s %s)" % (dp(n[1], iv, c), dp(n[2], iv, c))
    if h == '&':      return "(& %s %s)" % (dp(n[1], iv, c), dp(n[2], iv, c))
    if h == '+':      return "(or %s %s)" % (dp(n[1], iv, c), dp(n[2], iv, c))
    if h == '=':      return "(= %s %s)" % (dt(n[1], iv), dt(n[2], iv))
    if h == 'bot':    return "bot"
    if h == 'Pred':   return "(pred %s %s)" % (n[1], dt(n[2], iv))
    if h == 'Rel':    return "(rel %s %s %s)" % (n[1], dt(n[2], iv), dt(n[3], iv))
    raise SystemExit("delab: bad prop %r" % (n,))

def dpf(n, iv, hy, c):  # decompile a proof
    h = n[0]
    if h == 'hyp':    return ref(hy, n[1])
    if h == 'use':    return "(use %s)" % n[1]
    if h == 'gen':    x = c.fresh('x'); return "(gen %s %s)" % (x, dpf(n[1], iv + [x], hy, c))
    if h == 'lam':    z = c.fresh('h'); return "(lam %s %s %s)" % (z, dp(n[1], iv, c), dpf(n[2], iv, hy + [z], c))
    if h == 'app':    return "(app %s %s)" % (dpf(n[1], iv, hy, c), dpf(n[2], iv, hy, c))
    if h == 'pair':   return "(pair %s %s)" % (dpf(n[1], iv, hy, c), dpf(n[2], iv, hy, c))
    if h == 'fst':    return "(fst %s)" % dpf(n[1], iv, hy, c)
    if h == 'snd':    return "(snd %s)" % dpf(n[1], iv, hy, c)
    if h == 'inl':    return "(inl %s %s)" % (dp(n[1], iv, c), dpf(n[2], iv, hy, c))
    if h == 'inr':    return "(inr %s %s)" % (dp(n[1], iv, c), dpf(n[2], iv, hy, c))
    if h == 'case':   return "(case %s %s %s)" % (dpf(n[1], iv, hy, c), dpf(n[2], iv, hy, c), dpf(n[3], iv, hy, c))
    if h == 'absurd': return "(absurd %s %s)" % (dp(n[1], iv, c), dpf(n[2], iv, hy, c))
    if h == 'refl':   return "(refl %s)" % dt(n[1], iv)
    if h == 'inst':   return "(inst %s %s)" % (dpf(n[1], iv, hy, c), dt(n[2], iv))
    if h == 'disj':   return "(disj %s)" % dpf(n[1], iv, hy, c)
    if h == 'sinj':   return "(sinj %s)" % dpf(n[1], iv, hy, c)
    if h == 'unpack': return "(unpack %s %s)" % (dpf(n[1], iv, hy, c), dpf(n[2], iv, hy, c))
    if h == 'wit':    x = c.fresh('x'); return "(wit %s %s %s %s)" % (x, dp(n[1], iv + [x], c), dt(n[2], iv), dpf(n[3], iv, hy, c))
    if h == 'eqelim': x = c.fresh('x'); return "(eqelim %s %s %s %s)" % (x, dp(n[1], iv + [x], c), dpf(n[2], iv, hy, c), dpf(n[3], iv, hy, c))
    if h == 'natind': x = c.fresh('x'); return "(natind %s %s %s %s)" % (x, dp(n[1], iv + [x], c), dpf(n[2], iv, hy, c), dpf(n[3], iv, hy, c))
    if h == 'listind':x = c.fresh('x'); return "(listind %s %s %s %s)" % (x, dp(n[1], iv + [x], c), dpf(n[2], iv, hy, c), dpf(n[3], iv, hy, c))
    if h == 'rec':    x = c.fresh('x'); return "(rec %s %s %s %s %s %s)" % (n[1], n[2], x, dp(n[3], iv + [x], c), dpf(n[4], iv, hy, c), dpf(n[5], iv, hy, c))
    if h == 'prodrec':x = c.fresh('x'); return "(prodrec %s %s %s %s)" % (n[1], x, dp(n[2], iv + [x], c), dpf(n[3], iv, hy, c))
    if h == 'memhead':return "(memhead %s %s)" % (dt(n[1], iv), dt(n[2], iv))
    if h == 'memtail':return "(memtail %s %s)" % (dt(n[1], iv), dpf(n[2], iv, hy, c))
    if h == 'memcons':return "(memcons %s)" % dpf(n[1], iv, hy, c)
    if h == 'memnil': return "(memnil %s)" % dpf(n[1], iv, hy, c)
    if h == 'pnil':   return "(pnil)"
    if h == 'pcons':  return "(pcons %s %s)" % (dt(n[1], iv), dpf(n[2], iv, hy, c))
    if h == 'prodnilinv':  return "(prodnilinv %s)" % dpf(n[1], iv, hy, c)
    if h == 'prodconsinv': return "(prodconsinv %s)" % dpf(n[1], iv, hy, c)
    raise SystemExit("delab: bad proof %r" % (h,))

def decompile(src):
    forms = parse(tokenize(src))
    out = []
    i = 0
    while i < len(forms):
        f = forms[i]
        c = Ctx()
        if isinstance(f, list) and f and f[0] == 'data':
            out.append("(data %s)" % ' '.join(f[1:])); i += 1
        elif isinstance(f, list) and f and f[0] == 'prod':
            out.append("(prod %s)" % f[1]); i += 1
        elif isinstance(f, list) and f and f[0] == 'fun':
            out.append("(fun %s %s %s)" % (f[1], f[2], dt(f[3], []))); i += 1
        elif isinstance(f, list) and f and f[0] == 'def':
            out.append("(def %s %s %s)" % (f[1], dp(f[2], [], c), dpf(f[3], [], [], c))); i += 1
        else:
            out.append(dp(forms[i], [], c))
            c2 = Ctx(); out.append(dpf(forms[i + 1], [], [], c2)); i += 2
    return '\n'.join(out)

if __name__ == '__main__':
    src = sys.stdin.read()
    surface = decompile(src)
    if '--roundtrip' in sys.argv:
        import subprocess
        from elab import elaborate
        exe = sys.argv[sys.argv.index('--roundtrip') + 1]
        recompiled = elaborate(surface)
        orig = ' '.join(tokenize(src))
        same = ' '.join(tokenize(recompiled)) == orig
        verdict = subprocess.run([exe], input=recompiled, capture_output=True, text=True).stdout.strip()
        print("byte-identical:" , same, "| recompiled checks:", verdict)
    else:
        print(surface)
