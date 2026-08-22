# Sourced by the bootstrap build scripts. Selects the per-platform alpha seed
# and the stamping mechanics, so one script set serves every host. The non-mac
# branch reproduces the original hand-audited Windows flow byte-for-byte
# (seed alpha_x64_windows.exe, hole at file offset 5120/5124, no signing).
#
# macOS arm64 differs in three ways, all OS-imposed: a Mach-O seed
# (alpha_arm64_macos), the hole at a different file offset, and a mandatory
# re-sign after stamping (dd invalidates the code signature; Apple Silicon
# refuses to exec an invalid one).
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    ALPHA_SEED=alpha_arm64_macos
    BETA_SEED=beta_arm64_macos
    HOLE_OFF=32768
    HOLE_SIZE=262144     # 256 KB tape hole (.space 0x40000)
    SEED_SIGN=1
    ;;
  *)
    ALPHA_SEED=alpha_x64_windows.exe
    BETA_SEED=beta_x64_windows.exe
    HOLE_OFF=5120
    HOLE_SIZE=262144     # 256 KB tape hole (audited PE section extent)
    SEED_SIGN=0
    ;;
esac

# stamp_seed TAPE SEED_BINARY OUT : copy SEED, memcpy [4-byte LE len][TAPE] into
# its hole, re-sign on macOS. The byte-identical content (modulo the macOS
# signature blob) is the bootstrap's reproducibility guarantee.
stamp_seed() {
  tape="$1"; seed="$2"; out="$3"
  L=$(wc -c < "$tape" | tr -d ' ')
  if [ $((L + 4)) -gt "$HOLE_SIZE" ]; then
    printf 'stamp_seed: tape (%s bytes plus length) exceeds %s-byte seed hole\n' \
      "$L" "$HOLE_SIZE" >&2
    return 1
  fi
  cp "$seed" "$out"
  printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((L & 255)) $(((L >> 8) & 255)) $(((L >> 16) & 255)) $(((L >> 24) & 255)))" \
    | dd of="$out" bs=1 seek="$HOLE_OFF" conv=notrunc status=none
  dd if="$tape" of="$out" bs=1 seek=$((HOLE_OFF + 4)) conv=notrunc status=none
  [ "$SEED_SIGN" = 1 ] && codesign -f -s - "$out" 2>/dev/null
  return 0
}

# tape_in_seed SEED_BINARY : extract the [len][tape] currently stamped in a seed's
# hole to stdout (signature-independent — the deterministic content to compare).
tape_in_seed() {
  L=$(od -An -tu4 -j "$HOLE_OFF" -N4 "$1" | tr -dc 0-9)
  dd if="$1" bs=1 skip="$HOLE_OFF" count=$((L + 4)) status=none
}
