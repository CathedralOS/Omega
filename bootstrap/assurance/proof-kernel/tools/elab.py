#!/usr/bin/env python3
"""
elab.py — an UNTRUSTED proof elaborator for the proof certificate checker.

The lattice's thesis is "trust by checking, not pedigree": only `check.beta` is trusted, so
any tool that *produces* certificates may be arbitrarily clever and stays outside the trust
path (like `beta-lang-rs` is throwaway scaffolding for `bc`). Writing certificates by hand
means counting de Bruijn indices for individual variables `(v N)` and hypotheses `(hyp N)`
under nested binders — the dominant source of errors. This tool lets proofs be written with
NAMED binders and compiles them to the exact raw syntax `check.beta` consumes.

Surface syntax (s-expressions). Binders name their bound variable; references use the name.

  props : (all x P) (ex x P) (-> P Q) (& P Q) (or P Q) (= A B) bot (pred ID A) (rel ID A B)
  terms : z | 0 1 2 ...        ; numerals expand to s/z
          (s A) (+ A B) (* A B) | nil (cons H T) (++ A B) (len A)
          (k CID A...) (f FID A...) (rec I) (y K)    ; user types/functions
          NAME                 ; resolves to the individual var (v K)
  proofs: (gen x PF) (lam h P PF) NAME(=hyp) (use N) (have h P PF BODY)   ; local lemma
          ; (have h P pf body) binds the proof `pf:P` as hypothesis `h` in `body`
          (app F A) (app* F A B …) (inst PF T) (inst* PF T1 T2 …)   ; *-forms fold the nesting
          (pair A B) (fst P) (snd P) (inl Q P) (inr Q P) (case S F G)
          (absurd Q P) (refl T) (inst PF T) (disj P) (sinj P) (unpack EPF H)
          (wit x BODY T PF)            ; BODY is the exists-body, binding x
          (eqelim x MOT EQ BASE)       ; MOT is the motive, binding the hole x
          (natind x MOT BASE STEP) (listind x MOT BASE STEP)
          (rec cidA cidB x MOT BASE STEP)
  top   : (data CID ARITY R0 R1) (fun FID CID BODY) ... (def N P PF) ... GOAL PROOF
          ; user-type/function declarations and named lemmas, then the goal and its proof

Usage:  elab.py < proof.elab            # prints the raw certificate
        elab.py --check < proof.elab    # elaborate, then run check.beta, print accept/reject
"""
import sys, re, os

def tokenize(s):
    s = re.sub(r';[^\n]*', ' ', s)               # strip ; comments
    return re.findall(r'\(|\)|[^\s()]+', s)

def parse(toks):
    out = []
    while toks:
        out.append(parse_one(toks))
    return out

def parse_one(toks):
    t = toks.pop(0)
    if t == '(':
        lst = []
        while toks[0] != ')':
            lst.append(parse_one(toks))
        toks.pop(0)
        return lst
    return t

def ix(stack, name):
    for i in range(len(stack) - 1, -1, -1):
        if stack[i] == name:
            return len(stack) - 1 - i
    raise SystemExit("elab error: unbound name %r (in scope: %s)" % (name, stack))

def num(n):
    out = "z"
    for _ in range(int(n)):
        out = "(s %s)" % out
    return out

def et(n, iv):  # elaborate a term
    if isinstance(n, str):
        for i in range(len(iv) - 1, -1, -1):      # a bound name shadows the z/nil literals
            if iv[i] == n: return "(v %d)" % (len(iv) - 1 - i)
        if n == 'z': return 'z'
        if n == 'nil': return 'nil'
        if re.fullmatch(r'\d+', n): return num(n)
        raise SystemExit("elab error: unbound term name %r (in scope: %s)" % (n, iv))
    h = n[0]
    if h == 's':   return "(s %s)" % et(n[1], iv)
    if h == '+':   return "(p %s %s)" % (et(n[1], iv), et(n[2], iv))
    if h == '*':   return "(m %s %s)" % (et(n[1], iv), et(n[2], iv))
    if h == 'cons':return "(cons %s %s)" % (et(n[1], iv), et(n[2], iv))
    if h == '++':  return "(app %s %s)" % (et(n[1], iv), et(n[2], iv))
    if h == 'len': return "(len %s)" % et(n[1], iv)
    if h == 'k':   return "(k %s)" % ' '.join([n[1]] + [et(a, iv) for a in n[2:]])
    if h == 'f':   return "(f %s)" % ' '.join([n[1]] + [et(a, iv) for a in n[2:]])
    if h == 'rec': return "(rec %s)" % n[1]
    if h == 'recx':return "(recx %s %s)" % (n[1], et(n[2], iv))   # accumulator recursion: field i, extra:=E
    if h == 'y':   return "(y %s)" % n[1]
    if h == 'v':   return "(v %s)" % n[1]     # raw de Bruijn (fun-rule pattern field)
    raise SystemExit("elab error: bad term %r" % (n,))

