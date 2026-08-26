# Delta and the Omega product-compiler source profile

[Lattice overview](bootstrap_lattice.md) | [Standing decisions](decisions.md) |
[Delta rung](rungs/delta.md) | [Psi/Omega toolchain](omega_toolchain.md)

The final bootstrap has exactly two source-surface inventories to settle: the
literal Delta language used to write the Delta compiler and bridge, and the
ordinary-Omega profile used to write the product compiler. The first is a
language design; the second is an authoring choice inside an existing language.
Full Omega is already the product language specification; generated-code
quality is an artifact property. The actual build sequence is:

```text
Delta compiler source ∈ Delta v1
             │
             └──[Delta→Gamma + Gamma execution]───────▶ delta compiler

omega-bootstrap source ∈ Delta v1
             │
             └──[delta compiler]──────────────────────▶ omega-bootstrap

production-compiler source ∈ Ωself
             │
             └──[omega-bootstrap]───────────────▶ omega
                                                   accepts full Ω
                                                   contains the optimizer
                                                   may itself be conservatively lowered

the same production source ──[optional omega rebuild]──▶ omega
                                                          same compiler,
                                                          better executable
```

Delta v1 and `Ωself` are the only remaining source-surface contracts in this
bootstrap design. They are deliberately asymmetric: Delta v1 is a literal
language specification, while `Ωself` is an incidental authoring restriction
on source written in an already-specified language. Do not turn the compiler
artifacts between them into extra languages.

Equivalently, the increasingly capable language view is
`Alpha → Beta → Gamma → Delta → Omega`; the executable build view inserts the
Delta-written `omega-bootstrap` between Delta and the first production
`omega`. The optional second `omega` is a rebuild of the same source, not a
third feature inventory.

Two source-surface selections therefore discharge three required artifact
obligations. The Delta compiler must first be published through the lower-rung
Delta→Gamma route and accept both required Delta closures under one language
contract. That compiler must build `omega-bootstrap`. The bridge must compile
every program admitted by `Ωself`, exactly but not necessarily efficiently.
The resulting production `omega` must implement full Omega, including the
optimizer and advanced lowering. The last obligation is tested against the full
language and compiler suites; it is never inferred from the smaller `Ωself`
census.

`Ωself` is retained as the short symbol for the product compiler's ordinary-
Omega source profile. The required use of that profile is a cross-language
hosted build by `omega-bootstrap`; the name does not imply that this edge is an
Omega self-rebuild. Only the optional later `omega` → `omega` build is strict
self-hosting.

| Source contract | What it is | Selected from | What does **not** define it |
| --- | --- | --- | --- |
| Delta v1 | an independent literal language specification | the complete Delta source closures of the canonical Delta compiler and `omega-bootstrap`, plus explicit compiler-host coherence, robustness, safety, and maintainability arguments | D0, samples, or whatever the Rust producer happens to accept |
| `Ωself` | a compositional profile of ordinary Omega | the complete Omega source closure of the production compiler, with retain/refactor settled by measured bridge cost | Delta's features, a file allowlist, or the current compiler's exact AST permutations |

Full Omega is the already-separate product language specification. It is what
the production compiler must implement, not a third bootstrap source-profile
choice. Likewise, "conservative" versus "optimized" describes how a compiler
binary was generated, not what source language it accepts or what compiler it
contains.

There is no separate `omega-bootstrap` language inventory: its implementation
surface is Delta v1 and its accepted-source surface is `Ωself`. There is also no
bootstrap vote on which user-facing Omega features the product compiler should
implement; the full Omega specification already answers that question. The
compiler artifacts instead have the following implementation obligations:

| Compiler artifact | Written in | Accepts | Obligation |
| --- | --- | --- | --- |
| canonical Delta compiler | Delta | Delta v1 | be publishable through Delta→Gamma/Gamma without Rust and compile both required Delta source closures |
| `omega-bootstrap` | Delta | `Ωself` | compile every admitted program with exact Omega meaning; unsupported Omega rejects |
| production `omega` | Omega constrained to `Ωself` | full Omega | implement the complete language, optimizer, and lowering pipeline |
| optional rebuilt `omega` | the same `Ωself`-constrained Omega source, now compiled by production `omega` | full Omega | improve the compiler executable itself and add reproducibility evidence |

The optional rebuild is not another language rung, another compiler
implementation, or a bootstrap dependency. It recompiles the same product
source. Likewise, a conservatively generated production-compiler executable
may still contain and run the full optimizer when it compiles user programs.

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

Keep these implications one-way:

| Fact | What follows | What does not follow |
| --- | --- | --- |
| Delta omits an Omega feature | the Delta bridge source cannot use that feature | `omega-bootstrap` cannot implement that feature for accepted `Ωself` source |
| `Ωself` omits an Omega feature | the production compiler source cannot use that feature to implement itself | permission for the resulting compiler to omit that feature from full-Omega acceptance |
| `omega-bootstrap` lowers conservatively | the first production compiler executable may be slow or poorly optimized | the compiler source lacks the optimizer or advanced lowering, or the resulting compiler cannot run them on later inputs |

