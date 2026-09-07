# Bootstrap tasks

This queue implements the selected trust-minimizing chain. Git history retains
the retired Alpha/Beta/Gamma/Delta/Epsilon baseline and completed experiments;
they are not tasks and have no compatibility requirement.

```text
audited Alpha VM + admitted Beta compiler tape
  -> Beta-written Gamma evaluator
  -> Gamma-authored Delta compiler
  -> Delta-authored Epsilon evaluator
  -> interpreted Epsilon-authored Omega compiler D
  -> Omega-written product compiler C for alpha_bootstrap
```

Alpha's opcode semantics are unchanged. Beta is the trusted imperative
tape-assembly language.
Gamma is the small typed scalar/effect functional language evaluated directly
by Beta. Delta is the richer typed functional language needed to write the
Epsilon evaluator.

## Rules

- A language exists only to deliver the next rung and named small checkers.
- Host scripts may invoke, stamp, compare, and report. They do not parse,
  lower, manufacture semantic evidence, or decide trust.
- Missing artifacts stay missing; no retired compiler or native route stands
  in for an open edge.
- Intermediate self-hosting, general-purpose completeness, compatibility, and
  hypothetical reuse are not acceptance conditions.
- Every retained feature must cite a current evaluator, compiler, checker, or
  edge-verification customer.
