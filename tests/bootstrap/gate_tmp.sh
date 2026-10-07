# Shared scratch preamble for the tests/bootstrap/*.sh gates: one temporary
# directory cleaned on exit and the fail() reporter used by every check.
TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT

fail() {
  echo "FAIL $1" >&2
  exit 1
}
