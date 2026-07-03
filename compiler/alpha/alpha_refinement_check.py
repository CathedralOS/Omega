#!/usr/bin/env python3
# alpha_refinement_check.py CHECK_EXE — the INSTRUCTION-LEVEL REFINEMENT gate (loop-free arithmetic fragment).
# For each hand-built Alpha program with a claimed source meaning, it:
#   1. SYMBOLICALLY EXECUTES the machine code (alpha_symbolic.py) to a closed-form expression over its inputs;
#   2. DIFFERENTIALLY validates that expression against the concrete VM (alpha_ref.py) on random inputs — so
#      the symbolic engine is pinned to real execution, exactly as vm-fuzz pins the seeds;
#   3. PROVES the derived expression equals the claimed meaning FOR ALL INPUTS by handing the universal goal
#      to prover.py and validating its certificate with the trust anchor (check.beta).
# A genuine refinement must pass all three; a WRONG claim (the compiler emitting code for a different function)
# must FAIL step 3 — no certificate the kernel accepts. This is "the output certifies the compiler" reaching
# down to the machine-code level: the alpha program provably computes the intended function, checked without
# running it. UNTRUSTED producer, kernel-checked result.
import sys, os, subprocess, tempfile, random

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, '..', 'beta-lang-py'))
import alpha_symbolic as S
import beta_symbolic as B                              # source-side symbolic evaluator (the auto-derived meaning)
import beta_interp                                     # concrete Beta interpreter (pins the source meaning)
import refinement_fuzz_gen                             # random straight-line arithmetic Beta programs
import refinement_loop_gen                              # random data-dependent linear-loop Beta programs
import refinement_compose_gen                           # random composed programs: pre-loop + loop + post-loop
from bc2 import lex, Parser
ALPHA_REF = os.path.join(HERE, 'alpha_ref.py')
PROVER = os.path.join(HERE, '..', 'delta', 'prover.py')
CHECK = sys.argv[1]
BC = sys.argv[2] if len(sys.argv) > 2 else None       # bc.exe — enables the real-bc-output samples
ASM = sys.argv[3] if len(sys.argv) > 3 else None      # the beta assembler exe

# ---- a minimal raw-byte Alpha assembler (encoding per alpha_ref.py) ---------------------------------
def imm(d, k): return bytes([0x01, d]) + int(k).to_bytes(8, 'little')
def mov(d, s): return bytes([0x02, d, s])
def add(d, s): return bytes([0x03, d, s])
def mul(d, s): return bytes([0x05, d, s])
def read(d):   return bytes([0x11, d])
def write(s):  return bytes([0x12, s])
def halt(d):   return bytes([0x00, d])

def nat_s(k):                                  # claimed-meaning constant as s^k z (source text)
    return 'z' if k == 0 else '(s %s)' % nat_s(k - 1)

# programs: (name, tape, claimed_meaning_over_(v i), expect)  where expect in {"refines","differs"}
# inputs are (v 0),(v 1),... in read order; a REFINES claim must be proven, a DIFFERS claim must NOT be.
PROGRAMS = [
    ("a+b  ⊑ a+b   (identity)",   read(0) + read(1) + add(0, 1) + write(0), "(p (v 0) (v 1))", "refines"),
    ("a+b  ⊑ b+a   (+ commutes)", read(0) + read(1) + add(0, 1) + write(0), "(p (v 1) (v 0))", "refines"),
    ("a*b  ⊑ b*a   (* commutes)", read(0) + read(1) + mul(0, 1) + write(0), "(m (v 1) (v 0))", "refines"),
    ("a+a  ⊑ 2*a",                read(0) + mov(1, 0) + add(0, 1) + write(0), "(m (s (s z)) (v 0))", "refines"),
    ("a+3  ⊑ 3+a",                read(0) + imm(1, 3) + add(0, 1) + write(0), "(p (s (s (s z))) (v 0))", "refines"),
    ("a*5  ⊑ 5*a   (halt-out)",   read(0) + imm(1, 5) + mul(0, 1) + halt(0), "(m (s (s (s (s (s z))))) (v 0))", "refines"),
    ("a+b  ⋢ a*b   (wrong claim)", read(0) + read(1) + add(0, 1) + write(0), "(m (v 0) (v 1))", "differs"),
    ("a+3  ⋢ 3*a   (wrong claim)", read(0) + imm(1, 3) + add(0, 1) + write(0), "(m (s (s (s z))) (v 0))", "differs"),
]

