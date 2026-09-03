# Bootstrap tasks

Last reset: 2026-09-01.

This queue implements the selected trust-minimizing lattice. Git retains the
retired Alpha/Beta/Gamma/Delta/Epsilon baseline; no compatibility route or
self-host milestone is required for an intermediate rung.

```text
audited Alpha VM + admitted Beta compiler tape
  -> Beta-written Gamma evaluator -> gamma_evaluator_bytecode.tape
  -> Gamma-authored staged source transformers
  -> Delta compiler -> canonical Gamma source
  -> Epsilon compiler -> canonical Delta -> ... -> epsilon_compiler_bytecode.tape
  -> Omega compiler D -> canonical Epsilon -> ... -> omega0_compiler_bytecode.tape
  -> Omega compiler C -> canonical Epsilon -> ... -> omega_compiler_bytecode.tape
```

Alpha is unchanged. The imperative tape-assembly language is trusted Beta.
Gamma is now the typed scalar/effect functional language formerly called
Delta0 and is evaluated directly by Beta. Delta remains the richer typed
functional language required by Epsilon.

## Rules

- A language exists only to deliver the next rung and named small tools.
- Beta alone encodes Alpha. Gamma is directly evaluated by a Beta artifact;
  higher source transformers publish canonical immediately-lower source.
- Intermediate self-hosting, general-purpose completeness, compatibility, and
  hypothetical reuse are not acceptance conditions.
- Host scripts invoke, stamp, compare, and report. They do not parse, lower,
  manufacture semantic evidence, or decide trust.
- Missing artifacts stay missing. The selected Gamma evaluator is an explicit
  edge; no old compiler or native route stands in for an open Delta edge.
- Every retained feature must cite a Gamma-evaluator, Delta-compiler,
  Epsilon-compiler, Omega-`D`, checker, or edge-verification customer.

## Current inventory

- [x] Alpha conformance passes all 26 cases.
- [x] Trusted Beta's 12,639-byte addressed source reconstructs its admitted 1,792-byte
  compiler tape byte-for-byte; the independent six-case differential and
  strict grammar regression pass.
- [ ] Gamma's typed scalar/effect contract is fixed at
  `source/gamma/LANGUAGE.md`. Its provisional 1,410-line Beta evaluator assembles
  to a 7,690-byte tape and runs integer/character literals, typed lexical lets, conditionals,
  scalar operators, forward calls, recursion, sealed input, indexed reads, and
  byte output plus nested immutable pairs. It executes the unchanged 85-line
  Gamma-authored augmenter and its exact result-42 receipt. Proper tail
  execution, static validation of unreachable bodies, bounded output, and pair
  allocation are implemented. Complete resource outcomes remain.
- [x] The former concatenative Gamma evaluator/compiler and Gamma-written Delta
  compiler are downgraded under `source/gamma/bootstrap/concatenative/` and
  `source/delta/bootstrap/concatenative-compiler/`. They remain comparison
  evidence and are excluded from selected-edge inventories.
- [ ] Gamma derivation checker is absent.
- [ ] The selected 852-line Gamma-authored Delta compiler supports finite ADTs
  whose constructors carry arbitrary `Int` or known nominal fields. Payload-bearing
  values use immutable `(pair tag product)` nodes with right-nested products, and
  declaration-order exhaustive matches recover binders through projections and
  ordinary Gamma lets. Exact nullary, payload, recursive unary, two-field List,
  and three-field Bytes-rope fixtures evaluate to 9, 9, 3, 9, and `0x42`; a
  3,001-function transformation also passes. Normative `Bytes`, complete checking,
  and profiles remain. The
  downgraded concatenative compiler proves broader expressiveness but is not the
  selected edge.
- [x] A matched direct Beta Delta evaluator experiment covers the selected
  recursive Nat, two-field List, three-field Bytes-rope, malformed-source,
  3,001-function, and 100,000-node proper-tail witnesses. It requires 2,019 Beta
  lines and an 11,004-byte tape: 609 additional low-level lines and 254 additional
  control transfers versus selected Gamma. D92 therefore retains Gamma and keeps
  constructor/match semantics in the more readable staged compiler.
- [x] A noncanonical 565-line current-Gamma compiler now emits Alpha directly
  for scalar Functional Delta: `Int` functions, parameters, lexical `let`,
  conditionals, arithmetic/comparison, nested calls, and direct recursion. Its
  native compiler tape is 22,214 bytes. One exact recursive workload is nine
  Functional Delta lines and an 842-byte tape versus 29 State Delta lines and a
  771-byte tape; both exit 15. Functional Delta wins authored compactness while
  State Delta wins this generated tape by 71 bytes. This is not the canonical
  edge: algebraic data, exhaustive `match`, `Bytes`, complete checking, proper
  tail calls, application profiles, transactional publication, and exact
  resource outcomes remain absent.