The first two rows are possible because compiler implementation code can parse,
check, and lower a feature using more primitive facilities than the feature
itself. The third separates the quality of the generated compiler executable
from the capabilities contained in that executable.

Here, “implements full Omega” describes the compiler's accepted language and
the meaning of the artifacts it produces. It does not require the bootstrap
source closure to contain every adjacent product tool. A standalone Terminal
Psi interpreter, REPL, proof explorer, viewer, or debugger belongs in the
closure only if the production compiler executable actually imports it.

Likewise, excluding a feature from `Ωself` is an authoring restriction, not a
request to delete its implementation from the production compiler. The
compiler source may implement that feature with ordinary records, sums,
tables, procedures, and explicit invariants that remain inside `Ωself`.

The bootstrap closure condition is therefore:

```text
the complete canonical Delta-compiler source closure ∈ Delta v1
the complete omega-bootstrap source closure ∈ Delta v1
the complete production-compiler source closure ∈ Ωself
omega-bootstrap correctly compiles every input admitted by Ωself
production omega correctly implements full Ω
```

It is not necessary for `omega-bootstrap` to accept every Omega program. Its
acceptance completeness is deliberately limited; its semantic correctness is
not. It must reject unsupported constructs rather than approximate them, and
every construct it does accept has exactly its normal Omega meaning and artifact
contract and conforms to every specified layout/ABI constraint.
Byte identity is required only where Omega pins a stable representation; the
compiler-controlled default layout need not match another conforming compiler.
General parsing, checking, and lowering rules
must implement the admitted profile; matching the present source file, statement
count, or syntax-tree permutation is not an implementation of `Ωself`.

## Delta design budget

Delta should have C-like systems power without inheriting C's undefined and
ambient behavior. Its v1 inventory is derived from the complete required Delta
source closures—the canonical Delta compiler and `omega-bootstrap`—rather than
from D0, the sample corpus, or the Rust producer. The following are candidate
tools, not facilities already voted into the language:

- fixed-width scalars, bytes, predictable aggregates, arrays, slices, and
  explicit representation;
- procedures, deterministic source bundling, loops, recursion,
  state-machine control, and payload-bearing sum data;
- explicit references or integer arena handles, checked indexing, and stable
  calling/layout conventions;
- deterministic trapping or checked arithmetic and explicit boundary I/O;
- runtime-sized allocation from fixed, bump, or paged backing, typed/indexed
  arenas, and bulk reclamation, with specified exhaustion; this may be general
  allocation from Delta's point of view without granting an ambient host heap;
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
canonical compiler or bridge source demonstrates an unavoidable requirement.
Fixed backing can be ordinary zero-initialized program storage rather than a
host service.

Discovery does not make the contract corpus-shaped. During construction, each
new facility must record either its concrete need in one of the two required
Delta programs or its explicit language-coherence, robustness, safety, or
maintainability argument, along with the simpler rejected alternative and its
lower-rung meaning and negative gates. Before freezing v1, publish both complete
source manifests and the feature inventory, remove accidental producer
behavior, then specify the retained grammar and edge cases independently of
those particular files. The working inventory lives in
the Delta rung's
[`FEATURE_LEDGER.md`](../../../bootstrap/rungs/delta/FEATURE_LEDGER.md).

## Working `Ωself` policy

The exact profile cannot be frozen before the production compiler source and
deterministic dependency manifest close. It can and should be derived
provisionally from versioned deterministic snapshots while that source is being
written. The working policy is specific enough to guide the first snapshot;
each later snapshot reruns the same feature census and retain/refactor analysis.

The first measured snapshot now exists:
[`checkpoint-000001.json`](../../../compiler/source-checkpoints/checkpoint-000001.json)
closes the product Psi source-to-token phase;
[`profile-000001.json`](../../../compiler/source-checkpoints/profile-000001.json)
mechanically binds its provisional normalized-syntax/resource admission rules,
census, canaries, and ceilings; and
[`profile-000001.md`](../../../compiler/source-checkpoints/profile-000001.md)
explains the evidence and unresolved decisions. The snapshot remains bounded
evidence for the pinned source it describes, but its fast gate currently
rejects compiled-source drift in `compiler/psi/lex/lexer.omg`,
`compiler/psi/tokens/tokens.omg`, and `omega/language/std/console.omg`, plus
provenance drift in `Cargo.lock` and
`bootstrap/onramps/omega-rust/omega/orchestration/omega-compiler/src/pipeline/stages.rs`.
That stages provider also differs from the pinned
`compiler/source-checkpoints/inputs/build-prelude.omg` snapshot. It must not be
presented as the current coherent product closure until the product owner
refreshes the manifest and profile together. The published evidence is enough
to begin evidence-led bridge work for those facilities only.
It supplies no evidence for
later parser, checker, terminal-Psi, optimizer, or emitter source needs, and it
does not settle typed semantics, ABI/layout, lowering, or bridge cost for the
general profile. A first record/array/attached-machine cluster now has private
checked-IR and lower-rooted artifact evidence; that bounded cluster does not
promote the rest of the checkpoint.

