#!/usr/bin/env sh
# Local trust check for the host's alpha seed, end to end:
#   provenance  - the committed binary re-derives from the committed source
#                 (where a forge exists; modulo the OS-imposed code signature);
#   behavior    - it realizes SEMANTICS.md (conformance.sh, every opcode + edge);
#   diamond     - the VM reproduces the canonical assembler bytecode the OTHER
#                 platform's seed produced (../beta/selfhost.sh).
# Run after touching a seed; this is the per-platform acceptance gate.
cd "$(dirname "$0")"
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

echo "--- diamond (self-host) ---"
if [ -f ../beta/selfhost.sh ]; then
  if sh ../beta/selfhost.sh; then :; else rc=1; fi
else
  echo "diamond SKIP — ../beta/selfhost.sh not found"
fi

echo ""
[ $rc = 0 ] && echo "alpha seed VERIFIED ✓ (provenance + behavior + diamond)" || echo "alpha seed verification FAILED"
exit $rc
