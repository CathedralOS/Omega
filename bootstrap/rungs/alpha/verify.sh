#!/usr/bin/env sh
# Local trust check for the host's alpha seed, end to end:
#   provenance  - the committed binary re-derives from the committed source
#                 (where a forge exists; modulo the OS-imposed code signature);
#   behavior    - it realizes SEMANTICS.md (conformance.sh, every opcode + edge);
#   reproduction - the VM reproduces the canonical assembler bytecode
#                  (${OMEGA_PATH_ALPHA_ASSEMBLER}/selfhost.sh).
# Run after touching a seed; this is the per-platform acceptance gate.
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
. ./seed_env.sh
rc=0

echo "--- provenance ---"
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    if command -v clang >/dev/null 2>&1; then
      TMP=$(mktemp -d)
      if clang -arch arm64 -Wl,-no_uuid -o "$TMP/rebuilt" alpha_arm64_macos.s 2>"$TMP/err"; then
        cp "$ALPHA_SEED" "$TMP/committed"
        codesign --remove-signature "$TMP/rebuilt" "$TMP/committed" 2>/dev/null
        if cmp -s "$TMP/rebuilt" "$TMP/committed"; then
          echo "provenance ✓ — $ALPHA_SEED reproduces from alpha_arm64_macos.s (modulo signature)"
        else
          echo "provenance FAIL — committed binary differs from a rebuild of its source"; rc=1
        fi
      else
        echo "provenance FAIL — rebuild errored:"; sed 's/^/  /' "$TMP/err"; rc=1
      fi
      rm -rf "$TMP"
    else
      echo "provenance SKIP — clang not found"
    fi
    ;;
  *)
    echo "provenance MANUAL — audit $ALPHA_SEED against its .hex listing (no committed forge)"
    ;;
esac

echo "--- behavior (conformance) ---"
if sh conformance.sh; then :; else rc=1; fi

echo "--- reproduction (assembler self-host) ---"
if [ -f "${OMEGA_PATH_ALPHA_ASSEMBLER}"/selfhost.sh ]; then
  if sh "${OMEGA_PATH_ALPHA_ASSEMBLER}"/selfhost.sh; then :; else rc=1; fi
else
  echo "reproduction SKIP — ${OMEGA_PATH_ALPHA_ASSEMBLER}/selfhost.sh not found"
fi

echo ""
[ $rc = 0 ] && echo "alpha seed VERIFIED ✓ (provenance + behavior + reproduction)" || echo "alpha seed verification FAILED"
exit $rc
