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

Alpha is unchanged. Beta is the trusted imperative tape-assembly language.
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

## P1 - Gamma checker

- **GAMMA-DERIVATION-CHECKER.** Implement the smallest proof checker required
  by concrete compiler-edge certificates as an ordinary Gamma program run by
  the Beta evaluator. It validates an explicit derivation for an independently
  reconstructed proposition and performs no proof search, artifact discovery,
  deployment policy, or source-to-obligation inference. Acceptance: malformed,
  cyclic, missing-premise, wrong-subject, wrong-rule, and resource-exhausted
  certificates cannot accept within the published Gamma bounds.

## P2 - Gamma to Delta

- **DELTA-COMPILER.** Complete
  `bootstrap/delta/compiler/delta_compiler.gamma` against the full Delta contract,
  including nominal types, exhaustiveness, checked arithmetic, proper-tail
  lowering, sealed profiles, deterministic failure selection, and canonical
  Gamma emission. DCREQ framing and `ConformanceBytesV1` are executable;
  implement canonical DCOUT boundary failures without Delta-specific Gamma
  primitives. Direct Epsilon-to-Alpha profile ID 2 is retired.
  The current Epsilon evaluator source plus a diagnostic entry
  compiles through the selected lower route;
  continue reducing general transformation costs rather than admitting a
  customer-specific shortcut.
  Acceptance: conformance and malformed-source suites pass, the complete
  Epsilon evaluator compiles, exact receipts execute D, and no host or retired
  compiler participates.

## P3 - Delta to Epsilon

- **EPSILON-EVALUATOR.** Complete
  `bootstrap/epsilon/compiler/epsilon_compiler.delta` against
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