- [x] Functional-Delta-to-Gamma elaboration is executable without an Alpha
  backend. The 239-line schema proof emits an exact five-line Gamma program
  with visible tail `jump`. The selected 550-line scalar compiler covers
  the direct scalar compiler's literals, bindings, conditionals, seven scalar
  operators, forward/nested calls, recursion, and arities through 13. Its
  19,238-byte tape is 2,976 bytes smaller than the 22,214-byte direct scalar
  compiler while retaining canonical Gamma receipts. One aggregate 15-level
  nesting bound is stricter than the direct experiment's separate bounds.
- [x] A noncanonical typed state-machine Delta experiment covers nominal sums,
  records, fixed arrays, typed machine variables, states, exhaustive
  transitions, calls, dynamic indexed arenas, nested scopes, deterministic
  source-offset diagnostics, one-word nominal arrays, recursive AST rewrites,
  typed multiplication/division, signed comparisons, and direct Alpha emission
  in 709 Gamma lines. A 427-line customer performs a
  five-variant postorder fold and subtree-selection transform over bounded
  arenas. A 552-line Epsilon-shaped customer lays out and encodes every Alpha
  instruction variant from symbolic items, compared with 834 retained functional
  backend lines. Two typed record-row arenas carry the full
  retained 1,048,572-item/label comparison bound in 118,488,640 bytes including the
  static base. State-machine Delta does not justify inserting another
  functional rung on implementation necessity or full-profile backend source cost;
  an additional experiment adds owner-scoped names and bounded typed call frames
  through the retained customer's maximum arity 13. Direct recursion uses
  fixed-size software frames and rejects collision with Alpha's descending
  return stack. This grows the compiler to 815 Gamma lines and 29,105 native
  bytes. A representable Epsilon parser-helper kernel needs 48 state-machine
  code lines and 10 states versus nine Functional Delta lines; its exact
  immutable-`Bytes` recursion remains inexpressible. Bounded calls therefore do
  not justify changing normative Functional Delta. Actual Epsilon semantic
  lowering remains incomplete. The latest implementation is retained with its
  gate at `tests/delta/state-machine-experiment/compiler.gamma`.
- [ ] `source/epsilon/compiler/epsilon_compiler.delta` is incomplete; its tape is
  absent.
- [ ] `source/omega/omega_compiler.epsilon` (`D`) is incomplete; `omega0` is
  absent.
- [ ] The Omega-written product closure `C` is incomplete; production `omega`
  is absent.

## P0 - Beta floor and Gamma evaluator

- **BETA-ROOT-AUDIT.** Publish the admitted Beta compiler tape's decoded
  instruction inventory, control-flow reconstruction, memory map, fixed
  ceilings, SHA-256 identity, exact self-reconstruction, and independent
  source-to-tape correspondence.

  Acceptance: an independent reviewer can trace every compiler operation to one
  Beta rule and bind `beta_compiler.beta` byte-identically to the executed tape.

- **GAMMA-EVALUATOR.** Complete the 1,410-line direct Beta evaluator for the typed
  scalar/effect contract in `source/gamma/LANGUAGE.md`.

  Retain declaration census and direct source evaluation; do not add an AST,
  generated code, concatenative machine, general mutable store, source-declared
  algebraic data, or ambient host access. Proper tail transfer, validation of
  every body, bounded output, and immutable pairs are implemented. Close every
  remaining explicit memory boundary with profile-owned `Incomplete` outcomes.

  The little-endian u32 request framing, status taxonomy, memory partition, and
  publication rule are fixed by `source/gamma/EVALUATOR_PROFILE.md`. The Beta
  source must continue to assemble directly through the admitted Beta compiler;
  no symbolic-address resolver is part of the selected edge.

  Acceptance: the scalar/effect and self-augmentation gates pass; malformed
  reached and unreachable definitions reject before publication; a 100,000-step
  direct tail recursion witness uses constant activation/context storage;
  wrapping integer edges and every capacity boundary have exact outcomes; source and tape
  identities reconstruct without host semantic translation.

## P1 - Gamma tools

