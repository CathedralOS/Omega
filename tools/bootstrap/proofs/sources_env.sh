#!/usr/bin/env sh
# Materialize the canonical Gamma proof-source closures: the derivation
# checker implementation and the Beta encoding theory.
# Source tools/bootstrap/paths.sh first.

[ -n "${OMEGA_PATH_DERIVATION_CHECKER_SOURCES:-}" ] && \
  [ -n "${OMEGA_PATH_BETA_ENCODING_SOURCES:-}" ] && \
  [ -n "${OMEGA_PATH_BETA_ENCODING_PACKAGE:-}" ] && \
  [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "Proof sources: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

# Bound proof-source subjects. These are ordinary Gamma member closures, not
# tapes or entries: each GammaSourceClosureV1 manifest is the ordered source
# authority whose rows bind every member length, digest, and path, and
# repacking it reproduces the packed member bytes pinned in the owning proof
# README. The bound closure carries members only; the derivation gates'
# diagnostic prefix entries packed on top of them bind below. A digest here
# is an identity check that
# the bytes being materialized are the bound ones; it is not a proof of the
# checker, the theory, or any derived certificate. Changing a member or a
# manifest invalidates the dependent evidence and must update every record.
DERIVATION_CHECKER_MANIFEST_SIZE=9046
DERIVATION_CHECKER_MANIFEST_SHA256=21ba1a712dd68e3492abb46d435a906fab8bbc49b67557d13910512e2c975f5b
DERIVATION_CHECKER_PACKED_SIZE=62357
DERIVATION_CHECKER_PACKED_SHA256=382531b910429c7111c2bc9e7dcce7e96394c45fa0a89b624debd061b69422b2
BETA_ENCODING_MANIFEST_SIZE=5536
BETA_ENCODING_MANIFEST_SHA256=f93d98315b2197d81babcc8b3345a1df0e4eb219bf7e5c3288132ec2d3fe2e5d
BETA_ENCODING_PACKED_SIZE=130363
BETA_ENCODING_PACKED_SHA256=632871b5c22c22a0ba397ad4dc6054af797a364f789570e6b25be08840b41ddf
BETA_ENCODING_DEFINITION_PACKAGE_SIZE=116900
BETA_ENCODING_DEFINITION_PACKAGE_SHA256=6bbdd15abac8060a9c5718f58944f758c5c647c1aae827d92f61231b01c3987c

# Bound gate-local prefix entries packed on top of the bound member bytes.
# The beta-encoding theory and certificate-check gates pack the shared
# producer entry on the packed theory closure; each derivation gate packs
# its own diagnostic entry on the packed checker or theory closure — the
# customer bytes are the gate's prefix plus the bound members — so the entries bind here the same way the staged-compiler
# development driver binds on the Delta edge and the omega-* customer
# entries bind on the Omega edge: separate inputs from the canonical
# closures, never part of the manifested members. The identical pins in each
# gate's README, and the packed customer identities recorded there, are
# records of these same subjects, not independent identities. A digest here
# is an identity check on the entry source; it is not a proof of the gate's
# judgment. Changing an entry changes the packed customer and must update
# every record together.
BETA_ENCODING_PRODUCER_ENTRY_SIZE=211
BETA_ENCODING_PRODUCER_ENTRY_SHA256=35577d248b7745f3a6f4d2615e81bb8aae40cb8608d5fbda0be55978b544b373
DERIVATION_ADMISSION_ENTRY_SIZE=1270
DERIVATION_ADMISSION_ENTRY_SHA256=d657d412c92123bdc6dc7c95c38eed50a158c5c986507c5eb3f5ae4420a96e88
DERIVATION_CHECKING_ENTRY_SIZE=1155
DERIVATION_CHECKING_ENTRY_SHA256=8601e23955e3054eba95a2b5e7e2dd2a92d4ae47c8cb9bf49d9ce77c295a16a2
DERIVATION_FORMATION_ENTRY_SIZE=1584
DERIVATION_FORMATION_ENTRY_SHA256=b5b807031d22f118977edbd63f05cdffa4cf5bd1d2d63c3334c8361e69d43cb4
DERIVATION_GROUND_ENTRY_SIZE=1777
DERIVATION_GROUND_ENTRY_SHA256=31c3cfe92700850c39b9bf2caeab75e41027538298612cf8128f40cd4c72da5d
DERIVATION_LAYOUT_ENTRY_SIZE=1198
DERIVATION_LAYOUT_ENTRY_SHA256=49b1d9e459cfcb6b81f84725fc5855f4554174102a3b78269627ae91de0e2985

DERIVATION_SUBSTITUTION_DIAGNOSTIC_SIZE=1638
DERIVATION_SUBSTITUTION_DIAGNOSTIC_SHA256=3b312fed1fc8a87293c04102e55e247c342109594b89483b19a5e717251e2d9d
DERIVATION_SUBSTITUTION_ENTRY_BUDGET_SIZE=872
DERIVATION_SUBSTITUTION_ENTRY_BUDGET_SHA256=29d8df5655066363f790568ea0c2385d4ced2a6f78919de94c7602962bcac371
DERIVATION_SUBSTITUTION_ENTRY_BULK_SIZE=1036
DERIVATION_SUBSTITUTION_ENTRY_BULK_SHA256=a06db5c3cb773b4df95902e9e0d138ae903dee98dffd2373b5f83dfa8f7f25e5
DERIVATION_SUBSTITUTION_ENTRY_CASE_SIZE=213
DERIVATION_SUBSTITUTION_ENTRY_CASE_SHA256=ca69d2d53087b8fd1ddf74ff8cdc7c362cc590a9a3637cad9b018d0d12b9b349
DERIVATION_SUBSTITUTION_ENTRY_CLAUSE_SIZE=333
DERIVATION_SUBSTITUTION_ENTRY_CLAUSE_SHA256=6b2b2e30a2f5c2e473b32c64069ddd8269f1d97bf76b13d8c99417860af3b596
DERIVATION_SUBSTITUTION_ENTRY_INVALID_SIZE=622
DERIVATION_SUBSTITUTION_ENTRY_INVALID_SHA256=b2263856681d7bb35e47d9603cb0302ddd0499ede5a8ab0620501e62ce6fbfbf
DERIVATION_SUBSTITUTION_ENTRY_RETENTION_SIZE=714
DERIVATION_SUBSTITUTION_ENTRY_RETENTION_SHA256=3361c3bec3523f6efe65f7128b512a9275462e225e63f8fc78266b47e35ecf54
DERIVATION_SUBSTITUTION_ENTRY_ROOT_SIZE=213
DERIVATION_SUBSTITUTION_ENTRY_ROOT_SHA256=e45f7f499e2bd67dde939e2a80ad4ca5d75778d94b22c94570f3ed52a1615263
DERIVATION_SUBSTITUTION_ENTRY_SESSION_SIZE=1003
DERIVATION_SUBSTITUTION_ENTRY_SESSION_SHA256=65f9a5a88735fc72aa8baba81500738305ff5e8d1515300bed0cad39eb090201
DERIVATION_SUBSTITUTION_ENTRY_WITNESS_SIZE=306
DERIVATION_SUBSTITUTION_ENTRY_WITNESS_SHA256=5b199af8fd19f3f93c5235182b32917d250ffb978845a18937da92d08f5fd860

DERIVATION_COMPARISON_DIAGNOSTIC_SIZE=2999
DERIVATION_COMPARISON_DIAGNOSTIC_SHA256=84e7a38014645379516949ce58fb9fb6f93ef87abb6b2bd2539b1f5795ccccd3
DERIVATION_COMPARISON_ENTRY_BUDGET_SIZE=905
DERIVATION_COMPARISON_ENTRY_BUDGET_SHA256=52f6ed0b0041a1427c8472269f0af8bcee1dbb5a5c187c21cb1a16cca9ccaf8a
DERIVATION_COMPARISON_ENTRY_INVALID_SIZE=750
DERIVATION_COMPARISON_ENTRY_INVALID_SHA256=9eee13b41741dafed34df7c3eed902c4fa1d26a4dc3cf2fdf6083250249a2a8d
DERIVATION_COMPARISON_ENTRY_PENDING_SIZE=708
DERIVATION_COMPARISON_ENTRY_PENDING_SHA256=df595624382e734d40c29f598bf304d5884d825c34cae0c2a044dc7475337d65
DERIVATION_COMPARISON_ENTRY_RESUME_SIZE=959
DERIVATION_COMPARISON_ENTRY_RESUME_SHA256=c65fd47ecd7a09c23b92793c3a1cc359a292b6fb7fc48f9e007d33cc9c12c676
DERIVATION_COMPARISON_ENTRY_RETENTION_SIZE=1017
DERIVATION_COMPARISON_ENTRY_RETENTION_SHA256=7a84754c984ca008e8408210cbc2be2710bdb459fa851a66a676bbe854f2a94a
DERIVATION_COMPARISON_ENTRY_ROOT_SIZE=390
DERIVATION_COMPARISON_ENTRY_ROOT_SHA256=8f4783644bc40aa45a1f58a1bdc4a11d7a1022e5f6e2d0f61a40fb0dc33ef1ad
DERIVATION_COMPARISON_ENTRY_SESSION_SIZE=368
DERIVATION_COMPARISON_ENTRY_SESSION_SHA256=8f1ea6dd838b0c770cdfc56d745a1bfa7f493fef677118378d376bc103a47a4b
DERIVATION_COMPARISON_ENTRY_WITNESS_SIZE=349
DERIVATION_COMPARISON_ENTRY_WITNESS_SHA256=2ba9d62188b05e802231ab0da5be0add5751e866bb2b4a2342d1a7019ccc0375

# require_bound_manifest_closure LABEL MANIFEST SIZE SHA256 PACKED_SIZE
# PACKED_SHA256 RECORD : shared manifest-then-repack check behind both proof
# subjects. Repacking needs python3; without it the closure identity cannot
# be established and the check refuses rather than skipping.
require_bound_manifest_closure() {
  require_bound_identity "$1" "$2" "$3" "$4" "$7" || return $?
  command -v python3 >/dev/null 2>&1 || {
    echo "bootstrap artifact: no python3 to repack the $1 source closure" >&2
    return 2
  }
  PROOF_IDENTITY_TMP=$(mktemp -d)
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$2" "$PROOF_IDENTITY_TMP/closure.gamma" || {
      PROOF_IDENTITY_RC=$?
      rm -rf -- "$PROOF_IDENTITY_TMP"
      return "$PROOF_IDENTITY_RC"
    }
  require_bound_identity "canonical $1 closure" \
    "$PROOF_IDENTITY_TMP/closure.gamma" "$5" "$6" "$7"
  PROOF_IDENTITY_RC=$?
  rm -rf -- "$PROOF_IDENTITY_TMP"
  return "$PROOF_IDENTITY_RC"
}

