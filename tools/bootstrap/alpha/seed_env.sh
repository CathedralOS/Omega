#!/usr/bin/env sh
# Sourced by the bootstrap build scripts. Selects the per-platform alpha seed
# and the stamping mechanics, so one script set serves every host. The non-mac
# branch selects the hand-audited Windows flow (seed alpha_x64_windows.exe,
# hole at file offset 5120/5124, no signing).
#
# macOS arm64 differs in three ways, all OS-imposed: a Mach-O seed
# (alpha_arm64_macos), the hole at a different file offset, and a mandatory
# re-sign after stamping (dd invalidates the code signature; Apple Silicon
# refuses to exec an invalid one). AlphaBootstrapV4 gives both containers one
# exact 16 MiB hole including the four-byte length.
ALPHA_SEED_HOLE_SIZE=16777216
ALPHA_MAX_RAW_TAPE_SIZE=16777212

# Bound audited seed identities. These are the same SHA-256 commitments as the
# bootstrap/0_alpha/README.md retention inventory; the byte sizes are recorded
# beside them. A digest here is an identity check that the container being
# stamped is the audited one; it is not a correctness proof of the VM. A
# rebuilt or re-signed container that differs by one byte is not the audited
# seed, and stamping refuses below rather than carrying it.
ALPHA_SEED_ARM64_MACOS_SIZE=16942384
ALPHA_SEED_ARM64_MACOS_SHA256=348bc9601a9f44d4afa98febd7292f77d016b3c1060e20b15768dc23e4061082
ALPHA_SEED_X64_WINDOWS_SIZE=16782336
ALPHA_SEED_X64_WINDOWS_SHA256=bc71f8bee48cbd4c70c533e57b5dfcd04e04199ac3cf055cbed8e76ad6fb1c40

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    ALPHA_SEED=alpha_arm64_macos
    ALPHA_SEED_SIZE=$ALPHA_SEED_ARM64_MACOS_SIZE
    ALPHA_SEED_SHA256=$ALPHA_SEED_ARM64_MACOS_SHA256
    HOLE_OFF=32768
    HOLE_SIZE=$ALPHA_SEED_HOLE_SIZE
    SEED_SIGN=1
    ;;
  *)
    ALPHA_SEED=alpha_x64_windows.exe
    ALPHA_SEED_SIZE=$ALPHA_SEED_X64_WINDOWS_SIZE
    ALPHA_SEED_SHA256=$ALPHA_SEED_X64_WINDOWS_SHA256
    HOLE_OFF=5120
    HOLE_SIZE=$ALPHA_SEED_HOLE_SIZE
    SEED_SIGN=0
    ;;
esac

# bootstrap_sha256 FILE : print the lowercase hex SHA-256 of FILE using the
# host's standard digest tool (sha256sum on Linux and Git Bash, shasum on
# macOS, python3 as the last route). Returns 2 when no route exists; identity
# then cannot be established and materialization refuses rather than skipping.
bootstrap_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -- "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -- "$1" | cut -d' ' -f1
  elif command -v python3 >/dev/null 2>&1; then
    python3 -c 'import hashlib, sys; print(hashlib.sha256(open(sys.argv[1], "rb").read()).hexdigest())' "$1"
  else
    echo "bootstrap artifact: no SHA-256 tool (sha256sum, shasum, python3) to bind identity" >&2
    return 2
  fi
}

# require_bound_identity LABEL FILE SIZE SHA256 RECORD : exact size then
# digest. RECORD names the audit document the caller's constants come from.
require_bound_identity() {
  [ -f "$2" ] || {
    echo "bootstrap artifact: missing $2" >&2
    return 2
  }
  BOUND_SIZE=$(wc -c < "$2" | tr -d ' ')
  [ "$BOUND_SIZE" = "$3" ] || {
    echo "bootstrap artifact: $1 is $BOUND_SIZE bytes; the audited subject is $3 ($5)" >&2
    return 3
  }
  BOUND_SHA256=$(bootstrap_sha256 "$2") || return $?
  [ "$BOUND_SHA256" = "$4" ] || {
    echo "bootstrap artifact: $1 SHA-256 $BOUND_SHA256 differs from the audited subject $4 ($5)" >&2
    return 3
  }
}

