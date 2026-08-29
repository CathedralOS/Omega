#!/usr/bin/env python3
"""Bounded symbolic differential for the canonical Beta compiler edge.

Both symbolic evaluators and both executable references are untrusted. The
rooted checker validates equality of the generated closed-form terms, but that
is not an exact operational-refinement certificate: finite differential trials
are the only connection between those terms and the two written semantics.
This diagnostic catches code-generation drift in the listed fragment while
the exact edge proof remains open.
"""
import sys, os, subprocess, tempfile, random

HERE = os.path.dirname(os.path.abspath(__file__))

def find_repo_root(start):
    """Find the manifest root when this helper is invoked outside a gate script."""
    current = start
    while True:
        if os.path.isfile(os.path.join(current, 'tools', 'lattice', 'paths.sh')):
            return current
        parent = os.path.dirname(current)
        if parent == current:
            raise RuntimeError('cannot find repository root from %s' % start)
        current = parent

REPO_ROOT = os.environ.get('OMEGA_REPO_ROOT')
if not REPO_ROOT:
    REPO_ROOT = find_repo_root(HERE)
BETA_REFERENCE = os.environ.get(
    'OMEGA_PATH_BETA_REFERENCE',
    os.path.join(REPO_ROOT, 'source', 'beta', 'reference'))
BETA_DIFFERENTIAL = HERE
PROOF_KERNEL = os.environ.get(
    'OMEGA_PATH_ALPHA_CHECKER',
    os.path.join(REPO_ROOT, 'source', 'alpha', 'checker'))
ALPHA = os.environ.get(
    'OMEGA_PATH_ALPHA', os.path.join(REPO_ROOT, 'source', 'alpha'))
sys.path.insert(0, HERE)
sys.path.insert(0, BETA_REFERENCE)
sys.path.insert(0, BETA_DIFFERENTIAL)
import alpha_symbolic as S
import beta_symbolic as B                              # source-side symbolic evaluator (the auto-derived meaning)
import beta_interp                                     # concrete Beta interpreter (pins the source meaning)
import generate_straightline
import generate_loop
import generate_composition
import generate_nested
import generate_branch
from beta_parser import lex, Parser
ALPHA_REF = os.path.join(ALPHA, 'alpha_ref.py')
PROVER = os.path.join(PROOF_KERNEL, 'tools', 'prover.py')
if len(sys.argv) != 3:
    raise SystemExit("usage: symbolic_differential.py CHECKER BETA_COMPILER")
CHECK = sys.argv[1]
BC = sys.argv[2]
# The recurrence prelude: user-Nat (data 2=Z, 3=S) + the triangular-sum fun g(0)=0, g(s k)=g(k)+k (fun 90),
# which alpha_symbolic/beta_symbolic emit as ('f',90,t) for `acc += i` loops. Prepended to a cert that mentions it.
REC_PRELUDE = '(data 2 0 0 0) (data 3 1 1 0) (fun 90 2 (k 2)) (fun 90 3 (p (rec 0) (v 0)))'
ZZ_PRELUDE = '(data 5 2 0 0)'                          # the ℤ difference-pair constructor (k 5 pos neg) = pos - neg
MN_PRELUDE = '(data 6 2 0 0)'                          # the monus constructor (k 6 a b) = max(0, a - b)
SV_PRELUDE = '(data 7 1 1 0)'                          # the stream-element constructor (k 7 t) = input[t]
SSUM_PRELUDE = '(data 8 2 0 0)'                        # the stream-sum constructor (k 8 lo hi) = Σ input[lo..hi)
COND_PRELUDE = '(data 9 3 0 0) (data 10 2 0 0) (data 11 2 0 0) (data 12 2 0 0) (data 13 2 0 0)'
DIVMOD_PRELUDE = '(data 14 2 0 0) (data 15 2 0 0)'     # div (k 14 a b) = a/b and mod (k 15 a b) = a%b — opaque
                                                       # (k 9 b t f) = if b then t else f; (k 10..13 L R) = </<=/==/!=