# REAL bc-compiled programs — the genuine payoff, now FULLY AUTOMATIC: no human writes the meaning. For each
# source, alpha_symbolic derives what the COMPILED code computes and beta_symbolic derives what the SOURCE
# means; the gate proves the two agree for ALL inputs. The loop/recursion samples have data-independent
# control flow, so BOTH evaluators unroll them identically. Each entry: (label, source-path).
AUTO_SAMPLES = [
    ("sum2      (a+b)",              "refinement-samples/sum2.beta"),
    ("prod2     (a*b)",              "refinement-samples/prod2.beta"),
    ("dbl       (a+a)",             "refinement-samples/dbl.beta"),
    ("affine    (a+a+a+1)",         "refinement-samples/affine.beta"),
    ("triple    (LOOP: +=a ×3)",    "refinement-samples/triple.beta"),
    ("muln      (DATA-DEP LOOP →n*a)", "refinement-samples/muln.beta"),
    ("muln_le   (DATA-DEP LOOP i<=n →(n+1)*a)", "refinement-samples/muln_le.beta"),
    ("countn    (DATA-DEP LOOP →n)",   "refinement-samples/countn.beta"),
    ("sumto(10) (concrete LOOP)",   "../beta-lang-rs/examples/sumto.beta"),
    ("fact(5)   (RECURSION)",       "../beta-lang-rs/examples/factorial.beta"),
    ("answer    (6*7)",             "../beta-lang-rs/examples/answer.beta"),
    ("double    (double(21))",      "../beta-lang-rs/examples/double.beta"),
]

def compile_beta_text(text):
    """bc.exe < text | asm  ->  raw alpha tape bytes."""
    asm = subprocess.run([BC], input=text.encode(), capture_output=True).stdout
    with tempfile.NamedTemporaryFile(delete=False) as f:
        f.write(asm); apath = f.name
    try:
        tape = subprocess.run([ASM], stdin=open(apath, 'rb'), capture_output=True).stdout
    finally:
        os.unlink(apath)
    return tape

def beta_ref_observe(procs, env, n):
    """Run the concrete Beta interpreter on the given inputs; observe the exit code / stdout byte (mod 256)."""
    rc, out = beta_interp.interpret(procs, bytes(env[i] for i in range(n)))
    return (out[0] if out else rc) & 0xFF

def prove_equiv(label, text, tape, ok_msg, quiet_perturb=False, trials=40, teeth=True):
    """Derive the COMPILED meaning (alpha_symbolic) and the SOURCE meaning (beta_symbolic); pin each
    INDEPENDENTLY to its own reference — the compiled meaning to the actual bytecode (alpha_ref), the source
    meaning to the source interpreter (beta_interp) — then prove they are equal for all inputs (and a
    perturbation is not). The two pins are what make the kernel proof of (= C M) certify the COMPILER: C is
    tied to what the machine really does, M to what the source really means, and the proof ties C to M."""
    try:
        C, nC = S.symexec(tape)                         # what the machine code computes
        M, nM = B.meaning(text)                         # what the source means
    except (S.Unsupported, B.Unsupported) as e:
        print("  FAIL %-26s : outside the modelled fragment (%s)" % (label, e)); return False
    if nC != nM:
        print("  FAIL %-26s : arity mismatch code=%d source=%d" % (label, nC, nM)); return False
    procs = Parser(lex(text)).parse()
    for _ in range(1 if nC == 0 else trials):          # differential: each derivation vs its OWN reference VM
        env = {i: random.randint(0, 6) for i in range(nC)}
        vc, vm = S.evaluate(C, env), B.evaluate(M, env)
        if vc >= 256 or vm >= 256:
            continue
        va = run_ref(tape, [env[i] for i in range(nC)])          # the actual bytecode (alpha VM)
        vb = beta_ref_observe(procs, env, nC)                    # the actual source (beta interpreter)
        if vc % 256 != va:
            print("  FAIL %-26s : alpha_symbolic ≠ bytecode (sym=%s vm=%s at %s)\n%s" % (label, vc, va, env, text)); return False
        if vm % 256 != vb:
            print("  FAIL %-26s : beta_symbolic ≠ source interp (sym=%s interp=%s at %s)\n%s" % (label, vm, vb, env, text)); return False
    def univ(rhs):
        g = '(= %s %s)' % (S.render(C), rhs)
        for _ in range(nC):
            g = '(All %s)' % g
        return g
    if not prove(univ(B.render(M))):                   # bc output ≡ source meaning, ∀ inputs
        print("  FAIL %-26s : could not prove code ≡ source meaning\n%s" % (label, text)); return False
    if teeth and prove(univ('(s %s)' % B.render(M))):  # teeth: a perturbed meaning must NOT be provable
        print("  FAIL %-26s : proved a WRONG (perturbed) meaning\n%s" % (label, text)); return False
    if not quiet_perturb:
        print("  ok   %-26s : %s" % (label, ok_msg))
    return True

def check_auto(label, srcrel):
    src = os.path.join(HERE, srcrel)
    text = open(src).read()
    return prove_equiv(label, text, compile_beta_text(text),
                       "bc output ≡ source meaning  (proof-carrying, both derivations pinned)")

