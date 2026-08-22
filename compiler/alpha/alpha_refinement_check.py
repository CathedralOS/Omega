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
REPO_ROOT = os.environ.get(
    'OMEGA_REPO_ROOT', os.path.abspath(os.path.join(HERE, '..', '..')))
BETA_REFERENCE = os.environ.get(
    'OMEGA_PATH_BETA_REFERENCE', os.path.join(REPO_ROOT, 'compiler', 'beta-lang-py'))
BETA_RUST = os.environ.get(
    'OMEGA_PATH_BETA_RUST', os.path.join(REPO_ROOT, 'compiler', 'beta-lang-rs'))
PROOF_KERNEL = os.environ.get(
    'OMEGA_PATH_PROOF_KERNEL', os.path.join(REPO_ROOT, 'compiler', 'proof-kernel'))
sys.path.insert(0, HERE)
sys.path.insert(0, BETA_REFERENCE)
import alpha_symbolic as S
import beta_symbolic as B                              # source-side symbolic evaluator (the auto-derived meaning)
import beta_interp                                     # concrete Beta interpreter (pins the source meaning)
import refinement_fuzz_gen                             # random straight-line arithmetic Beta programs
import refinement_loop_gen                              # random data-dependent linear-loop Beta programs
import refinement_compose_gen                           # random composed programs: pre-loop + loop + post-loop
import refinement_nested_gen                            # random NESTED loops (recursive summarization)
import refinement_fork_gen                              # random BRANCHING programs (conditional terms)
from beta_parser import lex, Parser
ALPHA_REF = os.path.join(HERE, 'alpha_ref.py')
PROVER = os.path.join(PROOF_KERNEL, 'prover.py')
CHECK = sys.argv[1]
BC = sys.argv[2] if len(sys.argv) > 2 else None       # bc.exe — enables the real-bc-output samples
ASM = sys.argv[3] if len(sys.argv) > 3 else None      # the beta assembler exe
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
    ("tri       (Σi LOOP total+=i →g(n))", "refinement-samples/tri.beta"),
    ("weighted  (LINEAR δ a*i+b)", "refinement-samples/weighted.beta"),
    ("diff      (SUBTRACTION (a-b)*c)", "refinement-samples/diff.beta"),
    ("drain     (SUBTRACTING LOOP n*a -= a)", "refinement-samples/drain.beta"),
    ("countdown (DOWN-COUNT LOOP 0<i, i-=1 →n*a)", "refinement-samples/countdown.beta"),
    ("tri_down  (DOWN-COUNT Σi →n²-g(n))", "refinement-samples/tri_down.beta"),
    ("gtguard   (>-SPELLED GUARD i>0 →n*a)", "refinement-samples/gtguard.beta"),
    ("neq       (!=-GUARDED LOOP i!=n →n*a)", "refinement-samples/neq.beta"),
    ("nested    (OUTER-SYM × INNER-CONCRETE →3na)", "refinement-samples/nested.beta"),
    ("nested_sym (RECURSIVE SUMMARY j<m →n·m·a)", "refinement-samples/nested_sym.beta"),
    ("tri_nested (TRIANGULAR j<i →a·g(n))", "refinement-samples/tri_nested.beta"),
    ("callloop  (PROC CALL IN LOOP →2·g(n))", "refinement-samples/callloop.beta"),
    ("temploop  (REWRITE TEMP t=a*i →a·g(n))", "refinement-samples/temploop.beta"),
    ("bytemem   (BYTE MEMORY roundtrip+truncation)", "refinement-samples/bytemem.beta"),
    ("fromto    (MONUS TRIP i=a..n →(n∸a)a+g(n∸a))", "refinement-samples/fromto.beta"),
    ("sumbytes  (READ-LOOP →Σ input[1..1+n) + next)", "refinement-samples/sumbytes.beta"),
    ("weightedread (a·Σ input + g(n))", "refinement-samples/weightedread.beta"),
    ("absdiff   (BRANCH ON DATA →cond/ℤ pairs)", "refinement-samples/absdiff.beta"),
    ("maxmin    (NESTED BRANCHES →cond in cond)", "refinement-samples/maxmin.beta"),
    ("boolval   (STORED COMPARISON as a value)", "refinement-samples/boolval.beta"),
    ("condloop  (CONDITIONAL DELTA in a loop body)", "refinement-samples/condloop.beta"),
    ("bufcopy   (BUFFER COPY →segment/cond reads)", "refinement-samples/bufcopy.beta"),
    ("divten    (INTEGER DIVISION a/10)", "refinement-samples/divten.beta"),
    ("modten    (REMAINDER a%10)",  "refinement-samples/modten.beta"),
    ("divmod    ((a/10)*10 + a%10)", "refinement-samples/divmod.beta"),
    ("divplus   (VAR divisor a/(b+1))", "refinement-samples/divplus.beta"),
    ("divguard  (GUARDED b!=0 ? a/b : 0)", "refinement-samples/divguard.beta"),
    ("sumto(10) (concrete LOOP)",   os.path.join(BETA_RUST, "examples", "sumto.beta")),
    ("fact(5)   (RECURSION)",       os.path.join(BETA_RUST, "examples", "factorial.beta")),
    ("answer    (6*7)",             os.path.join(BETA_RUST, "examples", "answer.beta")),
    ("double    (double(21))",      os.path.join(BETA_RUST, "examples", "double.beta")),
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
            cdir = os.environ.get('REFINE_CERT_DIR')
            if cdir:                                   # tee the cert + check.beta's verdict for the cert diamond
                nc = len(os.listdir(cdir))
                with open(os.path.join(cdir, 'cert-%03d-%s.beta' % (nc, verdict or 'reject')), 'w') as cf:
                    cf.write(cert + '\n')
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

