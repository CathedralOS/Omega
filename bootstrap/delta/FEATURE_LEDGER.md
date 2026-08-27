# Delta v1 provisional feature ledger

This ledger guides discovery while the complete canonical Delta compiler and
`omega-bootstrap` sources are being written. It is not a Delta specification or
an admission list. D0, the sample corpus, and the Rust producer establish
implementation evidence only.

A construct may enter Delta v1 when a concrete requirement in the canonical
compiler or bridge, or an explicit language-coherence, robustness, safety, or
maintainability argument, shows that retaining it lowers whole-bootstrap cost.
Its entry must identify that reason, exact semantics, lower-rung meaning,
positive coverage, and a negative gate for the nearest excluded form. Accidental
producer/corpus behavior is removed before the v1 freeze; Delta is not reduced
to a whitelist of tokens used by one source revision.

This ledger is not the `Ωself` profile. `Ωself` records which ordinary Omega
features the production compiler source uses and `omega-bootstrap` must accept;
this file records which Delta facilities are justified by the implementation of
the canonical compiler or bridge. A feature excluded from one surface is not
thereby excluded from, or admitted to, the other.

No third bridge feature ledger is needed. `omega-bootstrap`'s implementation
features are Delta-v1 entries here; its accepted Omega features are `Ωself`
entries in the separate product-source profile. The full Omega specification
already governs what the resulting production compiler implements.

The objective is a small, robust compiler-host language, not the smallest token
census. A modest facility may remain without many textual occurrences when it
makes either required Delta program materially safer, clearer, more modular, or
easier to assure. Conversely, similarity to Omega is a consistency benefit
rather than a subset requirement.

The design floor is ordinary C-like compiler power with specified behavior:
structured control, predictable data, explicit memory/resource handling, and
sealed byte I/O. Delta may exceed the literal token census of one bridge
revision when a modest companion feature makes that floor coherent across the
compiler and bridge. It need not inherit Omega's proof surface, dependent types,
production allocation model, or general host abstractions merely to look more
like the product language.

“Explicit memory/resource handling” does not require every program to hand-roll
fixed partitions. Delta may expose general runtime-sized allocation over fixed,
bump, typed/indexed-arena, or paged backing when that makes the compiler sources
substantially more robust. Such an interface must specify its capacity,
lifetime/reclamation, aliasing, and exhaustion behavior and must not imply an
ambient host heap.

## Fixed constraints

- deterministic specified behavior, with no undefined behavior;
- no ambient host authority; every failure is a checked result, static rejection,
  or defined trap rather than truncation or undefined behavior;
- lower-rung meaning for every admitted construct;
- Omega spelling, grammar, precedence, and ordinary meaning when Delta retains
  the same construct and that choice is not materially more expensive; and
- explicit rejection of unsupported source rather than producer-shaped
  acceptance.

## Current candidates

This table records decisions still to make, not the history of every gate that
produced the evidence. Exact formats, fixture shapes, resource counts, and
negative matrices belong in the linked bridge contracts and beside their gates.

