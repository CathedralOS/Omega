# Crate-root responsibility audit

Sweep of every workspace crate root (`omega-rust/**/src/{lib.rs,main.rs}`) at
`4dbdaa9bc3` against the responsibility rule — `omega-rust/README.md`: "Public
crate roots map responsibilities; test families and implementation details
belong in named modules, not giant entrypoint files" — and AGENTS.md's
discoverability contract: every crate has an obvious starting point that
explains its responsibility through code.

Counts: 117 crate roots (116 lib.rs + 1 main.rs), all under `omega-rust/`.

## What now holds

- Every crate root opens with a `//!` responsibility doc. `omega/src/main.rs`
  was the only exception; it now carries one. The invariant is pinned by
  `tests/architecture/crate_root_responsibility.rs`:
  `every_crate_root_opens_with_a_responsibility_doc` requires the header before
  the first item, `use`, or `mod` (inner attributes and plain comments may
  precede it).
- The same target's representation inventory (`REPRESENTATION_ROOTS`,
  `LIB_RS_OWNED`, the subordinate-area and `mod`-wiring checks) already pins
  each representation crate's named domain roots. Its task-plans row was
  stale — `composition_model` landed without registration; now registered.

## Findings

- **F1 — doc-less root, fixed.** `omega/src/main.rs` (the CLI binary root)
  opened directly on `mod cli;`. It now documents its dispatch role.

- **F2 — implementation details in a stage root, REPAIRED.**
  `psi/pipeline/typed-trees-to-checked-trees/src/lib.rs` defined 22 free
  helper functions at the crate root (`typed_operator_has_no_authored_selection`,
  `typed_build_provider_selection`, `late_bound_member_declaration_from_exact_owner`,
  `resolve_checked_builtin_float_operator_requirement`, …) beside the stage
  re-exports — the exact "implementation details belong in named modules"
  case. They are all compiler-internal package-review seams, so they now live
  in `src/package_review.rs`, re-exported by name from the root; call sites
  are unchanged.

- **F3 — representation vocabulary lives in `lib.rs`.** Three representation
  roots carry their defining types at the root instead of a named domain file:
  `omega/representations/target` (1013 lines — `Architecture`, `ObjectFormat`,
  `TargetProfile`, `TargetProfileIdentity`, `ProgramEntrySchema`, the UEFI/x86
  re-export surface), `omega/backend/layout` (430 — `TypeLayout`,
  `TypeLayoutDescriptor`, `ENUM_TAG_BYTES`), `omega/backend/runtime/runtime-abi`
  (341 — `RuntimeAbiPlan`, `FatDescriptorAbi`, descriptor layouts). The
  `LIB_RS_OWNED` exemption exists for *small* leaf vocabulary crates
  (function-identity, installation-evidence); these three are the largest
  lib.rs files in the tree, not leaves. Judgment call: the vocabulary IS the
  crate's responsibility, but at this size a named root keeps the root honest.

- **F4 — orchestrator-at-root, documented.** `component-publication` (553 lines)
  keeps `bind_installed_runnable_component` and `InstalledRunnableComponent` in
  `lib.rs` and its doc explicitly starts readers there — deliberate and
  internally consistent, unlike F2/F3 where the root file is heavyweight while
  pointing readers elsewhere.

- **F5 — "Start at" coverage.** 69 of 117 root docs name a starting file
  ("Start at …"); 48 do not. Docs without the pointer still state
  responsibilities; the pointer convention is the repo's dominant form and the
  cheapest discoverability improvement available. Recorded as a recommendation,
  not a ratchet — a hard requirement would churn 48 files for marginal gain.

## Non-findings

- Every pipeline stage crate's root is a thin map (doc + `mod` + `pub use`)
  matching the ownership README's route order.
- `omega/src/main.rs`'s stack-provision comment and worker-spawn dispatch are
  documented startup mechanics, not hidden implementation.

## Residual risk

The mechanical half of this audit is pinned; the qualitative half — whether a
root's doc actually maps the crate's responsibility vs. reciting its name — is
review judgment. The impl-in-root inventory (F2/F3) is measured by item
definitions inside `lib.rs`, which deliberately tolerates small carriers; the
boundary it suggests is a convention, not a checked rule.
