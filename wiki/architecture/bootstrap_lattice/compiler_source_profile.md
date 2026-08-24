# Delta and the Omega product-compiler source profile

[Lattice overview](bootstrap_lattice.md) | [Standing decisions](decisions.md) |
[Delta rung](rungs/delta.md) | [Psi/Omega toolchain](omega_toolchain.md)

The final bootstrap has two deliberately different source surfaces and one
product language specification:

```text
Delta source              Omega source constrained to Ωself
     │                                  │
     ▼                                  ▼
omega-bootstrap  ─────────────────▶  production omega
 accepts Ωself only                    implements full Ω
```

Delta v1 and `Ωself` are the only remaining source-surface contracts in this
bootstrap design. They are deliberately asymmetric: Delta v1 is a literal
language specification, while `Ωself` is a restriction on source written in an
already-specified language. Do not turn the compiler artifacts between them
into extra languages:

`Ωself` is retained as the short symbol for the product compiler's ordinary-
Omega source profile. The required use of that profile is a cross-language
hosted build by `omega-bootstrap`; the name does not imply that this edge is an
Omega self-rebuild. Only the optional later `omega` → `omega` build is strict
self-hosting.

| Source contract | What it is | Selected from | What does **not** define it |
| --- | --- | --- | --- |
| Delta v1 | an independent literal language specification | the complete Delta source closure of `omega-bootstrap` | D0, samples, or whatever the Rust producer happens to accept |
| `Ωself` | a compositional profile of ordinary Omega | the complete Omega source closure of the production compiler, with retain/refactor settled by measured bridge cost | Delta's features, a file allowlist, or the current compiler's exact AST permutations |

Full Omega is the already-separate product language specification. It is what
the production compiler must implement, not a third bootstrap source-profile
choice. The compiler artifacts have corresponding implementation obligations:

There is no separate `omega-bootstrap` language inventory: its implementation
surface is Delta v1 and its accepted-source surface is `Ωself`. There is also no
bootstrap vote on which user-facing Omega features the product compiler should
implement; the full Omega specification already answers that question.

| Compiler artifact | Written in | Accepts | Obligation |
| --- | --- | --- | --- |
| `omega-bootstrap` | Delta | `Ωself` | compile every admitted program with exact Omega meaning; unsupported Omega rejects |
| production `omega` | Omega constrained to `Ωself` | full Omega | implement the complete language, optimizer, and lowering pipeline |
| optional rebuilt `omega` | the same `Ωself`-constrained Omega source, now compiled by production `omega` | full Omega | improve the compiler executable itself and add reproducibility evidence |

The optional rebuild is not another language rung or bootstrap dependency.
Likewise, a conservatively generated production-compiler executable may still
contain and run the full optimizer when it compiles user programs.

The names are deliberately non-generational. `omega-bootstrap` is a role, not
“Omega 0”; the production compiler is `omega`, not “Omega 1”; and an optional
self-rebuild creates a better executable of the same compiler rather than a new
language or rung. O0/O1 are only names for already-frozen vertical canaries in
the bridge implementation.

- **Delta** is an independent, robust compiler-host language. It may resemble
  Omega in spelling and shape, but it is not required to be an Omega subset.
- **`Ωself`** is the Omega product-compiler source profile: a compositional subset
  of ordinary Omega accepted by `omega-bootstrap`. It introduces no syntax or
  semantics of its own. It is a feature-and-resource contract, not a whitelist
  of the current compiler files or a collection of recognized AST shapes.
- **Full Omega** is the language implemented by the resulting production
  compiler. A compiler can implement a feature without using that feature in
  its own source.

Here, “implements full Omega” describes the compiler's accepted language and
the meaning of the artifacts it produces. It does not require the bootstrap
source closure to contain every adjacent product tool. A standalone Terminal
Psi interpreter, REPL, proof explorer, viewer, or debugger belongs in the
closure only if the production compiler executable actually imports it.

The bootstrap closure condition is therefore:

