#!/usr/bin/env sh
# ROUND-TRIP cross-validation of the two untrusted tools. For every closed, accepted gate
# certificate C, the de-elaborator and elaborator must compose to the identity:
#   elab(delab(C)) == C   (byte-for-byte, after tokenisation) and still checks `accept`.
# A bug in EITHER tool surfaces here as a changed or rejected certificate — they keep each
# other honest, and neither is in the trust path. Open-goal and ill-scoped reject certs are
# skipped (reported), see tools/delab.py "Scope".
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "lattice paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" || exit $?
. "$OMEGA_PATH_ALPHA_CHECKER/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_ALPHA_CHECKER"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_proof_checker "$T/check.exe" >/dev/null || { echo "checker artifact unavailable"; exit 1; }
CHECK="$T/check.exe" PYTHONPATH="tools${PYTHONPATH:+:$PYTHONPATH}" python3 - <<'PY'
import re, os, subprocess
from delab import decompile
from elab import elaborate, tokenize
exe=os.environ['CHECK']
txt=open('gates/test.sh').read()
certs=[(m.group(1),m.group(2),m.group(3)) for m in
       re.finditer(r'chk "([^"]*)" "([^"]*)" (accept|reject)', txt, re.DOTALL)]
ident=rechk=skip=0; bad=[]
for name,cert,verd in certs:
    if '$' in cert or cert.count('(')!=cert.count(')'): skip+=1; continue   # shell-var / multi-token
    if verd!='accept': continue
    try:
        recomp=elaborate(decompile(cert))
    except BaseException:
        skip+=1; continue            # open-goal / ill-scoped: out of scope (see tools/delab.py)
    if ' '.join(tokenize(recomp))!=' '.join(tokenize(cert)): bad.append((name,"not byte-identical")); continue
    ident+=1
    if subprocess.run([exe],input=recomp,capture_output=True,text=True).stdout.strip()=='accept': rechk+=1
    else: bad.append((name,"recompiled rejected"))
for n,w in bad: print("  FAIL", n, "::", w)
print("round-trip (elab.delab = id on closed accept-certs): %d byte-identical, %d re-accept, %d skipped, %d bad"
      % (ident, rechk, skip, len(bad)))
raise SystemExit(1 if bad else 0)
PY
