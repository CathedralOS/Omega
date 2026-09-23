# Bootstrap tasks

Build the smallest **human-auditable proof chain to self-hosted Omega**.
Runnable compilers and checked edge evidence are both required; neither replaces
the other. Minimize the total human audit burden of semantics, admitted seeds,
implementations, checker rules, certificates, profiles, and permanent tooling.
The governing contracts are [bootstrap minimization](bootstrap/MINIMIZATION.md)
and the [derivation checker](bootstrap/proofs/checker/README.md).

The currently selected construction route is:

```text
audited Alpha VM + admitted Beta compiler tape
  -> Beta-written Gamma evaluator
  -> Gamma-authored Delta compiler
  -> Delta-authored Epsilon evaluator
  -> interpreted Epsilon-authored Omega compiler D
  -> Omega-written product compiler C for alpha_bootstrap
  -> C rebuilds itself
```

The Gamma checker sits beside these edges; it is not another language rung.
Its implementation is a means to explicit, independently rooted proofs, not
a general-purpose proof-system project or an end in itself.

Prioritize the executable path to Omega and reductions in its human audit burden.
Proof-checker experimentation is secondary in scheduling; this does not remove
the required evidence for final chain closure. Retained rung features serve their
actual next compiler/evaluator customer, not general-purpose language completeness
beyond the selected contracts. Language and topology experiments follow
whole-chain minimization without an owner-approval prerequisite. Adoption must
reconcile contracts and evidence; proposed weakening of required assurances
still requires owner escalation. Tag an added item's provenance on its first
line: `(new-scope)` for newly discovered work, `(split-of:<parent-item>)` when
it decomposes an existing item. Items added before 2026-09-14 are untagged.

## Selection and stopping rules