| Candidate | Current evidence | Simpler form to test first | Status |
| --- | --- | --- | --- |
| machines, states, transitions, loops, and calls | `lowermachine` self-host, the exact contextual-state-identifier native/self/runtime gate, historical O0/O1 canaries, and the CKIR2 exact-root tranche demonstrate structural contextual declarations and typed finite acyclic attached-machine calls across sources in one logical module; producer, Rust-free meaning, lower-rooted reconstruction, and one-frame composition are closed | keep the finite static-call form; add recursion, general member receivers, or broader module/package calls only when a required Delta source needs them | structural contextual declarations and finite static calls demonstrated; broader forms and final disposition unresolved |
| integer arithmetic | D0 and the Rust producer accept several overflow policies and disagree at some edges | use Exact throughout; add only a narrow modular operation if artifact encoding requires it | unresolved |
| implementation of Omega exact integer widening and policy qualification | the Delta-written OMGLOWB/CKIR10 path implements the 22 checkpoint `u8 as u32 in Trapping` uses without requiring a new resolver schema or a general Delta domain system; independent meaning, conservative emission, and OMGRFN12 R1–R5 are closed | keep Delta implementation arithmetic Exact and encode only the bridge's checked source/IR rule unless one of the two required Delta source closures demonstrates a need for Delta-level cast or policy syntax | selected product-source path demonstrated; no Delta cast/domain feature inferred |
| implementation of Omega trapping addition | the Delta-written OMGLOWC/CKIR11 path implements the 78 direct canonical trapping-u32 leaf-plus-anonymous-literal uses among 104 checkpoint additions; independent meaning, runtime-overflow behavior, conservative carry/range/store emission, and OMGRFN13 R1–R5 are closed | encode the bridge's selected Omega rule with Delta's existing checked arithmetic machinery; do not infer Delta domains, Omega policy syntax, full-width public `u32`, or general arithmetic from this product implementation | selected product-source path demonstrated; no Delta language feature inferred |
| implementation of Omega compositional full-width trapping arithmetic | OMGRSW7/OMGLOWF/CKIR14/OMGRFN16 close one recursive pure same-carrier `u32 in Trapping` relation over `+`, `-`, and `*`; the gate carries the actual UTF-8 trees, ordinary precedence/association, first-trap behavior, exact high-word literals and widening, assignment/guard/call/transition contexts with complete ordered pure siblings, inherited CKIR12 views, native/self identity, independent meaning, conservative artifacts, and persisted lower-rooted R1–R5 | retain the single bounded operation-tree implementation and its modular gates; do not replace it with compiler-file-shaped expression/context permutations, widen effectful operand-order custody, or infer a Delta arithmetic-policy surface from this private bridge implementation | selected product-source path demonstrated through persisted lower-rooted OMGRFN16; final Delta-v1 disposition remains open |
| implementation of Omega direct pure full-width `u64 < u64` | OMGRSW8/OMGLOWH/CKIR16/OMGRFN18 close one same-carrier relation with four-word interval custody, exact contextual literals, storage/call/edge transport, true-edge predecessor/intersection facts, paired-word meaning, conservative unsigned emission, and persisted lower-rooted R1–R5 | implement the selected Omega relation with explicit paired words and existing deterministic Delta storage; do not infer Delta `u64` syntax, a public 64-bit ABI, arithmetic, dynamic indexing, mixed carriers, other comparisons, or dependent types from this private bridge representation | selected product-source path demonstrated through persisted lower-rooted OMGRFN18; no Delta language feature inferred |
| implementation of Omega guarded full-width fixed-buffer indexing and mutation | OMGRSWA10/OMGLOWJ19/CKIR18/OMGRFN21 close one name/order-independent `SourceUnit`-like projection through a Trapping full-`u64` lookup index, guarded load/store, authored Exact `length + 1`, defensive carry/range traps, native/self production, modular Rust-free meaning, conservative artifacts, resource teeth, and lower-rooted R1–R5 | keep the selected carrier as explicit paired-word data and deterministic fixed storage in Delta; do not infer Delta `u64` syntax, Omega arithmetic-policy qualification, general dynamic indexing, mutable slices, allocation, or unrelated full-width operations from the bridge implementation | selected product-source path demonstrated; no Delta `u64`, policy, slice, or allocation feature inferred |
| implementation of Omega guarded full-width record-array indexing and nested fields | OMGRSWB11/OMGLOWK20/CKIR19/OMGRFN22 extend the preceding scalar-buffer relation to an actual-capacity `[Observation; 16384]`, exact 40-byte record stride and field layout, nine typed nested stores, tag readback, Exact count increment, and two real receiver calls with up to nine pure scalar arguments; name/field/declaration reorderings, resources, native/self production, Rust-free meaning, conservative artifacts, and lower-rooted R1–R5 are closed | keep records, arrays, paired-word indexes, and finite calls as private deterministic bridge representations; do not infer Delta record/array syntax, public layout or ABI, `u64`, general allocation, effectful argument-order custody, payload-sum copying, or structural-value projection | selected product-source path demonstrated; no additional Delta-v1 language feature inferred |
| implementation of Omega's actual `TokenStream::push` shape | OMGRSWC12/OMGLOWL21/CKIR20/OMGRFN23 compose the prior record-array and pure-sum lanes over eight records, five copyable sum families, two actual-capacity record arrays, semantic nested record and active-payload sum Copy, fifteen writes, a ten-argument call carrying `SourceId` and `TokenKind`, nested `source.value`, and indexed Float payload dispatch; modular Rust-free meaning, corrected conservative artifacts, native/self production, resources, and responsibility-local R1–R5 are closed | keep the selected layouts, copies, paired-word indexes, and finite calls as private deterministic bridge machinery; do not infer Delta records/sums/arrays syntax, a public ABI, general allocation, explicit discriminants, generic/mixed sums, effectful argument order, or the complete lexer from this implementation | selected product-source path demonstrated; no additional Delta-v1 language feature inferred |
| implementation of selected Omega `Console` checked adapters | OMGRSW9/OMGLOWI18/CKIR17/OMGRFN20 close the exact six-requirement static plan, receiverless `write`/`write_line` adapters, ranked recurrent helper, explicit `u8 as i32`, and ordered abstract `write_byte` events through native/self identity and lower-rooted reconstruction | keep this as private bridge checking over an already reconciled graph; do not infer Delta boundary traits, provider selection, calling conventions, syscalls, native effects, or an `i32` cast surface from the implementation | selected platform-neutral checked execution demonstrated; provider admission, target calling-plan custody, native effects, artifacts, and authority remain open |
| implementation of bounded SHA-256 | the Delta-written structural producer hashes exact raw envelopes through the public 267,280-byte OMGCOMP ceiling; fixed vectors, padding edges, native/self agreement, adjacent exhaustion, and one lower-rung Gamma observation are closed | retain byte-wise 32-bit words and exact small carries; do not infer Delta wrapping arithmetic, a public full-width `u32`, or package/lock authority from a bridge-local digest implementation | selected bridge prerequisite demonstrated; no Delta arithmetic or authority feature inferred |
| bool-only prefix logical negation | the selected OMGLOW7/CKIR6 relation closes least-OMGRSW1/2/3 production, lower-rung meaning, conservative emission, and all OMGRFN8 R1–R5 joins over one immutable payload-sum carrier | retain ordinary Omega spelling and exact Boolean meaning; do not add integer truthiness, bitwise complement, or user-defined unary dispatch | selected compiler path demonstrated; broader expression coverage and final disposition unresolved |
| pure, total, nontrapping bool-only `&&` and `||` | checkpoint 000001 contains 38 conjunctions and 6 disjunctions over primitive predicate leaves; OMGLOW8/CKIR7 closes exact precedence/association, least-OMGRSW1/2/3 production, Rust-free meaning, truth tables, conservative emission, and OMGRFN9 R1–R5 over one immutable payload-sum carrier | retain ordinary short-circuit source meaning; the private eager truth-function lowering is equivalent only after independently proving both operands terminating, pure, and nontrapping; reject calls, indexing, Trapping arithmetic, mutation, bitwise spelling, and observable skipped work in this selected relation | selected compiler path demonstrated; effectful/general short-circuit lowering and final disposition unresolved |
| pure, total, nontrapping primitive scalar `==` | checkpoint 000001 contains 60 same-carrier `u32` and one same-carrier `u8` authored equality; OMGLOW9/CKIR8 closes exact `bool`/`u8`/`u32` carrier equality, precedence and authored order, least-OMGRSW1/2/3 production, Rust-free meaning, conservative emission, and OMGRFN10 R1–R5 with reachable equal/unequal rows | retain the ordinary compiler-owned primitive route; keep records, sums, `!=`, `u64`, cross-carrier conversion, calls, indexing, trapping arithmetic, and user-defined dispatch outside this selected relation | selected compiler path demonstrated; structural/general equality and final disposition unresolved |
| pure, total, nontrapping primitive scalar `>` and `>=` | checkpoint 000001 contains 34 same-carrier `u32 >=` and one same-carrier `u32 >`; OMGLOWA/CKIR9 closes direct authored-order unsigned `u8`/`u32` comparison, precedence and chain rejection, least-OMGRSW1/2/3 production, Rust-free meaning, conservative SETA/SETAE emission, and OMGRFN11 R1–R5 with reachable true/false rows for both operators and carriers | retain ordinary compiler-owned primitive routes; do not implement them by swapping operands into `<`/`<=`, and keep Boolean, `u64`, cross-carrier conversion, effects, indexing, trapping arithmetic, user dispatch, and new transition facts outside this selected relation | selected compiler path demonstrated; broader ordering/fact behavior and final disposition unresolved |
| records, fixed arrays, and recursively constant aggregates | checkpoint 000001 and selected CKIR3/CKIR4 paths establish bounded nominal layout, constant graphs, runtime named construction, structural Call/Copy, native/self/mixed production, Rust-free meaning, and responsibility-local lower-rooted reconstruction. OMGRSWC12/CKIR20 additionally close the actual TokenStream owner, copyable nested records and pure sums, active-payload semantic Copy, actual-capacity arrays, chained places, fifteen writes, and structural-value transport | retain the typed constant graph/pool, aggregate copy, ordinary named fields, and deterministic fixed storage while completing general bridge coverage; the measured cost no longer argues for hand-expanded initialization, tag-plus-payload duplication, or positional compiler data | selected constant-aggregate, runtime-record, direct-field, nested record-array, and pure-sum structural closures are complete; general coverage and final disposition remain unresolved |
| implementation of Omega slices and runtime views | checkpoint 000001 uses source, decoded-byte, token, and spelling views; the Delta-written OMGRSW4/OMGLOWD/CKIR12 path closes one exact program-static shared-byte window, and OMGLOWG/CKIR15/OMGRFN17 generalize it to a runtime-capable direct view parameter, recurrent guarded head/tail, and exact ordered pass-through vectors through independent meaning, conservative emission, runtime-only/no-static-root custody, and lower-rooted R1–R5 | compare the remaining general slice facility with explicit backing-plus-span records; do not infer Delta slice syntax, pointer semantics, mutable views, dynamic indexing, computed/effectful siblings, allocation, UTF-8, or same-carrier `u64` collection operations from the bridge's private descriptor implementation | selected recurrent product-source path demonstrated; general bridge cost and Delta-v1 disposition unresolved |
| payload-bearing sum data | checkpoint 000001 uses token, numeric-base, diagnostic, and console-result sums; the selected unnumbered pure-sum tranche closes OMGRSW3 resolution, general OMGLOW6 construction/Copy/Call/dispatch lowering, CKIR5 publication and independent meaning, conservative CKIR5→ELF emission native/self, and all OMGRFN7 R1–R5 lower-rooted joins in one immutable frame | compare general tagged data with explicit tag-plus-payload records; do not force the split when it increases invalid states or duplicated dispatch | selected compiler path demonstrated; general coverage and final disposition unresolved |
| runtime-sized reservation from fixed backing and integer-offset arenas | storage canaries and current compiler tables demonstrate fixed partitioning, checked exhaustion, and bulk reset without a general heap | keep fixed arrays or library arenas while sufficient; add deterministic bump/paged reservation only when a required Delta source needs it | fixed partitioning demonstrated; runtime reservation unpresumed |
| host boundary | current source declares `boundary trait Console` with partly hardwired operations | use one sealed interface for source bytes, artifact bytes, diagnostics, and termination | general boundary traits not presumed |
| source units and modular organization | the bundle, generic OMGCOMP1 envelope, target/configuration-bearing OMGCOMP2 envelope, and explicit root-package build-source role in OMGCOMP3 close bounded public multi-unit custody and nominal identity; OMGRSW9/OMGRFN19 additionally close the exact product `Console` build selection and complete six-row static provider plan, while OMGLOWI18/CKIR17/OMGRFN20 carry its selected checked adapters through platform-neutral abstract execution. Accepted-lock authority and private cross-module semantics remain separate | keep package/module/provider semantics in Omega and pass Delta one reconciled graph; decoding, statically planning, or abstractly executing that graph does not make those semantics Delta features, and the build-role bit must not turn source labels, defaults, or candidate discovery into authority | bounded public multi-unit custody, exact target/configuration/build-role custody, complete static product-provider planning, and selected checked-adapter execution demonstrated; provider admission, target calling-plan custody, native effects, authority, and general closure open |
| contracts, refinements, and proof-oriented syntax | experimental producer corpus | runtime/static checks plus externally checked emitted certificates | not presumed |
| mixed field-plus-case data and other producer experiments | Rust-producer acceptance or planned slices | separate records and sums | not presumed |