- **GAMMA-DERIVATION-CHECKER.** Derive the smallest proof calculus required by
  the selected compiler-edge certificates and implement it as an ordinary Gamma
  program executed by the trusted Beta-authored evaluator.

  The checker validates an explicit derivation for an independently
  reconstructed proposition. It performs no proof search, artifact discovery,
  deployment policy, or source-to-obligation inference. Add a rule only for a
  concrete edge theorem; product proof ambitions do not enlarge this root tool.

  Acceptance: malformed, cyclic, missing-premise, wrong-subject, wrong-rule,
  and resource-exhausted certificates cannot accept; the complete checker
  source and evaluator profile fit the published Gamma bounds.

## P2 - Gamma to Delta

- **DELTA-COMPILER.** Extend
  `source/delta/compiler/delta_compiler.gamma` from its nullary-ADT/match stage
  until it accepts the complete Delta contract, type-checks it, and emits
  canonical Gamma source.

  Build it in auditable stages authored in scalar/effect Gamma, moving richer
  source representations upward only after the preceding stage can check and
  emit them. Do not expand the Gamma evaluator with Delta-shaped primitives.
  The compiler owns Delta names, nominal types, exhaustiveness, checked
  arithmetic, proper-tail-call lowering, sealed application profiles, exact
  failure selection, and Gamma elaboration. It may know the exact
  Epsilon-compiler customer profile but may not parse Epsilon.

  Acceptance: Delta language conformance and malformed-source suites pass; the
  compiler compiles representative and complete `epsilon_compiler.delta` source;
  exact Gamma receipts, evaluator composition, and source-to-Alpha refinement
  are checked; no host parser, downgraded concatenative compiler, or alternate
  tape participates.

## P3 - Delta to Epsilon

- **EPSILON-COMPILER.** Complete the renamed existing source at
  `source/epsilon/compiler/epsilon_compiler.delta` against
  `source/epsilon/LANGUAGE.md`.

  First audit every retained declaration and helper against the exact Epsilon
  compiler customer. Remove structures inherited only from the retired
  baseline. Finish body/control checking, fixed-storage realization, complete
  deterministic diagnostics, canonical Delta elaboration, entry adapter, and
  atomic publication.

  Acceptance: the compiler consumes arbitrary valid Epsilon within its published
  bounds, rejects every malformed contract case deterministically, emits the
  exact canonical Delta receipt for `source/omega/omega_compiler.epsilon`, and
  composes through selected lower compilers to the exact Alpha tape. No Delta
  interpreter or host translation stage is retained.

## P4 - Epsilon to Omega

- **OMEGA-D.** Complete `source/omega/omega_compiler.epsilon` as the first full
  Omega compiler implementation. `D` implements the same accepted Omega
  language and artifact meaning as `C`; conservative and slow lowering is
  acceptable.

  Epsilon features are justified only by this source. Do not add standalone
  viewers, REPLs, debuggers, proof explorers, package services, or target tools
  unless `D` imports them to compile Omega.

  Acceptance: the Delta-built Epsilon compiler produces canonical Epsilon from
  the exact `D` closure and selected lower compilers produce
  `omega0_compiler_bytecode.tape`; `omega0` accepts
  the complete Omega language, emits required target artifacts, and passes the
  product language suites and direct refinement checks.

- **OMEGA-C.** Use `omega0` to compile the exact Omega-written product closure
  rooted at `source/omega/{build.omg,main.omg}`.

  This is the only meaningful self-host edge. `D` and `C` are distinct source
  closures implementing the same complete language; `C` may contain the
  production optimizer and better lowering without expanding accepted meaning.

  Acceptance: `omega0 -> C -> omega` is deterministic, `omega` recompiles `C`
  under the same exact source and target profiles, product suites pass, and the
  transitive source/build/tool manifest contains no Rust comparator or retired
  rung.

## P5 - Chain closure

- **CHAIN-MANIFEST.** For every row, retain exact source closure, tape identity,
  language/Alpha semantics versions, observation and resource profiles,
  reconstructed obligations, certificates, and disclosed admissions.

- **CHAIN-HYGIENE.** Keep `tools/bootstrap/check-chain-hygiene.sh` green. It
  rejects retired tool/checker owners, old assembler identities,
  intermediate self-host owners, unimplemented compiler tapes, and source
  suffixes outside the selected immediate-predecessor map.

- **OFFLINE-REBUILD.** On an otherwise blank supported host, reconstruct and
  check the chain from one audited Alpha seed and repository-owned bytes. Host
  Python, Rust, network access, and package managers are optional diagnostics,
  never semantic stages.
