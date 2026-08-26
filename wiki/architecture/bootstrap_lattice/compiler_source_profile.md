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

Use “spec-compliant” only with its subject named. Delta conforms to the Delta
specification. `omega-bootstrap` is deliberately incomplete in Omega input
coverage but must compile every admitted `Ωself` program according to the full
Omega meaning of the forms it accepts. The produced `omega` is the compiler
that must accept and implement the full Omega specification. Separately, that
compiler's own executable may have conservative machine code. Conflating these
four claims recreates the discarded Omega0/Omega1 ambiguity.

## Decision status

The architecture is settled; only the exact contents of the two source
contracts remain open:

| Settled | Still evidence-driven |
| --- | --- |
| Delta is an independent language, not restricted to valid Omega. | The exact Delta-v1 grammar and facilities needed to implement the canonical Delta compiler and `omega-bootstrap` robustly. |
| `Ωself` is ordinary valid Omega with features rejected, never altered meaning or private syntax. | The exact ordinary-Omega facilities used by the completed production compiler source after measured retain/refactor decisions. |
| `omega-bootstrap` is written in Delta, accepts `Ωself`, and may lower conservatively. | The complete general bridge implementation and its resource limits. |
| The first bridge-built `omega` already implements full Omega, including advanced features omitted from its own source. | The complete product source closure and proof that the resulting compiler covers the full specification. |
| A later `omega` → `omega` rebuild is optional executable optimization and reproducibility evidence. | Whether that optional rebuild is worth doing for a release; it cannot change bootstrap closure. |
| Standalone interpreters, viewers, REPLs, proof explorers, and debuggers are outside the required closure unless imported by the compiler. | Which representation, checking, lowering, and support modules the final compiler executable actually imports. |

The current authoring bias is settled enough to guide implementation without
prematurely freezing either contract:

| Surface | Presumption | Reason |
| --- | --- | --- |
| Delta v1 | provide robust deterministic C-class compiler power: regular data/control/modules, explicit failure, and deterministic bounded storage or allocation | Delta exists to make two large compilers maintainable, not to win a token-count contest |
| `Ωself`: proof-program mathematics and dependent/proof-indexed typing, including linear-dependent forms | exclude unless the compiler source demonstrates a concrete implementation need | production `omega` can implement these facilities for users without using them to implement itself |
| `Ωself`: named records, sums, arrays/views, modules, calls, ordinary ownership, basic generics, and concrete domains | retain when used unless a concrete refactor lowers total source, bridge, and assurance cost | these are ordinary compiler-building tools; removing them can create duplication and invalid intermediate states |
| `Ωself`: numeric/schema field tags such as `0:`, mixed field-plus-case declarations, advanced generic/domain machinery, and aggregate transition payloads | measure against simpler ordinary-Omega encodings | these can sometimes be removed without making all data positional or giving the compiler a private dialect |
| hosted source closure | include only modules transitively imported by the compiler executable | implementing full Omega does not require bundling standalone Terminal-Psi interpreters, REPLs, proof explorers, viewers, or debuggers |

These are disposition defaults, not final admissions. A later complete source
checkpoint supplies usage evidence; the general bridge and assurance path
supplies cost evidence. Final `Ωself` decisions use both.

## Closure rules

The names are deliberately non-generational. `omega-bootstrap` is a compiler
role, not “Omega 0”; production `omega` is not “Omega 1”; and O0/O1 remain only
historical names for frozen bridge canaries. Delta is the independent source
language of the bridge. `Ωself` is a compositional feature-and-resource profile
of ordinary Omega with no syntax or semantics of its own. Full Omega is the
language implemented by the resulting production compiler.

Keep these implications one-way:

| Fact | What follows | What does not follow |
| --- | --- | --- |
| Delta omits an Omega feature | the Delta bridge source cannot use that feature | `omega-bootstrap` cannot implement that feature for accepted `Ωself` source |
| `Ωself` omits an Omega feature | the production compiler source cannot use that feature to implement itself | permission for the resulting compiler to omit that feature from full-Omega acceptance |
| `omega-bootstrap` lowers conservatively | the first production compiler executable may be slow or poorly optimized | the compiler source lacks the optimizer or advanced lowering, or the resulting compiler cannot run them on later inputs |

Compiler implementation code can parse, check, and lower a facility without
using that facility in its own implementation. Therefore an `Ωself` exclusion
is only an authoring restriction; it never requests removal from full Omega.
Likewise, “implements full Omega” governs accepted source and artifact meaning,
not adjacent tools. A standalone Terminal-Psi interpreter, REPL, proof
explorer, viewer, or debugger enters the closure only when the production
compiler executable imports it.

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
[`FEATURE_LEDGER.md`](../../../bootstrap/delta/FEATURE_LEDGER.md).