Evidence for these rows stays with the bridge contracts rather than being
duplicated here. Start at the
[`omega-bootstrap` status](../omega-bootstrap/README.md), the base
[`checked-IR contract`](../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR.md),
the current entry-bearing
[`CKIR20`](../omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V20.md)
successor, and the current
[`OMGRFN23`](../../source/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V23.md)
lower-rooted contract. The separate CKIR17/OMGRFN20 line remains the active
platform-neutral provider-adapter evidence. Git and those versioned contract directories retain the
earlier milestones; this ledger records only the current design evidence and
open Delta-v1 disposition.

The path-independent
[`source-closure V1`](DELTA_SOURCE_CLOSURE_SNAPSHOT_V1.md) machinery now binds
the exact canonical compiler source image and one provisional three-root bridge
action DAG, including normalized unsigned imported-tool identity. This proves
the manifest mechanism and selected snapshot contents. It does not complete the
two final source manifests required below or freeze any ledger row.

## Freeze gate

Before Delta v1 is named complete:

1. Publish separate complete deterministic source manifests for the canonical
   Delta compiler and `omega-bootstrap`.
2. Classify every retained feature as required by either closure or justified
   by an explicit coherence, robustness, safety, or maintainability argument,
   and record why the simpler alternative was rejected.
3. Remove accidental parser/backend behavior and experimental corpus features.
4. Publish normative grammar and semantic edge tables independent of the source
   files that motivated them.
5. Prove both complete closures valid and run native, self-hosted, and lower-rung
   differentials plus phase-isolated negative gates.

This ledger and the `Ωself` inventory answer different questions. Delta may
retain a facility the product compiler source never uses when it materially
simplifies implementation of the canonical compiler or bridge, and the product
compiler may use an Omega feature Delta does not have when `omega-bootstrap` can
implement that feature directly. There is no requirement that either inventory
be a subset of the other.