- Apply [scope checkpoints](AGENTS.md#scope-checkpoints) before each milestone.
  Challenge the task's premise against the final audit goal, not just its tests.
  A concrete compiler customer or named proof obligation is necessary but does
  not alone establish that the proposed mechanism is the simplest solution.
- Count progress in execution, human auditability, and measured proof feasibility.
  Helper/test counts, smaller files, and preserving current limits are not goals.
  Compare simpler implementations and coherent private-capacity changes before
  adding workarounds; preserve required semantics and fail-closed behavior.
- No intermediate self-hosting, hypothetical reuse, permanent compatibility
  layers, host semantic stages, or customer-specific acceptance shortcuts.
  Host tools may invoke, stamp, compare, and report, never manufacture authority.
- Keep topology/source ownership green with
  `sh tools/bootstrap/check-chain-hygiene.sh`; this is validation, not an
  evergreen feature task. Remove completed tasks rather than logging milestones.
- Question questionable checker/encoding designs as well as compiler designs.
  Report evidence and alternatives before extending a faulty premise. Changes
  to ratified language/trust contracts follow [owner escalation](AGENTS.md#workflow);
  engineering review does not authorize silently weakening the proof claim.
- Keep language-facility and implementation comparisons on this board under
  [whole-chain minimization](bootstrap/MINIMIZATION.md). Experiments remain
  non-authoritative and do not need an owner ruling. Escalate proposed weakening
  of trust, observation, identity, or fail-closed guarantees before relying on it.

## Dependencies

The `P`-numbered groups below are task-group labels, not owner-question
numbers or a mandatory serial schedule. P1 is the Gamma checker's proof that
the entire selected evaluator Beta source encodes to its exact persisted
Alpha tape; it is not a proof that the evaluator implements Gamma. An empty
`OWNER_QUESTIONS.md` means there is no unanswered owner decision, not that
these implementation tasks pass.
Skip a task-local pause or blocker and continue independent work on this board.
Proof work can advance against existing exact artifacts before Omega is complete.
Runnable lower-rung development can also proceed under disclosed trust assumptions;
it does not close a proof edge. Do not redirect all effort to execution merely because the
checker has a longer acceptance path.

Full self-hosting remains dependent on settled exercised Omega behavior, the
[Rust product completion plan](wiki/drafts/reference/rust_compiler_completion.md),
complete D, and `OMEGA-PRODUCT-COMPILER-SOURCE` in [TASKS.md](TASKS.md).
Rust remains a comparator, not bootstrap authority. Optimization matters where
measured execution or audit feasibility requires it, not as an unbounded
prerequisite to every lower-rung milestone.

## Alpha execution hardening

- **ALPHA-WINDOWS-CONFORMANCE.** Execute `sh tests/bootstrap/alpha-beta-edge.sh`
  and `sh tests/alpha/reference/diamond-py.sh` on Windows x64 using the
  [selected seed](bootstrap/0_alpha/README.md#retention-inventory).
  Owners: `bootstrap/0_alpha/` and `tests/alpha/`. Retain exact bounds/Trap
  observations and register preservation through host I/O: `io-registers.hex`
  must return zero and `ABCDEF` for input `AB`. PE reconstruction, Wine and
  other hosts' runs do not establish Windows execution.
  Preserve [coverage limits](tests/alpha/README.md#bounds-conformance) and
  explicit unsupported-host refusal.

- **ALPHA-SEED-MEMSIZE-NATIVE-VALIDATION.** Re-forge and re-sign the macOS ARM64
  Alpha container from the corrected 128-GiB startup extent, repin its audit
  inventory and `ALPHA_SEED_ARM64_MACOS_*` identities, then run native bounds,
  conformance and source-reconstruction gates. The
  [recorded container divergence](bootstrap/0_alpha/README.md#ratified-bounds-hardening-contract)
  is 32 TiB in the committed container versus 128 GiB in the corrected source
  and selected semantics; a source edit does not repair the shipped seed.
  Preserve exact tape bytes, I/O and stack behavior. Windows extent/runtime
  acceptance stays with ALPHA-WINDOWS-CONFORMANCE.

  The candidate clang rebuild also changes linker encodings, not just the two
  startup immediates: the committed seed was produced by `macho_v5_transform.py`.
  Reproduce with `clang -arch arm64 -Wl,-no_uuid`, Apple clang 17.0.0 and
  CommandLineTools SDK 15.5. Candidate evidence is in `b725fb771e`; it does
  not close `tests/alpha/bounds.py`'s `ret-stack-exact`, `ret-stack-adjacent`
  and `call-at-memory-end` cases, which were killed on a loaded host.
  Establish those results before repinning. Replacing this audited trust root
  requires the owner's explicit approval; source cleanup alone cannot grant it.

## P1 - Gamma checker and first complete encoding proof

- **GAMMA-DERIVATION-CHECKER.** Close the first artifact-specific encoding proof
  with the ordinary-Gamma [checker](bootstrap/proofs/checker/CHECKING.md) and
  [Beta definitions](bootstrap/proofs/beta_encoding/README.md). Follow the
  [full acceptance](bootstrap/proofs/beta_encoding/ACCEPTANCE.md), not a smaller
  equation batch: the subject is the entire selected Gamma evaluator's raw Beta
  source and persisted Alpha tape, with independently reconstructed proposition
  `encode_Beta(S, 0x4000000, 0xfffffc) = Success(T)`.

  Current frontier: the complete untrusted derivation and source-owned theory
  exist, and request admission now allows 136,314,880 bytes. The
  [native check record](tests/gamma/beta-encoding-check/README.md) progressed
  past request admission to comparison-budget exhaustion; the source's
  `implementation/comparison/session.gamma` now uses the selected 2^26 shared
  work bound. Successful full checking and mutation acceptance remain owed.
  Reuse the coupled evaluator/storage provisions in
  [CHECKING.md](bootstrap/proofs/checker/CHECKING.md#complete-generic-execution-provision)
  and [PROFILE.md](bootstrap/proofs/beta_encoding/PROFILE.md); do not rediscover
  the retired small request limit or add a second work allowance.

  Remaining:
  - Produce the complete certificate through the selected evaluator chain from
    the source-owned definitions. `tests/gamma/beta-encoding-theory/run.sh
    --full-subject` reproduces and measures host-stepper output; it is not
    selected-chain production.
  - Run `tests/gamma/beta-encoding-check/run.sh` against the current pinned
    checker on supported native seeds. Require the exact `Checked` observation
    for 3,182,974 rows within the selected provision, record actual time/storage,
    then execute the full-subject mutation controls. A timeout, reference-VM
    result or partial run is not admission.
  - Preserve the retained-role census and justify every remaining checker rule,
    definition and admission against this subject. Remove unjustified helpers;
    totality cases and owner-fixed rules need not occur in this successful
    derivation. Reuse the existing census rather than opening another inventory
    task.

  More native backing, not a new composition rule, is the selected admission
  route. The [encoder candidate continuation condition](bootstrap/proofs/beta_encoding/ENCODER_CANDIDATE.md#continuation-condition)
  still governs that separate experiment. The macOS seed repair is owned by
  ALPHA-SEED-MEMSIZE-NATIVE-VALIDATION; it does not prevent work on another
  supported seed.

  Acceptance: full evidence checks under the exact
  [result/resource profile](bootstrap/proofs/checker/FORMAT.md), with measured
  bytes, storage, depth and time. Malformed, cyclic, missing-premise,
  wrong-subject, wrong-rule and exhausted requests cannot accept. Keep the
  explicit limitation: encoding equality does not prove evaluator semantics.
  No proof search, producer-selected root, trusted assembler primitive or
  general-purpose checker extension.
## P4 - Epsilon to Omega and self-hosting

- **OMEGA-D.** Complete the Epsilon closure selected by
  `bootstrap/5_omega/omega_compiler.epsilon.sources` as the first full Omega
  compiler. Work against settled product semantics and actual C requirements;
  conservative, slow interpreted execution is acceptable when feasible.
  Complete the [standalone request](wiki/spec/build/compiler_request.md) field/tag,
  outcome/phase, and scalar-resource tables with C; exact/adjacent vectors,
  malformed-input rejection, bounded arithmetic, and Complete-only publication
  must agree before either implementation claims the V1 boundary.
  Continue from the [source-to-executable gate](tests/bootstrap/omega-executable/README.md)
  and `bootstrap/5_omega/scalar_compilation.epsilon`: extend parsed scalar
  operations and checked call/state sequencing through the existing Alpha emitter.
  Retain changed-source, entry-selection, invalid-body and no-partial-output
  controls as the executable path grows; do not replace the gate with hand-built
  tapes or shape-specific source recognizers. Its diagnostic scalar entry adapter
  is not package/Build admission or the final ProgramEntry contract. Replace that
  adapter through the real request and target route, preserving actual emitted-byte
  execution as the outer acceptance check.
  Acceptance: interpreted D compiles the exact Omega C closure for its ordinary
  `alpha_bootstrap` target and produces `omega0_compiler_bytecode.tape`.
  Depends on the product-source work in `TASKS.md`.
  Rust Alpha emission is not a dependency: the reference compiler may report
  that selected operation as not implemented, per the
  [bootstrap contract](bootstrap/CONTRACT.md#selected-execution-chain).

- **OMEGA-C.** Once the product source and D are ready, compile the exact
  Omega-written closure rooted at `source/omega/{build.omg,main.omg}` with D,
  then with `omega0`. This is the sole self-host edge.
  Acceptance: `D -> C/omega0 -> C/omega` is deterministic, `omega` recompiles
  C under the same source/target profile, and shared product suites pass.

## P5 - Audited chain closure

- **CHAIN-MANIFEST.** Bind the remaining self-host compiler tapes, certificates
  and disclosed admissions in edge-owned records and shared
  `tools/bootstrap/` orchestration. Source closures, seed/evaluator artifacts,
  request entries and gate-local drivers already use exact bindings; preserve
  their `*_env.sh` materializers and `tests/bootstrap/*-identity.sh` refusal
  controls rather than creating another binding scheme.

  Missing subjects: the `omega0` and `omega` tapes produced by OMEGA-C, and
  proof/admission records as their edges close. Each binding must join the exact
  source closure, artifact, semantics and resource/observation profile,
  reconstructed obligation and evidence before the consuming gate executes.
  Identity-only tests are not execution or proof evidence.

  Acceptance: a reviewer can follow each dependency to the audited root without
  treating hashes, execution success or producer assertions as proofs. Retain
  only plumbing with an exact customer under
  [whole-chain minimization](bootstrap/MINIMIZATION.md#retention-test).
  OFFLINE-REBUILD owns complete blank-host reconstruction.

- **OFFLINE-REBUILD.** Close `tests/bootstrap/` reconstruction from the audited
  Alpha seed and repository-owned bytes on supported hosts. Rust, Python,
  networking and package managers are never semantic stages; the manifest must
  expose all admissions and contain no retired rung.

  Reuse the existing identity/source-closure/hygiene gates and seed execution
  routes. Linux x86-64 has a selected native seed and executable lower-rung
  coverage. Windows PE runs
  under Wine do not validate a Windows host.

  Remaining:
  - Execute Gamma, Delta, Epsilon and `omega-*` customer gates on Windows x64
    through the documented Git Bash route, recording exact observations and
    resource prerequisites. ALPHA-WINDOWS-CONFORMANCE owns the Alpha/Beta edge,
    not this whole roster. Byte-only identity checks do not replace execution.
  - Complete the chain beyond interpreted D once OMEGA-D, OMEGA-C and their
    required proof edges close, then reconstruct/check it on a blank supported
    host without unavailable-host success or an undisclosed authority substitute.
    Apply ALPHA-SEED-MEMSIZE-NATIVE-VALIDATION before claiming the corrected
    macOS extent.

  Acceptance is the complete offline self-host chain, not another passing
  subset or an aggregate of incompatible source/profile pins.
