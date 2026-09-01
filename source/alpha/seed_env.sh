# Sourced by the bootstrap build scripts. Selects the per-platform alpha seed
# and the stamping mechanics, so one script set serves every host. The non-mac
# branch selects the hand-audited Windows flow (seed alpha_x64_windows.exe,
# hole at file offset 5120/5124, no signing).
#
# macOS arm64 differs in three ways, all OS-imposed: a Mach-O seed
# (alpha_arm64_macos), the hole at a different file offset, and a mandatory
# re-sign after stamping (dd invalidates the code signature; Apple Silicon
# refuses to exec an invalid one). AlphaBootstrapV2 gives both containers one
# exact 1 MiB hole including the four-byte length.
ALPHA_SEED_HOLE_SIZE=1048576
ALPHA_MAX_RAW_TAPE_SIZE=1048572

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    ALPHA_SEED=alpha_arm64_macos
    HOLE_OFF=32768
    HOLE_SIZE=$ALPHA_SEED_HOLE_SIZE
    SEED_SIGN=1
    ;;
  *)
    ALPHA_SEED=alpha_x64_windows.exe
    HOLE_OFF=5120
    HOLE_SIZE=$ALPHA_SEED_HOLE_SIZE
    SEED_SIGN=0
    ;;
esac

# stamp_seed TAPE SEED_BINARY OUT : copy SEED, memcpy [4-byte LE len][TAPE] into
# its hole, re-sign on macOS. The byte-identical content (modulo the macOS
# signature blob) is the bootstrap's reproducibility guarantee.
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
  L=$(wc -c < "$tape" | tr -d ' ')
  if [ "$L" -gt "$ALPHA_MAX_RAW_TAPE_SIZE" ]; then
    printf 'stamp_seed: tape (%s bytes) exceeds %s-byte AlphaBootstrapV2 raw maximum\n' \
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