The checkpoint's present dependency replay resolves `use` components through
repository paths, and most product units omit explicit `module` items. This
records the exact provisional closure but is not the final module contract.
The bridge must consume resolver-owned logical placement from its accepted
compilation input, require any authored module declaration to agree, and apply
the normative requester-local reach and visibility rules. The final product
source checkpoint must close under those rules rather than making the bridge
preserve the Rust on-ramp's compatibility scanner.

Delta v1 and `Ωself` remain separate contracts even though their discovery can
co-evolve. Delta is derived from the cost of implementing and assuring the
complete canonical-compiler and `omega-bootstrap` closures; `Ωself` is derived
from the cost and robustness of the production compiler source. Neither
contract should be made artificially resemble the other, and no exact source
manifest is allowed to stand in for a language/profile definition.

That gives the design loop exactly two source-surface inventories:

1. What literal facilities must Delta provide so its canonical compiler and the
   bridge can both be implemented robustly?
2. Which ordinary Omega facilities may the production compiler use in its own
   source while the bridge remains tractable?

Everything else in the hosted build is implementation, validation, or optional
optimization work under those two inventories.

The current working baseline is deliberately asymmetric. It is a planning
baseline, not a premature freeze:

| Contract | Settled floor | Strong working default | Still decided by measured source/bridge cost |
| --- | --- | --- | --- |
| Delta v1 | independent, deterministic, specified, C-class compiler host with no undefined behavior or ambient authority | provide regular compiler-building data and control, deterministic storage/allocation with explicit exhaustion, and a sealed byte/artifact/diagnostic/exit boundary | the exact scalar, aggregate, slice, arena, call, module, arithmetic, and representation inventory |
| `Ωself` | ordinary Omega with exact Omega meaning; the resulting compiler still implements full Omega | retain ordinary compiler-building facilities when used—modules, named records, sums, arrays/views, ownership, mutation/control, calls, basic generics, and concrete domains—unless a measured source refactor is clearly cheaper overall | advanced or proof-coupled facilities, and any ordinary facility whose bridge/assurance cost materially exceeds its source benefit |

The burden is deliberately asymmetric. Proof-program mathematics, proof
contracts used only for internal implementation, and dependent or proof-indexed
types (including linear-dependent forms) are presumptive `Ωself` exclusions:
the compiler can implement those user-facing features without using them in its
own source. Ordinary compiler facilities are presumptive retentions once real
source uses them. Basic generics and concrete domains therefore begin on the
retention side, while domain polymorphism and advanced generic constraints are
separate cost questions. Numeric/schema field tags, mixed field-plus-case
declarations, and aggregate transition payloads remain explicit simplification
candidates. Ordinary named fields and ordinary sum data are not implicated by
those candidates.

This baseline favors the most expressive profile that remains cheap and
regular, not the smallest feature count. For an ordinary compiler facility,
retention is the default after a checkpoint demonstrates real use; exclusion
must show that a concrete refactor removes material bridge or assurance cost
without replacing it with duplicated source, invalid intermediate states,
hand-expanded variants, or private AST permutations. This is an authoring rule
while the final manifest is incomplete, not premature admission to the frozen
profile.

Route a question by the subject it changes, not by which compiler happens to
encounter it:

| Question | Governing contract or owner |
| --- | --- |
| May the canonical Delta compiler or bridge source use construct X? | Delta v1 |
| May the production compiler's own Omega source use construct X? | `Ωself` |
| Must production `omega` accept and implement Omega feature X? | the full Omega specification and product task; not a bootstrap-profile decision |
| Must `omega-bootstrap` accept arbitrary Omega source using X? | only when X is retained by the general compositional `Ωself` profile |
| Must an interpreter, viewer, REPL, proof explorer, or debugger be built? | only when the production compiler executable imports it; otherwise ordinary product-tool work |
| Must the first production compiler executable be well optimized? | no; executable quality belongs to the optional rebuild and later product optimization |

This routing is also an ownership rule. Product-source implementation and any
refactor used to remove a facility from that source live under the product
compiler task. Bootstrap work consumes the resulting checkpoint, measures and
implements the profile, and validates the hosted edge. Neither queue may create
a third language inventory to avoid coordinating those two responsibilities.

The topology itself is settled: Delta is independent rather than an Omega
subset; `Ωself` is a profile rather than a rung; production `omega` implements
full Omega; conservative generation is sufficient; and the Omega→Omega rebuild
is optional. What remains open is the exact Delta-v1 facility inventory and the
exact `Ωself` disposition, each closed from its complete sources and measured
cost rather than intuition.

