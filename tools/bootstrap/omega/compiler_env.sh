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
# bootstrap/interpreted/5_omega/README.md. A digest here is an identity check that the
# bytes being materialized are the bound ones; it is not a proof that D
# implements Omega. Changing a member or the manifest invalidates the
# dependent evidence and must update every record.
OMEGA_COMPILER_MANIFEST_SIZE=1339
OMEGA_COMPILER_MANIFEST_SHA256=e51b06c19e546db857805b0aee9bb66d23b3b3efb107926a3825539007951221
OMEGA_COMPILER_PACKED_SIZE=778742
OMEGA_COMPILER_PACKED_SHA256=f250f98315032419393d2a297cf5ff892bcfddb95346cf62f2cc32a2019659c5

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
OMEGA_PARSER_ENTRY_SIZE=58867
OMEGA_PARSER_ENTRY_SHA256=4a8cbbcf6525fa9a719f54769906f508a9c078d73f69adcce5849b574304a772
OMEGA_OUTCOME_ENTRY_SIZE=19632
OMEGA_OUTCOME_ENTRY_SHA256=ce58f84f280c4f7682cb4be3f9db1763a165fb82afffa0cd5da8df413c21fa16
OMEGA_REQUEST_ENTRY_SIZE=4112
OMEGA_REQUEST_ENTRY_SHA256=9368297baef947465d5f1ee11df8f1a0fdf60a01e369836ce0555df02890d9ca
OMEGA_REQUEST_FIXTURE_SIZE=132
OMEGA_REQUEST_FIXTURE_SHA256=ab2e980a89d20651b69782446cd8a8333313dce109636fd3e26cc7f52bc98062
OMEGA_EXECUTABLE_MAIN_ENTRY_SIZE=1757
OMEGA_EXECUTABLE_MAIN_ENTRY_SHA256=c0af3126f13c8c511d04f224e630f60f3316c0f9fa6e310e72f66779c7c3ce9e
OMEGA_EXECUTABLE_OCREQ_ENTRY_SIZE=19249
OMEGA_EXECUTABLE_OCREQ_ENTRY_SHA256=5d5d0b8ed0146b055ffdbb6d680bb902a0e350c80b13577bf148879c6c753943
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
OMEGA_EXECUTABLE_CONTROLS_F_ENTRY_SHA256=a144d2beb326b1b4f6d0251c97089c5b54dae5d3fc737d6b573a7e2ef7888190
OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SIZE=3175
OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SHA256=c3fb524c98a52ad999832af0b7525e75f98900ec69b6f6ac965348a3a20da137
OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SIZE=3249
OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SHA256=30a4006338f1eb08a169fb6b2d844881adb3314b9c39c51fcaea6029838964bc
OMEGA_EXECUTABLE_CONTROLS_I_ENTRY_SIZE=3754
OMEGA_EXECUTABLE_CONTROLS_I_ENTRY_SHA256=23139dabc77402e41850ff841c1961fba6d0c3713cf30392634186d0159c678f
OMEGA_EXECUTABLE_CONTROLS_J_ENTRY_SIZE=4571
OMEGA_EXECUTABLE_CONTROLS_J_ENTRY_SHA256=c0226b7ac5ffab6e91fc8edfcc3f64fa3e4e179b20489b77722e8db6d81e96cd

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
# require_bound_identity live in alpha/seed_env.sh. Repacking needs python3;
# without it the closure identity cannot be established and the check refuses
# rather than skipping.
require_omega_compiler_identity() {
  require_bound_identity "omega_compiler.epsilon.sources" \
    "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" \
    "$OMEGA_COMPILER_MANIFEST_SIZE" "$OMEGA_COMPILER_MANIFEST_SHA256" \
    "bootstrap/interpreted/5_omega/README.md" || return $?
  require_packed_closure_identity "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" compiler.epsilon \
    "$OMEGA_COMPILER_PACKED_SIZE" "$OMEGA_COMPILER_PACKED_SHA256" \
    "bootstrap/interpreted/5_omega/README.md" "canonical Omega D closure" "Omega D source closure"
}

# materialize_omega_compiler DEST : write the canonical packed compiler D
# Epsilon source to DEST after the bound identity check. source_closure.py
# checks every member row and writes atomically; a changed member refuses
# before DEST is written.
materialize_omega_compiler() {
  OMEGA_COMPILER_DEST=$1
  require_omega_compiler_identity || return $?
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
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
    "tests/bootstrap/omega-parser/README.md"
}

# require_omega_outcome_entry_identity : the outcome gate's main.epsilon is
# the bound file. Same contract as require_omega_parser_entry_identity.
require_omega_outcome_entry_identity() {
  require_bound_identity "main.epsilon" \
    "$OMEGA_PATH_OMEGA_OUTCOME_ENTRY" "$OMEGA_OUTCOME_ENTRY_SIZE" \
    "$OMEGA_OUTCOME_ENTRY_SHA256" \
    "tests/bootstrap/omega-outcome/README.md"
}

# require_omega_request_entry_identity : the request gate's main.epsilon is
# the bound file — the canonical OCREQ request entry, not an ad-hoc per-gate
# customer. Same contract as require_omega_parser_entry_identity.
require_omega_request_entry_identity() {
  require_bound_identity "main.epsilon" \
    "$OMEGA_PATH_OMEGA_REQUEST_ENTRY" "$OMEGA_REQUEST_ENTRY_SIZE" \
    "$OMEGA_REQUEST_ENTRY_SHA256" \
    "tests/bootstrap/omega-request/README.md"
}

# require_omega_request_fixture_identity : the request gate's canonical
# sealed request is the bound byte stream. Same contract.
require_omega_request_fixture_identity() {
  require_bound_identity "request.bin" \
    "$OMEGA_PATH_OMEGA_REQUEST_FIXTURE" "$OMEGA_REQUEST_FIXTURE_SIZE" \
    "$OMEGA_REQUEST_FIXTURE_SHA256" \
    "tests/bootstrap/omega-request/README.md"
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
      "tests/bootstrap/omega-executable/README.md" || return $?
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