def check_nested_fuzz(seed):
    text = refinement_nested_gen.program(seed)            # nested loops: RECURSIVE summarization fuzzed
    return prove_equiv("nested seed %d" % seed, text, compile_beta_text(text), "", quiet_perturb=True, trials=8, teeth=False)

def check_fork_fuzz(seed):
    text = refinement_fork_gen.program(seed)              # if-diamonds: conditional terms fuzzed
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

def differential(tape, term, n, trials=40):
    """Instantiate the symbolic term at random small inputs and compare to the concrete VM (mod 256).
    A term with STREAM constructs (a read-loop's Σ input[lo..hi)) gets a padded input vector — the machine
    consumes exactly what it reads (loop bounds are drawn ≤ 6, so 40 bytes of slack always suffice)."""
    if n == 0 and not S._has_stream(term):
        trials = 1                                     # input-free: one run settles it
    for _ in range(trials):
        vec = [random.randint(0, 6) for _ in range(n + (40 if S._has_stream(term) else 0))]
        env = {i: vec[i] for i in range(len(vec))}
        env['in'] = vec
        v = S.evaluate(term, env)
        if v >= 256:
            continue                                   # keep below the write/halt mod-256 truncation
        if run_ref(tape, vec) != v % 256:
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
        nnest = int(os.environ.get('REFINE_NESTED_FUZZ', '8'))
        print(" NESTED FUZZ: random nested loops (inner loop summarized recursively inside the outer, ∀ inputs):")
        npass = 0
        for seed in range(1, nnest + 1):
            total += 1; ok = check_nested_fuzz(seed); passed += ok; npass += ok
        print("   %d/%d random nested programs certified (recursive summarization matches the machine)" % (npass, nnest))
        nfork = int(os.environ.get('REFINE_FORK_FUZZ', '8'))
        print(" FORK FUZZ: random branching programs (both sides fork into conditional terms, ∀ inputs):")
        kpass = 0
        for seed in range(1, nfork + 1):
            total += 1; ok = check_fork_fuzz(seed); passed += ok; kpass += ok
        print("   %d/%d random branching programs certified (conditional terms match the machine)" % (kpass, nfork))
    else:
        print(" (real-bc samples skipped: bc.exe / assembler not provided)")
    print("%d/%d refinement checks passed" % (passed, total))
    sys.exit(0 if passed == total else 1)

if __name__ == '__main__':
    main()