# require_alpha_seed_identity SEED_BINARY : the container is exactly the
# audited seed selected above. stamp_seed runs it on every stamp, so every
# materialized artifact carries the audited VM rather than an assumed one.
require_alpha_seed_identity() {
  require_bound_identity "$ALPHA_SEED" "$1" \
    "$ALPHA_SEED_SIZE" "$ALPHA_SEED_SHA256" "bootstrap/0_alpha/README.md"
}

# stamp_seed TAPE SEED_BINARY OUT : copy SEED, memcpy [4-byte LE len][TAPE] into
# its hole, re-sign on macOS. The byte-identical content (modulo the macOS
# signature blob) is the bootstrap's reproducibility guarantee. SEED_BINARY
# must be the audited container; anything else refuses before OUT is written.
stamp_seed() {
  tape="$1"; seed="$2"; out="$3"
  [ -f "$tape" ] || {
    printf 'stamp_seed: missing tape %s\n' "$tape" >&2
    return 1
  }
  [ -f "$seed" ] || {
    printf 'stamp_seed: missing seed %s\n' "$seed" >&2
    return 1
  }
  require_alpha_seed_identity "$seed" || return $?
  L=$(wc -c < "$tape" | tr -d ' ')
  if [ "$L" -gt "$ALPHA_MAX_RAW_TAPE_SIZE" ]; then
    printf 'stamp_seed: tape (%s bytes) exceeds %s-byte AlphaBootstrapV4 raw maximum\n' \
      "$L" "$ALPHA_MAX_RAW_TAPE_SIZE" >&2
    return 1
  fi
  SEED_SIZE=$(wc -c < "$seed" | tr -d ' ')
  REQUIRED_SIZE=$((HOLE_OFF + ALPHA_SEED_HOLE_SIZE))
  if [ "$SEED_SIZE" -lt "$REQUIRED_SIZE" ]; then
    printf 'stamp_seed: seed container is %s bytes; profile requires at least %s\n' \
      "$SEED_SIZE" "$REQUIRED_SIZE" >&2
    return 1
  fi
  cp "$seed" "$out"
  printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((L & 255)) $(((L >> 8) & 255)) $(((L >> 16) & 255)) $(((L >> 24) & 255)))" \
    | dd of="$out" bs=1 seek="$HOLE_OFF" conv=notrunc status=none
  dd if="$tape" of="$out" bs=1 seek=$((HOLE_OFF + 4)) conv=notrunc status=none
  if [ "$SEED_SIGN" = 1 ]; then
    codesign -f -s - "$out" 2>/dev/null || return 1
  fi
  return 0
}

# tape_in_seed SEED_BINARY : extract the [len][tape] currently stamped in a seed's
# hole to stdout (signature-independent — the deterministic content to compare).
tape_in_seed() {
  [ -f "$1" ] || {
    printf 'tape_in_seed: missing seed %s\n' "$1" >&2
    return 1
  }
  SEED_SIZE=$(wc -c < "$1" | tr -d ' ')
  REQUIRED_SIZE=$((HOLE_OFF + ALPHA_SEED_HOLE_SIZE))
  if [ "$SEED_SIZE" -lt "$REQUIRED_SIZE" ]; then
    printf 'tape_in_seed: seed container is %s bytes; profile requires at least %s\n' \
      "$SEED_SIZE" "$REQUIRED_SIZE" >&2
    return 1
  fi
  L=$(od -An -tu4 -j "$HOLE_OFF" -N4 "$1" | tr -dc 0-9)
  if [ -z "$L" ] || [ "$L" -gt "$ALPHA_MAX_RAW_TAPE_SIZE" ]; then
    printf 'tape_in_seed: embedded length %s exceeds profile maximum %s\n' \
      "${L:-invalid}" "$ALPHA_MAX_RAW_TAPE_SIZE" >&2
    return 1
  fi
  if [ $((HOLE_OFF + 4 + L)) -gt "$SEED_SIZE" ]; then
    printf 'tape_in_seed: embedded length exceeds the physical container\n' >&2
    return 1
  fi
  dd if="$1" bs=1 skip="$HOLE_OFF" count=$((L + 4)) status=none
}