# require_derivation_checker_identity / require_beta_encoding_theory_identity /
# require_beta_encoding_definition_package_identity :
# the canonical manifest is the bound file and repacking it reproduces exactly
# the bound member closure. Every materialization runs its check; tests may
# call them directly. bootstrap_sha256 and require_bound_identity live in
# alpha/seed_env.sh.
require_derivation_checker_identity() {
  require_bound_manifest_closure "implementation.gamma.sources" \
    "$OMEGA_PATH_DERIVATION_CHECKER_SOURCES" \
    "$DERIVATION_CHECKER_MANIFEST_SIZE" "$DERIVATION_CHECKER_MANIFEST_SHA256" \
    "$DERIVATION_CHECKER_PACKED_SIZE" "$DERIVATION_CHECKER_PACKED_SHA256" \
    "bootstrap/proofs/checker/README.md"
}

require_beta_encoding_theory_identity() {
  require_bound_manifest_closure "theory.gamma.sources" \
    "$OMEGA_PATH_BETA_ENCODING_SOURCES" \
    "$BETA_ENCODING_MANIFEST_SIZE" "$BETA_ENCODING_MANIFEST_SHA256" \
    "$BETA_ENCODING_PACKED_SIZE" "$BETA_ENCODING_PACKED_SHA256" \
    "bootstrap/proofs/beta_encoding/README.md"
}