def ep(n, iv):  # elaborate a prop
    if isinstance(n, str):
        if n == 'bot': return '(bot)'
        if re.fullmatch(r'[A-Z]', n): return n     # atomic proposition (ATOM char)
        raise SystemExit("elab error: bad prop atom %r" % n)
    h = n[0]
    if h == 'all':  return "(All %s)"    % ep(n[2], iv + [n[1]])
    if h == 'ex':   return "(Exists %s)" % ep(n[2], iv + [n[1]])
    if h == '->':   return "(-> %s %s)"  % (ep(n[1], iv), ep(n[2], iv))
    if h == '&':    return "(& %s %s)"   % (ep(n[1], iv), ep(n[2], iv))
    if h == 'or':   return "(+ %s %s)"   % (ep(n[1], iv), ep(n[2], iv))
    if h == '=':    return "(= %s %s)"   % (et(n[1], iv), et(n[2], iv))
    if h == 'pred': return "(Pred %s %s)" % (n[1], et(n[2], iv))
    if h == 'rel':  return "(Rel %s %s %s)" % (n[1], et(n[2], iv), et(n[3], iv))
    if h == 'bot':  return '(bot)'
    raise SystemExit("elab error: bad prop %r" % (n,))

def epf(n, iv, hy):  # elaborate a proof term
    if isinstance(n, str):
        return "(hyp %d)" % ix(hy, n)             # hypothesis reference
    h = n[0]
    if h == 'gen':    return "(gen %s)" % epf(n[2], iv + [n[1]], hy)
    if h == 'lam':    return "(lam %s %s)" % (ep(n[2], iv), epf(n[3], iv, hy + [n[1]]))
    if h == 'use':    return "(use %s)" % n[1]
    if h == 'have':   # (have name P pf body) -> local lemma: (app (lam name:P body) pf)
        return "(app (lam %s %s) %s)" % (ep(n[2], iv), epf(n[4], iv, hy + [n[1]]), epf(n[3], iv, hy))
    if h == 'app':    return "(app %s %s)" % (epf(n[1], iv, hy), epf(n[2], iv, hy))
    if h == 'app*':                               # (app* F a b …) -> (app (app F a) b) …
        out = epf(n[1], iv, hy)
        for a in n[2:]: out = "(app %s %s)" % (out, epf(a, iv, hy))
        return out
    if h == 'inst*':                              # (inst* PF t1 t2 …) -> nested inst, outermost first
        out = epf(n[1], iv, hy)
        for t in n[2:]: out = "(inst %s %s)" % (out, et(t, iv))
        return out
    if h == 'pair':   return "(pair %s %s)" % (epf(n[1], iv, hy), epf(n[2], iv, hy))
    if h == 'fst':    return "(fst %s)" % epf(n[1], iv, hy)
    if h == 'snd':    return "(snd %s)" % epf(n[1], iv, hy)
    if h == 'inl':    return "(inl %s %s)" % (ep(n[1], iv), epf(n[2], iv, hy))
    if h == 'inr':    return "(inr %s %s)" % (ep(n[1], iv), epf(n[2], iv, hy))
    if h == 'case':   return "(case %s %s %s)" % (epf(n[1], iv, hy), epf(n[2], iv, hy), epf(n[3], iv, hy))
    if h == 'absurd': return "(absurd %s %s)" % (ep(n[1], iv), epf(n[2], iv, hy))
    if h == 'refl':   return "(refl %s)" % et(n[1], iv)
    if h == 'inst':   return "(inst %s %s)" % (epf(n[1], iv, hy), et(n[2], iv))
    if h == 'disj':   return "(disj %s)" % epf(n[1], iv, hy)
    if h == 'sinj':   return "(sinj %s)" % epf(n[1], iv, hy)
    if h == 'unpack': return "(unpack %s %s)" % (epf(n[1], iv, hy), epf(n[2], iv, hy))
    if h == 'wit':    return "(wit %s %s %s)" % (ep(n[2], iv + [n[1]]), et(n[3], iv), epf(n[4], iv, hy))
    if h == 'eqelim': return "(eqelim %s %s %s)" % (ep(n[2], iv + [n[1]]), epf(n[3], iv, hy), epf(n[4], iv, hy))
    if h == 'natind': return "(natind %s %s %s)" % (ep(n[2], iv + [n[1]]), epf(n[3], iv, hy), epf(n[4], iv, hy))
    if h == 'listind':return "(listind %s %s %s)" % (ep(n[2], iv + [n[1]]), epf(n[3], iv, hy), epf(n[4], iv, hy))
    if h == 'rec':    return "(rec %s %s %s %s %s)" % (n[1], n[2], ep(n[4], iv + [n[3]]), epf(n[5], iv, hy), epf(n[6], iv, hy))
    if h == 'prodrec':# (prodrec CID x MOT CASE) — product elim: from MOT on the sole ctor conclude ∀x. MOT
        return "(prodrec %s %s %s)" % (n[1], ep(n[3], iv + [n[2]]), epf(n[4], iv, hy))
    if h == 'memhead':return "(memhead %s %s)" % (et(n[1], iv), et(n[2], iv))      # Mem(x, cons x t)
    if h == 'memtail':return "(memtail %s %s)" % (et(n[1], iv), epf(n[2], iv, hy)) # Mem(x,t) -> Mem(x, cons h t)
    if h == 'memcons':return "(memcons %s)" % epf(n[1], iv, hy)                    # invert on cons
    if h == 'memnil': return "(memnil %s)" % epf(n[1], iv, hy)                     # invert on nil
    if h == 'pnil':   return "(pnil)"                                              # ProdIs(nil, 1)
    if h == 'pcons':  return "(pcons %s %s)" % (et(n[1], iv), epf(n[2], iv, hy))   # ProdIs(t,m)->ProdIs(cons h t, h*m)
    if h == 'prodnilinv':  return "(prodnilinv %s)" % epf(n[1], iv, hy)            # ProdIs(nil,n) -> n=1
    if h == 'prodconsinv': return "(prodconsinv %s)" % epf(n[1], iv, hy)           # ProdIs(cons h t,n) -> ex m. n=h*m & ProdIs(t,m)
    if h == 'permnil':  return "(permnil)"                                         # Perm(nil, nil)
    if h == 'permskip': return "(permskip %s %s)" % (et(n[1], iv), epf(n[2], iv, hy))     # Perm(t1,t2) -> Perm(cons x t1, cons x t2)
    if h == 'permswap': return "(permswap %s %s %s)" % (et(n[1], iv), et(n[2], iv), et(n[3], iv))  # Perm(cons x cons y r, cons y cons x r)
    if h == 'permtrans':return "(permtrans %s %s)" % (epf(n[1], iv, hy), epf(n[2], iv, hy))  # Perm(a,b) & Perm(b,c) -> Perm(a,c)
    raise SystemExit("elab error: bad proof %r" % (n[0],))

