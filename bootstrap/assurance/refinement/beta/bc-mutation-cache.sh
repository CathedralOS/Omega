#!/usr/bin/env sh
# Shared timing and content-keyed mutation-cache helpers for bc block control.

bc_timing_start() { # phase
  BC_TIMING_PHASE=$1
  BC_TIMING_STARTED=$(date +%s)
  echo "bc timing: $BC_TIMING_PHASE started"
}

bc_timing_finish() {
  BC_TIMING_FINISHED=$(date +%s)
  BC_TIMING_SECONDS=$((BC_TIMING_FINISHED - BC_TIMING_STARTED))
  echo "bc timing: $BC_TIMING_PHASE ${BC_TIMING_SECONDS}s"
}

u32_file() { # value output
  python3 -c 'import struct,sys; sys.stdout.buffer.write(struct.pack("<I", int(sys.argv[1])))' "$1" > "$2"
}

case_run() { # label expected-status input
  set +e
  "$T/control-check" < "$3" > "$T/stdout"
  got=$?
  set -e
  if [ "$got" != "$2" ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $1: expected $2/empty, got $got/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
}

# Cache only complete negative-test responsibilities. Canonical theorem owners
# are deliberately rebuilt and smoked in this process before this helper is
# reached; a receipt never recreates a BC_OWNER_* capability or a host
# executable. Set BC_BLOCK_CACHE=0 for a physically cold exhaustive replay.
bc_run_cached_teeth() { # name inventory build-function reject-function key-files...
  bc_shard_name=$1
  bc_shard_inventory=$2
  bc_shard_build=$3
  bc_shard_reject=$4
  shift 4

  bc_shard_cache_dir="$OMEGA_REPO_ROOT/.lattice-cache/bc-block-control"
  mkdir -p "$bc_shard_cache_dir"
  for bc_shard_file in "$@"; do
    if [ ! -f "$bc_shard_file" ] || [ ! -r "$bc_shard_file" ]; then
      echo "bc block control FAIL — unreadable cache-key input: $bc_shard_file" >&2
      exit 2
    fi
  done
  bc_shard_key=$(
    {
      printf '%s\n' 'bc-block-control-teeth-v1'
      printf 'name=%s\ninventory=%s\n' "$bc_shard_name" "$bc_shard_inventory"
      shasum < "$GATE_DIR/bc-mutation-cache.sh"
      bc_shard_index=0
      for bc_shard_file in "$@"; do
        bc_shard_index=$((bc_shard_index + 1))
        printf 'file-index=%s\n' "$bc_shard_index"
        shasum < "$bc_shard_file"
      done
    } | shasum | cut -d' ' -f1
  )
  bc_shard_receipt="$bc_shard_cache_dir/$bc_shard_name.green"
  bc_shard_expected="$T/$bc_shard_name.receipt"
  {
    printf 'schema=bc-block-control-teeth-v1\n'
    printf 'name=%s\n' "$bc_shard_name"
    printf 'inventory=%s\n' "$bc_shard_inventory"
    printf 'key=%s\n' "$bc_shard_key"
  } > "$bc_shard_expected"

  if [ "${BC_BLOCK_CACHE:-1}" != 0 ] &&
     [ -f "$bc_shard_receipt" ] &&
     cmp -s "$bc_shard_expected" "$bc_shard_receipt"; then
    echo "bc timing: $bc_shard_name 0s (cached: exact $bc_shard_inventory inventory unchanged)"
    return
  fi

  # A forced audit or changed key revokes the previous result before work
  # starts. An interrupted/failed replay must not leave an older green usable.
  rm -f "$bc_shard_receipt"
  bc_timing_start "$bc_shard_name"
  "$bc_shard_build"
  "$bc_shard_reject"
  bc_timing_finish
  bc_shard_receipt_tmp="$bc_shard_receipt.tmp.$$"
  cp "$bc_shard_expected" "$bc_shard_receipt_tmp"
  mv "$bc_shard_receipt_tmp" "$bc_shard_receipt"
}

# The oldest Checker-A matrix keeps its historical two-phase order: construct
# every selected mutant first, then execute every selected rejection. Each
# responsibility lives in its own module, so changing one module does not
# invalidate its siblings; common harness changes correctly invalidate all.
bc_prepare_phased_teeth() { # safe-name inventory module key-files...
  bc_inline_name=$1
  bc_inline_inventory=$2
  bc_inline_module=$3
  shift 3
  case "$bc_inline_name" in
    ''|*[!a-z0-9_]*)
      echo "bc block control FAIL — invalid inline shard name: $bc_inline_name" >&2
      exit 2
      ;;
  esac

  bc_inline_cache_dir="$OMEGA_REPO_ROOT/.lattice-cache/bc-block-control"
  mkdir -p "$bc_inline_cache_dir"
  if [ ! -f "$bc_inline_module" ] || [ ! -r "$bc_inline_module" ]; then
    echo "bc block control FAIL — unreadable shard module: $bc_inline_module" >&2
    exit 2
  fi
  for bc_inline_file in "$@"; do
    if [ ! -f "$bc_inline_file" ] || [ ! -r "$bc_inline_file" ]; then
      echo "bc block control FAIL — unreadable cache-key input: $bc_inline_file" >&2
      exit 2
    fi
  done
  bc_inline_build=${bc_inline_name}_build_teeth
  bc_inline_reject=${bc_inline_name}_reject_teeth
  command -v "$bc_inline_build" >/dev/null 2>&1 || {
    echo "bc block control FAIL — missing shard builder: $bc_inline_build" >&2
    exit 2
  }
  command -v "$bc_inline_reject" >/dev/null 2>&1 || {
    echo "bc block control FAIL — missing shard rejector: $bc_inline_reject" >&2
    exit 2
  }
  bc_inline_key=$(
    {
      printf '%s\n' 'bc-block-control-inline-v1'
      printf 'name=%s\ninventory=%s\n' \
        "$bc_inline_name" "$bc_inline_inventory"
      printf 'build=%s\nreject=%s\n' "$bc_inline_build" "$bc_inline_reject"
      shasum < "$GATE_DIR/bc-mutation-cache.sh"
      shasum < "$bc_inline_module"
      bc_inline_index=0
      for bc_inline_file in "$@"; do
        bc_inline_index=$((bc_inline_index + 1))
        printf 'file-index=%s\n' "$bc_inline_index"
        shasum < "$bc_inline_file"
      done
    } | shasum | cut -d' ' -f1
  )
  bc_inline_receipt="$bc_inline_cache_dir/checker-a-$bc_inline_name.green"
  bc_inline_expected="$T/checker-a-$bc_inline_name.receipt"
  {
    printf 'schema=bc-block-control-inline-v1\n'
    printf 'name=%s\n' "$bc_inline_name"
    printf 'inventory=%s\n' "$bc_inline_inventory"
    printf 'key=%s\n' "$bc_inline_key"
  } > "$bc_inline_expected"

  if [ "${BC_BLOCK_CACHE:-1}" != 0 ] &&
     [ -f "$bc_inline_receipt" ] &&
     cmp -s "$bc_inline_expected" "$bc_inline_receipt"; then
    echo "bc timing: checker-a-$bc_inline_name 0s (cached: exact $bc_inline_inventory inventory unchanged)"
    return
  fi
  rm -f "$bc_inline_receipt"
  : > "$T/checker-a-$bc_inline_name.run"
  printf '0\n' > "$T/checker-a-$bc_inline_name.seconds"
}

