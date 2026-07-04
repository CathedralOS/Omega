#!/usr/bin/env sh
# RECX SEAM — accumulator recursion vs independent evaluation (D4: one seam per δ-capability).
#
# (recx i E) lets a rule's recursive call REPLACE the extra argument (accumulator recursion), the
# capability the summit's ∀-input theorems need. Soundness rides the same structural-termination
# argument as (rec i) — only the extra changes, the scrutinee still decreases — and THIS gate pins the
# semantics: random accumulator loops (count and sum over random lists, random starting accumulators)
# whose expected values are computed INDEPENDENTLY in Python; the kernel must accept exactly the true
# equations and reject off-by-one perturbations, and check_ref.py must agree verdict-for-verdict.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "recx seam: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "recx seam FAIL — bc build"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
../beta-lang-rs/build/bc.exe < check.beta > "$T/c.asm" 2>/dev/null \
  && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
  && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1 \
  || { echo "recx seam FAIL — build check.beta"; exit 1; }

python3 - "$T/check.exe" <<'EOF'
import random
import subprocess
import sys

check = sys.argv[1]
random.seed(20260705)


def unat(k):
    return '(k 2)' if k == 0 else '(k 3 %s)' % unat(k - 1)


def ulist(xs):
    out = '(k 60)'
    for x in reversed(xs):
        out = '(k 61 %s %s)' % (unat(x), out)
    return out


PRE = ('(data 2 0 0 0) (data 3 1 1 0) (data 60 0 0 0) (data 61 2 0 0) '
       '(fun 21 2 (y 0)) (fun 21 3 (k 3 (rec 0))) '
       '(fun 91 60 (y 0)) (fun 91 61 (recx 1 (k 3 (y 0)))) '        # count: acc+1 per element
       '(fun 92 60 (y 0)) (fun 92 61 (recx 1 (f 21 (v 0) (y 0))))')  # sum: acc+head per element


def verdict(cert):
    a = subprocess.run([check], input=cert, capture_output=True, text=True, timeout=60).stdout.strip()
    b = subprocess.run(['python3', 'check_ref.py'], input=cert, capture_output=True,
                       text=True, timeout=60).stdout.strip()
    return a, b


ok = bad = 0
for case in range(30):
    xs = [random.randint(0, 6) for _ in range(random.randint(0, 5))]
    acc = random.randint(0, 5)
    fid, true = random.choice([(91, acc + len(xs)), (92, acc + sum(xs))])
    lhs = '(f %d %s %s)' % (fid, ulist(xs), unat(acc))
    good = '%s (= %s %s) (refl %s)' % (PRE, lhs, unat(true), unat(true))
    liar = '%s (= %s %s) (refl %s)' % (PRE, lhs, unat(true + 1), unat(true + 1))
    va, vb = verdict(good)
    wa, wb = verdict(liar)
    if va == vb == 'accept' and wa == wb == 'reject':
        ok += 1
    else:
        bad += 1
        print('  FAIL case %d: xs=%s acc=%d fid=%d -> good=(%s,%s) liar=(%s,%s)'
              % (case, xs, acc, fid, va, vb, wa, wb))
print('recx seam (accumulator recursion vs independent evaluation; kernel AND check_ref agree): '
      '%d ok, %d failed' % (ok, bad))
sys.exit(1 if bad or not ok else 0)
EOF