# The emitted definition package is the bound file itself, not a manifest:
# the artifact owner fixes these bytes independently of the certificate
# producer (bootstrap/proofs/beta_encoding/ACCEPTANCE.md), and the checker
# request envelope's theory section is exactly this file. theory.gamma emits
# it under the selected evaluator; the host-side mirror reproduces identical
# bytes. Every consumer that trusts the package verifies this identity first.
require_beta_encoding_definition_package_identity() {
  require_bound_identity "definition_package.bin" \
    "$OMEGA_PATH_BETA_ENCODING_PACKAGE" \
    "$BETA_ENCODING_DEFINITION_PACKAGE_SIZE" \
    "$BETA_ENCODING_DEFINITION_PACKAGE_SHA256" \
    "bootstrap/proofs/beta_encoding/README.md"
}

# materialize_derivation_checker DEST / materialize_beta_encoding_theory DEST :
# write the canonical packed member Gamma source to DEST after the bound
# identity check. source_closure.py checks every member row and writes
# atomically; a changed member refuses before DEST is written.
materialize_derivation_checker() {
  DERIVATION_CHECKER_DEST=$1
  require_derivation_checker_identity || return $?
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DERIVATION_CHECKER_SOURCES" "$DERIVATION_CHECKER_DEST"
}

materialize_beta_encoding_theory() {
  BETA_ENCODING_DEST=$1
  require_beta_encoding_theory_identity || return $?
  python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_BETA_ENCODING_SOURCES" "$BETA_ENCODING_DEST"
}