Until those measurements close, keep proof-program mathematics and dependent
or proof-indexed typing, including linear-dependent forms, out of the product
compiler's own source. Retain ordinary compiler data and control when they make
that source clearer and more robust. Domain polymorphism, advanced generic
constraints, numeric schema tags, mixed record/sum declarations, and aggregate
transition payloads remain explicit simplification candidates. Ordinary named
fields are distinct from numeric tags such as `0:`, and splitting a mixed
declaration into a record and a sum is an available refactor rather than a
standing requirement.

For every disputed `Ωself` facility, use one decision procedure:

1. If the complete production-compiler closure does not use the facility,
   exclude it and add a phase-appropriate negative canary.
2. If the closure uses an ordinary compiler-building facility, provisionally
   retain its general compositional form and implement its vertical bridge
   slice. Refactor only when a concrete alternative is available to compare.
3. Retain it unless that refactor demonstrably reduces total implementation and
   assurance cost while preserving source clarity, regularity, reuse, safety,
   and valid-state modeling. Otherwise keep the ordinary facility.
4. For proof-coupled or unusually broad facilities, reverse the presumption:
   keep them excluded unless the complete source demonstrates a material need
   that a simpler ordinary encoding cannot meet.

This procedure does not reward feature removal by itself. A small general
facility is preferable to monomorphic duplication, hand-expanded variants, or
hard-coded compiler-source shapes when it lowers total cost. Conversely, a
powerful facility used only incidentally should not enter the bridge merely
because the production compiler can express itself with it.

The working feature disposition is below. These are biases for authoring and
measurement, not ratified exclusions or admissions. A checkpoint may omit and
provisionally reject a facility that its source does not use, but that absence
does not settle the final profile while later compiler phases remain unwritten.
A row becomes resolved only when the final deterministic compiler-source closure
establishes the source need or absence and the general Delta-written bridge
establishes the implementation and assurance cost.

