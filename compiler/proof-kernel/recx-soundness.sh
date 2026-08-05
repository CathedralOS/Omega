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
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b check.beta        "$T/check.exe"  || { echo "recx seam FAIL — build check.beta"; exit 1; }
b ../gamma/interp.beta "$T/interp.exe" || { echo "recx seam FAIL — build interp.beta"; exit 1; }

python3 - "$T/check.exe" "$T/interp.exe" ../gamma/checker.gamma <<'EOF'
import random
import subprocess
import sys

check, interp, cgamma = sys.argv[1], sys.argv[2], sys.argv[3]
defs = open(cgamma).read()
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
    tr = subprocess.run(['python3', '../gamma/refcert_to_gamma.py'], input=cert,
                        capture_output=True, text=True, timeout=60)
    if tr.returncode != 0:
        return a, b, 'untranslatable'
    r = subprocess.run([interp], input=defs + '\n' + tr.stdout, capture_output=True, text=True, timeout=60)
    g = 'accept' if (r.stdout.strip() == '1' and r.returncode == 1) else \
        'reject' if (r.stdout.strip() == '0' and r.returncode == 0) else 'undecided'
    return a, b, g


ok = bad = 0
for case in range(30):
    xs = [random.randint(0, 6) for _ in range(random.randint(0, 5))]
    acc = random.randint(0, 5)
    fid, true = random.choice([(91, acc + len(xs)), (92, acc + sum(xs))])
    lhs = '(f %d %s %s)' % (fid, ulist(xs), unat(acc))
    good = '%s (= %s %s) (refl %s)' % (PRE, lhs, unat(true), unat(true))
    liar = '%s (= %s %s) (refl %s)' % (PRE, lhs, unat(true + 1), unat(true + 1))
    va, vb, vg = verdict(good)
    wa, wb, wg = verdict(liar)
    if va == vb == vg == 'accept' and wa == wb == wg == 'reject':
        ok += 1
    else:
        bad += 1
        print('  FAIL case %d: xs=%s acc=%d fid=%d -> good=(%s,%s,%s) liar=(%s,%s,%s)'
              % (case, xs, acc, fid, va, vb, vg, wa, wb, wg))
print('recx seam (accumulator recursion vs independent evaluation; check.beta, check_ref AND '
      'checker.gamma agree): %d ok, %d failed' % (ok, bad))
sys.exit(1 if bad or not ok else 0)
EOF
