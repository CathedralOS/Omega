# Bootstrap tasks

Last reset: 2026-09-01.

This queue implements the selected trust-minimizing lattice. Git retains the
retired Alpha/Beta/Gamma/Delta/Epsilon baseline; no compatibility route or
self-host milestone is required for an intermediate rung.

```text
audited Alpha VM + directly audited Beta evaluator tape
  -> Beta-written Gamma compiler -> gamma_compiler_bytecode.tape
  -> Gamma-written Delta compiler -> delta_compiler_bytecode.tape
  -> Delta-written Omega compiler D -> omega0_compiler_bytecode.tape
  -> Omega-written Omega compiler C -> omega_compiler_bytecode.tape
```

Alpha is unchanged. Alpha Tape Assembly is off-chain tooling under
`tools/alpha/tape-assembly/`. Epsilon no longer exists as a live rung. Gamma is
the former typed functional Delta; Delta is the former fixed-storage Epsilon.

## Rules

- A language exists only to deliver the next rung and named small tools.
- A compiler accepts one language and emits exact Alpha tape directly.
- Intermediate self-hosting, general-purpose completeness, compatibility, and
  hypothetical reuse are not acceptance conditions.
- Host scripts invoke, stamp, compare, and report. They do not parse, lower,
  manufacture semantic evidence, or decide trust.
- Missing artifacts stay missing. No interpreter, old compiler, or native route
  stands in for an open edge.
- Every retained feature must cite a Gamma-compiler, Delta-compiler, Omega-`D`,
  checker, or edge-verification customer.

## Current inventory

- [x] Alpha conformance passes all 26 cases.
- [x] Alpha Tape Assembly moved off-chain; its 29,747-byte source reconstructs
  its 6,816-byte tape byte-for-byte, the independent six-case assembler
  differential passes, and strict grammar regression passes.
- [x] Beta's minimal functional contract is fixed at `source/beta/LANGUAGE.md`.
- [ ] A non-canonical Beta evaluator development source and 42-case focused
  gate now cover the request boundary and expression core. Declaration tables,
  general calls, constructors, `match`, proper tail calls, the admitted direct
  Alpha tape, audit listing, and complete conformance suite remain absent.
- [ ] Beta derivation checker is absent.
- [ ] Beta-written Gamma compiler source and tape are absent.
- [ ] `source/delta/compiler/delta_compiler.gamma` is incomplete; its tape is
  absent.
- [ ] `source/omega/omega_compiler.delta` (`D`) is incomplete; `omega0` is
  absent.
- [ ] The Omega-written product closure `C` is incomplete; production `omega`
  is absent.

## P0 - Beta evaluator root

- **BETA-EVALUATOR.** Implement one directly audited Alpha tape for the exact
  strict first-order calculus in `source/beta/LANGUAGE.md`.

  Keep only signed checked `Int`, compact immutable `Bytes`, immutable tagged
  constructors, exhaustive `match`, `if`, one-binding `let`, first-order calls,
  mutual recursion, and proper tail calls. Retain exact immutable source bytes
  plus declaration spans only; resolve global and local names through exact
  source-order linear byte scans. Persistent values use a bounded bump arena;
  bindings and continuations use a separate reusable bounded stack. Do not add
  an AST, token array, hashes, caches, interning, general GC, closures, function
  values, mutation, raw memory, macros, polymorphism, source-visible
  continuations, exceptions, modules, packages, interactive evaluation, or
  ambient effects.

  The first evaluator uses the fixed `AlphaBootstrapV2` partition recorded in
  the Beta contract: 1 MiB tape, 8 MiB request, 8 MiB declarations, 16 MiB
  evaluator stack, 191 MiB immutable arena, and 32 MiB reserved for Alpha's
  hidden call stack. Regions do not share spare capacity.

  The exact-ended `BETAREQ` v1 invocation supplies u32-length-delimited Beta
  source and sealed input bytes. The entry returns only `(Complete Bytes)` or
  `Reject`. Status alone distinguishes invalid request/source, authored
  trap/rejection, incomplete capacity, and internal contradiction. There is no
  failure frame, stable reason taxonomy, source coordinate, detailed capacity
  report, fuel, or call ceiling. No nonzero result publishes an artifact; recursive
  divergence remains divergence. `Complete` validates and streams its rope in
  one pass. A late failure may leave stdout bytes, but invocation plumbing
  discards them unless status is 0; only status-0 stdout is an artifact.
  Successful output is capped at Alpha's 1,048,572-byte raw tape maximum.

  A readable `.alphaasm` reconstruction may live under the Alpha tool owner,
  but the exact evaluator tape is admitted and instruction-audited directly.
  The assembler's correctness is not a premise of Beta meaning.

  Implementation checkpoint: `tools/alpha/beta-evaluator/evaluator.alphaasm`
  is a 42-case-passing development slice, not the admitted tape. It implements
  framing, one unary entry function, structural checks for its accepted forms,
  `if`, `let`, checked integer operations, every byte primitive, total equality
  over values it can construct, outcomes, and bounded arena/stack operation.
  The declaration/call/constructor/match/tail-call work remains on this task.

  Acceptance: a closed positive/negative suite pins lexical rejection,
  complete structural syntax rejection, declaration census, runtime name and
  arity traps, runtime declaration-order match checking, left-to-right
  strictness, branch selectivity, checked integer edges, `bytes-single`, rope
  and view bounds, total structural equality, forward/mutual calls, exact
  linear name lookup, deep proper tail recursion, arena exhaustion, frame
  exhaustion, malformed private state, and deterministic replay. Mutating an
  evaluator opcode, branch target, allocation bound, constructor tag, or trap
  path is detected by audit or a focused case. Output gates cover successful
  single-pass streaming plus late malformed-rope and oversize prefixes that
  remain unpublished under nonzero status.

