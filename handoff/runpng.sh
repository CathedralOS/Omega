#!/bin/sh
# usage: runpng.sh dir n -> median omega us and ratio
cd "$1"; for i in $(seq 1 $2); do PNGDECODE_SEED=$i LD_LIBRARY_PATH=. ./omega-program 2>/dev/null | tail -1; done | awk -F, '{print $4, $6}' | sort -n -k2 | awk '{a[NR]=$1; r[NR]=$2} END {m=int((NR+1)/2); print "median ratio", r[m], "omega_us(at median)", a[m], "n", NR}'
