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
import alpha_symbolic as S
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

# REAL bc-compiled programs: the genuine payoff — prove the ACTUAL compiler's straight-line output refines its
# Beta source meaning (enabled when bc.exe + the assembler are provided). Files live in refinement-samples/.
NAT3 = '(s (s (s z)))'
REAL_SAMPLES = [
    ("sum2.beta   ⊑ b+a",   "(p (v 1) (v 0))", "refines"),                         # read a,b; a+b  (commuted)
    ("prod2.beta  ⊑ b*a",   "(m (v 1) (v 0))", "refines"),                         # read a,b; a*b  (commuted)
    ("dbl.beta    ⊑ 2*a",   "(m (s (s z)) (v 0))", "refines"),                     # read a; a+a
    ("affine.beta ⊑ 3*a+1", "(p (m %s (v 0)) (s z))" % NAT3, "refines"),           # read a; a+a+a+1
    ("sum2.beta   ⋢ a*b",   "(m (v 0) (v 1))", "differs"),                         # wrong claim -> no proof
]

def compile_beta(src_path):
    """bc.exe < src | asm  ->  raw alpha tape bytes."""
    asm = subprocess.run([BC], stdin=open(src_path, 'rb'), capture_output=True).stdout
    with tempfile.NamedTemporaryFile(delete=False) as f:
        f.write(asm); apath = f.name
    try:
        tape = subprocess.run([ASM], stdin=open(apath, 'rb'), capture_output=True).stdout
    finally:
        os.unlink(apath)
    return tape

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
        print(" real bc-compiled Beta sources:")
        for name, claim, expect in REAL_SAMPLES:
            src = os.path.join(HERE, 'refinement-samples', name.split()[0])
            tape = compile_beta(src)
            total += 1; passed += check_one(name, tape, claim, expect)
    else:
        print(" (real-bc samples skipped: bc.exe / assembler not provided)")
    print("%d/%d refinement checks passed" % (passed, total))
    sys.exit(0 if passed == total else 1)

if __name__ == '__main__':
    main()
