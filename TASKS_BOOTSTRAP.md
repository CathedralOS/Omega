# Bootstrap tasks

Last reset: 2026-09-01.

This queue implements the selected trust-minimizing lattice. Git retains the
retired Alpha/Beta/Gamma/Delta/Epsilon baseline; no compatibility route or
self-host milestone is required for an intermediate rung.

```text
audited Alpha VM + admitted Beta compiler tape
  -> Beta-written Gamma evaluator -> gamma_evaluator_bytecode.tape
  -> Gamma-written Delta compiler -> delta_compiler_bytecode.tape
  -> Delta-written Epsilon compiler -> epsilon_compiler_bytecode.tape
  -> Epsilon-written Omega compiler D -> omega0_compiler_bytecode.tape
  -> Omega-written Omega compiler C -> omega_compiler_bytecode.tape
```

Alpha is unchanged. The imperative tape-assembly language is trusted Beta.
Gamma is now a bounded concatenative compiler machine. The former Gamma is
Delta, and the former Delta is Epsilon.

## Rules

- A language exists only to deliver the next rung and named small tools.
- A compiler accepts one language and emits exact Alpha tape directly.
- Intermediate self-hosting, general-purpose completeness, compatibility, and
  hypothetical reuse are not acceptance conditions.
- Host scripts invoke, stamp, compare, and report. They do not parse, lower,
  manufacture semantic evidence, or decide trust.
- Missing artifacts stay missing. No interpreter, old compiler, or native route
  stands in for an open edge.
- Every retained feature must cite a Gamma-evaluator, Delta-compiler,
  Epsilon-compiler, Omega-`D`, checker, or edge-verification customer.

## Current inventory

- [x] Alpha conformance passes all 26 cases.
- [x] Trusted Beta's 14,696-byte addressed source reconstructs its admitted 2,135-byte
  compiler tape byte-for-byte; the independent six-case differential and
  strict grammar regression pass.
- [x] Gamma's concatenative compiler-machine contract is fixed at
  `source/gamma/LANGUAGE.md`; an 81-line Gamma compiler emits and runs an exact
  35-byte addressed-CFG customer tape.
- [ ] A 738-line Beta-authored Gamma evaluator source and 29-case focused gate
  cover words, stacks, cells, sealed input, append-only output, ordinary calls,
  and explicit tail CFG transfers. A 213-line Gamma reconstructor emits the
  exact 4,289-byte evaluator tape from canonical Beta source. Tape admission and
  the complete conformance suite remain absent.
- [x] A 532-line experimental Gamma-written native compiler reaches an exact
  19,681-byte fixed point and agrees across seeded/native Delta0 compilation.
  Its runtime emission is mnemonic-level, fixed cells are named, and reserved
  names use exact byte comparison rather than hash identity.
  It remains a diagnostic side artifact, not an admitted bootstrap edge.
- [ ] Gamma derivation checker is absent.
- [ ] Gamma-written Delta compiler source and tape are absent.
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
  1,048,572-item/label and payload bounds in 118,488,640 bytes including the
  static base. State-machine Delta does not justify inserting another
  functional rung on implementation necessity or full-profile backend source cost;
  actual Epsilon semantic lowering and the need for independent raw-tape replay
  are now the remaining language-choice questions. Its latest implementation is retained
  at `source/delta/compiler/experiments/state_machine/delta_compiler.gamma`.
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

