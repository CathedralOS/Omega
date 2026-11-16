#!/bin/bash
# run.sh SEED COUNT RUNDIR
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
WT=$(cd "$HERE/../.." && pwd)
SEED=$1; COUNT=$2; RUN=$(realpath -m $3); mkdir -p $RUN
python3 $HERE/gen.py $SEED $COUNT $RUN
declare -A TRIPLE=([linux_x86_64]=x86_64-linux-gnu [linux_arm64]=aarch64-linux-gnu [macos_arm64]=arm64-apple-macos [windows_x86_64]=x86_64-pc-windows-msvc)
for t in "${!TRIPLE[@]}"; do
  clang --target=${TRIPLE[$t]} -O0 -ffreestanding -fno-stack-protector -S -mllvm -stop-after=finalize-isel $RUN/sigs.c -o $RUN/clang.${TRIPLE[$t]}.mir &
done; wait
cd $WT
for t in "${!TRIPLE[@]}"; do
  ABI_PILOT_IN=$RUN/spec.$t.txt ABI_PILOT_OUT=$RUN/omega.$t.txt CARGO_PROFILE_DEV_OPT_LEVEL=1 nice -n 19 mbx test -p calling-conventions --lib abi_pilot_dump < /dev/null > $RUN/mbx.$t.log 2>&1 || { tail -30 $RUN/mbx.$t.log; exit 1; }
done
cd $HERE
for t in linux_x86_64 linux_arm64 macos_arm64 windows_x86_64; do
  python3 diff.py $RUN $t ${TRIPLE[$t]} > $RUN/diff.$t.txt
  tail -1 $RUN/diff.$t.txt
done