- **BETA-ROOT-AUDIT.** Publish the evaluator's exact tape bytes, decoded
  instruction inventory, control-flow reconstruction, memory map, mutable table
  inventory, fixed spatial ceilings, and SHA-256 identity. Compare its total
  review cost with the removed assembler-plus-imperative-rung root; source and
  tape size alone are not the verdict.

  Acceptance: an independent reviewer can trace every evaluator operation to
  one Beta rule without trusting source reconstruction, a host implementation,
  or a filename convention.

## P1 - Beta tools

- **BETA-DERIVATION-CHECKER.** Derive the smallest proof calculus required by
  the selected compiler-edge certificates and implement it as an ordinary Beta
  program executed by the audited evaluator.

  The checker validates an explicit derivation for an independently
  reconstructed proposition. It performs no proof search, artifact discovery,
  deployment policy, or source-to-obligation inference. Add a rule only for a
  concrete edge theorem; product proof ambitions do not enlarge this root tool.

  Acceptance: malformed, cyclic, missing-premise, wrong-subject, wrong-rule,
  and resource-exhausted certificates cannot accept; the complete checker
  source and evaluator profile fit the published Beta bounds.

## P2 - Beta to Gamma

- **GAMMA-COMPILER.** Author
  `source/gamma/compiler/gamma_compiler.beta`. It accepts the complete Gamma
  contract, type-checks it, and emits direct Alpha tape.

  Reuse Beta's immutable values and compact bytes; do not add compiler-shaped
  evaluator primitives. The compiler owns Gamma names, nominal types,
  exhaustiveness, checked arithmetic, proper-tail-call lowering, sealed
  application profiles, exact failure selection, and Alpha emission. It may
  know the exact Delta-compiler customer profile but may not parse Delta.

  Acceptance: Gamma language conformance and malformed-source suites pass; the
  compiler compiles representative and complete `delta_compiler.gamma` source;
  exact tape reconstruction and source-to-Alpha refinement are checked; no host
  parser, old imperative compiler, serialized interpreter, or alternate tape
  participates.

## P3 - Gamma to Delta

- **DELTA-COMPILER.** Complete the renamed existing source at
  `source/delta/compiler/delta_compiler.gamma` against
  `source/delta/LANGUAGE.md`.

  First audit every retained declaration and helper against the exact Delta
  compiler customer. Remove structures inherited only from the retired
  baseline. Finish body/control checking, fixed-storage realization, complete
  deterministic diagnostics, direct Alpha lowering, entry adapter, and atomic
  publication.

  Acceptance: the compiler consumes arbitrary valid Delta within its published
  bounds, rejects every malformed contract case deterministically, emits the
  exact Alpha tape for `source/omega/omega_compiler.delta`, and passes direct
  source-to-tape refinement. No Gamma interpreter or host translation stage is
  retained.

## P4 - Delta to Omega

- **OMEGA-D.** Complete `source/omega/omega_compiler.delta` as the first full
  Omega compiler implementation. `D` implements the same accepted Omega
  language and artifact meaning as `C`; conservative and slow lowering is
  acceptable.

  Delta features are justified only by this source. Do not add standalone
  viewers, REPLs, debuggers, proof explorers, package services, or target tools
  unless `D` imports them to compile Omega.

  Acceptance: the Gamma-built Delta compiler produces
  `omega0_compiler_bytecode.tape` from the exact `D` closure; `omega0` accepts
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
  rejects the retired Epsilon and checker owners, old assembler identities,
  intermediate self-host owners, unimplemented compiler tapes, and source
  suffixes outside the selected immediate-predecessor map.

- **OFFLINE-REBUILD.** On an otherwise blank supported host, reconstruct and
  check the chain from one audited Alpha seed and repository-owned bytes. Host
  Python, Rust, network access, package managers, and Alpha Tape Assembly are
  optional diagnostics, never semantic stages.
