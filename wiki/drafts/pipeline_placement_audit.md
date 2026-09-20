# Pipeline placement audit

Mechanical sweep at `37d18e2104` of code placement against the
[placement and semantic ownership](../../omega-rust/pipeline.md) table. Scope:
which ownership bucket each retained construct sits in, not whether the
construct itself is correct. A finding means the code lives in the wrong
bucket; it does not mean the code is wrong.

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