| Omega facility in the compiler's own source | Working disposition | Decision test |
| --- | --- | --- |
| modules, imports, authored aliases, and logical source placement | presumptively retain; final placement input still open | checkpoint 000001 uses ten imports but mostly omits explicit `module` items; require resolver-owned logical placements and normative visibility rather than repository-path inference, while keeping private cross-module access blocked pending its language ruling |
| propositions, proof facts/contracts, quotients, and proof-program mathematics | avoid in new compiler source; presumptively exclude | retain only if the compiler implementation itself has an unavoidable use; implementing proof checking for user programs is not such a use |
| executable termination/ranking clauses | measure | do not conflate ranking evidence used by compiler control flow with the excluded proof surface; checkpoint 000001 already uses one ranking clause |
| dependent types and proof-indexed data/control | avoid in new compiler source; presumptively exclude | same source-need and total-cost test; implementing these features for user programs is not itself a reason to use them in compiler implementation code |
| ordinary ownership, linearity, and multiplicity | measure | distinguish routine resource discipline from dependent/proof-indexed typing; retain the ordinary form when avoiding it requires profile exceptions, unsafe encodings, or pervasive manual bookkeeping |
| concrete literal scalar ranges | measure | checkpoint 000001 uses them for fixed-buffer lengths and indexing without dependent bounds; OMGRSW5/CKIR13 demonstrates exact full-width decimal custody and semantic words for one subtraction relation, while authored range bounds and unrelated carriers remain narrower; compare general full-width representation with narrow checked helpers |
| explicit exact `u8 as u32 in Trapping` | presumptively retain; selected lower-rooted cost closed | checkpoint 000001 contains 22 pure widening casts; OMGLOWB/CKIR10 and OMGRFN12 close the compositional pure-leaf relation, exact payload preservation, least-OMGRSW1/2/3 production, conservative `movzx` emission, and immutable R1–R5 reconstruction without adding a resolver schema; this does not authorize the checkpoint's direct `u32` uses against `u64` interfaces |
| concrete `u32 in Trapping` leaf-plus-literal addition | presumptively retain; selected lower-rooted cost closed | checkpoint 000001 has 104 authored additions, of which 78 are direct canonical trapping-u32 leaf-plus-anonymous-literal forms; OMGLOWC/CKIR11 and OMGRFN13 close assignment, guard, call, and transition-argument contexts, least-OMGRSW1/2/3 production, successful near-limit meaning, runtime overflow traps, conservative emission, and immutable R1–R5 reconstruction; CKIR11 itself still stops at 2147483647, and literal-left, nested, other-carrier/policy, and calls with multiple potentially observable arguments remain outside this relation |
| direct full-width `u32 in Trapping` subtraction | presumptively retain; compiler/backend cost closed, lower-rooted admission open | checkpoint 000001 has 15 authored subtractions: OMGRSW5/OMGLOWE/CKIR13 close the two direct leaf-minus-leaf and four direct leaf-minus-literal forms in assignment, guard, call, and transition contexts, with exact `0..=4294967295` custody, independent reference meaning, runtime underflow, least-version behavior, and conservative `sub`/borrow-trap emission; nine UTF-8 subtractions nested inside multiplication/addition remain outside, while Rust-free lowering meaning and persisted-Beta OMGRFN15 R1–R5 remain open |
| remaining concrete Trapping arithmetic and casts | measure | checkpoint 000001 retains nonselected additions, nine nested UTF-8 subtractions, multiplication and other cursor/scalar arithmetic, plus 12 proof-gated narrowing or other casts; compare complete ordinary rules with narrow checked helpers, and keep implicit widening, heterogeneous comparison, signed conversion, unrelated full-width carriers, and unresolved `u32` index/cursor versus `u64` interfaces outside the closed relations |
| same-carrier primitive `<` and `<=` | presumptively retain; selected private-IR cost closed | CKIR1/CKIR3 carry exact unsigned carrier-compatible `u8`/`u32` comparisons and Boolean results; broader operands, `u64`, cross-carrier conversion, and observable operand effects remain separate |
| exact primitive `==` | presumptively retain; selected lower-rooted cost closed | checkpoint 000001 has 63 authored equality tokens—60 same-carrier `u32`, one same-carrier `u8`, one unresolved `u32`/slice-`u64`, and one payload-free-sum case—plus equality introduced by normalized transition guards; OMGLOW9/CKIR8 and OMGRFN10 close exact `bool` and same-carrier `u8`/`u32` scalar equality without claiming structural, sum, `!=`, `u64`, or cross-carrier equality |
| same-carrier primitive `>` and `>=` | presumptively retain; selected lower-rooted cost closed | checkpoint 000001 has 34 same-carrier `u32 >=` uses and one same-carrier `u32 >` use; OMGLOWA/CKIR9 and OMGRFN11 close exact unsigned same-carrier `u8`/`u32` authored-order operations, source/CKIR meaning, conservative SETA/SETAE emission, and immutable R1–R5 composition without swapping operands or inventing an upper-bound fact; two slice-length `u64 >` uses, cross-carrier conversion, effectful operands, and general transition-fact refinement remain outside this selected relation |
| bool-only prefix logical negation | presumptively retain; selected lower-rooted cost closed | checkpoint 000001 uses ordinary `!` in product lexer state; OMGLOW7/CKIR6 and OMGRFN8 now measure least-version OMGRSW1/2/3 production, exact Boolean meaning through independent source and CKIR evaluators plus the Rust-free route, conservative native/self artifact emission, and one immutable R1–R5 source-to-artifact composition without adding a resolution schema |
| pure, total, nontrapping bool-only `&&` and `||` | presumptively retain; selected lower-rooted cost closed | checkpoint 000001 uses 38 conjunctions and 6 disjunctions over primitive predicate leaves; OMGLOW8/CKIR7 and OMGRFN9 measure exact `&&`-before-`||` precedence, left association, token/operation custody, all truth rows, least-version OMGRSW1/2/3, Rust-free lowering meaning, conservative native/self emission, and immutable R1–R5 composition; calls, indexing, Trapping arithmetic, mutation, and other observable skipped work remain outside this eager private-IR relation |
| ordinary named record fields | presumptively retain | the frontend probe and closed CKIR3/CKIR4 tranches establish checking, nominal layout, aggregate copy, runtime declaration-order construction, structural Call/Copy, Rust-free meaning, independent result/ELF reconstruction, adjacent resource teeth, and lower-rooted same-frame composition for the selected `source.omg` dependency; compare this measured cost with the clarity and regularity loss from positional compiler data |
| fixed arrays and checked indexing | measure | the same probe closes general frontend rules and guarded-index obligations through length 65,536, and the selected private checked-IR tranche measures direct layout/lowering; final ordinary indexing still needs the `u32` product indexes reconciled with core `Array::index(..., index: u64)`, then compare the total cost with arena/library encodings |
| borrowed slices and byte-string literals | presumptively retain; selected static shared-view cost closed | checkpoint 000001 uses shared `&[u8]`, mutable `&mut [u8]`, `.len`, guarded indexing, tail subslicing, and differently sized keyword literals; OMGRSW4/CKIR12/OMGRFN14 close exact 0–32-byte plain-ASCII literals, immutable descriptor transport, `.len > 0`, `[0]`, and `[1..]` with a deferred nonempty true edge; compare the remaining general facility with fixed-buffer-plus-span duplication while keeping mutable/dynamic views, allocation, UTF-8, general indexing, and the `u32` cursor versus `u64` interface ruling separate |
| payload-bearing enums/sum data | presumptively retain | compare direct syntax/IR modeling with separate explicit-tag records; splitting is a cost option, not a prior ruling |
| state machines, state parameters, mutation, calls, and explicit result fields | presumptively retain for the observed finite forms | checkpoint 000001 expresses every lexical loop with this surface; widen from the closed finite call tranches compositionally, while continuing to exclude observable argument-order combinations and implicit branching value results until their separate rules are settled |
| boundary traits, target-qualified/bodyless machines, `satisfies`, and compiler-intrinsic realizations | measure the exact sealed product forms | checkpoint 000001 contains one boundary trait, 20 target-qualified machines, 18 `satisfies` clauses, 16 bodyless leaves, and 16 compiler-intrinsic realizations; price that product source cluster without importing general boundary traits into Delta's separately sealed host interface; a one-requirement fixture measures the relation but does not establish complete conformance for the six-requirement product `Console` |
| static provider path arguments and default selection | measure from checkpoint 000001; exact structural carrier closed | OMGCOMP2 binds an exact Linux-x86-64/native-provider two-package, three-source fixture while leaving provider spellings opaque; its reduced `Console` has only `exit_process`, so it cannot prove the complete product provider plan, and the product's current `Owner::provider_defaults` declaration convention still needs a normative source ruling or refactor to the specified `Build::select_provider` form; replacement-cohort closure after a completed selection is now specified, but source selection, realization, lowering, admission, and authority remain open, and the checkpoint is not evidence for general generics |
| basic generic declarations and calls | presumptively retain when used; not yet observed | collection, result, arena-ID, and compiler-data reuse versus monomorphic duplication; require a later checkpoint with actual declarations before implementing or admitting the general bridge surface |
| generated ordinary-Omega data and pinned generators | presumptively retain closure rules | checkpoint 000001 imports generated Unicode range arrays; bind generated source, generator, and external data as deterministic inputs while treating the arrays as ordinary admitted Omega rather than a private compiler exception |
| concrete domains and domain arithmetic | presumptively retain when used; not yet observed | retain ordinary named domains when a later checkpoint uses them to keep arithmetic or compiler contexts regular; compare unusually broad domain machinery with explicit contexts and narrow operations |
| domain polymorphism | measure | admit only the forms used by the closed source manifest |
| advanced authored generic constraints | measure | source benefit versus bridge and assurance cost |
| specialization and reflection | no source-profile ruling yet | neither has a distinct accepted authored source spelling to admit or exclude; add a row when there is a censusable Omega surface |
| numeric/schema field tags such as `0:` | measure | compare with ordinary named fields; these are distinct from named record fields and may be omitted without making records positional |
| complex transition payloads | measure | compare with transitions over simple values plus explicit compiler context |
| mixed field-plus-case data | measure | compare with separate record and sum-data types; either shape remains ordinary Omega |

