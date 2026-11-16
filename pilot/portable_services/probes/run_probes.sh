#!/usr/bin/env bash
# Run the portable-service probes on this host. Usage: run_probes.sh <omega binary>
# Each scenario prepares /tmp state, runs fs_probe with `omega run --both`
# (Psi interpreter and native, both must agree), and decodes the exit code.
set -u
OMEGA=$1; HERE=$(cd "$(dirname "$0")" && pwd)
decode() {
  local c=$1
  local a=$((c%4)) b=$(((c/4)%4)) cc=$(((c/16)%4)) d=$(((c/64)%2)) f=$(((c/128)%2))
  local A=(refused-InvalidInput opened-and-EMPTIED opened-kept other)
  local B=(refused-InvalidInput clamped-Ok4 Ok-other other-error)
  local C=(ok-leaf-exists Error ok-leaf-MISSING ?)
  echo "  open_with(read+truncate)=${A[$a]}  read(count>buf)=${B[$b]}  create_dir_all(281B)=${C[$cc]}  remove_dir_all=$([ $d = 0 ] && echo Ok || echo Error)  set_permissions(Default)=$([ $f = 0 ] && echo Ok || echo Error)"
}
prep_common() {
  rm -rf /tmp/opr_trunc /tmp/opr_read /tmp/opr_mk /tmp/opr_tree /tmp/opr_perm
  umask 022; touch /tmp/opr_perm; chmod 600 /tmp/opr_perm
}
run() {
  local label=$1
  echo "== $label"
  ( cd "$HERE/fs_probe" && timeout 600 "$OMEGA" run --both --build-input build.omg --build-input main.omg main.omg < /dev/null > "$HERE/$label.log" 2>&1 ); local rc=$?
  tail -5 "$HERE/$label.log" | sed 's/^/  | /'
  echo "  exit=$rc"; decode $rc
  echo "  tree left behind: $( [ -e /tmp/opr_tree ] && find /tmp/opr_tree | wc -l || echo none ) ; perm mode now: $(stat -c %a /tmp/opr_perm)"
}
# Scenario 1: deep tree, 20 nested directories (POSIX descent stack is 16).
prep_common; d=/tmp/opr_tree; for i in $(seq 1 20); do d=$d/n$i; done; mkdir -p $d; touch $d/leaf
run deep20
# Scenario 2: wide tree, 5000 files in one directory (POSIX walk budget is 4096 steps).
prep_common; mkdir -p /tmp/opr_tree; (cd /tmp/opr_tree && seq 1 5000 | xargs touch)
run wide5000
# Scenario 3: control, small tree (depth 3, 10 files).
prep_common; mkdir -p /tmp/opr_tree/x/y; (cd /tmp/opr_tree/x/y && seq 1 10 | xargs touch)
run control
