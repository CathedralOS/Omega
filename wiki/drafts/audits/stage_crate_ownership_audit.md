# Stage-crate ownership audit

Audit of every pipeline stage crate against the ownership rules in
[AGENTS.md](../../../AGENTS.md) — pipeline crates (`X-to-Y` / `X-to-X`) own
transformations and private working state, not public program structs
containing previous stage objects; representations hold durable IR; semantics
crates hold meaning and independent verification; backends own only
unavoidable ISA, ABI, object-format, and relocation detail. Recorded on
2026-09-20 on linux x86-64 at `280c4a83b6` (origin/main). Read-only audit:
no transform source was modified.

This is the stage-crate leg of the ownership series: coordinators and entry
files are covered by
[coordinator_overownership_audit.md](coordinator_overownership_audit.md) and
[coordinator_scope_audit.md](coordinator_scope_audit.md). Rewrite-bearing
module trees were audited clean, and entrance connectivity is now enforced by
`tests/architecture/stage_crate_ownership.rs` rather than catalogued. This leg
classifies the *module inventory* of each stage crate — every top-level `src/`
entry is declared transform, declared private working state, or foreign
(representation data, semantics algorithms, backend detail). Delete this audit
once F1 through F5 are boarded or repaired, or when a later ownership sweep
supersedes it.

## Method

Enumerate `src/` top-level files and module directories for all 21 stage
crates under `omega-rust/{psi,omega}/pipeline/` (the two route tables in
[omega-rust/pipeline.md](../../../omega-rust/pipeline.md)). For each module, read
its entry file and classify the resident code. Foreign-classified modules are
read further to distinguish a real violation from sanctioned ownership. Grep
sweeps cover the discriminators the rule names: ISA/OS tokens (`x86`,
`aarch64`, `MachO`, `COFF`), durable-codec markers (`MAGIC`, `_VERSION`,
`b"OMG`), wholesale representation re-exports (`pub use ::`), and embedded
artifacts (`include_str!`/`include_bytes!`).

## Per-crate verdicts

### Psi half

| Stage crate | Declared transform | Top-level shape | Verdict |
| --- | --- | --- | --- |
| `source-files-to-tokens` | source → tokens | `lexer/` | Owned |
| `tokens-to-syntax-trees` | tokens → syntax trees | grammar areas (`bodies`, `contracts`, `declarations`, `expressions`, `input`, `parameters`, `type_syntax`), `diagnostics`, `parser.rs`, `tests/` | Owned |
| `syntax-trees-to-symbol-resolved-trees` | syntax → symbol-resolved | `constant`, `lowering`, `preparation` (syntax→syntax helpers), `resolution`, `selection`, `symbols` | Owned |
| `symbol-resolved-trees-to-typed-trees` | resolved → typed | `contracts`, `declarations`, `expressions`, `lowerer`, `signatures`, `type_reference` | Owned |
| `typed-trees-to-checked-trees` | typed → checked | `borrow`, `checking`, `checks`, `conformance`, `execution`, `facts`, `flow`, `labels`, `lookup`, `monomorphization`, `operators`, `product_pruning`, `proof`, `semantic`, `values` — all checking internals; `semantic_calls`/`semantic_places`/`authored_selections` custody | Owned — one documented `::validation` re-export seam (**F4**) |
| `checked-trees-to-lowered-psi` | checked → lowered Psi | `emission`, `expression_preparation`, `machine_lowering`, `proofs` (proposition/contract/certificate lowering), `retention`, `returns`, `scalar_graph`, `unit`, `tests/` | Owned |
| `lowered-psi-to-lowered-psi` | Psi optimizer (X-to-X) | seven pass families (`control_flow_cleanup`, `copy_propagation`, `dead_scalar_elimination`, `global_value_numbering`, `proof_check_elision`, `sparse_conditional_constant_propagation`, `state_specialization`), `retained_identities` working state, `psi_optimization` entrance | Owned — dormant codec re-export (**F5**) |
| `lowered-psi-to-terminal-psi` | lowered → Terminal Psi | `boundary_operator_custody` occurrence replay, `publish_artifact` thin forwarder to `terminal_codec` | Owned |

### Omega half