# require_beta_encoding_producer_entry_identity : the shared producer prefix
# entry the beta-encoding theory and certificate-check gates pack on the
# bound theory members is the bound file. The entry is a separate input
# from the canonical closure and never part of the manifested members, so
# consumers that pack it run this before packing; tests may call it
# directly. It does not run during materialization.
require_beta_encoding_producer_entry_identity() {
  require_bound_identity "main.gamma" \
    "$OMEGA_PATH_BETA_ENCODING_PRODUCER_ENTRY" \
    "$BETA_ENCODING_PRODUCER_ENTRY_SIZE" \
    "$BETA_ENCODING_PRODUCER_ENTRY_SHA256" \
    "tests/gamma/beta-encoding-theory/README.md"
}

# require_derivation_*_entry_identity : the gate's diagnostic prefix entry is
# the bound file. The entry is a separate input from the canonical closure
# and never part of the manifested members, so consumers that pack it run
# this before packing; tests may call it directly. It does not run during
# materialization.
require_derivation_admission_entry_identity() {
  require_bound_identity "main.gamma" \
    "$OMEGA_PATH_DERIVATION_ADMISSION_ENTRY" \
    "$DERIVATION_ADMISSION_ENTRY_SIZE" "$DERIVATION_ADMISSION_ENTRY_SHA256" \
    "tests/gamma/derivation-admission/README.md"
}

require_derivation_checking_entry_identity() {
  require_bound_identity "main.gamma" \
    "$OMEGA_PATH_DERIVATION_CHECKING_ENTRY" \
    "$DERIVATION_CHECKING_ENTRY_SIZE" "$DERIVATION_CHECKING_ENTRY_SHA256" \
    "tests/gamma/derivation-checking/README.md"
}

require_derivation_formation_entry_identity() {
  require_bound_identity "main.gamma" \
    "$OMEGA_PATH_DERIVATION_FORMATION_ENTRY" \
    "$DERIVATION_FORMATION_ENTRY_SIZE" "$DERIVATION_FORMATION_ENTRY_SHA256" \
    "tests/gamma/derivation-formation/README.md"
}

require_derivation_ground_entry_identity() {
  require_bound_identity "main.gamma" \
    "$OMEGA_PATH_DERIVATION_GROUND_ENTRY" \
    "$DERIVATION_GROUND_ENTRY_SIZE" "$DERIVATION_GROUND_ENTRY_SHA256" \
    "tests/gamma/derivation-ground/README.md"
}

require_derivation_layout_entry_identity() {
  require_bound_identity "main.gamma" \
    "$OMEGA_PATH_DERIVATION_LAYOUT_ENTRY" \
    "$DERIVATION_LAYOUT_ENTRY_SIZE" "$DERIVATION_LAYOUT_ENTRY_SHA256" \
    "tests/gamma/derivation-layout/README.md"
}

