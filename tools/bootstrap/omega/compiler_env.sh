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
OMEGA_COMPILER_MANIFEST_SHA256=07e0205f88ae4cb4b9ec799cf8f524eaacb50fe2cae0e97ce5eb31e0d8fb50cb
OMEGA_COMPILER_PACKED_SIZE=558161
OMEGA_COMPILER_PACKED_SHA256=8f0321344c893c3c64bb631bfde10e3ecbde4384e130dca9f7e2c21818a9eca3

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
OMEGA_OUTCOME_ENTRY_SIZE=18230
OMEGA_OUTCOME_ENTRY_SHA256=c92fdbd62f7933859922481c021b951ffb01baec7efee5d4ee8f72c9f3d8ca4d
OMEGA_REQUEST_ENTRY_SIZE=4115
OMEGA_REQUEST_ENTRY_SHA256=0d612813e17cfbe2e755b7398d90bb3572f5ed32da249c8863b37f545d3822c0
OMEGA_REQUEST_FIXTURE_SIZE=132
OMEGA_REQUEST_FIXTURE_SHA256=ab2e980a89d20651b69782446cd8a8333313dce109636fd3e26cc7f52bc98062
OMEGA_EXECUTABLE_MAIN_ENTRY_SIZE=1757
OMEGA_EXECUTABLE_MAIN_ENTRY_SHA256=c0af3126f13c8c511d04f224e630f60f3316c0f9fa6e310e72f66779c7c3ce9e
OMEGA_EXECUTABLE_OCREQ_ENTRY_SIZE=19253
OMEGA_EXECUTABLE_OCREQ_ENTRY_SHA256=9573d73423c2ed3e586b0298ae333733d8828c958a38f1859e1baff3d5ac4a9d
OMEGA_EXECUTABLE_CONTROLS_ENTRY_SIZE=3339
OMEGA_EXECUTABLE_CONTROLS_ENTRY_SHA256=44b8f0d15df414a80728918560ef988341537cfa25c0e21d6240a52c7f72f91b
OMEGA_EXECUTABLE_CONTROLS_B_ENTRY_SIZE=3084
OMEGA_EXECUTABLE_CONTROLS_B_ENTRY_SHA256=261d1529b50ab7b36c9dd228a0df7a4250d46d2913dcd85897ee8b911e98dbc3
OMEGA_EXECUTABLE_CONTROLS_C_ENTRY_SIZE=2824
OMEGA_EXECUTABLE_CONTROLS_C_ENTRY_SHA256=0dbc7da705e7da63a7589b49a25677037dcd43c31c3266f31511986b3eba54ae
OMEGA_EXECUTABLE_CONTROLS_D_ENTRY_SIZE=2850
OMEGA_EXECUTABLE_CONTROLS_D_ENTRY_SHA256=916218b57476fe59f22a2d493b6529502e3ac3a4fda856d4e16b9a156a0f57c9
OMEGA_EXECUTABLE_CONTROLS_E_ENTRY_SIZE=2703
OMEGA_EXECUTABLE_CONTROLS_E_ENTRY_SHA256=42090d41fbfa2068e1063bebfa7caae4373ac7b5825ede94b8edbc8877338248
OMEGA_EXECUTABLE_CONTROLS_F_ENTRY_SIZE=4425
OMEGA_EXECUTABLE_CONTROLS_F_ENTRY_SHA256=fbc7ed2868f9e70833fdfc36c927238c8fd11184e5127e372d732c8ab6ebff0e
OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SIZE=3127
OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SHA256=3c94d2e5430226dbeb20b311d5336f57ab11c8fd49ace137e44785fcbac6ecb9
OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SIZE=3193
OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SHA256=b48c672f09c8263d9d352fdb37af66a82c3083df38dabd533a93e0573e9e5c0e

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
    "controls_h.epsilon $OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SIZE $OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SHA256"
  do
    set -- $OMEGA_EXECUTABLE_ENTRY
    require_bound_identity "$1" \
      "$OMEGA_PATH_OMEGA_EXECUTABLE_ENTRIES/$1" "$2" "$3" \
      "tests/bootstrap/omega-executable/README.md" || return $?
  done
}
