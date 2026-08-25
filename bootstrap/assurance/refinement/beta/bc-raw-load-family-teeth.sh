#!/usr/bin/env sh
# Extracted Checker-A historical canary for fixed raw-load family ownership.
# This one-case module is also the cache-invalidation canary.

raw_load_family_build_teeth() {
# Phase-isolated fixed-load tooth: keep the exact source, artifact, witness,
# grammar counts, and every prior phase unchanged while omitting one load-class
# owner.  The exhaustive 95-row family scan must find the unclassified load.
sed '/call composition_load_parse_fixed/{n;s/imm r6, 1/imm r6, 0/;}' \
  "$T/control-check.alpha" > "$T/load-missing-class.alpha"
"$ASM" < "$T/load-missing-class.alpha" > "$T/load-missing-class.tape"
stamp_seed "$T/load-missing-class.tape" "$SEED" "$T/load-missing-class" >/dev/null
}

raw_load_family_reject_teeth() {
set +e
"$T/load-missing-class" < "$T/control.bundle" > "$T/stdout"
load_missing_class_status=$?
set -e
if [ "$load_missing_class_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — missing fixed raw-load class was not rejected" >&2
  exit 1
fi
}
