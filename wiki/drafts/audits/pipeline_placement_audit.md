# Pipeline placement audit

Historical record of the cited revisions. Superseded gates, artifact formats
and checker names below are not current requirements or work to resume.
Use [verification placement](../../../omega-rust/pipeline.md#verification-placement)
and the [current task board](../../../TASKS.md) for the maintained model.

Mechanical sweep at `37d18e2104`, re-swept at `be496d9a90`, of code placement
against the [placement and semantic ownership](../../../omega-rust/pipeline.md)
table. Scope: which ownership bucket each retained construct sits in, not
whether the construct itself is correct. A finding means the code lives in the
wrong bucket; it does not mean the code is wrong. Rows for files deleted since
the sweep are removed; the remaining row's path was rechecked at `3b24dc9383`.
Delete this audit once the open row-1 violation it lists moves to
`representations/`.

## Method

- `rg 'public struct \w*Identity'` over every `omega-rust/*/pipeline/` `src/`
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
  `omega-rust/*/representations/` (allocation counters outside
  `tooling/artifacts`).
- `rg 'public struct \w*Arena|struct Interner'` over `omega-rust/omega/` (Omega
  re-implementing Psi foundation primitives — zero hits).
- Workspace member naming: every `omega-rust/*/pipeline/` crate name follows
  `X-to-Y`; no umbrella/helper crate names found.

## Findings

**A durable identity producer still lives inside a transform crate** —
violates row 1 (`representations/` owns durable current program, identities,
raw evidence and codecs).

| Location | Content | Disposition |
| --- | --- | --- |
| `omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/literal_fold_rewrites.rs` (`identity` module) | the `LiteralFoldIdentity` content encoding and producer; the identity type itself is imported from `representations/selected-instructions` | Open: the other two named identities (fixed-view-copy, pressure-rematerialization) moved to `representations/selected-instructions` at `dfb414e84c`; literal fold alone remains |

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

## Residual risk and trigger

Like the lookup-map census, this is a point-in-time picture, not a gate. A new
durable identity or wire codec inside a `pipeline/` or `compiler/` crate
violates row 1 regardless of how temporary it looks; the measured-reason
escape does not apply to placement — the durable form moves to
`representations/` or the PR needs an explicit record of why it cannot.

## Rulings

Future sweeps cite this rather than re-audit.

- **Boundary rule for future findings:** row 1 covers codecs that persist or
  transport a *compiler artifact* (representation round-trip formats).
  Implementations of a *language-level* wire encode/decode call — its
  evaluation in the interpreter and its validation — are semantics-side even
  though they emit bytes, because the emitted format is a user-program
  construct whose canonical primitives already live in representations.