# require_derivation_substitution_prefixes_identity : the shared diagnostic
# prefix and every entry the substitution gate may select are the bound
# files. That gate's run.sh runs it before packing a customer; tests may
# call it directly. The bound set is enumerated here, not by scanning the
# directory, so an added or removed entry is a change to this record.
require_derivation_substitution_prefixes_identity() {
  for DERIVATION_SUBSTITUTION_ENTRY in \
    "diagnostic.gamma $DERIVATION_SUBSTITUTION_DIAGNOSTIC_SIZE $DERIVATION_SUBSTITUTION_DIAGNOSTIC_SHA256 ." \
    "budget.gamma $DERIVATION_SUBSTITUTION_ENTRY_BUDGET_SIZE $DERIVATION_SUBSTITUTION_ENTRY_BUDGET_SHA256 entries" \
    "bulk.gamma $DERIVATION_SUBSTITUTION_ENTRY_BULK_SIZE $DERIVATION_SUBSTITUTION_ENTRY_BULK_SHA256 entries" \
    "case.gamma $DERIVATION_SUBSTITUTION_ENTRY_CASE_SIZE $DERIVATION_SUBSTITUTION_ENTRY_CASE_SHA256 entries" \
    "clause.gamma $DERIVATION_SUBSTITUTION_ENTRY_CLAUSE_SIZE $DERIVATION_SUBSTITUTION_ENTRY_CLAUSE_SHA256 entries" \
    "invalid.gamma $DERIVATION_SUBSTITUTION_ENTRY_INVALID_SIZE $DERIVATION_SUBSTITUTION_ENTRY_INVALID_SHA256 entries" \
    "retention.gamma $DERIVATION_SUBSTITUTION_ENTRY_RETENTION_SIZE $DERIVATION_SUBSTITUTION_ENTRY_RETENTION_SHA256 entries" \
    "root.gamma $DERIVATION_SUBSTITUTION_ENTRY_ROOT_SIZE $DERIVATION_SUBSTITUTION_ENTRY_ROOT_SHA256 entries" \
    "session.gamma $DERIVATION_SUBSTITUTION_ENTRY_SESSION_SIZE $DERIVATION_SUBSTITUTION_ENTRY_SESSION_SHA256 entries" \
    "witness.gamma $DERIVATION_SUBSTITUTION_ENTRY_WITNESS_SIZE $DERIVATION_SUBSTITUTION_ENTRY_WITNESS_SHA256 entries"
  do
    set -- $DERIVATION_SUBSTITUTION_ENTRY
    require_bound_identity "$1" \
      "$OMEGA_PATH_DERIVATION_SUBSTITUTION_ENTRIES/$4/$1" "$2" "$3" \
      "tests/gamma/derivation-substitution/README.md" || return $?
  done
}

# require_derivation_comparison_prefixes_identity : same contract as
# require_derivation_substitution_prefixes_identity for the comparison gate.
require_derivation_comparison_prefixes_identity() {
  for DERIVATION_COMPARISON_ENTRY in \
    "diagnostic.gamma $DERIVATION_COMPARISON_DIAGNOSTIC_SIZE $DERIVATION_COMPARISON_DIAGNOSTIC_SHA256 ." \
    "budget.gamma $DERIVATION_COMPARISON_ENTRY_BUDGET_SIZE $DERIVATION_COMPARISON_ENTRY_BUDGET_SHA256 entries" \
    "invalid.gamma $DERIVATION_COMPARISON_ENTRY_INVALID_SIZE $DERIVATION_COMPARISON_ENTRY_INVALID_SHA256 entries" \
    "pending.gamma $DERIVATION_COMPARISON_ENTRY_PENDING_SIZE $DERIVATION_COMPARISON_ENTRY_PENDING_SHA256 entries" \
    "resume.gamma $DERIVATION_COMPARISON_ENTRY_RESUME_SIZE $DERIVATION_COMPARISON_ENTRY_RESUME_SHA256 entries" \
    "retention.gamma $DERIVATION_COMPARISON_ENTRY_RETENTION_SIZE $DERIVATION_COMPARISON_ENTRY_RETENTION_SHA256 entries" \
    "root.gamma $DERIVATION_COMPARISON_ENTRY_ROOT_SIZE $DERIVATION_COMPARISON_ENTRY_ROOT_SHA256 entries" \
    "session.gamma $DERIVATION_COMPARISON_ENTRY_SESSION_SIZE $DERIVATION_COMPARISON_ENTRY_SESSION_SHA256 entries" \
    "witness.gamma $DERIVATION_COMPARISON_ENTRY_WITNESS_SIZE $DERIVATION_COMPARISON_ENTRY_WITNESS_SHA256 entries"
  do
    set -- $DERIVATION_COMPARISON_ENTRY
    require_bound_identity "$1" \
      "$OMEGA_PATH_DERIVATION_COMPARISON_ENTRIES/$4/$1" "$2" "$3" \
      "tests/gamma/derivation-comparison/README.md" || return $?
  done
}