## Working `Ωself` policy

The exact profile cannot be frozen before the production compiler source and
deterministic dependency manifest close. It can and should be derived
provisionally from versioned deterministic snapshots while that source is being
written. The working policy is specific enough to guide the first snapshot;
each later snapshot reruns the same feature census and retain/refactor analysis.

The first measured snapshot now exists:
[`checkpoint-000001.json`](../../../source/compiler/omega/source-checkpoints/checkpoint-000001.json)
closes the product Psi source-to-token phase;
[`profile-000001.json`](../../../source/compiler/omega/source-checkpoints/profile-000001.json)
mechanically binds its provisional normalized-syntax/resource admission rules,
census, canaries, and ceilings; and
[`profile-000001.md`](../../../source/compiler/omega/source-checkpoints/profile-000001.md)
explains the evidence and unresolved decisions. The snapshot remains bounded
evidence for the pinned source it describes. Its manifest, profile,
Cargo/provider provenance, and extracted build prelude were refreshed
coherently, and its complete gate compares the product compiler's versioned
structural lexical observation with an independently encoded Rust observation
across success, retained-prefix rejection, and capacity cases. The fast gate
rejects later compiled-source, provenance, prelude, feature, or resource drift
until the evidence set is refreshed again. The current evidence is coherent;
the published snapshot is enough to begin evidence-led bridge work for those
facilities only.
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
recreate the deleted Rust on-ramp dependency scanner.

