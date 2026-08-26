#!/bin/sh
# Focused generated ordinary-source recipe, reproduction, resource, and
# OMGCOMP1 source-extent custody gate.
set -eu

gate_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
repo_root=$gate_dir
while [ ! -f "$repo_root/bootstrap/paths.sh" ]; do
  parent=$(dirname -- "$repo_root")
  [ "$parent" != "$repo_root" ] || {
    echo "generated source custody: repository root not found" >&2
    exit 2
  }
  repo_root=$parent
done
cd "$repo_root"

tool="$gate_dir/generated_source_custody.py"
recipe="$gate_dir/fixtures/generated-source-custody/unicode-tables.recipe.json"
for required in "$tool" "$recipe"; do
  [ -f "$required" ] || {
    echo "generated source custody: required input absent: $required" >&2
    exit 1
  }
done
for command in python3 cargo cmp wc; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "generated source custody: skipped ($command absent)"
    exit 0
  }
done

stage=$(mktemp -d "${TMPDIR:-/tmp}/omega-generated-source-custody.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT HUP INT TERM

python3 -B "$tool" verify "$recipe" > "$stage/verify.out"
[ ! -s "$stage/verify.out" ] || {
  echo "generated source custody: verifier published unexpected bytes" >&2
  exit 1
}
python3 -B "$tool" teeth "$recipe" > "$stage/teeth.out"
[ ! -s "$stage/teeth.out" ] || {
  echo "generated source custody: mutation/resource teeth published unexpected bytes" >&2
  exit 1
}

python3 -B "$tool" materialize "$recipe" > "$stage/generated.omgc"
python3 -B "$tool" verify-carrier "$recipe" "$stage/generated.omgc" \
  > "$stage/carrier.out"
[ ! -s "$stage/carrier.out" ] || {
  echo "generated source custody: carrier verifier published unexpected bytes" >&2
  exit 1
}

carrier_bytes=$(wc -c < "$stage/generated.omgc" | tr -d ' ')
[ "$carrier_bytes" -eq 84140 ] || {
  echo "generated source custody: OMGCOMP1 size $carrier_bytes, expected 84140" >&2
  exit 1
}

python3 -B "$tool" materialize "$recipe" > "$stage/generated-repeat.omgc"
cmp "$stage/generated.omgc" "$stage/generated-repeat.omgc"

echo "generated source custody: canonical recipe, two-run reproduction, exact/adjacent/no-publication teeth, and 84140-byte OMGCOMP1 source-extent join passed"
