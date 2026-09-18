#!/usr/bin/env sh
# Materialize the canonical Delta-authored Epsilon evaluator closure.
# Source tools/bootstrap/paths.sh first.

[ -n "${OMEGA_PATH_EPSILON_COMPILER_SOURCES:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "Epsilon evaluator: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

# Bound Delta-to-Epsilon edge subject. The evaluator is not a tape:
# epsilon_compiler.delta.sources is the ordered source authority whose
# DeltaSourceClosureV1 rows bind every member length, digest, and path, and
# repacking it reproduces the packed evaluator bytes pinned in
# bootstrap/4_epsilon/README.md ("Exact source closure"). A digest here is an
# identity check that the bytes being materialized are the bound ones; it is
# not a proof that the evaluator implements Epsilon. Changing a member or the
# manifest invalidates the dependent evidence and must update every record.
EPSILON_EVALUATOR_MANIFEST_SIZE=15163
EPSILON_EVALUATOR_MANIFEST_SHA256=717e6bdc90850b9cacc0858279f3483d10c93f3289f43d9ff83ac27b4e862061
EPSILON_EVALUATOR_PACKED_SIZE=617354
EPSILON_EVALUATOR_PACKED_SHA256=4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e

# Bound edge adapter and reconstructed obligation. The canonical execution
# driver tests/epsilon/interpreted-omega-experiment/execution_driver.delta is
# the Delta adapter every cross-rung consumer appends after the packed
# evaluator; compiling the bound evaluator plus this driver and the bound
# Delta support section through the bound Delta compiler reconstructs exactly
# one evaluator receipt, which those consumers then execute. The identical
# pins in tests/epsilon/interpreted-omega-experiment/{README.md,run.sh},
# tests/epsilon/array-storage/gate.py, and tests/bootstrap/omega-*/gate.py are
# records of this one obligation, not independent identities. A digest here is
# an identity check on the driver or the reconstructed bytes; it is not a
# proof of evaluation. Changing the driver or any bound input changes the
# receipt and must update every record together.
EPSILON_EXECUTION_DRIVER_SIZE=2565
EPSILON_EXECUTION_DRIVER_SHA256=ba509602e6873117e59ffc544ada6c8aa16e20b08311e69a01b7cb3897199b38
EPSILON_EVALUATOR_RECEIPT_SIZE=721484
EPSILON_EVALUATOR_RECEIPT_SHA256=71a016f53f63501760e3a10632d86c9561aa0e8387b794b074d98ce98a823082

# Bound canonical section-11 entry and its reconstructed obligation. The
# canonical evaluator entry tests/epsilon/evaluator-entry/evaluator_entry.delta
# is the evaluator's real `main`: appended after the packed evaluator it forms
# the Delta subject whose compilation through the bound Delta compiler and
# support section reconstructs exactly one canonical evaluator receipt - the
# artifact that consumes the EREQ envelope and publishes canonical
# observations or EEOUT refusal frames
# (bootstrap/4_epsilon/EVALUATOR_ENTRY.md). The entry lives outside the
# closure because the closure's source inventory owns every .delta file under
# bootstrap/4_epsilon/; it sits beside its boundary gate under the same
# placement rule as the diagnostic driver. The identical pins in
# tests/epsilon/evaluator-entry/{README.md,run.sh} and
# bootstrap/4_epsilon/EVALUATOR_ENTRY.md are records of this one obligation,
# not independent identities. A digest here is an identity check on the entry
# source or the reconstructed bytes; it is not a proof of evaluation. Changing
# the entry, the packed closure, the compiler, or the support section changes
# the receipt and must update every record together.
EPSILON_EVALUATOR_ENTRY_SIZE=10950
EPSILON_EVALUATOR_ENTRY_SHA256=52032438c1236f51095b761afcb3111df2ae2d73ac9be7e91883bbfbd273e5e3
EPSILON_EVALUATOR_ENTRY_RECEIPT_SIZE=729060
EPSILON_EVALUATOR_ENTRY_RECEIPT_SHA256=bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368

# require_epsilon_evaluator_identity : the canonical manifest is the bound
# file and repacking it reproduces exactly the bound evaluator closure.
# Every materialization runs it; tests may call it directly. bootstrap_sha256
# and require_bound_identity live in alpha/seed_env.sh. Repacking needs
# python3; without it the closure identity cannot be established and the
# check refuses rather than skipping.
require_epsilon_evaluator_identity() {
  require_bound_identity "epsilon_compiler.delta.sources" \
    "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" \
    "$EPSILON_EVALUATOR_MANIFEST_SIZE" "$EPSILON_EVALUATOR_MANIFEST_SHA256" \
    "bootstrap/4_epsilon/README.md" || return $?
  command -v python3 >/dev/null 2>&1 || {
    echo "bootstrap artifact: no python3 to repack the Epsilon source closure" >&2
    return 2
  }
  EPSILON_IDENTITY_TMP=$(mktemp -d)
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$EPSILON_IDENTITY_TMP/evaluator.delta" || {
      EPSILON_IDENTITY_RC=$?
      rm -rf -- "$EPSILON_IDENTITY_TMP"
      return "$EPSILON_IDENTITY_RC"
    }
  require_bound_identity "canonical Epsilon evaluator closure" \
    "$EPSILON_IDENTITY_TMP/evaluator.delta" \
    "$EPSILON_EVALUATOR_PACKED_SIZE" "$EPSILON_EVALUATOR_PACKED_SHA256" \
    "bootstrap/4_epsilon/README.md"
  EPSILON_IDENTITY_RC=$?
  rm -rf -- "$EPSILON_IDENTITY_TMP"
  return "$EPSILON_IDENTITY_RC"
}

