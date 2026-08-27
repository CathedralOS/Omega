#!/usr/bin/env sh
set -e
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] \
      || { echo "omega-bootstrap bundle: repository root not found" >&2; exit 2; }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
BUNDLE_TOOL="$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega_bootstrap_bundle.py"
command -v python3 >/dev/null 2>&1 || { echo "omega-bootstrap bundle: skipped (python3 absent)"; exit 0; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

printf 'machine A {}' > "$T/a.omg"
printf 'use z;\n\000tail' > "$T/z.omg"

python3 "$BUNDLE_TOOL" pack z/main.omg="$T/z.omg" a/main.omg="$T/a.omg" > "$T/one.bundle"
python3 "$BUNDLE_TOOL" pack a/main.omg="$T/a.omg" z/main.omg="$T/z.omg" > "$T/two.bundle"
cmp "$T/one.bundle" "$T/two.bundle" >/dev/null \
  || { echo "omega-bootstrap bundle FAIL — invocation order changed bytes"; exit 1; }
python3 "$BUNDLE_TOOL" verify "$T/one.bundle"

python3 "$BUNDLE_TOOL" get "$T/one.bundle" a/main.omg > "$T/a.out"
python3 "$BUNDLE_TOOL" get "$T/one.bundle" z/main.omg > "$T/z.out"
cmp "$T/a.omg" "$T/a.out" >/dev/null && cmp "$T/z.omg" "$T/z.out" >/dev/null \
  || { echo "omega-bootstrap bundle FAIL — source bytes did not round-trip"; exit 1; }

manifest=$(python3 "$BUNDLE_TOOL" manifest "$T/one.bundle" | cut -f1,2)
expected=$(printf 'a/main.omg\t12\nz/main.omg\t12')
[ "$manifest" = "$expected" ] \
  || { echo "omega-bootstrap bundle FAIL — noncanonical manifest"; exit 1; }

if python3 "$BUNDLE_TOOL" pack a/main.omg="$T/a.omg" a/main.omg="$T/z.omg" > /dev/null 2>&1; then
  echo "omega-bootstrap bundle FAIL — duplicate label accepted"; exit 1
fi
if python3 "$BUNDLE_TOOL" pack ../escape.omg="$T/a.omg" > /dev/null 2>&1; then
  echo "omega-bootstrap bundle FAIL — noncanonical path accepted"; exit 1
fi

cp "$T/one.bundle" "$T/trailing.bundle"
printf x >> "$T/trailing.bundle"
if python3 "$BUNDLE_TOOL" verify "$T/trailing.bundle" > /dev/null 2>&1; then
  echo "omega-bootstrap bundle FAIL — trailing byte accepted"; exit 1
fi

dd if="$T/one.bundle" of="$T/truncated.bundle" bs=1 count=25 2>/dev/null
if python3 "$BUNDLE_TOOL" verify "$T/truncated.bundle" > /dev/null 2>&1; then
  echo "omega-bootstrap bundle FAIL — truncated entry accepted"; exit 1
fi

echo "omega-bootstrap bundle: deterministic, exact-byte round-trip, malformed inputs rejected"