def check_fuzz(seed):
    text = refinement_fuzz_gen.program(seed)
    # teeth/perturbation is exercised by the curated samples; fuzz keeps it lean (positive proof + both pins)
    return prove_equiv("fuzz seed %d" % seed, text, compile_beta_text(text), "", quiet_perturb=True, trials=8, teeth=False)

def check_loop_fuzz(seed):
    text = refinement_loop_gen.program(seed)              # a data-dependent loop: BOTH sides must summarize
    return prove_equiv("loop seed %d" % seed, text, compile_beta_text(text), "", quiet_perturb=True, trials=8, teeth=False)

def check_compose_fuzz(seed):
    text = refinement_compose_gen.program(seed)           # pre-loop + loop + post-loop: the summarizers COMPOSED
    return prove_equiv("compose seed %d" % seed, text, compile_beta_text(text), "", quiet_perturb=True, trials=8, teeth=False)

def run_ref(tape, stdin_bytes):
    with tempfile.NamedTemporaryFile(delete=False) as f:
        f.write(tape); path = f.name
    try:
        r = subprocess.run([sys.executable, ALPHA_REF, path], input=bytes(stdin_bytes), capture_output=True)
        return (r.stdout[0] if r.stdout else r.returncode) & 0xFF     # write -> byte ; halt -> exit code
    finally:
        os.unlink(path)

def differential(tape, term, n, trials=40):
    """Instantiate the symbolic term at random small inputs and compare to the concrete VM (mod 256)."""
    if n == 0:
        trials = 1                                     # input-free: one run settles it
    for _ in range(trials):
        env = {i: random.randint(0, 6) for i in range(n)}
        v = S.evaluate(term, env)
        if v >= 256:
            continue                                   # keep below the write/halt mod-256 truncation
        if run_ref(tape, [env[i] for i in range(n)]) != v % 256:
            return False
    return True

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

def check_one(name, tape, claim, expect):
    """symexec -> differential pin -> ∀-refinement proof. Returns True iff the outcome matches `expect`."""
    try:
        term, n = S.symexec(tape)
    except S.Unsupported as e:
        print("  FAIL %-22s : outside the modelled fragment (%s)" % (name, e))
        return False
    if not differential(tape, term, n):
        print("  FAIL %-22s : symbolic engine disagrees with alpha_ref (unsound derivation)" % name)
        return False
    goal = '(= %s %s)' % (S.render(term), claim)
    for _ in range(n):
        goal = '(All %s)' % goal                       # ∀ over every input
    proven = prove(goal)
    if proven != (expect == "refines"):
        print("  FAIL %-22s : proven=%s expect=%s  goal=%s" % (name, proven, expect, goal))
        return False
    print("  ok   %-22s : %s" % (name, "REFINES (proof-carrying)" if proven else "differs (no kernel-accepted proof)"))
    return True

def main():
    random.seed(1234321)
    total = 0; passed = 0
    print(" hand-built tapes:")
    for name, tape, claim, expect in PROGRAMS:
        total += 1; passed += check_one(name, tape, claim, expect)
    if BC and ASM:
        print(" real bc-compiled Beta sources (meaning auto-derived from source — no hand claim):")
        for label, srcrel in AUTO_SAMPLES:
            total += 1; passed += check_auto(label, srcrel)
        nfuzz = int(os.environ.get('REFINE_FUZZ', '15'))
        print(" FUZZ: random straight-line arithmetic programs (bc output ≡ source meaning, ∀ inputs):")
        fpass = 0
        for seed in range(1, nfuzz + 1):
            total += 1; ok = check_fuzz(seed); passed += ok; fpass += ok
        print("   %d/%d random programs certified (bc compiles arithmetic correctly for all inputs)" % (fpass, nfuzz))
        nloop = int(os.environ.get('REFINE_LOOP_FUZZ', '12'))
        print(" LOOP FUZZ: random DATA-DEPENDENT loops (both sides summarize a symbolic trip count, ∀ inputs):")
        lpass = 0
        for seed in range(1, nloop + 1):
            total += 1; ok = check_loop_fuzz(seed); passed += ok; lpass += ok
        print("   %d/%d random loop programs certified (bc compiles counter loops correctly for all inputs)" % (lpass, nloop))
        ncomp = int(os.environ.get('REFINE_COMPOSE_FUZZ', '10'))
        print(" COMPOSE FUZZ: random pre-loop + loop + post-loop programs (summarizers composed, ∀ inputs):")
        cpass = 0
        for seed in range(1, ncomp + 1):
            total += 1; ok = check_compose_fuzz(seed); passed += ok; cpass += ok
        print("   %d/%d random composed programs certified (loop result flows through further arithmetic)" % (cpass, ncomp))
    else:
        print(" (real-bc samples skipped: bc.exe / assembler not provided)")
    print("%d/%d refinement checks passed" % (passed, total))
    sys.exit(0 if passed == total else 1)

if __name__ == '__main__':
    main()
