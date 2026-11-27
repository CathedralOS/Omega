#!/usr/bin/env sh
# Materialize the canonical Epsilon-written Omega compiler D closure.
# Source tools/bootstrap/paths.sh first.

[ -n "${OMEGA_PATH_OMEGA_COMPILER_SOURCES:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "Omega compiler D: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

# Bound Epsilon-to-Omega edge subject. D is not a tape:
# omega_compiler.epsilon.sources is the ordered source authority whose
# EpsilonSourceClosureV1 rows bind every member length, digest, and path, and
# repacking it reproduces the packed compiler bytes pinned in
# bootstrap/omega/compiler/README.md. A digest here is an identity check that the
# bytes being materialized are the bound ones; it is not a proof that D
# implements Omega. Changing a member or the manifest invalidates the
# dependent evidence and must update every record.
OMEGA_COMPILER_MANIFEST_SIZE=3249
OMEGA_COMPILER_MANIFEST_SHA256=bc3fcef87f792458d6a49b61d8fcf5564e4974bee0207c0f23bc0327dd294675
OMEGA_COMPILER_PACKED_SIZE=1198182
OMEGA_COMPILER_PACKED_SHA256=2bffd9f7ca8c3d8297b31e4667baa870c5b5a717c66471f336a9999e2b233db5

# Bound gate-local entries packed on top of the bound member bytes. Each
# omega-* gate appends its own customer entry after the packed D closure —
# the customer bytes are compiler plus entry — so the entries bind here the
# same way the staged-compiler development driver binds on the Delta edge:
# separate inputs from the canonical closure, never part of the manifested
# members. The identical pins in each gate's README and gate.py are records
# of these same subjects, not independent identities. A digest here is an
# identity check on the entry source; it is not a proof of the gate's
# judgment. Changing an entry changes the packed customer and must update
# every record together.
OMEGA_PARSER_ENTRY_SIZE=88096
OMEGA_PARSER_ENTRY_SHA256=83a82f0621f5166fc1900b16acd7bde5e9ef8d86a4b8b56f9708c495c83c8e3b
OMEGA_OUTCOME_ENTRY_SIZE=19642
OMEGA_OUTCOME_ENTRY_SHA256=61fb1ed6f8721c2efb4921e1eba1ae2432f2746571c58c3a769bfc918884dd5c
OMEGA_REQUEST_ENTRY_SIZE=4112
OMEGA_REQUEST_ENTRY_SHA256=9368297baef947465d5f1ee11df8f1a0fdf60a01e369836ce0555df02890d9ca
OMEGA_REQUEST_FIXTURE_SIZE=135
OMEGA_REQUEST_FIXTURE_SHA256=3273ea92c9ebf7b1f7a6768bd8cf786706744c559c29d84ee738af1f7968f6fd
OMEGA_EXECUTABLE_MAIN_ENTRY_SIZE=1757
OMEGA_EXECUTABLE_MAIN_ENTRY_SHA256=c0af3126f13c8c511d04f224e630f60f3316c0f9fa6e310e72f66779c7c3ce9e
OMEGA_EXECUTABLE_OCREQ_ENTRY_SIZE=19043
OMEGA_EXECUTABLE_OCREQ_ENTRY_SHA256=bff0a5aa5df1c4fa0f47ab267d05fa91c48238c81674be5a525435b73e0f668e
OMEGA_EXECUTABLE_CONTROLS_ENTRY_SIZE=3539
OMEGA_EXECUTABLE_CONTROLS_ENTRY_SHA256=16a4915fedac6e95a50cb2b9405c988d995935146d0552606b698d3f52be8740
OMEGA_EXECUTABLE_CONTROLS_B_ENTRY_SIZE=2804
OMEGA_EXECUTABLE_CONTROLS_B_ENTRY_SHA256=28701862d665951b2e432c58c77b7b1bcbc9d2b8ae53417f95bc76fa19908c6e
OMEGA_EXECUTABLE_CONTROLS_C_ENTRY_SIZE=2834
OMEGA_EXECUTABLE_CONTROLS_C_ENTRY_SHA256=1ec2b16b0d97631bc88f23c0d39c417e6184875c1178913fba4629b815fc3465
OMEGA_EXECUTABLE_CONTROLS_D_ENTRY_SIZE=2852
OMEGA_EXECUTABLE_CONTROLS_D_ENTRY_SHA256=b54adbcd30b9f192217b89b96efd3b548443a6e9426bb7891c98845fa89e8adb
OMEGA_EXECUTABLE_CONTROLS_E_ENTRY_SIZE=2795
OMEGA_EXECUTABLE_CONTROLS_E_ENTRY_SHA256=881feab7746fe17185dfa163629b13c788274891f43c9ab7d72d1f07a3272d72
OMEGA_EXECUTABLE_CONTROLS_F_ENTRY_SIZE=4465
OMEGA_EXECUTABLE_CONTROLS_F_ENTRY_SHA256=17c38612a596bd23ec8bee9f48a43e9bf6591595ba7ec819122acd9692033fc2
OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SIZE=3175
OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SHA256=c3fb524c98a52ad999832af0b7525e75f98900ec69b6f6ac965348a3a20da137
OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SIZE=4385
OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SHA256=87738a1eb03678fb979c48b2770c91a9d03a2f0539c19a49101233b6e786d5c3
OMEGA_EXECUTABLE_CONTROLS_I_ENTRY_SIZE=3754
OMEGA_EXECUTABLE_CONTROLS_I_ENTRY_SHA256=5b1181c8e56caf2e1ff5ca4d1a7f4b61f0f16ba4488a645418985a73b8b9c6f4
OMEGA_EXECUTABLE_CONTROLS_J_ENTRY_SIZE=5585
OMEGA_EXECUTABLE_CONTROLS_J_ENTRY_SHA256=d0fe765d9d0efa6aab8ab8b0444e87571b005b25d657572753b3e06bc915c4af

# D's OCREQ request entry: program.omg, the canonical Omega source the
# request names as D's compilation subject. The executable gate feeds it to
# the bound customer as sealed input after the packed compiler and entry
# bytes; other gates frame their own requests. Like the gate-local entries
# above it is a separate input from the manifested members, bound here so
# the canonical request's subject is an audited identity rather than a
# per-gate framing. The identical record lives in tools/bootstrap/README.md;
# the gate's own README record and run.sh/identity-gate wiring land with the
# omega-executable claim fence.
OMEGA_OCREQ_ENTRY_SIZE=180
OMEGA_OCREQ_ENTRY_SHA256=9cc2d065cf33e3a9de241d061f407665892f7171a431936a0e5ad9e5d3eeea85

# require_omega_compiler_identity : the canonical manifest is the bound file
# and repacking it reproduces exactly the bound D closure. Every
# materialization runs it; tests may call it directly. bootstrap_sha256 and
# require_bound_identity live in alpha/seed_env.sh. Repacking needs the built
# xtask binary; without it the closure identity cannot be established and the
# check refuses rather than skipping.
require_omega_compiler_identity() {
  require_bound_identity "omega_compiler.epsilon.sources" \
    "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" \
    "$OMEGA_COMPILER_MANIFEST_SIZE" "$OMEGA_COMPILER_MANIFEST_SHA256" \
    "bootstrap/omega/compiler/README.md" || return $?
  require_packed_closure_identity "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" compiler.epsilon \
    "$OMEGA_COMPILER_PACKED_SIZE" "$OMEGA_COMPILER_PACKED_SHA256" \
    "bootstrap/omega/compiler/README.md" "canonical Omega D closure" "Omega D source closure"
}

# materialize_omega_compiler DEST : write the canonical packed compiler D
# Epsilon source to DEST after the bound identity check. source-closure
# checks every member row and writes atomically; a changed member refuses
# before DEST is written.
materialize_omega_compiler() {
  OMEGA_COMPILER_DEST=$1
  require_omega_compiler_identity || return $?
  bootstrap_xtask source-closure \
    "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" "$OMEGA_COMPILER_DEST"
}

# require_omega_parser_entry_identity : the parser gate's main.epsilon is
# the bound file. The entry is a separate input from the canonical closure
# and never part of the manifested members, so consumers that pack it run
# this before packing; tests may call it directly. It does not run during
# materialization.
require_omega_parser_entry_identity() {
  require_bound_identity "main.epsilon" \
    "$OMEGA_PATH_OMEGA_PARSER_ENTRY" "$OMEGA_PARSER_ENTRY_SIZE" \
    "$OMEGA_PARSER_ENTRY_SHA256" \
    "_recycling/tests/bootstrap/omega-parser/README.md"
}

# require_omega_outcome_entry_identity : the outcome gate's main.epsilon is
# the bound file. Same contract as require_omega_parser_entry_identity.
require_omega_outcome_entry_identity() {
  require_bound_identity "main.epsilon" \
    "$OMEGA_PATH_OMEGA_OUTCOME_ENTRY" "$OMEGA_OUTCOME_ENTRY_SIZE" \
    "$OMEGA_OUTCOME_ENTRY_SHA256" \
    "_recycling/tests/bootstrap/omega-outcome/README.md"
}

# require_omega_request_entry_identity : the request gate's main.epsilon is
# the bound file — the canonical OCREQ request entry, not an ad-hoc per-gate
# customer. Same contract as require_omega_parser_entry_identity.
require_omega_request_entry_identity() {
  require_bound_identity "main.epsilon" \
    "$OMEGA_PATH_OMEGA_REQUEST_ENTRY" "$OMEGA_REQUEST_ENTRY_SIZE" \
    "$OMEGA_REQUEST_ENTRY_SHA256" \
    "_recycling/tests/bootstrap/omega-request/README.md"
}

# require_omega_request_fixture_identity : the request gate's canonical
# sealed request is the bound byte stream. Same contract.
require_omega_request_fixture_identity() {
  require_bound_identity "request.bin" \
    "$OMEGA_PATH_OMEGA_REQUEST_FIXTURE" "$OMEGA_REQUEST_FIXTURE_SIZE" \
    "$OMEGA_REQUEST_FIXTURE_SHA256" \
    "_recycling/tests/bootstrap/omega-request/README.md"
}

# require_omega_executable_entries_identity : every entry the executable
# gate may select is the bound file. That gate's run.sh runs it before
# packing a customer; tests may call it directly. It is not part of the
# canonical closure and does not run during materialization. The bound set
# is enumerated here, not by scanning the directory, so an added or
# removed entry is a change to this record.
require_omega_executable_entries_identity() {
  for OMEGA_EXECUTABLE_ENTRY in \
    "main.epsilon $OMEGA_EXECUTABLE_MAIN_ENTRY_SIZE $OMEGA_EXECUTABLE_MAIN_ENTRY_SHA256" \
    "main_ocreq.epsilon $OMEGA_EXECUTABLE_OCREQ_ENTRY_SIZE $OMEGA_EXECUTABLE_OCREQ_ENTRY_SHA256" \
    "controls.epsilon $OMEGA_EXECUTABLE_CONTROLS_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_ENTRY_SHA256" \
    "controls_b.epsilon $OMEGA_EXECUTABLE_CONTROLS_B_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_B_ENTRY_SHA256" \
    "controls_c.epsilon $OMEGA_EXECUTABLE_CONTROLS_C_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_C_ENTRY_SHA256" \
    "controls_d.epsilon $OMEGA_EXECUTABLE_CONTROLS_D_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_D_ENTRY_SHA256" \
    "controls_e.epsilon $OMEGA_EXECUTABLE_CONTROLS_E_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_E_ENTRY_SHA256" \
    "controls_f.epsilon $OMEGA_EXECUTABLE_CONTROLS_F_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_F_ENTRY_SHA256" \
    "controls_g.epsilon $OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SHA256" \
    "controls_h.epsilon $OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SHA256" \
    "controls_i.epsilon $OMEGA_EXECUTABLE_CONTROLS_I_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_I_ENTRY_SHA256" \
    "controls_j.epsilon $OMEGA_EXECUTABLE_CONTROLS_J_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_J_ENTRY_SHA256"
  do
    set -- $OMEGA_EXECUTABLE_ENTRY
    require_bound_identity "$1" \
      "$OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/$1" "$2" "$3" \
      "tests/bootstrap/interpreted/omega-executable/README.md" || return $?
  done
}

# require_omega_ocreq_entry_identity : the canonical OCREQ request entry —
# the Omega source D's request asks it to compile — is the bound file. The
# request entry is a separate input from the canonical closure and never
# part of the manifested members, so consumers that frame a request around
# it run this before packing; tests may call it directly. It is not part of
# the canonical closure and does not run during materialization. A digest
# here is an identity check on the request entry source; it is not a proof
# of the request's judgment.
require_omega_ocreq_entry_identity() {
  require_bound_identity "program.omg" \
    "$OMEGA_PATH_OMEGA_OCREQ_ENTRY" "$OMEGA_OCREQ_ENTRY_SIZE" \
    "$OMEGA_OCREQ_ENTRY_SHA256" \
    "tools/bootstrap/README.md"
}
