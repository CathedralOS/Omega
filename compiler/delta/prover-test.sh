#!/usr/bin/env sh
# PROOF-AUTOMATION FRONT LINE -- the Omega pattern (automation discharges, the kernel checks). The
# untrusted prover (prover.py) searches for a proof of an intuitionistic {-> , &} propositional goal and
# emits a certificate; the trusted kernel (check.beta, alpha-rooted) must ACCEPT it. This is the
# "authority in the kernel, cleverness on the untrusted side" split: the prover is sound by construction,
# but the kernel -- not the prover -- is what we trust, so EVERY certificate it emits is re-checked.
#   - curated tautologies: the prover finds a proof check.beta accepts;
#   - non-tautologies: the prover correctly emits no proof (it never fabricates one);
#   - a random fuzz: for every goal the prover proves, check.beta accepts the cert (broad soundness).
# Needs python3.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "prover-test: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
../beta-lang-rs/build/bc.exe < check.beta > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" \
  && stamp_seed "$T/x.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1 || { echo "build check.beta failed"; exit 1; }
CHECK="$T/check.exe"

PASS=0; FAIL=0
ok() {  # a tautology the prover must prove AND the kernel must accept
  cert=$(python3 prover.py "$1")
  if [ "$cert" = unprovable ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : prover found no proof"; return; fi
  v=$(printf '%s' "$cert" | "$CHECK")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL $1 : kernel rejected the prover's cert [$v]"; fi
}
no() {  # NOT a tautology: the prover must emit no proof (never fabricate authority)
  cert=$(python3 prover.py "$1")
  if [ "$cert" = unprovable ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL $1 : prover fabricated a proof of a non-tautology: $cert"; fi
}

# curated {->,&} intuitionistic tautologies (the schemas check.beta's corpus uses, propositional part)
ok "(-> P P)"
ok "(-> (& P Q) P)"
ok "(-> (& P Q) Q)"
ok "(-> P (-> Q P))"
ok "(-> (& P Q) (& Q P))"
ok "(-> (& (-> P Q) P) Q)"
ok "(-> (-> (& P Q) R) (-> P (-> Q R)))"
ok "(-> (& (-> P Q) (-> Q R)) (-> P R))"
ok "(-> P (-> (-> P Q) Q))"
ok "(-> (& P (& Q R)) (& (& P Q) R))"
# non-tautologies: provability must fail (soundness of the front line)
no "(-> P Q)"
no "(& P P)"
no "(-> (-> P Q) P)"
no "(-> (-> P Q) Q)"

# random fuzz: for every goal the prover proves, the kernel must accept the certificate. A single
# `--batch` process generates+proves all goals and prints "<goal>\t<cert>" lines for the provable ones,
# so the only per-goal cost is the (unavoidable) kernel check.
nproved=0
python3 prover.py --batch 150 7 > "$T/certs"
while IFS="$(printf '\t')" read -r goal cert; do
  nproved=$((nproved+1))
  v=$(printf '%s' "$cert" | "$CHECK")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL fuzz $goal : kernel rejected [$v]"; fi
done < "$T/certs"

echo "proof-automation front line (prover discharges; check.beta validates): $PASS ok ($nproved fuzz-proved), $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
