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
# bootstrap/5_omega/README.md. A digest here is an identity check that the
# bytes being materialized are the bound ones; it is not a proof that D
# implements Omega. Changing a member or the manifest invalidates the
# dependent evidence and must update every record.
OMEGA_COMPILER_MANIFEST_SIZE=1338
OMEGA_COMPILER_MANIFEST_SHA256=cc0b7e320a26ba4906a1bc177e90cd85c106de8a45e1500179819cc6c60af959
OMEGA_COMPILER_PACKED_SIZE=561794
OMEGA_COMPILER_PACKED_SHA256=60754c730dfb928f9b2b6edbf2904d9a7bb292b0657eb6656a31930c28be05af

# Bound gate-local entries packed on top of the bound member bytes. Each
# omega-* gate appends its own customer entry after the packed D closure —
# the customer bytes are compiler plus entry — so the entries bind here the
# same way the staged-compiler development driver binds on the Delta edge:
# separate inputs from the canonical closure, never part of the manifested
# members. The identical pins in each gate's README and gate.py, and the
# packed customer identities recorded in each gate's README, are records
# of these same subjects, not independent identities. A digest here is an
# identity check on the entry source; it is not a proof of the gate's
# judgment. Changing an entry changes the packed customer and must update
# every record together.
OMEGA_PARSER_ENTRY_SIZE=4583
OMEGA_PARSER_ENTRY_SHA256=61f988109564e8ca58d6590941aa1aba3dfc2f07af101fb082b38ff25623e618
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
OMEGA_EXECUTABLE_CONTROLS_ENTRY_SIZE=3631
OMEGA_EXECUTABLE_CONTROLS_ENTRY_SHA256=78995d1f7975bbd7b8d82230b557f43bb263addb5be3deb2b9bf56cb03efa0a9
OMEGA_EXECUTABLE_CONTROLS_B_ENTRY_SIZE=3084
OMEGA_EXECUTABLE_CONTROLS_B_ENTRY_SHA256=261d1529b50ab7b36c9dd228a0df7a4250d46d2913dcd85897ee8b911e98dbc3
OMEGA_EXECUTABLE_CONTROLS_C_ENTRY_SIZE=2824
OMEGA_EXECUTABLE_CONTROLS_C_ENTRY_SHA256=0dbc7da705e7da63a7589b49a25677037dcd43c31c3266f31511986b3eba54ae
OMEGA_EXECUTABLE_CONTROLS_D_ENTRY_SIZE=2850
OMEGA_EXECUTABLE_CONTROLS_D_ENTRY_SHA256=916218b57476fe59f22a2d493b6529502e3ac3a4fda856d4e16b9a156a0f57c9
OMEGA_EXECUTABLE_CONTROLS_E_ENTRY_SIZE=2797
OMEGA_EXECUTABLE_CONTROLS_E_ENTRY_SHA256=83ce536dacd5efb9238d7f5869ed5d6269c6481a24b4cdd0b7d3985777c150bc
OMEGA_EXECUTABLE_CONTROLS_F_ENTRY_SIZE=4425
OMEGA_EXECUTABLE_CONTROLS_F_ENTRY_SHA256=fbc7ed2868f9e70833fdfc36c927238c8fd11184e5127e372d732c8ab6ebff0e
OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SIZE=3127
OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SHA256=3c94d2e5430226dbeb20b311d5336f57ab11c8fd49ace137e44785fcbac6ecb9
OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SIZE=3193
OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SHA256=b48c672f09c8263d9d352fdb37af66a82c3083df38dabd533a93e0573e9e5c0e
OMEGA_EXECUTABLE_CONTROLS_I_ENTRY_SIZE=3863
OMEGA_EXECUTABLE_CONTROLS_I_ENTRY_SHA256=8f583b6510c940e3ef0cdc0223a1e263da37ea4f6decbea1a3130f4ba33645d0

# D's OCREQ request entry: program.omg, the canonical Omega source the
# request names as D's compilation subject. The executable gate feeds it to
# the bound customer as sealed input after the packed compiler and entry
# bytes; other gates frame their own requests. Like the gate-local entries
# above it is a separate input from the manifested members, bound here so
# the canonical request's subject is an audited identity rather than a
# per-gate framing. The identical record lives in tools/bootstrap/README.md;
# the gate's own README record and run.sh/identity-gate wiring land with the
# omega-executable claim fence.
OMEGA_OCREQ_ENTRY_SIZE=174
OMEGA_OCREQ_ENTRY_SHA256=b880031336a824e41ce6021dda44e1a64aaa9e849a6b25ab658ea6fc612c1b2e

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
    "bootstrap/5_omega/README.md" || return $?
  command -v python3 >/dev/null 2>&1 || {
    echo "bootstrap artifact: no python3 to repack the Omega D source closure" >&2
    return 2
  }
  OMEGA_IDENTITY_TMP=$(mktemp -d)
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" "$OMEGA_IDENTITY_TMP/compiler.epsilon" || {
      OMEGA_IDENTITY_RC=$?
      rm -rf -- "$OMEGA_IDENTITY_TMP"
      return "$OMEGA_IDENTITY_RC"
    }
  require_bound_identity "canonical Omega D closure" \
    "$OMEGA_IDENTITY_TMP/compiler.epsilon" \
    "$OMEGA_COMPILER_PACKED_SIZE" "$OMEGA_COMPILER_PACKED_SHA256" \
    "bootstrap/5_omega/README.md"
  OMEGA_IDENTITY_RC=$?
  rm -rf -- "$OMEGA_IDENTITY_TMP"
  return "$OMEGA_IDENTITY_RC"
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
    "controls_i.epsilon $OMEGA_EXECUTABLE_CONTROLS_I_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_I_ENTRY_SHA256"
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
