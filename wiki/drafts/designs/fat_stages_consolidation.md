# Fat stages — maximal-fold consolidation design

Decision (Jarod): the pipeline keeps stages 0–9 — no new numbered stages —
and every non-pipeline dir dies except universal substrate and the binary.
A stage owns its produced IR, its machinery (ISA tables, encoders, emitters,
evaluators), its checker module, and its drive of the next stage. Substeps are
modules inside a stage crate, never crates.

## The rule this replaces

`omega-rust/pipeline.md` blesses `backend/`, `build/`, `packages/`,
`tooling/`, `compiler/`, `representations/`, `semantics/` as peer dirs. That
produced the parking lot: code doing stage work parked beside the chain.
New test for surviving outside `pipeline/`:

- **Universal substrate** (used by most stages; no producing owner):
  `arena` (31 consumers), `symbols` (35), `diagnostics` (35),
  `semantic-vocabulary` (65), `language-core`, `language-semantics`,
  `numerics`, `source`.
- **The binary** (`omega` crate): product surface — CLI entry, build/packages/
  tooling modules, artifact verification, run-path interpreters.

Independent verification survives as *independent passes*, not folders:
a stage's checker module still walks that stage's output separately; artifact
verification moves to the consumer side (binary/codec/verifier crates used by
whoever receives the artifact).

## End state — 3 things

- `pipeline/` — 18 fat stage crates, ~1.5M LOC (~75%). Each stage crate
  contains: its produced IR, machinery, checker module, next-stage drive.
- `foundation/` — ~8 crates, ~35k: true universal substrate only.
- `omega` binary — ~350k: entry, build/packages/tooling internals,
  artifact verification, interpreters.

`terminal-psi` (8k) stays the one standalone representation crate —
decided: it is the published portable boundary consumed by codec,
verifier, installer, tests, and Omega stage 00, not internal plumbing.
Consumers should not have to import stage 07's machinery to read the
contract.

Dies entirely: `representations/` (IRs move into producing stages),
`semantics/` (judges into stage checker modules; verifier/codec/interpreters
into the binary), `backend/` (ISA→02–05, emission tail→08–09),
`compiler/`, `psi/compiler/` (→07), `build/`, `packages/`, `tooling/`
(→binary modules).

## Migration order — leaf-up, workspace green at every step

1. Foundation strays to consumers: `layout-plans`, `extents`,
   `access-plans`, `mutation-matrix`.
2. ISA encoders (`isa-x86_64`, `isa-aarch64`, `register-environment`,
   `x86-encoding`) → stages 02–05 internals.
3. `terminal-production` → stage 07.
4. Emission tail = **reduce + move**, not a lift (217k on disk):
   ~60k deleted when the legacy lane dies, ~127k emission into stage 09
   modules, ~90k custody machinery into the binary (step 9). Confirmed
   linear: layout → object → image → install. Each surviving substep
   enters as a module with an honest `in → out` signature
   (e.g. `fn(...) -> ObjectArtifact`) — promotion to a numbered stage is
   a cheap private operation only if these boundaries stay typed, never
   shared mutable state. Fold first because demoting a numbered crate is
   public unwiring; promoting a module is nearly free. Numbers get spent
   only when a seam proves it needs a gate (the OMGTRO optimizer lane is
   the candidate).
   Two findings from the lib.rs headers that shape this step:
   - Two parallel emission lanes exist: legacy
     (`ObjectPlan`/`RelocationPlan` OMGOBJ → `image/` FinalImage →
     pe/elf/macho writers) and the clean lane
     (`image-emission`'s `ObjectArtifact` → `image_output` →
     `installation_record`, which refuses the legacy carrier). Fold first
     lets the lanes unify privately inside stage 09; the surviving shape
     is what could be promoted later.
   - ~90k of the tail is not emission: `executable-installation` is the
     authority ladder (admit→freeze→validate→install→seal→retire),
     `external-roots` (48.6k) is the outside-entry ledger (interrupts,
     UEFI bootstrap — Cathedral surface), `component-*` is publication
     machinery. Trust/custody product code — binary/runtime layer
     (step 9), not a stage. The legacy `ObjectPlan`/`FinalImage`/format
     writer lane (~60k) is retired as the fold lands rather than ported —
     the target absorbs less than the source held.
5. Semantics engines (build-time-evaluation 50k, checked-interpreter 34k,
   terminal-fixed-fuel) → stages 04–05 / binary run paths.
6. `omega/compiler` dissolves — upstream items ONE-DRIVER-PER-STAGE,
   SOURCE-SET-UNION, BUILD-EVALUATES-ONCE are mid-flight on this lane;
   sequence against them, not ahead.
7. IRs move into producing stages (typed-trees→03, …). `terminal-psi`
   does not move — it stays the lone standalone boundary crate.
8. Validators → checker modules inside their stages;
   terminal-verifier/terminal-codec/interpreters → binary.
9. `build/`, `packages/`, `tooling/` → binary modules (biggest blast radius,
   keep last).
10. Rewrite the placement rule in `omega-rust/pipeline.md` — this design is
    the spec.

Measured against `main` @ 354622565c via `cargo metadata` production deps;
numbers in `build/crates.json`; visual map beside this doc in
`fat_stages_map.html`.