def _read_forms(path):
    # Resolve caller-provided paths first, then canonical proof-corpus paths.  The
    # elaborator is untrusted tooling and does not own the libraries it consumes.
    here = os.path.dirname(os.path.abspath(__file__))
    corpus = os.path.join(os.path.dirname(here), 'corpus')
    for cand in (path, os.path.join(corpus, path), os.path.join(here, path)):
        if os.path.exists(cand):
            with open(cand) as fh:
                return parse(tokenize(fh.read()))
    raise SystemExit("elab error: include file not found: %r" % path)

def elaborate(src):
    # (include FILE) splices a shared library: its decls (data/prod/fun/def) are emitted, and its
    # (lemma NAME TYPE PROOF) forms WRAP the main proof as nested `have`s (SRP: shared monoid lemmas
    # live in ONE file, not copied into every fold proof). Purely front-end sugar — the emitted cert
    # is still verified by all three checkers.
    lemmas = []          # (name, type_ast, pf_ast), in declaration order; wrap the final proof
    flat = []            # forms with includes expanded and lemmas hoisted out
    def expand(forms):
        for f in forms:
            if isinstance(f, list) and f and f[0] == 'include':
                expand(_read_forms(f[1]))
            elif isinstance(f, list) and f and f[0] == 'lemma':
                lemmas.append((f[1], f[2], f[3]))
            else:
                flat.append(f)
    expand(parse(tokenize(src)))
    forms = flat
    out = []
    i = 0
    while i < len(forms):
        f = forms[i]
        if isinstance(f, list) and f and f[0] == 'data':
            # (data cid arity r0 r1) — all literals, pass through
            out.append("(data %s)" % ' '.join(f[1:])); i += 1
        elif isinstance(f, list) and f and f[0] == 'prod':
            # (prod cid) — sole-constructor product marker, licenses prodrec; literal pass-through
            out.append("(prod %s)" % f[1]); i += 1
        elif isinstance(f, list) and f and f[0] == 'fun':
            # (fun FID CID body) — body is a term over (y k) args / (rec i) recursion
            out.append("(fun %s %s %s)" % (f[1], f[2], et(f[3], []))); i += 1
        elif isinstance(f, list) and f and f[0] == 'def':
            # (def N P PF)
            out.append("(def %s %s %s)" % (f[1], ep(f[2], []), epf(f[3], [], [])))
            i += 1
        else:
            # remaining two forms: goal prop, then proof — wrap the proof in any included lemmas
            goal = ep(forms[i], [])
            proof_ast = forms[i + 1]
            for (name, ty, pf) in reversed(lemmas):
                proof_ast = ['have', name, ty, pf, proof_ast]
            proof = epf(proof_ast, [], [])
            out.append(goal); out.append(proof)
            i += 2
    return ' '.join(out)

if __name__ == '__main__':
    src = sys.stdin.read()
    cert = elaborate(src)
    if '--check' in sys.argv:
        import subprocess
        exe = sys.argv[sys.argv.index('--check') + 1] if len(sys.argv) > sys.argv.index('--check') + 1 else '/tmp/check.exe'
        r = subprocess.run([exe], input=cert, capture_output=True, text=True)
        print(r.stdout.strip())
    else:
        print(cert)