- Apply the [scope checkpoints](AGENTS.md#scope-checkpoints) and
  [whole-chain retention test](wiki/design_briefs/bootstrap_minimization.md)
  before expanding a rung. Helper completion alone is not chain progress.

## Scheduling and current dependencies

The sections below group obligations; they are not a serial instruction to
finish every checker helper before advancing evaluator execution. P1 is proof
closure, not a prerequisite for developing the selected runnable Delta/Epsilon
route under its existing disclosed trust status. No such development counts as
an admitted edge or substitutes for the required certificate.

End-to-end self-host acceptance depends on settled exercised Omega behavior,
the [Rust product completion contract](wiki/releases/rust_compiler_completion_contract.md),
the `OMEGA-PRODUCT-COMPILER-SOURCE` work in [TASKS.md](TASKS.md), and complete D.
Rust is a development comparator, not a canonical bootstrap stage. Lower-rung
work can proceed for existing customer slices without pretending C is ready.
Optimization quality is separate from semantic completion; require a particular
optimization only when measured customer feasibility depends on it.

## Next checkpoint - justify the remaining machinery

- **BOOTSTRAP-COMPLEXITY-REVIEW.** Before another P1/P2/P3 infrastructure
  milestone, apply the retention test to the current customer path. Owners:
  `bootstrap/delta/compiler/implementation/normalization/` and
  `checking/names/`, `bootstrap/gamma/{evaluator,derivation_checker,beta_encoding}/`,
  and the retired concatenative inventories in the Gamma/Delta READMEs.
  Compare retaining the 255-depth workaround with a coherently enlarged Gamma
  profile using the actual Epsilon lowering demand; distinguish required
  frontend semantics from storage optimizations and file fragmentation.
  For P1, identify the remaining full-source encoder, owner-root reconstruction,
  and source-owned certificate producer, with a bounded cost/feasibility probe
  before extending another arithmetic/helper family. A partial proof is only
  a measurement, never certificate acceptance. Inspect legacy consumers before
  proposing deletion. Acceptance: present measured keep/simplify/defer/remove
  recommendations, exact dependencies and validation costs, and one bounded
  next customer milestone for user direction. Do not build an alternative
  language, remove required validation, or change the trusted boundary as part
  of this review; actual contract changes follow `OWNER_QUESTIONS.md`.

## P1 - Gamma checker

- **GAMMA-DERIVATION-CHECKER.** Implement the smallest proof checker required
  by concrete compiler-edge certificates as an ordinary Gamma program run by
  the Beta evaluator. It validates an explicit derivation for an independently
  reconstructed proposition and performs no proof search, artifact discovery,
  deployment policy, or source-to-obligation inference. Follow the
  [ground equality implementation design](wiki/architecture/bootstrap_chain/derivation_calculus.md):
  extend the [source-owned Beta theory](bootstrap/gamma/beta_encoding/README.md)
  into the complete Beta definition package and owner-fixed encoding proposition
  for the [generic checker](bootstrap/gamma/derivation_checker/CHECKING.md), then
  produce the untrusted certificate through the selected source-owned chain.
  Retain the exact checker entry and artifact-specific result/resource profile
  under the [inner encoding](bootstrap/gamma/derivation_checker/FORMAT.md).
  The first certificate must cover the
  entire selected Gamma evaluator's Beta source and persisted Alpha tape;
  rule-unit tests or assembler agreement cannot replace it. Acceptance: that
  full certificate passes with measured storage, size, depth, and time; malformed,
  cyclic, missing-premise, wrong-subject, wrong-rule, and resource-exhausted
  certificates cannot accept within the published Gamma bounds.

## P2 - Gamma to Delta

- **DELTA-COMPILER.** Complete
  the Gamma source closure entered at
  `bootstrap/delta/compiler/delta_compiler.gamma` against the full Delta contract,
  including nominal types, exhaustiveness, checked arithmetic, proper-tail
  lowering, sealed profiles, deterministic failure selection, and canonical
  Gamma emission. DCREQ framing and `ConformanceBytesV1` are executable;
  finish the remaining canonical DCOUT resource and internal failures
  without Delta-specific Gamma primitives. Direct Epsilon-to-Alpha profile ID 2
  is retired.
  The current Epsilon evaluator source plus a diagnostic entry
  compiles through the selected lower route;
  additional transformation optimization needs measured pressure from that
  customer or a required conformance boundary. Compare coherent private-profile
  growth before adding another optimization or limit workaround; retain general
  source semantics and never add a customer-specific shortcut. Separate remaining
  DCOUT conformance from execution progress through the Epsilon evaluator.
  Acceptance: conformance and malformed-source suites pass, the complete
  Epsilon evaluator compiles, exact receipts execute D, and no host or retired
  compiler participates. The final execution clause depends on P3 and complete
  D in P4; missing downstream implementation is not a reason to keep adding
  Delta mechanisms after its own applicable conformance and customer checks pass.

## P3 - Delta to Epsilon

- **EPSILON-EVALUATOR.** Complete the Delta source closure selected by
  `bootstrap/epsilon/compiler/epsilon_compiler.delta.sources` against
  `bootstrap/epsilon/LANGUAGE.md`, deleting inherited structures with no current
  customer. Finish checking, fixed-storage realization, deterministic
  diagnostics, execution, the evaluator entry, and exact composition with D.
  Acceptance: exact Epsilon-written Omega D executes under the selected lower
  chain and its behavior refines `RunEpsilon` without an Epsilon-owned Alpha
  backend.

## P4 - Epsilon to Omega

- **OMEGA-D.** Complete the Epsilon source closure selected by
  `bootstrap/omega/omega_compiler.epsilon.sources` as the first full Omega
  compiler. Conservative and slow interpreted execution is acceptable;
  Epsilon features are justified only by this source. Acceptance: interpreted D
  compiles the exact Omega C closure for its ordinary `alpha_bootstrap` target
  and produces `omega0_compiler_bytecode.tape`.

- **OMEGA-C.** Compile the exact Omega-written product closure rooted at
  `source/omega/{build.omg,main.omg}` with interpreted D for
  `alpha_bootstrap`, then with `omega0`. This is the only meaningful self-host
  edge. Acceptance: `D -> C/omega0 -> C/omega` is deterministic, `omega`
  recompiles C under the same source and target profile, product suites pass,
  and the transitive manifest contains no Rust comparator or retired rung.

## P5 - Chain closure

- **CHAIN-MANIFEST.** Retain, for every edge, the exact source closure, tape,
  language/Alpha semantics versions, observation/resource profiles,
  reconstructed obligations, certificates, and disclosed admissions.

- **CHAIN-HYGIENE.** Keep `tools/bootstrap/check-chain-hygiene.sh` green. It
  rejects retired owners, obsolete assembler identities, intermediate
  self-hosting, unimplemented tapes, and source suffixes outside the selected
  immediate-predecessor map.

- **OFFLINE-REBUILD.** Reconstruct and check the complete chain on a blank
  supported host from one audited Alpha seed and repository-owned bytes. Host
  Python, Rust, networking, and package managers may assist diagnostics but are
  never semantic stages.
