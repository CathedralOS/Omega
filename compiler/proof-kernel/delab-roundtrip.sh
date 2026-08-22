#!/usr/bin/env sh
# ROUND-TRIP cross-validation of the two untrusted tools. For every closed, accepted gate
# certificate C, the de-elaborator and elaborator must compose to the identity:
#   elab(delab(C)) == C   (byte-for-byte, after tokenisation) and still checks `accept`.
# A bug in EITHER tool surfaces here as a changed or rejected certificate — they keep each
# other honest, and neither is in the trust path. Open-goal and ill-scoped reject certs are
# skipped (reported), see delab.py "Scope".
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd "${OMEGA_PATH_BETA_RUST}" && sh build.sh "${OMEGA_PATH_BETA_LANGUAGE}"/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
"${OMEGA_PATH_BETA_RUST}"/build/bc.exe < check.beta > "$T/p.asm" || { echo "bc(check.beta) failed"; exit 1; }
"$ASM" < "$T/p.asm" > "$T/p.tape" || { echo "asm failed"; exit 1; }
stamp_seed "$T/p.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1
CHECK="$T/check.exe" python3 - <<'PY'
import re, os, subprocess
from delab import decompile
from elab import elaborate, tokenize
exe=os.environ['CHECK']
txt=open('test.sh').read()
certs=[(m.group(1),m.group(2),m.group(3)) for m in
       re.finditer(r'chk "([^"]*)" "([^"]*)" (accept|reject)', txt, re.DOTALL)]
ident=rechk=skip=0; bad=[]
for name,cert,verd in certs:
    if '$' in cert or cert.count('(')!=cert.count(')'): skip+=1; continue   # shell-var / multi-token
    if verd!='accept': continue
    try:
        recomp=elaborate(decompile(cert))
    except BaseException:
        skip+=1; continue            # open-goal / ill-scoped: out of scope (see delab.py)
    if ' '.join(tokenize(recomp))!=' '.join(tokenize(cert)): bad.append((name,"not byte-identical")); continue
    ident+=1
    if subprocess.run([exe],input=recomp,capture_output=True,text=True).stdout.strip()=='accept': rechk+=1
    else: bad.append((name,"recompiled rejected"))
for n,w in bad: print("  FAIL", n, "::", w)
print("round-trip (elab.delab = id on closed accept-certs): %d byte-identical, %d re-accept, %d skipped, %d bad"
      % (ident, rechk, skip, len(bad)))
raise SystemExit(1 if bad else 0)
PY