# require_epsilon_execution_driver_identity : the canonical slice driver
# appended after the packed evaluator by every cross-rung consumer is the
# bound file. The driver is a separate input to the DCREQ subject, so
# consumers that frame evaluator+driver requests run this before compiling;
# tests may call it directly. It is not part of the source closure and does
# not run during materialization.
require_epsilon_execution_driver_identity() {
  require_bound_identity "execution_driver.delta" \
    "$OMEGA_PATH_EPSILON_EXECUTION_DRIVER" \
    "$EPSILON_EXECUTION_DRIVER_SIZE" "$EPSILON_EXECUTION_DRIVER_SHA256" \
    "tests/epsilon/interpreted-omega-experiment/README.md"
}

# require_epsilon_evaluator_receipt_identity RECEIPT : the receipt a caller
# reconstructed by executing the bound Delta compiler over the bound
# evaluator, bound driver, and bound support section is exactly the bound
# obligation. The check executes nothing; a divergent reconstruction is
# refused before the caller could consume it.
require_epsilon_evaluator_receipt_identity() {
  require_bound_identity "Epsilon evaluator receipt" "$1" \
    "$EPSILON_EVALUATOR_RECEIPT_SIZE" "$EPSILON_EVALUATOR_RECEIPT_SHA256" \
    "tests/epsilon/interpreted-omega-experiment/README.md"
}

# require_epsilon_evaluator_entry_identity : the canonical section-11 entry
# appended after the packed evaluator to form the canonical Delta subject is
# the bound file. The entry is a separate input to the DCREQ subject, so
# consumers that frame evaluator+entry requests run this before compiling;
# tests may call it directly. It is not part of the source closure and does
# not run during materialization.
require_epsilon_evaluator_entry_identity() {
  require_bound_identity "evaluator_entry.delta" \
    "${OMEGA_PATH_EPSILON_EVALUATOR_ENTRY:-$OMEGA_REPO_ROOT/tests/epsilon/evaluator-entry/evaluator_entry.delta}" \
    "$EPSILON_EVALUATOR_ENTRY_SIZE" "$EPSILON_EVALUATOR_ENTRY_SHA256" \
    "tests/epsilon/evaluator-entry/README.md"
}

# require_epsilon_evaluator_entry_receipt_identity RECEIPT : the canonical
# receipt a caller reconstructed by executing the bound Delta compiler over
# the bound evaluator, bound entry, and bound support section is exactly the
# bound obligation. The check executes nothing; a divergent reconstruction is
# refused before the caller could consume it.
require_epsilon_evaluator_entry_receipt_identity() {
  require_bound_identity "Epsilon evaluator canonical receipt" "$1" \
    "$EPSILON_EVALUATOR_ENTRY_RECEIPT_SIZE" \
    "$EPSILON_EVALUATOR_ENTRY_RECEIPT_SHA256" \
    "tests/epsilon/evaluator-entry/README.md"
}

# materialize_epsilon_evaluator DEST : write the canonical packed evaluator
# Delta source to DEST after the bound identity check. source_closure.py
# checks every member row and writes atomically; a changed member refuses
# before DEST is written.
materialize_epsilon_evaluator() {
  EPSILON_EVALUATOR_DEST=$1
  require_epsilon_evaluator_identity || return $?
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$EPSILON_EVALUATOR_DEST"
}