```text
the complete omega-bootstrap source closure ∈ Delta v1
the complete production-compiler source closure ∈ Ωself
omega-bootstrap correctly compiles every input admitted by Ωself
production omega correctly implements full Ω
```

It is not necessary for `omega-bootstrap` to accept every Omega program. Its
acceptance completeness is deliberately limited; its semantic correctness is
not. It must reject unsupported constructs rather than approximate them, and
every construct it does accept has exactly its normal Omega meaning, ABI,
layout, and artifact contract. General parsing, checking, and lowering rules
must implement the admitted profile; matching the present source file, statement
count, or syntax-tree permutation is not an implementation of `Ωself`.

## Delta design budget

Delta should have C-like systems power without inheriting C's undefined and
ambient behavior. Its v1 inventory is derived from the complete
`omega-bootstrap` source closure rather than from D0, the sample corpus, or the
Rust producer. The following are candidate tools, not facilities already voted
into the language:

- fixed-width scalars, bytes, predictable aggregates, arrays, slices, and
  explicit representation;
- procedures, deterministic source bundling, loops, recursion,
  state-machine control, and payload-bearing sum data;
- explicit references or integer arena handles, checked indexing, and stable
  calling/layout conventions;
- deterministic trapping or checked arithmetic and explicit boundary I/O;
- runtime-sized allocation from fixed backing, typed/indexed arenas, and bulk
  reclamation, with specified exhaustion;
- conservative lowering and auditable code generation.

The non-negotiable properties are deterministic specified behavior, no
undefined behavior or ambient host authority, specified failure, and lower-rung
meaning for every admitted construct. Failure may be a checked result, static
rejection, or defined trap according to the retained operation; it may not be
silent truncation or undefined behavior. Within those constraints, minimize
whole-bootstrap cost rather than feature count in isolation. If Exact arithmetic
is sufficient, Delta need not acquire Wrapping or Saturating merely because the
current producer accepts them. If artifact encoding needs one modular operation,
prefer that narrow operation over a pervasive arithmetic-policy system unless
the source demonstrates the broader system pays for itself. Apply the same test
to sums, references, arenas, contracts, refinements, and every other corpus
feature.

Omega-like lexical and structural conventions reduce cognitive and tooling
distance. When both languages retain the same construct, Delta should use
Omega's spelling, grammar, precedence, and ordinary meaning unless doing so
materially increases the bootstrap or assurance burden. Unsupported Omega
constructs reject; shared syntax does not make Delta an Omega subset or couple
its versioning to Omega. Delta-only bootstrap facilities are acceptable when
they reduce total cost and remain explicit in Delta's specification.

Delta has one intended program class, so host extensibility is not presumed.
The working boundary is sealed byte input, artifact output, diagnostic output,
and process termination. Target configuration belongs in the deterministic
input. Filesystem traversal, environment access, clocks, networking, process
spawning, and general boundary-trait realization remain outside v1 unless the
bridge source demonstrates an unavoidable requirement. Fixed backing can be
ordinary zero-initialized program storage rather than a host service.

Discovery does not make the contract corpus-shaped. During construction, each
new facility must record either its concrete bridge requirement or its explicit
language-coherence, robustness, safety, or maintainability argument, along with
the simpler rejected alternative and its lower-rung meaning and negative gates.
Before freezing v1, publish the complete source manifest and feature inventory,
remove accidental producer behavior, then specify the retained grammar and edge
cases independently of those particular files. The working inventory lives in
the Delta rung's
[`FEATURE_LEDGER.md`](../../../bootstrap/rungs/delta/FEATURE_LEDGER.md).

## Working `Ωself` policy

The exact profile cannot be frozen before the production compiler source and
deterministic dependency manifest exist. The working policy is nevertheless
specific enough to guide that source.