# Every case crosses the real canonical compiler and represents a symbolic
# shape not already covered cheaply by the focused compiler suite.
AUTO_SAMPLES = [
    ("weighted", "cases/weighted.beta"),
    ("tri_down", "cases/tri_down.beta"),
    ("neq", "cases/neq.beta"),
    ("tri_nested", "cases/tri_nested.beta"),
    ("callloop", "cases/callloop.beta"),
    ("temploop", "cases/temploop.beta"),
    ("bytemem", "cases/bytemem.beta"),
    ("fromto", "cases/fromto.beta"),
    ("weightedread", "cases/weightedread.beta"),
    ("absdiff", "cases/absdiff.beta"),
    ("boolval", "cases/boolval.beta"),
    ("condloop", "cases/condloop.beta"),
    ("divguard", "cases/divguard.beta"),
]

def compile_beta_text(text):
    """Run the canonical candidate's direct Beta-to-Alpha-tape edge."""
    return subprocess.run(
        [BC], input=text.encode(), capture_output=True, check=True
    ).stdout

def beta_ref_observe(procs, env, n):
    """Run the concrete Beta interpreter on the given inputs; observe the exit code / stdout byte (mod 256)."""
    rc, out = beta_interp.interpret(procs, bytes(env[i] for i in range(n)))
    return (out[0] if out else rc) & 0xFF

def prove_equiv(label, text, tape, ok_msg, quiet_perturb=False, trials=8, teeth=True):
    """Compare generated symbolic terms and differentially ground both sides."""
    try:
        C, nC = S.symexec(tape)                         # what the machine code computes
        M, nM = B.meaning(text)                         # what the source means
    except (S.Unsupported, B.Unsupported) as e:
        print("  FAIL %-26s : outside the modelled fragment (%s)" % (label, e)); return False
    if nC != nM:
        print("  FAIL %-26s : arity mismatch code=%d source=%d" % (label, nC, nM)); return False
    procs = Parser(lex(text)).parse()
    streamy = S._has_stream(C) or B._has_stream(M)     # a read-loop: pad the input vector (bounds are ≤ 6)
    for _ in range(1 if (nC == 0 and not streamy) else trials):  # differential vs each derivation's OWN reference
        vec = [random.randint(0, 6) for _ in range(nC + (40 if streamy else 0))]
        env = {i: vec[i] for i in range(len(vec))}
        env['in'] = vec
        vc, vm = S.evaluate(C, env), B.evaluate(M, env)
        if vc >= 256 or vm >= 256:
            continue
        va = run_ref(tape, vec)                                  # the actual bytecode (alpha VM)
        vb = beta_ref_observe(procs, env, len(vec))              # the actual source (beta interpreter)
        if vc % 256 != va:
            print("  FAIL %-26s : alpha_symbolic ≠ bytecode (sym=%s vm=%s at %s)\n%s" % (label, vc, va, env, text)); return False
        if vm % 256 != vb:
            print("  FAIL %-26s : beta_symbolic ≠ source interp (sym=%s interp=%s at %s)\n%s" % (label, vm, vb, env, text)); return False
    cterm = S.render(C)
    def prove_eq(rhs):
        g = '(= %s %s)' % (cterm, rhs)
        for _ in range(nC):
            g = '(All %s)' % g
        if '(f ' in g or '(k ' in g:                   # user-fun recurrence and/or ℤ difference-pair: prover.py
            prelude = ((REC_PRELUDE if '(f ' in g else '')
                       + (' ' + ZZ_PRELUDE if '(k 5 ' in g else '')
                       + (' ' + MN_PRELUDE if '(k 6 ' in g else '')
                       + (' ' + SV_PRELUDE if '(k 7 ' in g else '')
                       + (' ' + SSUM_PRELUDE if '(k 8 ' in g else '')
                       + (' ' + COND_PRELUDE if any(('(k %d ' % i) in g for i in (9, 10, 11, 12, 13)) else '')
                       + (' ' + DIVMOD_PRELUDE if any(('(k %d ' % i) in g for i in (14, 15)) else ''))
            proof = '(refl %s)' % cterm                # can't parse these, so emit a direct refl cert (valid iff
            for _ in range(nC):                        # C conv rhs) with the needed decls prepended; check.beta decides
                proof = '(gen %s)' % proof
            cert = '%s %s %s' % (prelude.strip(), g, proof)
            verdict = subprocess.run([CHECK], input=cert, capture_output=True, text=True).stdout.strip()
            return verdict == 'accept'
        return prove(g)                                # Peano goal: prover.py searches, check.beta validates
    if not prove_eq(B.render(M)):                      # bc output ≡ source meaning, ∀ inputs
        print("  FAIL %-26s : could not prove code ≡ source meaning\n%s" % (label, text)); return False
    if teeth and prove_eq('(s %s)' % B.render(M)):     # teeth: a perturbed meaning must NOT be provable
        print("  FAIL %-26s : proved a WRONG (perturbed) meaning\n%s" % (label, text)); return False
    if not quiet_perturb:
        print("  ok   %-26s : %s" % (label, ok_msg))
    return True

