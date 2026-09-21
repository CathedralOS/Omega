#!/usr/bin/env sh
# Local trust check for the host's Alpha seed, end to end:
#   provenance  - the committed binary re-derives from the committed source
#                 (macOS arm64: clang rebuild modulo the OS-imposed code
#                 signature; Windows x64: the committed forge re-emits the
#                 audited .hex listing — checkable on any host with Python 3;
#                 Linux x86-64: GNU binutils re-serializes alpha_x64_linux.s
#                 into a byte-parity clone of the committed ELF);
#   container   - both audited seeds parse as native executables and the
#                 recorded stamping hole is the tape section's raw extent —
#                 checkable on any host with Python 3;
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
      if [ ! -x "$ALPHA_CLANG" ] || [ ! -d "$ALPHA_SDK" ]; then
        ALPHA_CLANG=$(xcrun --find clang 2>/dev/null || true)
        ALPHA_SDK=$(xcrun --show-sdk-path 2>/dev/null || true)
      fi
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
        echo "provenance SKIP — selected Xcode/CommandLineTools clang or macOS SDK not found"
      fi
      unset ALPHA_DEVELOPER_DIR ALPHA_CLANG ALPHA_SDK
      ;;
    *)
      # The selected seed is the Windows x64 container; its audited source is
      # the annotated .hex listing and the committed forge re-emits it. That
      # comparison needs only Python 3, so it also runs on hosts (like this
      # Linux checkout) where the container itself cannot execute.
      if command -v python3 >/dev/null 2>&1; then
        if python3 "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/forge.py" \
            "$OMEGA_PATH_ALPHA/alpha_x64_windows.hex" --check \
            "$OMEGA_PATH_ALPHA/alpha_x64_windows.exe"; then
          echo "provenance ✓ — alpha_x64_windows.exe reproduces from alpha_x64_windows.hex (committed forge)"
        else
          echo "provenance FAIL — committed binary differs from a forge of its listing"; rc=1
        fi
      else
        echo "provenance MANUAL — audit $ALPHA_SEED against its .hex listing (committed forge needs python3)"
      fi
      ;;
  esac

  # The third audited seed is a static ELF64 whose provenance is a byte-exact
  # GNU binutils rebuild of alpha_x64_linux.s (as --64 + ld -s --build-id=none).
  # That toolchain produces ELF only on Linux x86-64, so the clone check lives
  # there — the same shape as the macOS leg living under Xcode clang.
  if [ "$(uname -s)-$(uname -m)" = "Linux-x86_64" ]; then
    if command -v as >/dev/null 2>&1 && command -v ld >/dev/null 2>&1; then
      TMP=$(mktemp -d)
      if as --64 -o "$TMP/a.o" "$OMEGA_PATH_ALPHA/alpha_x64_linux.s" 2>"$TMP/err" &&
         ld -s -o "$TMP/rebuilt" --build-id=none -e _start "$TMP/a.o" 2>>"$TMP/err"; then
        if cmp -s "$TMP/rebuilt" "$OMEGA_PATH_ALPHA/alpha_x64_linux"; then
          echo "provenance ✓ — alpha_x64_linux reproduces from alpha_x64_linux.s (GNU binutils)"
        else
          echo "provenance FAIL — committed alpha_x64_linux differs from a rebuild of its source"; rc=1
        fi
      else
        echo "provenance FAIL — alpha_x64_linux rebuild errored:"; sed 's/^/  /' "$TMP/err"; rc=1
      fi
      rm -rf "$TMP"
    else
      echo "provenance SKIP — GNU binutils as/ld not found for alpha_x64_linux rebuild"
    fi
  fi
fi

# The container leg inspects the committed seeds' native structure, so it runs
# on every host with Python 3, including ones that cannot execute a seed.
echo "--- container (both seeds' native structure) ---"
if sh "$OMEGA_REPO_ROOT/tests/alpha/container.sh"; then :; else rc=1; fi

# Seed-execution legs need a host that can run an audited Alpha container
# (seed_env.sh's ALPHA_SEED_EXECUTABLE). On any other host they refuse
# (exit 2 at the leaf gates), and the edge then reports unavailable rather
# than failed: no seed case ran, so none failed.
refused=0

echo "--- behavior (conformance) ---"
if [ "$ALPHA_SEED_EXECUTABLE" = 1 ]; then
  if sh "$OMEGA_REPO_ROOT/tests/alpha/conformance.sh"; then :; else rc=1; fi
else
  echo "alpha conformance: requires macOS arm64, Linux x86-64, or Windows x64" >&2
  refused=1
fi

echo "--- reconstruction (trusted Beta compiler) ---"
if [ "$ALPHA_SEED_EXECUTABLE" = 1 ]; then
  if [ -f "$OMEGA_REPO_ROOT/tests/beta/compiler/reconstruction.sh" ]; then
    if sh "$OMEGA_REPO_ROOT/tests/beta/compiler/reconstruction.sh"; then :; else rc=1; fi
  else
    echo "reconstruction SKIP - Beta compiler gate not found"
  fi
else
  echo "Beta compiler reconstruction: requires macOS arm64, Linux x86-64, or Windows x64" >&2
  refused=1
fi

echo "--- finite root audit (diagnostic decoder/correspondence) ---"
if command -v python3 >/dev/null 2>&1; then
  if python3 "$OMEGA_REPO_ROOT/tests/beta/compiler/root-audit.py"; then :; else rc=1; fi
else
  echo "finite root audit SKIP - python3 not found"
fi

echo "--- shared hexadecimal prefix (strict grammar) ---"
if [ "$ALPHA_SEED_EXECUTABLE" = 1 ]; then
  if sh "$OMEGA_REPO_ROOT/tests/beta/compiler/word-prefix.sh"; then :; else rc=1; fi
else
  echo "Beta word prefix: requires macOS arm64, Linux x86-64, or Windows x64" >&2
  refused=1
fi

echo ""
if [ $rc != 0 ]; then
  echo "alpha seed verification FAILED"
elif [ $refused != 0 ]; then
  echo "Alpha-to-Beta edge UNAVAILABLE — seed execution requires macOS arm64, Linux x86-64, or Windows x64"
  rc=2
elif [ "$ALPHA_VERIFY_MODE" = full ]; then
  echo "Alpha-to-Beta edge VERIFIED (provenance diagnostic + behavior + Beta compiler construction)"
else
  echo "Alpha-to-Beta edge VERIFIED (behavior + exact Beta compiler construction; provenance diagnostic omitted)"
fi
exit $rc