Delta v1 and `Ωself` remain separate contracts even though their discovery can
co-evolve. Delta is derived from the cost of implementing and assuring the
complete `omega-bootstrap` closure; `Ωself` is derived from the cost and
robustness of the production compiler source. Neither contract should be made
artificially resemble the other, and neither source manifest is allowed to
stand in for a language/profile definition.

That gives the design loop exactly two feature inventories:

1. What literal facilities must Delta provide so the bridge can be implemented
   robustly?
2. Which ordinary Omega facilities may the production compiler use in its own
   source while the bridge remains tractable?

Everything else in the hosted build is implementation, validation, or optional
optimization work under those two inventories.

For every disputed `Ωself` facility, use one decision procedure:

1. If the complete production-compiler closure does not use the facility,
   exclude it and add a phase-appropriate negative canary.
2. If the closure uses it, compare retaining a general compositional form with
   refactoring the product source to a simpler form.
3. Retain it when the source clarity, regularity, reuse, or safety benefit
   exceeds the measured Delta implementation and assurance cost. Otherwise
   refactor it out and keep it excluded.

This procedure does not reward feature removal by itself. A small general
facility is preferable to monomorphic duplication, hand-expanded variants, or
hard-coded compiler-source shapes when it lowers total cost. Conversely, a
powerful facility used only incidentally should not enter the bridge merely
because the production compiler can express itself with it.

The working feature disposition is:

| Omega facility in the compiler's own source | Working disposition | Decision test |
| --- | --- | --- |
| mathematical proof/program surface | presumptively exclude | retain only if the compiler implementation itself has an unavoidable use; implementing proof checking for user programs is not such a use |
| linear and dependent types | presumptively exclude | same source-need and total-cost test |
| ordinary named record fields | presumptively retain | removing them is likely to make compiler data less clear and more brittle |
| payload-bearing enums/sum data | presumptively retain | syntax and IR modeling cost versus explicit-tag records |
| basic generics | presumptively retain | collection, result, arena-ID, and compiler-data reuse versus monomorphic duplication |
| concrete domains and domain arithmetic | measure | compare with explicit compiler contexts and narrow operations |
| domain polymorphism | measure | admit only the forms used by the closed source manifest |
| advanced generic constraints, specialization, and reflection | measure | source benefit versus bridge and assurance cost |
| numeric/schema field tags such as `0:` | measure | compare with ordinary named fields; these are distinct from named record fields |
| complex transition payloads | measure | compare with a simple discriminant plus explicit compiler context |
| mixed field-plus-case data | measure | compare with separate record and sum-data types |

Source-unit membership is a separate question from language features.
Standalone terminal-Psi tools, interpreters, REPLs, proof explorers, viewers,
debuggers, and other product tooling are presumptively outside the hosted
closure unless the compiler executable imports them.

The current product architecture does use Terminal Psi as the target-neutral
compiler boundary. Consequently, the representation and lowering modules the
compiler links are ordinary members of the product source manifest. This does
not imply that a standalone Terminal-Psi interpreter, artifact viewer, or debug
tool belongs to the manifest, nor that `omega-bootstrap` must use Terminal Psi
as its own internal IR. The bridge must compile those product modules as
ordinary `Ωself` source; it need not duplicate or execute their product role.

These are `Ωself` source-profile choices, not proposals to remove the features
from full Omega. Conversely, Delta does not acquire or reject any of them merely
because the production compiler source does: Delta's literal feature set is a
separate compiler-host design decision.

The distinction is syntactic as well as architectural. Excluding proof syntax,
dependent types, or another advanced facility from `Ωself` means that the
compiler's *own implementation* does not use that facility to express itself.
It does not prevent ordinary `Ωself` records, sums, tables, and procedures from
implementing the parser, checker, and lowering rules for that facility. The
full-Omega language suites, rather than the `Ωself` feature census, establish
that the resulting product compiler actually implements it.

For a retained feature, `omega-bootstrap` need implement the feature only within
the structurally declared bounds of `Ωself`; the production compiler implements
the complete feature. Those bounds must be general enough to compile any source
that satisfies the published profile, not encoded as permutations of the
current compiler source. Simplified cases must preserve full Omega semantics.
No bootstrap-only Omega dialect or private extension is permitted.