def check_auto(label, srcrel):
    src = os.path.join(HERE, srcrel)
    text = open(src).read()
    return prove_equiv(label, text, compile_beta_text(text),
                       "generated terms equal; finite pins agree")

def check_fuzz(seed):
    text = generate_straightline.program(seed)
    # teeth/perturbation is exercised by the curated samples; fuzz keeps it lean (positive proof + both pins)
    return prove_equiv("fuzz seed %d" % seed, text, compile_beta_text(text), "", quiet_perturb=True, trials=8, teeth=False)

def check_loop_fuzz(seed):
    text = generate_loop.program(seed)
    return prove_equiv("loop seed %d" % seed, text, compile_beta_text(text), "", quiet_perturb=True, trials=8, teeth=False)

def check_compose_fuzz(seed):
    text = generate_composition.program(seed)
    return prove_equiv("compose seed %d" % seed, text, compile_beta_text(text), "", quiet_perturb=True, trials=8, teeth=False)

def check_nested_fuzz(seed):
    text = generate_nested.program(seed)
    return prove_equiv("nested seed %d" % seed, text, compile_beta_text(text), "", quiet_perturb=True, trials=8, teeth=False)

def check_fork_fuzz(seed):
    text = generate_branch.program(seed)
    return prove_equiv("fork seed %d" % seed, text, compile_beta_text(text), "", quiet_perturb=True, trials=8, teeth=False)

def run_ref(tape, stdin_bytes):
    with tempfile.NamedTemporaryFile(delete=False) as f:
        f.write(tape); path = f.name
    try:
        try:
            r = subprocess.run([sys.executable, ALPHA_REF, path], input=bytes(stdin_bytes),
                               capture_output=True, timeout=30)
        except subprocess.TimeoutExpired:
            return None                                # a DIVERGING tape: report as a differential mismatch
        return (r.stdout[0] if r.stdout else r.returncode) & 0xFF     # write -> byte ; halt -> exit code
    finally:
        os.unlink(path)

def prove(goal):
    """Ask prover.py for a certificate of `goal` and validate it with check.beta. Returns True iff accepted."""
    try:
        cert = subprocess.run([sys.executable, PROVER, goal], capture_output=True, text=True, timeout=60).stdout.strip()
    except subprocess.TimeoutExpired:
        return False
    if not cert or cert == 'unprovable':
        return False
    v = subprocess.run([CHECK], input=cert, capture_output=True, text=True).stdout.strip()
    return v == 'accept'

def main():
    random.seed(1234321)
    total = 0; passed = 0
    print(" canonical compiler symbolic cases:")
    for label, srcrel in AUTO_SAMPLES:
        total += 1; passed += check_auto(label, srcrel)

    generated = [
        ("straight-line", check_fuzz, int(os.environ.get('BETA_DIFF_FUZZ', '1'))),
        ("loop", check_loop_fuzz, int(os.environ.get('BETA_DIFF_LOOP', '1'))),
        ("composition", check_compose_fuzz, int(os.environ.get('BETA_DIFF_COMPOSE', '1'))),
        ("nested", check_nested_fuzz, int(os.environ.get('BETA_DIFF_NESTED', '1'))),
        ("branch", check_fork_fuzz, int(os.environ.get('BETA_DIFF_BRANCH', '1'))),
    ]
    for family, check, count in generated:
        family_passed = 0
        for seed in range(1, count + 1):
            total += 1
            ok = check(seed)
            passed += ok
            family_passed += ok
        print("   %s: %d/%d bounded generated cases agree" %
              (family, family_passed, count))

    print("%d/%d bounded symbolic differential checks passed" % (passed, total))
    sys.exit(0 if passed == total else 1)

if __name__ == '__main__':
    main()