- **GAMMA-EVALUATOR.** Complete the Beta-authored evaluator for the exact strict
  first-order calculus in `source/gamma/LANGUAGE.md`.

  Keep only 64-bit words, an explicit checked data stack, fixed cells, sealed
  input, append-only byte/word output, named words, ordinary calls, and explicit
  tail `jump`/`branch`. Retain exact source bytes plus 32-byte definition rows;
  resolve names through exact source-order linear scans. Do not add an AST,
  token array, hashes, caches, interning, heap values, garbage collection,
  locals, algebraic data, pattern matching, closures, computed jumps,
  exceptions, modules, packages, interactive evaluation, or ambient effects.

  The evaluator uses the fixed `AlphaBootstrapV2` partition recorded in the
  Gamma profile: 1 MiB tape, 8 MiB request, 8 MiB definitions, 16 MiB data
  stack, 159 MiB cells, 16 MiB word continuations, 16 MiB reserved, and 32 MiB
  for Alpha's hidden call stack. Regions do not share spare capacity.

  The exact-ended v2 invocation supplies a little-endian u32 Gamma-source length,
  exact source, and all remaining bytes as sealed input. Returning from `main`
  succeeds with stdout. Status alone distinguishes invalid request/source,
  authored trap, incomplete capacity, and internal contradiction. There is no
  failure frame, stable reason taxonomy, source coordinate, detailed capacity
  report, fuel, or source-visible call ceiling. No nonzero result publishes an
  artifact; looping remains divergence. Output writes immediately. A late
  failure may leave stdout bytes, but invocation plumbing
  discards them unless status is 0; only status-0 stdout is an artifact.
  Successful output is capped at Alpha's 1,048,572-byte raw tape maximum.

  The trusted Beta source is `source/gamma/evaluator/gamma_evaluator.beta`.
  Its derived Alpha tape is bound through the admitted Beta compiler rather
  than separately admitted as opaque bytecode.

  The current source is a 29-case-passing, 738-line evaluator core producing a
  4,289-byte tape. An 81-line Gamma-written Delta0 compiler exercises cells,
  stack effects, source traversal, exact address assertions, byte emission, and
  direct CFG execution.

  Acceptance: a closed positive/negative suite pins lexical rejection,
  source-envelope and definition rejection, duplicate/builtin names, runtime
  name traps, exact stack effects, wrapping and signed arithmetic edges, input,
  cell and output bounds, forward/nested calls, exact linear lookup, deep tail
  transitions, bounded ordinary recursion, stack/continuation exhaustion,
  malformed private state, and deterministic replay. Mutating an evaluator
  opcode, branch target, extent bound, or trap
  path is detected by audit or a focused case. Output gates cover successful
  single-pass streaming plus late malformed-rope and oversize prefixes that
  remain unpublished under nonzero status.

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

- **DELTA-COMPILER.** Author
  `source/delta/compiler/delta_compiler.gamma`. It accepts the complete Delta
  contract, type-checks it, and emits direct Alpha tape.

  Reuse Gamma's immutable values and compact bytes; do not add compiler-shaped
  evaluator primitives. The compiler owns Delta names, nominal types,
  exhaustiveness, checked arithmetic, proper-tail-call lowering, sealed
  application profiles, exact failure selection, and Alpha emission. It may
  know the exact Epsilon-compiler customer profile but may not parse Epsilon.

  Acceptance: Delta language conformance and malformed-source suites pass; the
  compiler compiles representative and complete `epsilon_compiler.delta` source;
  exact tape reconstruction and source-to-Alpha refinement are checked; no host
  parser, old imperative compiler, serialized interpreter, or alternate tape
  participates.

## P3 - Delta to Epsilon

- **EPSILON-COMPILER.** Complete the renamed existing source at
  `source/epsilon/compiler/epsilon_compiler.delta` against
  `source/epsilon/LANGUAGE.md`.

  First audit every retained declaration and helper against the exact Epsilon
  compiler customer. Remove structures inherited only from the retired
  baseline. Finish body/control checking, fixed-storage realization, complete
  deterministic diagnostics, direct Alpha lowering, entry adapter, and atomic
  publication.

  Acceptance: the compiler consumes arbitrary valid Epsilon within its published
  bounds, rejects every malformed contract case deterministically, emits the
  exact Alpha tape for `source/omega/omega_compiler.epsilon`, and passes direct
  source-to-tape refinement. No Delta interpreter or host translation stage is
  retained.

## P4 - Epsilon to Omega

- **OMEGA-D.** Complete `source/omega/omega_compiler.epsilon` as the first full
  Omega compiler implementation. `D` implements the same accepted Omega
  language and artifact meaning as `C`; conservative and slow lowering is
  acceptable.

  Epsilon features are justified only by this source. Do not add standalone
  viewers, REPLs, debuggers, proof explorers, package services, or target tools
  unless `D` imports them to compile Omega.

  Acceptance: the Delta-built Epsilon compiler produces
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
  rejects retired tool/checker owners, old assembler identities,
  intermediate self-host owners, unimplemented compiler tapes, and source
  suffixes outside the selected immediate-predecessor map.

- **OFFLINE-REBUILD.** On an otherwise blank supported host, reconstruct and
  check the chain from one audited Alpha seed and repository-owned bytes. Host
  Python, Rust, network access, and package managers are optional diagnostics,
  never semantic stages.
