#!/usr/bin/env sh
# Materialize the direct Beta compiler tape in the selected audited Alpha VM.
# Source tools/bootstrap/paths.sh first.

[ -n "${OMEGA_PATH_BETA_COMPILER:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "Beta artifact: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

# Bound Alpha-to-Beta edge subject. These are the same identities audited in
# bootstrap/1_beta/AUDIT.md ("Bound subject") and independently decoded by
# tests/beta/compiler/root-audit.py. A digest here is an identity check that
# the bytes being stamped are the audited ones; it is not a proof of the
# compiler and does not replace reading that audit. Changing the source or
# tape invalidates the dependent evidence and must update all three records.
BETA_COMPILER_SOURCE_SIZE=12536
BETA_COMPILER_SOURCE_SHA256=2f9a9f55a2c708731567367380521bfe35b1c13a00a367febd1f9f654c25f320
BETA_COMPILER_TAPE_SIZE=1773
BETA_COMPILER_TAPE_SHA256=4b1572a5ce406fc5b194e047ac6c5c601c1860261933381443a08c9041b77361

# bootstrap_sha256 FILE : print the lowercase hex SHA-256 of FILE using the
# host's standard digest tool (sha256sum on Linux and Git Bash, shasum on
# macOS, python3 as the last route). Returns 2 when no route exists; identity
# then cannot be established and materialization refuses rather than skipping.
bootstrap_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -- "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -- "$1" | cut -d' ' -f1
  elif command -v python3 >/dev/null 2>&1; then
    python3 -c 'import hashlib, sys; print(hashlib.sha256(open(sys.argv[1], "rb").read()).hexdigest())' "$1"
  else
    echo "Beta artifact: no SHA-256 tool (sha256sum, shasum, python3) to bind identity" >&2
    return 2
  fi
}

# require_bound_identity LABEL FILE SIZE SHA256 : exact size then digest.
require_bound_identity() {
  [ -f "$2" ] || {
    echo "Beta artifact: missing $2" >&2
    return 2
  }
  BOUND_SIZE=$(wc -c < "$2" | tr -d ' ')
  [ "$BOUND_SIZE" = "$3" ] || {
    echo "Beta artifact: $1 is $BOUND_SIZE bytes; the audited subject is $3 (bootstrap/1_beta/AUDIT.md)" >&2
    return 3
  }
  BOUND_SHA256=$(bootstrap_sha256 "$2") || return $?
  [ "$BOUND_SHA256" = "$4" ] || {
    echo "Beta artifact: $1 SHA-256 $BOUND_SHA256 differs from the audited subject $4 (bootstrap/1_beta/AUDIT.md)" >&2
    return 3
  }
}

# require_beta_compiler_identity : the canonical source and tape are exactly
# the audited pair. Every materialization runs it; tests may call it directly.
require_beta_compiler_identity() {
  require_bound_identity "beta_compiler.beta" "$OMEGA_PATH_BETA_COMPILER_SOURCE" \
    "$BETA_COMPILER_SOURCE_SIZE" "$BETA_COMPILER_SOURCE_SHA256" || return $?
  require_bound_identity "beta_compiler_bytecode.tape" "$OMEGA_PATH_BETA_COMPILER_TAPE" \
    "$BETA_COMPILER_TAPE_SIZE" "$BETA_COMPILER_TAPE_SHA256" || return $?
}

materialize_beta_compiler() {
  BETA_COMPILER_DEST=$1
  require_beta_compiler_identity || return $?
  stamp_seed "$OMEGA_PATH_BETA_COMPILER_TAPE" \
    "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$BETA_COMPILER_DEST"
}