The selection rule is total cost, not the smallest feature count:

```text
benefit and robustness in the production Omega source closure
──────────────────────────────────
implementation + assurance cost in the omega-bootstrap Delta source closure
```

Basic generics and payload enums are likely favorable. Proof syntax and
dependent typing are not. This is a total-cost profile, not a contest to remove
the most features: retaining a cheap general facility is preferable to forcing
large, brittle, monomorphic compiler source. Profile growth is an architectural
change and must update the profile rules, compiler, meaning route, diagnostics,
and negative gates together.

## One required hosted production build

`omega-bootstrap` may itself be a slow binary and may lower `main.omg`
conservatively. It must understand enough `Ωself` to compile the source that
*implements* the production optimizer and advanced lowering; it does not need
to run those product passes during this build. The required hosted result is a
full optimizing compiler, although that compiler's own machine code may still
be conservatively generated.

```text
Delta bridge source ──[Delta compiler]──▶ omega-bootstrap (slow binary)
Ωself product source ──[omega-bootstrap]──▶ omega (full compiler; conservative binary)
the same product source ──[optional omega rebuild]──▶ omega (same compiler; optimized binary)
```

A later production-Omega rebuild can optimize the compiler binary itself and
provide fixed-point or reproducibility evidence. It is optional: neither full
language functionality nor bootstrap dependency closure waits for it.
Strictly, that optional `omega` → `omega` edge is the self-rebuild; the required
Delta-written-compiler → Omega-source edge is a cross-language hosted build.

## Mechanical enforcement

The production compiler task must publish one deterministic source/dependency
manifest. This is a staged discovery loop, not a circular build dependency: the
product source can be authored under the working policy while the Delta bridge
and ledger evolve against provisional closures. `Ωself` can be derived and
enforced provisionally once the product closure is visible, but its cost claims
cannot be final until the general bridge implementation exists. Keep
provisional derivation and the freeze join as distinct milestones:

1. Write the product compiler under the conservative working policy and publish
   its complete source closure.
2. Measure each used feature against its benefit in the product source and its
   implementation/assurance cost in Delta.
3. Either refactor the product source to remove the feature or admit it to the
   compositional `Ωself` contract.
4. Implement and assure those general profile rules in `omega-bootstrap`, feeding
   measured cost back into the retain/refactor decision.
5. Freeze the source manifest and `Ωself` together at that bridge join; the
   already-running mechanical enforcement becomes the frozen acceptance gate.

This process classifies facilities, not individual occurrences. A retained
facility is implemented compositionally for every source admitted by its
published bounds. An excluded facility receives a phase-appropriate negative
canary. The exact compiler manifest is then a closure witness over those rules,
not a special case accepted by them.

The bootstrap gate must compile that exact closure under the explicit `Ωself`
profile rules and reject an excluded-feature canary for every exclusion.
The manifest includes compiler modules, compile-time code, build/module
behavior, and runtime/library dependencies; hiding a feature in a library does
not remove it from the bootstrap surface.

Delta's freeze is adjacent but separate. Its provisional ledger evolves while
`omega-bootstrap` is written. Once the complete Delta source closure exists,
prune accidental producer/corpus behavior, publish the general Delta v1 grammar
and semantics, and prove that exact closure valid under the frozen language. The
Omega product-source manifest plus measured bridge cost decide `Ωself`; the
Delta bridge-source manifest plus explicit compiler-host coherence arguments
decide Delta v1. Neither manifest substitutes for the corresponding general
profile or language contract.

The current Rust Psi/Omega compiler remains a maintained reference and
differential producer while useful. It is neither a bootstrap dependency nor an
authority source: lower-rooted refinement and canonical meaning decide
acceptance. Cross-compiler diagnostics, normalized IR, artifacts, and execution
observations remain valuable bug-finding evidence.
