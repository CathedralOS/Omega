#!/usr/bin/env sh
# RECX SEAM — accumulator recursion vs independent evaluation (D4: one seam per δ-capability).
#
# (recx i E) lets a rule's recursive call REPLACE the extra argument (accumulator recursion), the
# capability the summit's ∀-input theorems need. Soundness rides the same structural-termination
# argument as (rec i) — only the extra changes, the scrutinee still decreases — and THIS gate pins the
# semantics: random accumulator loops (count and sum over random lists, random starting accumulators)
# whose expected values are computed INDEPENDENTLY in Python; the kernel must accept exactly the true
# equations and reject off-by-one perturbations, and implementations/reference/check_ref.py must agree verdict-for-verdict.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_PROOF_KERNEL"
command -v python3 >/dev/null 2>&1 || { echo "recx seam: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b implementations/beta/check.beta        "$T/check.exe"  || { echo "recx seam FAIL — build implementations/beta/check.beta"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "recx seam FAIL — build interp.beta"; exit 1; }

python3 - "$T/check.exe" "$T/interp.exe" "${OMEGA_PATH_PROOF_KERNEL}"/implementations/gamma/checker.gamma <<'EOF'
import random
import os
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
    b = subprocess.run(['python3', 'implementations/reference/check_ref.py'], input=cert, capture_output=True,
                       text=True, timeout=60).stdout.strip()
    tr = subprocess.run(['python3', os.path.join(os.environ['OMEGA_PATH_PROOF_KERNEL'],
                                                'tools/refcert_to_gamma.py')], input=cert,
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
print('recx seam (accumulator recursion vs independent evaluation; implementations/beta/check.beta, check_ref AND '
      'implementations/gamma/checker.gamma agree): %d ok, %d failed' % (ok, bad))
sys.exit(1 if bad or not ok else 0)
EOF
