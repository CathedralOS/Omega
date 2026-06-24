#!/usr/bin/env sh
# Verify bc.beta SELF-HOSTS: bc (the Beta compiler written in Beta) compiles its
# own source to a compiler that reproduces that compilation byte-for-byte.
#
#   bc0  = on-ramp(bc.beta)            (Rust cold-start, one time)
#   asm1 = bc0(bc.beta) ; bc1 = assemble+stamp(asm1)   (a bc built BY bc)
#   asm2 = bc1(bc.beta)
#   FIXED POINT iff asm1 == asm2  -> from bc1 on, no Rust is in the lineage.
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

# bc0: cold-start the Beta compiler through the Rust on-ramp
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc0 build failed"; exit 1; }
BC0=../beta-lang-rs/build/bc.exe

# asm1 = bc0(bc.beta) ; assemble + stamp -> bc1
"$BC0" < bc.beta > "$T/asm1" || { echo "bc0(bc.beta) failed"; exit 1; }
"$ASM" < "$T/asm1" > "$T/bc1.tape" || { echo "assemble asm1 failed"; exit 1; }
L=$(wc -c < "$T/bc1.tape" | tr -d ' ')
[ $((L + 4)) -le "$HOLE_SIZE" ] || { echo "FAIL: bc tape $L B exceeds the hole ($HOLE_SIZE B)"; exit 1; }
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1.exe" >/dev/null 2>&1

# asm2 = bc1(bc.beta)
"$T/bc1.exe" < bc.beta > "$T/asm2" || { echo "bc1(bc.beta) failed"; exit 1; }

if cmp -s "$T/asm1" "$T/asm2"; then
  echo "self-host ✓ — bc.beta reproduces its own compilation byte-for-byte (bc tape ${L} B); no Rust from bc1 on"
else
  echo "FAIL: bc.beta does not self-host (asm1 != asm2)"; cmp "$T/asm1" "$T/asm2" | head; exit 1
fi
