# Shared scratch preamble for the tests/bootstrap/*.sh gates: one temporary
# directory cleaned on exit and the fail() reporter used by every check.
TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT

fail() {
  echo "FAIL $1" >&2
  exit 1
}

# check_controls_corruption LABEL SOURCE_TREE MANIFEST MANIFEST_REL MEMBER_REL
# MANIFEST_SIZE ENVIRONMENT CHECKER ERROR_FILE : retain both negatives for one
# copied controls tree. Call unguarded so the gate's set -e context is preserved.
check_controls_corruption() {
  cp -R "$2" "$TMP/$1"
  head -c $(($6 - 1)) "$3" > "$TMP/$1/$4"
  rc=0
  (
    export "$7=$TMP/$1/$4"
    "$8"
  ) 2>/dev/null || rc=$?
  [ "$rc" = 3 ] ||
    fail "truncated $1 manifest: expected exit 3, got $rc"
  cp "$3" "$TMP/$1/$4"
  if [ "$(od -An -tc -j 100 -N1 "$TMP/$1/$5" | tr -d ' ')" = "a" ]; then
    printf 'b' | dd of="$TMP/$1/$5" bs=1 seek=100 conv=notrunc status=none
  else
    printf 'a' | dd of="$TMP/$1/$5" bs=1 seek=100 conv=notrunc status=none
  fi
  rc=0
  (
    export "$7=$TMP/$1/$4"
    "$8"
  ) 2>"$TMP/$9" || rc=$?
  [ "$rc" != 0 ] ||
    fail "corrupted $1 member: repack unexpectedly succeeded"
  grep -q 'digest' "$TMP/$9" ||
    fail "corrupted $1 member: refusal did not name the member digest"
}
