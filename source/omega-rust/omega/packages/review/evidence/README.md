# Omega Package Review Evidence

This crate records checked compiler facts for package review. Its output is
inert evidence: it is not package admission, an accepted lock, or proof that an
audit occurred. Start at [`src/lib.rs`](src/lib.rs), then follow the question
you are asking:

```text
src/
├── lib.rs       public entrance
├── record/      what stable package-review facts exist
├── capture/     how checked compiler state produces those facts
├── encoding/    how facts are canonically encoded and recovered
└── ledger/      how rows and supported open results are reconstructed locally
```

## Record

`record/` is the source-handle-free review vocabulary. Begin with
`record/mod.rs`; its children group package identity, public signatures and
contracts, authority and behavior, representation commitments, complete
package records, and canonical rows. It does not inspect compiler state or
encode persistence bytes.

The representation projection records package-owned opaque data as `Unbound`
and separately records each public producer candidate as exact
opaque/conformance/carrier availability. Availability accepts no consumer
choice and may coexist with `Unbound`; it says only what the producer exposes.
D26 consumer demand is owned by the selecting consumer and exists only for an
actual runtime by-value crossing. It retains the exact boundary requirement
application, complete checked shape graph, every opaque carrier occurrence and
its semantic path, replay-validated physical placement, target and calling
policy, and strong selection and boundary-plan commitments. Checked compilation
records the exact carrier shape root while materializing each opaque; capture
does not infer occurrences from equal layouts or an aggregate digest. Future
`PackageInstance` composition still owns the immutable foreign-source join.

## Capture

`capture/` is the only compiler-facing branch. Begin with `capture/mod.rs`,
then follow `package/` for whole-package assembly, `api/` for public
declarations, `callables/` for machines and realizations, `providers/` for
selection, `behavior/` for operational facts, `contracts/` for checked facts,
`semantics/` for typed identity, or `source/` for authored custody. Capture may
construct records; records never depend back on capture.

Within `contracts/expressions/`, `projection/mod.rs` retains recursive custody
and routes value forms, operator forms, calls, and members into named semantic
leaves. Its siblings own the narrower checked-resolution joins reused by that
descent.

## Encoding and ledger

`encoding/` owns canonical framing and bounded recovery. It consumes only
stable records and does not inspect compiler IR. `ledger/` owns the distinct
local reconstruction question: recovered producer rows remain inert until the
selected local compiler reconstructs the complete row set and requires exact
equality.

The supported result lanes do not pretend to prove a bodyless accepted claim,
grant dangerous authority, validate externally supplied executable code,
exercise or admit a terminal-authority permission, or discharge a compiler-
retained contract-entailment stand-down.
`ledger/results.rs` rejoins each typed compiler fact to its canonical row and
assigns either `OpenRootAdmission` or `OpenLaterDischarge`. The manager may
propagate those open obligations to a consuming root, but root policy cannot
admit a later-discharge obligation. This crate provides no certificate,
persistence codec, accepted lock, or `PackageInstance` route.

The crate root exports `project_checked_package_review` for ordinary checked
review and the separately non-executable
`project_non_executable_quotient_package_review` for the bounded proof-only
total-direct `define` and position-preserving transport-backed `lift`
correspondences. The manager owns comparison and policy; neither entrance
admits a package or executable operation.

The canonical review schema is version 124, row schema version 82, and
canonical-row recovery envelope version 20. Exact vocabulary and revision
notes live in
[`EVIDENCE_SCHEMA.md`](EVIDENCE_SCHEMA.md).