This table is the single working inventory for those choices. In particular,
the bootstrap task board must not sprout separate “remove proofs,” “remove
generics,” or “split every enum” projects. A facility becomes bridge work only
when a product checkpoint uses it or a retained general profile rule requires
it; a source refactor remains product work. Final retain/exclude decisions occur
at the completed source-and-bridge join, not by intuition ahead of either
implementation.

The table intentionally does not aim for the smallest possible source profile.
Named fields, basic generics, domains, payload sums, ordinary resource
linearity, or richer transitions may remain when their general bridge
implementation is cheaper and safer than the duplication or manual encodings
needed to avoid them. Proof-program mathematics and dependent/proof-indexed
typing are strong exclusion candidates because the compiler can implement
those user-facing features without expressing its own algorithms in them.
Every row still resolves by the same measured whole-bootstrap cost test; none
is a ruling merely because it appears in this table.

Current closed cost evidence is summarized here; schemas, fixtures, byte
contracts, mutation matrices, and version history remain beside their owning
gates rather than being repeated in this decision document.

| Selected capability evidence | Closed bounded path | Detail owner |
| --- | --- | --- |
| finite calls, constant aggregates, runtime named records, and same-module direct-field receivers | source production through CKIR4, Rust-free meaning, conservative artifacts, and lower-rooted reconstruction | [`SOURCE_CUSTODY_FRONTEND_PROBE.md`](../../../bootstrap/omega-bootstrap/compiler/SOURCE_CUSTODY_FRONTEND_PROBE.md) and versioned checked-IR contracts under [`bootstrap/omega-bootstrap/compiler/`](../../../bootstrap/omega-bootstrap/compiler/) |
| payload-bearing pure sums | OMGRSW3, CKIR5, conservative backend, and OMGRFN7 R1–R5 | versioned checked-IR contracts and [`OMGCOMP_REFINEMENT_WITNESS_V7.md`](../../../bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V7.md) |
| bool-only logical negation | CKIR6 and OMGRFN8 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V6.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V6.md) and [`OMGCOMP_REFINEMENT_WITNESS_V8.md`](../../../bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V8.md) |
| pure, total, nontrapping bool-only `&&` and `||` | CKIR7 and OMGRFN9 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V7.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V7.md) and [`OMGCOMP_REFINEMENT_WITNESS_V9.md`](../../../bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V9.md) |
| exact primitive same-carrier `bool`/`u8`/`u32` equality | CKIR8 and OMGRFN10 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V8.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V8.md) and [`OMGCOMP_REFINEMENT_WITNESS_V10.md`](../../../bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V10.md) |
| pure, total, nontrapping same-carrier unsigned `u8`/`u32` `>` and `>=` | CKIR9 and OMGRFN11 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V9.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V9.md) and [`OMGCOMP_REFINEMENT_WITNESS_V11.md`](../../../bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V11.md) |
| explicit exact pure-leaf `u8 as u32 in Trapping` | CKIR10 and OMGRFN12 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V10.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V10.md) and [`OMGCOMP_REFINEMENT_WITNESS_V12.md`](../../../bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V12.md) |
| canonical `u32 in Trapping` leaf-plus-anonymous-literal addition | CKIR11 and OMGRFN13 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V11.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V11.md) and [`OMGCOMP_REFINEMENT_WITNESS_V13.md`](../../../bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V13.md) |
| program-static shared byte views with a guarded head and one-byte tail | OMGRSW4, OMGLOWD/CKIR12, conservative backend, and OMGRFN14 R1–R5 | [`OMEGA_BOOTSTRAP_RESOLUTION_V4.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V4.md), [`OMEGA_BOOTSTRAP_CHECKED_IR_V12.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V12.md), and [`OMGCOMP_REFINEMENT_WITNESS_V14.md`](../../../bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V14.md) |
| generated ordinary-Omega source custody for the exact Unicode tuple | sealed locked/offline two-run reproduction, generic provenance roles, bounded/no-publication teeth, exact OMGCOMP1 source extent, and CKIR3/OMGRFN4 preflight composition | [`OMEGA_BOOTSTRAP_GENERATED_SOURCE_CUSTODY.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_GENERATED_SOURCE_CUSTODY.md); the product-owned checkpoint refresh over the same tuple remains open |

Every row is implementation-and-assurance cost evidence for a selected slice.
No row admits a facility to final `Ωself`, claims general checkpoint coverage,
or widens beyond the boundary stated in its linked contract.

Direct full-width subtraction is deliberately not in that closed-evidence
table yet. OMGRSW5/OMGLOWE/CKIR13 and its conservative backend close the
compiler-side cost relation, and the focused OMGRFN15 candidate freezes its
frame/reference boundary. Rust-free lowering meaning, persisted-Beta R1–R5,
and byte-complete ELF reconstruction remain open, so OMGRFN14 is still the
latest admitted lower-rooted carrier.

Structural multi-unit custody is separately closed by
[`OMEGA_BOOTSTRAP_COMPILATION.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_COMPILATION.md)
and the exact target/configuration successor
[`OMEGA_BOOTSTRAP_COMPILATION_V2.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_COMPILATION_V2.md).
The bounded Delta
[`SHA-256 producer`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_SHA256.md)
also closes exact hashing of any raw envelope through their public ceiling.
None grants source semantics, provider selection, the independently expected
commitment, or accepted-lock authority; that join remains an external
compilation-authority dependency rather than an `Ωself` feature.

The OMGCOMP2 provider fixture deliberately reduces `Console` to one requirement.
It may support a bounded requirement/realization/call-resolution witness, but
the real standard-library trait contains six requirements and selection admits
only a complete candidate. The fixture therefore cannot establish product
provider closure by itself. In addition, the language guide specifies explicit
`Build::select_provider<Service, Provider>` overrides and target-package
defaults, but not the current product source's `Owner::provider_defaults`
declaration convention. Bootstrap work must preserve that distinction instead
of elevating Rust on-ramp recognition rules into Omega semantics.
The language guide's replacement-cohort rule does settle what happens after a
selection: executable conformances and concrete runtime identities pull fused
consumers into one transitive cohort, while independent replacement crosses an
independently selected boundary requirement. It does not define this legacy
default declaration, its target scope, or its precedence.

Source-unit membership is a separate question from language features.
Standalone terminal-Psi tools, interpreters, REPLs, proof explorers, viewers,
debuggers, and other product tooling are presumptively outside the hosted
closure unless the compiler executable imports them.

The current product architecture does use Terminal Psi as the target-neutral
compiler boundary. Consequently, the representation and lowering modules the
compiler links are ordinary members of the product source manifest. This does
not imply that a standalone Terminal-Psi interpreter, artifact viewer, or debug
tool belongs to the manifest, nor that `omega-bootstrap` must use or validate
Terminal Psi as its own internal IR. The bridge must compile those product
modules as ordinary `Ωself` source; it need not duplicate or execute their
product role. A direct bridge-specific checked IR and conservative lowering are
equally valid when they reduce total bootstrap and assurance cost.

Checkpoint 000001's compiler-produced snapshot-v3/census-v2 profile establishes
that the current closure uses target-qualified and bodyless machines,
`satisfies` clauses, and sealed compiler-intrinsic bindings. It also establishes
a useful negative source convention:
branching computations publish explicit result fields before callers dispatch.
The closure does not require implicit branching value-machine result
materialization, dependent bounds, proof syntax, mixed field-plus-case data, or
inline aggregate transition literals. Aggregate-typed transition names and
calls still require typed census facts. Those absences are provisional profile
evidence, not full-Omega feature removals.

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

Frontend cost measurement and final profile settlement are separate milestones.
A bounded parser/typechecker probe may establish the real cost of a provisional
candidate without yet lowering it or settling its final `Ωself` disposition.
Final retention requires the general accepted-source rule, negative boundary,
Rust-free meaning, and chosen artifact path together. This separation prevents
an exploratory source unit from forcing Terminal Psi or any other product
representation into the bridge.

The selection rule is total cost, not the smallest feature count:

```text
benefit and robustness in the production Omega source closure
──────────────────────────────────
implementation + assurance cost in the required Delta compiler/bridge source closures
```

Payload sums, basic generics, and concrete domains are favorable ordinary
compiler facilities when real source uses them, but checkpoint 000001 currently
proves only static provider path arguments, not general generic declarations or
authored domain use. Proof syntax and dependent typing are not favorable
defaults. This is a total-cost profile, not a contest to remove the most
features: retaining a cheap general facility is preferable to forcing large,
brittle, monomorphic compiler source. Profile growth or a product-source closure
change reopens the profile and must update the rules, compiler, meaning route,
diagnostics, and negative gates together.

## One required hosted production build

`omega-bootstrap` may itself be a slow binary and may lower `main.omg`
conservatively. It must understand enough `Ωself` to compile the source that
*implements* the production optimizer and advanced lowering; it does not need
to run those product passes during this build. The required hosted result is a
full-spec compiler containing the production optimizer and advanced lowering
pipeline, although that compiler's own machine code may still be conservatively
generated.

```text
Delta compiler source ──[Delta→Gamma + Gamma execution]──▶ delta compiler
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

The production compiler task must publish a deterministic source/dependency
manifest at each coherent source checkpoint. This is a staged discovery loop,
not a circular build dependency: the product source can be authored under the
working policy while the Delta bridge and ledger evolve against versioned
provisional closures. `Ωself` can be derived and enforced provisionally from the
first complete checkpoint; neither the final file set nor final profile must be
pretended frozen at that point. Its cost claims cannot be final until the
general bridge implementation exists. Keep provisional derivation and the
freeze join as distinct milestones:

1. Write a coherent product-compiler source checkpoint under the conservative
   working policy and publish its complete deterministic transitive closure.
2. Derive or update provisional `Ωself`; measure each used feature against its
   benefit in that source snapshot and record the Delta implementation/assurance
   cost that is known or still missing.
3. Provisionally retain a general compositional form, refactor the product source
   and retain a negative canary, or leave the facility explicitly unresolved.
4. Implement and assure the retained candidate rules in `omega-bootstrap`,
   feeding measured cost back into the retain/refactor decision. A provisional
   retention is not a final admission until this evidence exists.
5. Repeat for later source checkpoints and resolve every remaining row. At the
   completed product-source/bridge join, freeze both publications: `Ωself`
   from the final product manifest plus measured bridge cost, and Delta v1 from
   the final canonical-compiler and bridge closures plus its explicit
   compiler-host arguments. The
   already-running mechanical enforcement becomes the frozen `Ωself`
   acceptance gate; the Delta conformance and lower-rung gates become the
   frozen Delta-v1 acceptance gates.

This process classifies facilities, not individual occurrences. A retained
facility is implemented compositionally for every source admitted by its
published bounds. An excluded facility receives a phase-appropriate negative
canary. The exact compiler manifest is then a closure witness over those rules,
not a special case accepted by them.

The final bootstrap gate must compile that exact closure under the explicit
`Ωself` profile rules and reject an excluded-feature canary for every exclusion.
Checkpoint 000001 now binds its manifest to a separately hashed provisional
normalized-syntax/resource profile, every-target compiler census, valid-Omega
positive and negative admission canaries, rounded ceilings, and mutation teeth.
That artifact explicitly leaves typed semantic distinctions, ABI/layout,
lowering coverage, Delta capacity behavior, and measured bridge costs for the
general checkpoint unresolved rather than presenting syntax census—or the
first bounded artifact cluster—as broader evidence.
The manifest includes compiler modules, compile-time code, build/module
behavior, and runtime/library dependencies; hiding a feature in a library does
not remove it from the bootstrap surface.

Delta's provisional ledger evolves while the canonical compiler and
`omega-bootstrap` are written. At the joint settlement, prune accidental
producer/corpus behavior, publish the general Delta v1 grammar and semantics,
and prove both exact Delta closures valid under it. The Omega product-source
manifest plus measured bridge cost decide `Ωself`; the two required Delta
source manifests plus explicit compiler-host coherence arguments decide Delta
v1. The two contracts remain separately scoped and versioned, but their freeze
is one milestone rather than sequential language rungs or a circular build
dependency. No exact manifest substitutes for the corresponding general
profile or language contract.

The current Rust Psi/Omega compiler remains a maintained reference and
differential producer while useful. It is neither a bootstrap dependency nor an
authority source: lower-rooted refinement and canonical meaning decide
acceptance. Cross-compiler diagnostics, normalized IR, artifacts, and execution
observations remain valuable bug-finding evidence.
