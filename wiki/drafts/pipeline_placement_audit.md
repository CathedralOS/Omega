# Pipeline placement audit

Mechanical sweep at `37d18e2104` of code placement against the
[placement and semantic ownership](../../omega-rust/pipeline.md) table. Scope:
which ownership bucket each retained construct sits in, not whether the
construct itself is correct. A finding means the code lives in the wrong
bucket; it does not mean the code is wrong.

Re-swept at `be496d9a90` — see [Re-sweep](#re-sweep-at-be496d9a90) and
[Rulings](#rulings) below.

## Method

- `rg 'pub struct \w*Identity'` over every `omega-rust/*/pipeline/` `src/`
  (durable identity newtypes outside `representations/`).
- `rg 'Sha256::digest|sha2::'` over the same set (identity producers inside
  transforms).
- `find omega-rust/*/pipeline -name "*.rs" -path "*codec*"` (durable codecs
  outside `representations/`).
- `rg 'fn rewrite_|fn optimize_|fn lower_'` over `omega-rust/*/representations/`
  (transformation code inside representation crates — zero hits).
- `rg 'omega-'` over every `psi/` `Cargo.toml` (Psi→Omega dependency — zero
  hits).
- `rg 'static \w*(COUNTER|COUNT)|AtomicUsize|AtomicU64'` over
  `omega-rust/*/representations/` and `omega-rust/*/semantics/` (allocation
  counters outside `tooling/artifacts`).
- `rg 'pub struct \w*Arena|struct Interner'` over `omega-rust/omega/` (Omega
  re-implementing Psi foundation primitives — zero hits).
- Workspace member naming: every `omega-rust/*/pipeline/` crate name follows
  `X-to-Y`; no umbrella/helper crate names found.

## Findings

**Durable identities and codecs still live inside transform/coordinator
crates** — violates row 1 (`representations/` owns durable current program,
identities, raw evidence and codecs). All confirmed against `pipeline.md`.

| Location | Content | Disposition |
| --- | --- | --- |
| `omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/literal_fold/` | `LiteralFoldIdentity` newtype + `identity.rs` Sha256 producer | Boarded: DURABLE-CODEC-RELOCATION leg (1) remainder — the other two named identities (fixed-view-copy, pressure-rematerialization) already moved to `representations/selected-instructions` at `dfb414e84c`; literal_fold alone remains, fenced at audit time by COMPOSABLE-PAIR-DESCRIPTORS |
| `omega/pipeline/selected-instructions-to-register-homes/src/unsequenced_spill_stages/` (18 families) | every family's `model.rs` defines its own `*Identity` newtype; every `identity.rs` is a Sha256 producer; `logical_spill_operations/codec/` and `stack_slot_coloring/codec/` are full codec subtrees | Boarded: DURABLE-CODEC-RELOCATION (identities/codecs) + UNSEQUENCED-SPILL-STAGES-DISPOSITION family (sequence-or-delete decides whether the durable forms stay at all) |
| `omega/pipeline/selected-instructions-to-register-homes/src/assignment/post_allocation_manifest/` | `model.rs` + `codec.rs` — a durable manifest wire format inside the transform | Boarded: DURABLE-CODEC-RELOCATION leg (2) — move to `representations/register-homes` |
| `omega/pipeline/selected-instructions-to-register-homes/src/preservation/identity.rs` | Sha256 identity producer for preservation receipts | Same family as above; fold into the relocation leg or justify as stage-private receipt |
| `omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/codec/` | ~50-file durable codec subtree (envelope/evidence/selected/structural) inside the rewrite | Boarded: DURABLE-CODEC-RELOCATION leg (3) — move to `representation-selections` or a named area |
| `omega/compiler/native-realization/src/optimized_semantic_wrapper_object/` | durable object model + `codec.rs` + manifest inside a compiler coordinator | Boarded: PIPELINE-WRAPPER-OBJECT-ORPHAN; codec leg deferred to PIPELINE-OWNER-CONSOLIDATION per the verified relocation plan |
| `psi/semantics/terminal-codec/` | the canonical binary codec and `TerminalPsiIdentity` for the whole Terminal Psi representation; the durable types live in `psi/representations/terminal-psi`, the codec+identity live in `semantics/` — the only codec crate in a directory of verifiers/interpreters/validators | **New finding.** Relocate to `psi/representations/` (e.g. beside `terminal-psi`). Note the bound surface: `terminal-codec/build.rs` folds `src/` into `source-closure`, so a move is a source-closure rebind ceremony (see PCC-CANONICAL-SEMANTIC-LEDGER's closure discipline), not a plain `git mv` |
| `psi/semantics/checked-interpreter/src/interpreter/evaluator/wire_codec.rs` | compact_binary v0 encoder inside the evaluator | Borderline: the evaluator *models* the wire operation's semantics (era discriminator, field order, bounded writes) — a semantics-side behavior, arguably justified. Record for review; not a clear violation |
| `psi/semantics/validation/src/machine_calls/calls/write_frames/wire_codecs/` | wire-codec tables inside call validation | Borderline: semantic-side checking of wire framing. Same review flag |

## Rows verified clean

- **Psi→Omega dependency direction**: no `psi/` `Cargo.toml` names an `omega-*`
  dependency anywhere in the workspace.
- **Transformation inside representations**: zero `rewrite_/optimize_/lower_`
  definitions under either `representations/` tree.
- **Omega re-implementing foundation primitives**: zero `Arena`/`Interner`
  types under `omega-rust/omega/` — arenas/handles consistently come from Psi
  `foundation/`.
- **Crate naming/umbrella hygiene**: every `omega-rust/*/pipeline/` member is
  an `X-to-Y` or `X-to-X` transform; no `helper`/`common`/`util`-style crates.
- **Allocation counters**: no `static`/atomic counters in representation
  crates (the `AtomicU64` hits in `typed-trees` are type-name matching, not
  counters); phase-report machinery already sits in `tooling/artifacts`
  (`compile_timings`) per the table.
- **Semantics doing transformation**: the only `rewrite`/`lower` hits under
  `psi/semantics/` are proof-contract entailment helpers and build-time layout
  evaluation — semantic judgment, not IR rewriting.

## Residual risk and trigger

Like the lookup-map census, this is a point-in-time picture, not a gate. A new
durable identity or wire codec inside a `pipeline/`, `compiler/`, or
`semantics/` crate violates row 1 regardless of how temporary it looks; the
measured-reason escape does not apply to placement — the durable form moves to
`representations/` or the PR needs an explicit record of why it cannot. The
borderline semantics-side wire-codec files (`checked-interpreter`,
`validation`) should get a written ruling on whether "evaluator models the
wire call's semantics" satisfies the table, so the exception is recorded once
rather than re-audited each sweep.

## Re-sweep at `be496d9a90`

Same method, same commands. Deltas against the `37d18e2104` picture:

**Resolved since the sweep**

- `allocation_recovery/fixed_view_copy/codec/` — the ~50-file durable codec
  subtree moved out of the rewrite to
  `representations/register-homes/src/register_homes/recovery/fixed_view_copy/codec/`.
  Row-1 satisfied; drop it from DURABLE-CODEC-RELOCATION's leg (3).

**Still open (unchanged substance)**

- `selected_lowering/literal_fold/identity.rs` — still the lone
  `LiteralFoldIdentity` Sha256 producer inside the transform.
- `unsequenced_spill_stages/` — 17 families (was 18); each retained family's
  `model.rs` still defines a `*Identity` newtype with a Sha256 `identity.rs`.
- `logical_spill_operations/codec/` — moved within the same crate from
  `unsequenced_spill_stages/` to `assignment/`; still a durable codec inside
  the pipeline crate, path updated.
- `assignment/post_allocation_manifest/{model,codec}.rs`,
  `preservation/identity.rs`, `native-realization/optimized_semantic_wrapper_object/`,
  `psi/semantics/terminal-codec/` — all still where the sweep found them.

**New finding**

- `psi/pipeline/typed-trees-to-checked-trees/src/product_pruning/mod.rs` —
  `CheckedTreeProductSelectionIdentity([u8; 32])` newtype plus a `Sha256`
  producer inside the transform. Row-1 violation; add to the
  DURABLE-CODEC-RELOCATION leg list (move to `representations/typed-trees` or
  the checked-trees representation crate).

**Reviewed, not row-1 hits** (name-shape matches, different semantics)

- `abstract-operations-to-abstract-operations/.../global_value_numbering/identities/*`
  and `proof_check_elision/scalar_identities.rs` — `*IdentityRule` structs are
  algebraic rule names, not durable content identities; no Sha256 producer.
- `checked-trees-to-lowered-psi/src/proofs/content_conservation.rs` —
  `LoweredContentIdentityReshuffles` is transform working state (private
  bookkeeping a pipeline crate may own), not a durable identity newtype.
- `checked-interpreter/src/filesystem_sponsor.rs` `NEXT_ACCOUNT_ID:
  AtomicU64` — synthetic account-ID mint inside the interpreter's filesystem
  simulation, i.e. modeled program state, not a compile-phase allocation
  counter; `tooling/artifacts` owns telemetry counters, this is not one.

All previously clean rows re-verified clean: zero `psi/`→`omega-*` manifest
deps, zero `rewrite_/optimize_/lower_` under either `representations/`, zero
`Arena`/`Interner` under `omega-rust/omega/`, every `pipeline/` member still
`X-to-Y`/`X-to-X`.

## Rulings

Recorded once per the residual-risk note; future sweeps cite these rather
than re-audit.

- **`checked-interpreter/src/interpreter/evaluator/wire_codec.rs` —
  satisfies the table.** The evaluator arm *is* the semantic definition of
  the `Schema::encode` machine call (era discriminator varint, field-number
  order, bounded write). `semantics/` owns language meaning; this is meaning.
  The durable wire framing primitives it sequences (`WireFieldEncoding`,
  `wire_varint_bytes`) already live in `typed_trees::wire`, so row 1's codec
  ownership is intact — the interpreter contributes the call's evaluation,
  not a second wire format.
- **`validation/src/machine_calls/calls/write_frames/wire_codecs.rs` —
  satisfies the table.** It derives write frames for the synthesized
  `Schema::encode`/`decode` calls — custody validation of arguments, squarely
  the `semantics/` bucket. Same distinction as above: the durable schema and
  framing vocabulary live in `typed_trees`; this leaf judges the call.
- **Boundary rule for future findings:** row 1 covers codecs that persist or
  transport a *compiler artifact* (representation round-trip formats).
  Implementations of a *language-level* wire encode/decode call — its
  evaluation in the interpreter and its validation — are semantics-side even
  though they emit bytes, because the emitted format is a user-program
  construct whose canonical primitives already live in representations.