The two contracts may co-evolve, but the [decision-status table](#decision-status)
continues to govern their separate evidence and asymmetric working defaults.
Neither an exact manifest nor a nearby bridge canary defines either contract.

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

The final disposition ledger records four separate facts for each disputed
facility: whether the production compiler source uses it, whether
`omega-bootstrap` accepts it, whether production `omega` implements it for
users, and whether any adjacent tool using it belongs to the transitive
compiler closure. These columns must not collapse into one "supported" bit.
In particular, an `Ωself` exclusion requires bridge rejection and product
acceptance evidence; it never licenses the production compiler to omit the
feature. The authoritative ledger is published at the completed source/bridge
join, while checkpoint tables remain provisional evidence.

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
| concrete literal scalar ranges | measure; selected full-width custody cost closed | checkpoint 000001 uses them for fixed-buffer lengths and indexing without dependent bounds; OMGRSW8/OMGLOWH/CKIR16/OMGRFN18 close four-word inclusive `u64` endpoints and a selected true-edge upper-bound fact for direct `<`, including unsigned predecessor borrow. This is not general range, indexing, arithmetic, or dependent-type support; compare the remaining general representation with narrow checked helpers while keeping authored bounds and unrelated carriers separate |
| explicit exact `u8 as u32 in Trapping` | presumptively retain; selected lower-rooted cost closed | checkpoint 000001 contains 22 pure widening casts; OMGLOWB/CKIR10 and OMGRFN12 close the compositional pure-leaf relation, exact payload preservation, least-OMGRSW1/2/3 production, conservative `movzx` emission, and immutable R1–R5 reconstruction without adding a resolver schema; collection coordinates now use `u64` directly, so this row licenses no implicit cross-carrier use |
| concrete `u32 in Trapping` leaf-plus-literal addition | presumptively retain; selected lower-rooted cost closed | checkpoint 000001 has 104 authored additions, of which 78 are direct canonical trapping-u32 leaf-plus-anonymous-literal forms; OMGLOWC/CKIR11 and OMGRFN13 close assignment, guard, call, and transition-argument contexts, least-OMGRSW1/2/3 production, successful near-limit meaning, runtime overflow traps, conservative emission, and immutable R1–R5 reconstruction; CKIR11 itself still stops at 2147483647, and literal-left, nested, other-carrier/policy, and calls with multiple potentially observable arguments remain outside this relation |
| pure same-carrier full-width `u32 in Trapping` `+`, `-`, and `*` composition | presumptively retain; selected cost closed through persisted lower-rooted OMGRFN16 | OMGRSW7/OMGLOWF/CKIR14/OMGRFN16 close the two-, three-, and four-byte UTF-8 trees as one bounded recursive relation with ordinary precedence, left association, parentheses, exact high-word literals and widening leaves, operation-by-operation first traps, representative assignment/guard/Call/CaseDispatch contexts, complete ordered pure siblings, inherited CKIR12 view composition, and independent native/self R1–R5 reconstruction. The implementation and gates are tree-shaped rather than compiler-file/context permutations. One potentially trapping computation is admitted per call or transition argument list while every sibling remains pure/nontrapping; effectful leaves, multiple observable trap sites, unrelated carriers/policies, division, and remainder remain excluded |
| remaining concrete Trapping arithmetic and casts | measure | outside those UTF-8 trees checkpoint 000001 retains six subtraction nodes, other nonselected additions and cursor/scalar arithmetic, plus 12 proof-gated narrowing or other casts; byte/scalar work remains `u32` while source coordinates and collection counts now use same-carrier `u64`; compare complete ordinary rules with narrow checked helpers, and keep implicit widening, heterogeneous comparison, signed conversion, and unrelated full-width carriers outside the closed relations |
| same-carrier primitive `<` and `<=` | presumptively retain; selected lower-rooted cost closed | CKIR1/CKIR3 carry exact unsigned carrier-compatible `u8`/`u32` comparisons and Boolean results. OMGRSW8/OMGLOWH/CKIR16/OMGRFN18 close direct pure full-width same-carrier `u64 < u64`, exact high/low halves, conservative unsigned emission, and true-edge range custody. `u64 <=`, other comparisons, computed or effectful operands, mixed carriers, arithmetic, and dynamic indexing remain separate |
| exact primitive `==` | presumptively retain; selected lower-rooted cost closed | checkpoint 000001 uses same-carrier equality for `u32` decoding/scalars, `u8`, payload-free sums, and `u64` coordinates/lengths; the former cross-carrier coordinate/`.len` comparison is gone. OMGLOW9/CKIR8 and OMGRFN10 close exact `bool` and same-carrier `u8`/`u32` scalar equality without claiming structural, sum, `!=`, `u64`, or cross-carrier equality; the `u64` source-profile cost remains to be measured separately |
| same-carrier primitive `>` and `>=` | presumptively retain; selected lower-rooted cost closed | checkpoint 000001 uses these relations across decoding scalars and same-carrier `u64` slice coordinates. OMGLOWA/CKIR9 and OMGRFN11 close exact unsigned same-carrier `u8`/`u32` authored-order operations, source/CKIR meaning, conservative SETA/SETAE emission, and immutable R1–R5 composition without swapping operands or inventing an upper-bound fact; `u64`, effectful operands, and general transition-fact refinement remain outside this selected relation |
| bool-only prefix logical negation | presumptively retain; selected lower-rooted cost closed | checkpoint 000001 uses ordinary `!` in product lexer state; OMGLOW7/CKIR6 and OMGRFN8 now measure least-version OMGRSW1/2/3 production, exact Boolean meaning through independent source and CKIR evaluators plus the Rust-free route, conservative native/self artifact emission, and one immutable R1–R5 source-to-artifact composition without adding a resolution schema |
| pure, total, nontrapping bool-only `&&` and `||` | presumptively retain; selected lower-rooted cost closed | checkpoint 000001 uses 38 conjunctions and 6 disjunctions over primitive predicate leaves; OMGLOW8/CKIR7 and OMGRFN9 measure exact `&&`-before-`||` precedence, left association, token/operation custody, all truth rows, least-version OMGRSW1/2/3, Rust-free lowering meaning, conservative native/self emission, and immutable R1–R5 composition; calls, indexing, Trapping arithmetic, mutation, and other observable skipped work remain outside this eager private-IR relation |
| ordinary named record fields | presumptively retain | the frontend probe and closed CKIR3/CKIR4 tranches establish checking, nominal layout, aggregate copy, runtime declaration-order construction, structural Call/Copy, Rust-free meaning, independent result/ELF reconstruction, adjacent resource teeth, and lower-rooted same-frame composition for the selected `source.omg` dependency; compare this measured cost with the clarity and regularity loss from positional compiler data |
| fixed arrays and checked indexing | measure; next direct cost slice selected | the same probe closes general frontend rules and guarded-index obligations through length 65,536, and the selected private checked-IR tranche measures direct layout/lowering; checkpoint 000001 now uses the core `u64` index carrier for fixed arrays. Measure the next vertical slice against `SourceUnit::append` and `byte_or_nul`: guarded full-width `u64` load/store indexing plus direct trapping leaf-plus-literal increment, with computed/effectful indexes, multiple observable traps, mutable slices, and unrelated `u64` operations kept separate. Then compare the remaining total cost with arena/library encodings |
| borrowed slices and byte-string literals | presumptively retain; selected recurrent shared-view cost closed | checkpoint 000001 uses shared `&[u8]`, mutable `&mut [u8]`, `.len`, guarded `u64` indexing, tail subslicing, and differently sized keyword literals; OMGRSW4/CKIR12/OMGRFN14 close exact 0–32-byte plain-ASCII literals and one guarded static head/tail, while CKIR15/OMGRFN17 generalize the direct shared-byte-view relation to a runtime-capable machine parameter, recurrent guarded head/tail, and exact ordered pass-through vectors on both edges. CKIR16/OMGRFN18 separately close the direct full-width `u64 < u64` guard and true-edge range prerequisite; compare the remaining general facility with fixed-buffer-plus-span duplication while keeping mutable views, dynamic indexing, computed/effectful siblings, allocation, UTF-8, and other same-carrier `u64` collection operations separate |
| payload-bearing enums/sum data | presumptively retain | compare direct syntax/IR modeling with separate explicit-tag records; splitting is a cost option, not a prior ruling |
| state machines, state parameters, mutation, calls, and explicit result fields | presumptively retain for the observed finite forms | checkpoint 000001 expresses every lexical loop with this surface; widen from the closed finite call tranches compositionally, while continuing to exclude observable argument-order combinations and implicit branching value results until their separate rules are settled |
| boundary traits, target-qualified/bodyless machines, `satisfies`, and compiler-intrinsic realizations | measure the exact sealed product forms; selected `Console` plan and checked-adapter cost closed | checkpoint 000001 contains one boundary trait, 20 target-qualified machines, 18 `satisfies` clauses, 16 bodyless leaves, and 16 compiler-intrinsic realizations. OMGRSW9/OMGLOWI18/CKIR17/OMGRFN20 close the selected six-requirement `Console` plan plus receiverless execution of its two checked adapters and ranked helper as ordered abstract byte events. Four compiler-intrinsic leaves remain plan custody only. Provider admission, native boundary effects, and the remaining general product surface stay separate; none imports general boundary traits into Delta's sealed host interface |
| static provider path arguments and explicit build selection | measure from checkpoint 000001; exact build-role, complete-plan, and platform-neutral checked-execution cost closed | OMGCOMP3 marks exactly one root-package build source without deriving the role from a filename, readable machine name, declaration order, provider defaults, or candidate uniqueness. OMGRSW9 harvests the explicit `Build::select_provider<Console, ConsoleNativeProvider>` only from that role and retains the complete six-requirement plan; OMGLOWI18/CKIR17/OMGRFN20 preserve the selected identities through abstract execution. Selection is not admission: installed-provider authority, the accepted-closure join, and native effects remain open. This checkpoint is not evidence for general generics or a ruling for target-package defaults outside this closure |
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
| payload-bearing pure sums | OMGRSW3, CKIR5, conservative backend, and OMGRFN7 R1–R5 | versioned checked-IR contracts and [`OMGCOMP_REFINEMENT_WITNESS_V7.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V7.md) |
| bool-only logical negation | CKIR6 and OMGRFN8 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V6.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V6.md) and [`OMGCOMP_REFINEMENT_WITNESS_V8.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V8.md) |
| pure, total, nontrapping bool-only `&&` and `||` | CKIR7 and OMGRFN9 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V7.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V7.md) and [`OMGCOMP_REFINEMENT_WITNESS_V9.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V9.md) |
| exact primitive same-carrier `bool`/`u8`/`u32` equality | CKIR8 and OMGRFN10 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V8.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V8.md) and [`OMGCOMP_REFINEMENT_WITNESS_V10.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V10.md) |
| pure, total, nontrapping same-carrier unsigned `u8`/`u32` `>` and `>=` | CKIR9 and OMGRFN11 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V9.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V9.md) and [`OMGCOMP_REFINEMENT_WITNESS_V11.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V11.md) |
| explicit exact pure-leaf `u8 as u32 in Trapping` | CKIR10 and OMGRFN12 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V10.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V10.md) and [`OMGCOMP_REFINEMENT_WITNESS_V12.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V12.md) |
| canonical `u32 in Trapping` leaf-plus-anonymous-literal addition | CKIR11 and OMGRFN13 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V11.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V11.md) and [`OMGCOMP_REFINEMENT_WITNESS_V13.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V13.md) |
| program-static shared byte views with a guarded head and one-byte tail | OMGRSW4, OMGLOWD/CKIR12, conservative backend, and OMGRFN14 R1–R5 | [`OMEGA_BOOTSTRAP_RESOLUTION_V4.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V4.md), [`OMEGA_BOOTSTRAP_CHECKED_IR_V12.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V12.md), and [`OMGCOMP_REFINEMENT_WITNESS_V14.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V14.md) |
| recursive pure full-width trapping-`u32` `+`/`-`/`*` | OMGRSW7, OMGLOWF/CKIR14, independent meaning, conservative backend, and persisted lower-rooted OMGRFN16 R1–R5 | [`OMEGA_BOOTSTRAP_RESOLUTION_V7.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V7.md), [`OMEGA_BOOTSTRAP_CHECKED_IR_V14.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V14.md), and [`OMGCOMP_REFINEMENT_WITNESS_V16.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V16.md) |
| recurrent guarded head/tail over a runtime-capable direct shared-byte view | OMGRSW4, OMGLOWG/CKIR15, exact ordered direct-binder pass-through vectors, runtime-only/no-static-root custody, independent meaning, conservative backend, and persisted lower-rooted OMGRFN17 R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V15.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V15.md), [`OMEGA_BOOTSTRAP_LOWERING_V16.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_LOWERING_V16.md), and [`OMGCOMP_REFINEMENT_WITNESS_V17.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V17.md) |
| direct pure full-width same-carrier `u64 < u64` with true-edge range custody | OMGRSW8, OMGLOWH/CKIR16, exact paired-word meaning, conservative unsigned backend, and persisted lower-rooted OMGRFN18 R1–R5 | [`OMEGA_BOOTSTRAP_RESOLUTION_V8.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V8.md), [`OMEGA_BOOTSTRAP_CHECKED_IR_V16.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V16.md), and [`OMGCOMP_REFINEMENT_WITNESS_V18.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V18.md) |
| complete static product `Console` provider plan | OMGRSW9 retains the authoritative build selection, all six requirements, two checked adapters, four Linux-x64 intrinsic leaves, complete plan rows, and requirement-targeted calls; OMGRFN19 independently reconstructs the structural relation with persisted-Beta projections and the actual native/self producer bytes | [`OMEGA_BOOTSTRAP_RESOLUTION_V9.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V9.md) and [`OMGCOMP_REFINEMENT_WITNESS_V19.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V19.md) |
| selected `Console` checked-adapter execution | OMGLOWI18/CKIR17 preserve exact service, requirement, plan, candidate, reach, ranking, receiverless/static-owner, recurrent head/tail, and explicit `u8 as i32` identities while observing ordered abstract byte events; OMGRFN20 independently joins exact source, OMGRSW9, and actual native/self CKIR17 bytes through R1–R5 | [`OMEGA_BOOTSTRAP_CHECKED_IR_V17.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V17.md), [`OMEGA_BOOTSTRAP_LOWERING_V18.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_LOWERING_V18.md), and [`OMGCOMP_REFINEMENT_WITNESS_V20.md`](../../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V20.md) |
| generated ordinary-Omega source custody for the exact Unicode tuple | sealed locked/offline two-run reproduction, generic provenance roles, bounded/no-publication teeth, exact OMGCOMP1 source extent, CKIR3/OMGRFN4 preflight composition, and a coherent product-owned checkpoint refresh over the same tuple | [`OMEGA_BOOTSTRAP_GENERATED_SOURCE_CUSTODY.md`](../../../bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_GENERATED_SOURCE_CUSTODY.md) and the [`source checkpoint status`](../../../source/compiler/omega/source-checkpoints/README.md) |