| Stage crate | Declared transform | Top-level shape | Verdict |
| --- | --- | --- | --- |
| `source-files-to-assembled-syntax` | source → assembled syntax | `frontend` (lex/parse drivers), `source` (SourceStorage/ImportQueue private working state), `source_assembly` | Owned — inline domain blocks already recorded (coordinator-scope F1/F2) |
| `assembled-syntax-to-checked-compilation` | assembled → checked compilation | `admission`, `checking` (+ compile-thread spawn helper, see coordinator audit), `optimization` (checked hooks + rollback), `package` | Owned |
| `checked-compilation-to-terminal-artifact` | checked → Terminal artifact | production checks (`application_coverage`, `float_comparisons`, `float_fma`, `integer_comparisons`, `native_proposal`), `terminal_artifact` (behavior_exclusions, composition_modes, verification) | Owned |
| `terminal-psi-to-abstract-operations` | Terminal Psi → abstract ops | `artifact_admission` (canonical admission boundary), `lowering`, `optimization` (unit-seed reconstruction), `provider_installation` (replayed-plan admission) | Owned |
| `abstract-operations-to-abstract-operations` | abstract optimizer (X-to-X) | `analyses`, `pass_manager`, `publication`, `ranked_rewrites`, `representation_specialization`, `rules`, `state_specialization`, `validation` | Owned (rewrite trees clean per pipeline-rewrites audit) |
| `abstract-operations-to-target-operations` | abstract → target ops | `lowering` (+ coordination roster, see coordinator-scope F2), `validation`, `tests/` | Owned |
| `target-operations-to-selected-instructions` | legalization + selection | `legalization`, `optimized`, `selection` (+ `scalar_call_abi` joining `calling_conventions` plans), `structural_inputs`, `tests/` | Owned — ISA specifics arrive via `target`/`register_environment` parameters, not inline detail |
| `selected-instructions-to-selected-instructions` | selected optimizer (X-to-X) | `analyses`, `peepholes`, `rewrites` (93 families), `selected_optimization` | Owned (audited per pipeline-rewrites audit) |
| `selected-instructions-to-register-homes` | selected → register homes | `assignment`, `output`, `preservation`, `register_allocation`, `rewrites`, `unsequenced_spill_stages` | Owned — one durable codec (**F2**) |
| `register-homes-to-post-allocation-machine` | homes → post-allocation machine | `plan` (compute/model/validate), `post_allocation_machine` | Owned — one glob re-export (**F3**) |
| `post-allocation-machine-to-selected-form-encoding` | machine → selected-form encoding | `frame_address`, `row_encoding`, `selected_form_encoding`, `validation` | Owned |
| `selected-form-encoding-to-resolved-layout` | encoding → resolved layout | `resolved_selected_form_layout` | Owned |
| `resolved-layout-to-resolved-layout` | layout optimizer (X-to-X) | `phase` entrance, `x86_branch_relaxation` | Owned transform — ISA detail inside (**F1**) |

## Findings

- **F1 — x86 opcode detail resident in a pipeline transform.**
  `omega-rust/omega/pipeline/12_resolved-layout-to-resolved-layout/src/x86_branch_relaxation/compute/branch_inspection.rs`
  pattern-matches literal x86 encoding bytes (`0x75`, `0x0F 0x85`, `0x72`,
  `0x0F 0x82`, `0x7C`, `0x0F 0x8C`) and hard-codes the rel8 window
  (`-128..=127`). The relaxation transform itself is correctly placed — the
  catalog gates it on `Architecture::X86_64` and it rewrites resolved-layout
  branch *forms*, not emitted bytes. But the predicate→opcode mapping table
  is ISA-encoding knowledge, the kind of detail the rule reserves for
  backends/representations. The `machine_code` representation already owns
  `ResolvedConditionalBranchPredicate` and the encoded row shapes; the
  natural follow-up is a representation-owned `opcode_form()` table the
  inspection code consults, keeping the transform target-parametric. The
  only literal opcode constants found anywhere in `pipeline/` are these.

- **F2 — a durable codec lives in `selected-instructions-to-register-homes`.**
  `assignment/post_allocation_manifest/codec.rs` implements canonical
  manifest persistence — magic bytes `b"OMGPAO\0\0"`, a `VERSION: u32`
  constant, `encode`/`decode` over `PostAllocationOptimizationManifest`
  (whose record type is also defined in the stage crate under `model.rs`).
  The manifest travels downstream (`register-homes-to-post-allocation-machine`
  replay, `machine-emission`). Same shape as coordinator audit F4
  (`optimized_semantic_wrapper` codec): durable, versioned persistence owned
  by a transform crate instead of a representation owner. Binds to the
  existing `DURABLE-CODEC-RELOCATION` item.

- **F3 — a whole representation surface re-exported through a stage module.**
  `omega-rust/omega/pipeline/09_register-homes-to-post-allocation-machine/src/plan/mod.rs:9`
  carries `pub use ::physical_instructions::*` — a glob re-export that
  republishes the entire `physical-instructions` representation surface
  under the stage's `plan` namespace, against the "not public program
  structs" rule's spirit. Today's external consumers import the stage's own
  `StagedOptimizedPostAllocationMachinePlan`; the glob is convenience, not
  load-bearing. Narrow it to the names plan APIs actually expose, or drop it.