bc_prepare_standard_phased() { # safe-name inventory module-basename
  bc_prepare_phased_teeth "$1" "$2" "$GATE_DIR/$3" \
    "$T/control-check.alpha" "$T/control.bundle" \
    "$ARTIFACT" "$ASM" "$SEED" \
    "$OMEGA_PATH_BETA/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh"
}

bc_inline_shard_should_run() { # safe-name
  [ -f "$T/checker-a-$1.run" ]
}

bc_inline_shard_phase_start() { # safe-name build|reject
  echo "bc timing: checker-a-$1-$2 started"
  date +%s > "$T/checker-a-$1.started"
}

bc_inline_shard_phase_finish() { # safe-name
  bc_inline_finished=$(date +%s)
  bc_inline_started=$(cat "$T/checker-a-$1.started")
  bc_inline_seconds=$(cat "$T/checker-a-$1.seconds")
  bc_inline_seconds=$((bc_inline_seconds + bc_inline_finished - bc_inline_started))
  printf '%s\n' "$bc_inline_seconds" > "$T/checker-a-$1.seconds"
}

bc_finish_inline_shard() { # safe-name
  bc_inline_seconds=$(cat "$T/checker-a-$1.seconds")
  echo "bc timing: checker-a-$1 ${bc_inline_seconds}s"
  bc_inline_receipt="$OMEGA_REPO_ROOT/.lattice-cache/bc-block-control/checker-a-$1.green"
  bc_inline_receipt_tmp="$bc_inline_receipt.tmp.$$"
  cp "$T/checker-a-$1.receipt" "$bc_inline_receipt_tmp"
  mv "$bc_inline_receipt_tmp" "$bc_inline_receipt"
}

bc_phased_build() { # safe-name
  if bc_inline_shard_should_run "$1"; then
    bc_inline_shard_phase_start "$1" build
    bc_inline_build=${1}_build_teeth
    "$bc_inline_build"
    bc_inline_shard_phase_finish "$1"
  fi
}

bc_phased_reject_commit() { # safe-name
  if bc_inline_shard_should_run "$1"; then
    bc_inline_shard_phase_start "$1" reject
    bc_inline_reject=${1}_reject_teeth
    "$bc_inline_reject"
    bc_inline_shard_phase_finish "$1"
    bc_finish_inline_shard "$1"
  fi
}