Every row is implementation-and-assurance cost evidence for a selected slice.
No row admits a facility to final `Ωself`, claims general checkpoint coverage,
or widens beyond the boundary stated in its linked contract.

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

The historical OMGCOMP2/OMGRSW6 provider fixture deliberately reduces
`Console` to one requirement and cannot establish product provider closure.
OMGCOMP3 closes the transport gap by assigning one exact root-package source
row the build role. OMGRSW9 then harvests the explicit
`Build::select_provider<Service, Provider>` call only from that role and retains
the selecting machine, source span, all six requirements, and the complete
candidate plan. A readable `build` name, filename convention, declaration
order, compatibility `Owner::provider_defaults` suffix, or candidate uniqueness
still cannot manufacture the role or selection. Accepted-lock/closure evidence
must separately join the exact envelope before either has package or
compilation authority.

The complete plan has two checked adapters (`write`, `write_line`) and four
compiler-intrinsic leaves. OMGRSW9 and OMGRFN19 close that static relation.
OMGLOWI18/CKIR17/OMGRFN20 then carry `console_write_bytes`, its ranking/reach
facts, two recurrent guarded head/tail paths, exact authored `output as i32`
casts, and requirement-targeted calls through platform-neutral checked
execution. The boundary sink records ordered abstract `Console::write_byte`
events without dispatching a candidate. It therefore cannot claim an installed
provider, syscall, or native artifact. Native boundary execution remains a
later join with provider admission and the target calling plan.

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