- **F4 — documented `::validation` re-export seam in `typed-trees-to-checked-trees`.**
  `lib.rs:327` re-exports `::validation::{AsmAuthorityAdmission,
  data_requires_establishment, validate_asm_discharge}` — semantics-crate
  functions surfaced through the stage root. The lib.rs comment documents
  the reason: the orchestration layer (`assembled-syntax-to-checked-compilation`'s
  settlement transition) owns the `BuildConfig` fact the gate consumes, and
  reaching it through the stage root keeps the seam single. Cataloged, not a
  defect — but it is a cross-crate surface and should not grow.

- **F5 — dormant representation re-export in `lowered-psi-to-lowered-psi`.**
  `lib.rs:30` re-exports `terminal_codec::{PsiOptimizationExecutionIdentity,
  PsiOptimizationExecutionRecord}` — codec-owned evidence record types
  through the optimizer root. No external consumer resolves these names
  through this crate today. Harmless surface, but the re-export implies a
  transform crate distributes codec records; drop it or move the records to
  the codec consumer that needs them.

## Cataloged, not flagged

- `08_selected-instructions-to-register-homes/unsequenced_spill_stages`
  (26.7k lines): honestly labeled "validated but not yet sequenced by
  register allocation" — staged transforms awaiting sequencing under the
  `SPILL-REALIZATION` board item, not misplaced ownership. The replay codecs
  inside it (`logical_spill_operations/codec`, `stack_slot_coloring/codec`)
  are transform-owned replay evidence, distinct from F2's durable manifest.
- `04_abstract-operations-to-abstract-operations/representation_specialization`:
  an X-to-X pass family — correctly placed transform; the
  `REPRESENTATION-SPECIALIZATION` board item owns its remaining gaps.
- `compiler/source-assembly/source_assembly.rs` constructor blocks
  and `build_vocabulary` (~300 lines): coordinator-scope audit F1/F2 already
  recorded them; `source/` and `frontend/` are private working state.
- `01_assembled-syntax-to-checked-compilation/checking/compile_thread.rs`:
  the 256 MiB-stack spawn helper cataloged by the over-ownership audit —
  host infrastructure in a stage crate, relocation optional.
- Replay-evidence codecs (`sis2sis/rewrites/allocation_recovery/fixed_view_copy/codec`,
  `literal_fold` version constants): per-transform canonical evidence
  encodings — the stage's own optimization-history replay, which the
  ownership rules make the stage's responsibility.
- `tests/` directories inside `src/`: the repo's test-source convention
  (the same `is_test_source` split the toolchain metrics use), not foreign
  ownership.
- `selected-instructions-to-selected-instructions` `analyses/` (23k lines)
  and `peepholes/` (12k): X-to-X working state and rewrites — audited clean
  by the pipeline-rewrites audit; `arm_relocation` is control-flow-arm
  relocation, not ARM ISA code. `address_fold` reads an AArch64 addressing
  bound in a comment but applies the shared rule to every target.
- `Architecture`/`NativeTarget`/`ObjectFormat` enum *parameters* throughout
  pipeline crates: target descriptors flowing through stages are inputs,
  not owned ISA detail — the discriminator is literal encoding knowledge,
  which only F1 carries.

## Architecture gates already covering this

`tests/architecture/stage_crate_ownership.rs` pins the X-to-Y/X-to-X crate
inventory, unique transform pairs, non-empty `lib.rs` entrypoints, chain
connectivity (no orphan inputs/outputs), external callers for designed
entrances, and `omega-rust/pipeline.md` documentation links.
`tests/architecture/entrypoint_module_layout.rs` pins the `Optimizer module
role:` header convention. `tests/architecture/layering.rs` pins dependency
direction. This audit adds the module-content classification those gates do
not check — it found no dependency-direction violation, and every flag is
placement debt, not dead code or a wrong-owner entrance.

## Conclusion

Stage-crate ownership is clean at this head: all 21 stage crates own their
named transform plus private working state, representations stay in
representation crates, and semantics decisions stay upstream in Psi. Five
items are worth recording: one ISA-detail island (F1), one durable codec
(F2), two misleading re-export surfaces (F3, F5), and one documented seam
(F4). None blocks the route; F1 and F2 are natural boarding candidates for
`DURABLE-CODEC-RELOCATION` and a new representation-owned opcode-table item.
