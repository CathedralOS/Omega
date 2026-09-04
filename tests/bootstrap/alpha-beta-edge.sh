#!/usr/bin/env sh
# Local trust check for the host's Alpha seed, end to end:
#   provenance  - the committed binary re-derives from the committed source
#                 (where a forge exists; modulo the OS-imposed code signature);
#   behavior    - it realizes SEMANTICS.md (conformance.sh, every opcode + edge);
#   reconstruction - the VM reproduces the admitted Beta compiler tape.
# Run after touching a seed; this is the per-platform acceptance gate.
# `--edge` omits the native-source provenance rebuild. The direct compiler
# chain starts from the already selected/audited seed and needs behavior plus
# exact Beta compiler construction; rebuilding the native container is a separate
# supply-chain diagnostic, not another compiler-correctness premise.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"
rc=0

ALPHA_VERIFY_MODE=full
if [ "$#" -gt 1 ]; then
  echo "usage: $0 [--edge]" >&2
  exit 2
fi
if [ "$#" -eq 1 ]; then
  [ "$1" = --edge ] || {
    echo "usage: $0 [--edge]" >&2
    exit 2
  }
  ALPHA_VERIFY_MODE=edge
fi

if [ "$ALPHA_VERIFY_MODE" = full ]; then
  echo "--- provenance (supply-chain diagnostic) ---"
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64)
      ALPHA_DEVELOPER_DIR=$(xcode-select -p 2>/dev/null || true)
      ALPHA_CLANG=$ALPHA_DEVELOPER_DIR/Toolchains/XcodeDefault.xctoolchain/usr/bin/clang
      ALPHA_SDK=$ALPHA_DEVELOPER_DIR/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk
      if [ -x "$ALPHA_CLANG" ] && [ -d "$ALPHA_SDK" ]; then
        TMP=$(mktemp -d)
        if "$ALPHA_CLANG" -arch arm64 -isysroot "$ALPHA_SDK" -Wl,-no_uuid \
            -o "$TMP/rebuilt" "$OMEGA_PATH_ALPHA/alpha_arm64_macos.s" 2>"$TMP/err"; then
          cp "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/committed"
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
        echo "provenance SKIP — selected Xcode clang or macOS SDK not found"
      fi
      unset ALPHA_DEVELOPER_DIR ALPHA_CLANG ALPHA_SDK
      ;;
    *)
      echo "provenance MANUAL — audit $ALPHA_SEED against its .hex listing (no committed forge)"
      ;;
  esac
fi

echo "--- behavior (conformance) ---"
if sh "$OMEGA_REPO_ROOT/tests/alpha/conformance.sh"; then :; else rc=1; fi

echo "--- reconstruction (trusted Beta compiler) ---"
if [ -f "$OMEGA_REPO_ROOT/tests/beta/compiler/reconstruction.sh" ]; then
  if sh "$OMEGA_REPO_ROOT/tests/beta/compiler/reconstruction.sh"; then :; else rc=1; fi
else
  echo "reconstruction SKIP - Beta compiler gate not found"
fi

echo "--- finite root audit (diagnostic decoder/correspondence) ---"
if command -v python3 >/dev/null 2>&1; then
  if python3 "$OMEGA_REPO_ROOT/tests/beta/compiler/root-audit.py"; then :; else rc=1; fi
else
  echo "finite root audit SKIP - python3 not found"
fi

echo ""
if [ $rc = 0 ]; then
  if [ "$ALPHA_VERIFY_MODE" = full ]; then
    echo "Alpha-to-Beta edge VERIFIED (provenance diagnostic + behavior + Beta compiler construction)"
  else
    echo "Alpha-to-Beta edge VERIFIED (behavior + exact Beta compiler construction; provenance diagnostic omitted)"
  fi
else
  echo "alpha seed verification FAILED"
fi
exit $rc
