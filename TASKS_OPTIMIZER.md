# Optimizer Tasks

This is the execution checklist. Architecture and rationale live in
[`wiki/design_briefs/optimizer_architecture.md`](wiki/design_briefs/optimizer_architecture.md)
and its linked briefs. Language-semantic blockers alone belong in
[`OWNER_QUESTIONS.md`](OWNER_QUESTIONS.md).

Selections are exact names. Do not add `O1`/`O2`/`O3`, `debug`/`release`, or
another broad alias while executing this plan.

## Status legend

- `[x]` implemented and tested at its current boundary;
- `[>]` active slice;
- `[ ]` not yet implemented; and
- `[?]` requires an owner language decision.

## Current stopping point

[x] Abstract-to-target translation validation now has a fifteenth exact family
for `IntegerBitwiseOr(parameter, parameter)`. OR reuses the named `bitwise`
rung but owns distinct source and target leaves, catalog identity, error,
receipt, fixtures, corruption suite, and optimized custody. Common bitwise ABI
and provenance replay now sits below the 48-line target entrance. Bitwise
error/receipt vocabulary and both parameter/Terminal fixture maps descend into
exact AND and OR leaves rather than becoming mixed files. Independent replay
covers all eight native fixed-width integer types, all five native targets,
register and incoming-stack placements, mixed rosters, reversed and identical
operands, and every retained target field; address and nonnative carriers fail
closed. Architecture gates forbid the retired flat bitwise vocabulary and
fixture paths.

[x] Abstract-to-target translation validation now has a fourteenth exact family
for `IntegerBitwiseAnd(parameter, parameter)`. Its small source and target
entrances establish a named `bitwise` rung, with exact `bitwise_and` leaves
rather than extending a mixed binary catchall. Independent replay covers all
eight native fixed-width signed/unsigned integer types, all five native targets,
register and incoming-stack placements, mixed rosters, reversed and identical
operands, and exact operation/edge/value/type/location custody. Address and
nonnative integer carriers fail closed. Optimized-pipeline custody retains the
typed receipt from a real Terminal artifact, and architecture gates pin the new
navigation path and keep derived grammar out of the common source envelope.

[x] Translation-validation coverage now mirrors the production taxonomy instead
of accumulating mixed flat test files. Integer/Boolean immediates, scalar
`Crash`, integer bitwise-not, and integer less-or-equal each descend through
small family entrances into `positive`, `source_corruption`, and
`target_corruption` leaves. Optimized target custody descends once more from
`unary` and `comparison` into exact-family leaves. The Widen custody matrix now
uses an independent explicit enumeration of all 18 native relations across all
five targets and both register/stack placements, retaining every receipt
identity. The source-organization gate forbids all retired flat test paths.

[x] Abstract-to-target translation validation now has a thirteenth exact family
for proof-bearing `IntegerExactCast(parameter)`. All 38 legal ordered casts
between native signed/unsigned 8/16/32/64-bit integers are covered after
excluding identities and widenings. Independent replay retains the cast
obligation, source/target types, operand/result identities, operation/edge
provenance, and full-roster register/stack ABI placement on all five native
targets. Optimized-pipeline custody starts from a real Terminal artifact whose
canonical representability goal is a machine precondition discharged by an
assumption certificate; the same obligation identity reaches the typed target
receipt. Parameter fixtures and Terminal translation fixtures now enter through
small taxonomy maps over `direct`, `unary`, and `comparison` leaves instead of
growing mixed builders. Unary model, error, and receipt catchalls are likewise
split into exact semantic leaves, and the source-organization gate pins the real
catalog join rather than forwarding modules.

[x] Abstract-to-target translation validation now has a twelfth exact family
for `IntegerWiden(parameter)`. All 18 legal native fixed-integer widenings are
covered: same-sign widening and unsigned-to-larger-signed widening across
8/16/32/64-bit carriers. Independent replay retains distinct source and target
types, operand/result identities, operation/edge provenance, full-roster ABI
placement, and register/stack custody on all five native targets; corruption
of either source or target fails closed. The former mixed parameter replay,
model, error, receipt, and catalog-adapter files now descend through explicit
`direct`, `unary`, `bitwise`, and `comparison` rungs. The sole enable/disable catalog stays
small and visible, and architecture gates forbid the retired flat and
`derived` taxonomies.

[x] Transition-free, spill-free register-home assignment is now a deterministic
constraint-graph allocator rather than a start-ordered greedy walk. Distinct
use/definition ties form allocation vertices with exact intersected candidate
domains; explicit symmetric interference and directional early-clobber edges
constrain physical storage/write footprints. Canonical placement chooses the
fewest currently viable views, greatest remaining constrained degree,
earliest point, and lowest VReg leader before selecting the lowest compatible
view. Producer and validator independently reconstruct domains, constraints,
and placement beneath separate small entrances. Focused coverage proves the
formerly rejected `{r0,r1}` versus `{r0}` feasible case, noninterfering reuse,
aliased physical views, ties, early clobbers, pressure, corruption, and
determinism.

[x] Abstract-to-target translation validation has an eleventh exact family
for integer bitwise-not of one parameter. The source grammar map first descends
through the small `source/integer/mod.rs` coordinator that owns typed parameter
lookup, then reaches named comparison and unary leaves. Independent replay covers signed
and unsigned 8/16/32/64-bit
types, register and incoming-stack placement on all five native targets, mixed
rosters, exact operand/result/type/provenance custody, and source/target
corruption. The optimized pipeline retains the typed receipt through target
custody, and architecture gates pin both meaningful entrances and forbid the
retired flat integer grammar leaves.

[x] Abstract-to-target translation validation now has a tenth exact family for
typed integer less-or-equal of two parameters. Its named source and target
leaves share only the generic binary-Boolean carrier and the governed ordering
reconstruction rung with strict less-than. Independent replay covers both
signednesses at every natively supported 8/16/32/64-bit width, register and
incoming-stack placement on all five native targets, reversed and identical
operands, mixed rosters, provenance, and operator/operand/type corruption. The
optimized pipeline retains the typed receipt through target custody. Catalog,
source-grammar, ordering, and typed-leaf navigation remain architecture-pinned;
the source entrance remains at 99 lines.

[x] Psi publication now retains every independently validated candidate
declaration, including genuine policy skips, rather than only retaining the
declaration inside an Applied commit. Run-to-abstract replay replaced its flat
Applied-only leaf with a 21-line `candidate_decisions/mod.rs` coordinator over
exact manifest binding, independent declaration replay against the retained
input revision, and baseline-policy evidence. Applied verdicts require exactly
one matching commit and skipped verdicts require none. Six-pass positive
coverage plus skipped roster, pass, validator, verdict, analyses, facts, and
cost corruption tests close the coordinated-mirror gap. Architecture gates pin
the coordinator and forbid restoration of the retired flat leaf.

[x] Mandatory selected-instruction construction now has the same visible
coordination shape as the optimizer catalogs it feeds. The 52-line
`construction/mod.rs` owns the complete scalar/plain-Unit/structural-Unit
function-roster join; structural ABI layout and optional-call mechanics descend
through their own entrance and leaves. Scalar construction enters through a
small context-to-catalog-to-complete-body join. Its sole seven-row ordered
catalog classifies immediate, entry-parameter, exact add/subtract, widened exact
add/subtract, and active-resident exact-add-chain families exactly once; each
selected leaf returns virtual registers and blocks together. Omission rejects
as unsupported and overlap fails closed with both family names. The former
332-line mixed plan file, 966-line duplicated scalar classifier, and six-line
forwarding entrance are gone, and architecture gates pin the real joins and
catalog.

[x] Abstract-to-target translation validation now has a sub-100-line independent
entrance that binds Psi identity, the requested target, entry, and the complete
function roster before descending into exact family replay. The adjacent
sub-100-line catalog is the sole enable/disable inventory; each descriptor
joins one source classifier to one typed replay adapter. Zero matches publish
`Uncovered`, one match publishes one receipt on that exact function-roster row,
and duplicate or overlapping matches fail closed. The first fifteen semantic
rows reconstruct parameterless straight-line integer and Boolean literal
returns, scalar `Crash`, direct integer and Boolean parameter returns, Boolean
negation of a parameter, ordered Boolean equality of two parameters, and typed
integer equality, strict ordering, or inclusive ordering of two same-type
integer parameters, plus integer bitwise-not, integer-widen, and proof-bearing
integer exact-cast of one parameter, and integer bitwise-AND or bitwise-OR of
two parameters.
The parameter-expression families descend through a governed source-grammar
map, integer-family coordinator, shared envelope, whole-roster ABI replay, and
explicit direct/unary/bitwise/comparison joins. Boolean-not, integer bitwise-not,
integer widen, and equality replay
retain their distinct operands, produced value, operation provenance, return
edge, exact integer type where applicable, and exact register or stack
locations.
Focused mutation
coverage rejects every source-shape, root, target, roster, provenance, and
operation-field substitution across all five native targets.

[x] Physical phase composition now has one catalog-backed route-policy
entrance instead of dispersed conditionals. Its closed result distinguishes
baseline, selected-lowering, allocation-recovery, post-allocation-machine, and
function-relative-layout routes, including the two admitted cross-phase
compositions. The exhaustive matrix covers every unordered pair of all 15
exact names on x86-64 and AArch64 (210 cells): 120 admitted, 50 composition
rejections, and 40 typed target rejections. It also proves exact Psi registry
projection, full-Psi overlay invariance, and higher-order selected-lowering
composition anchors.

[x] External Psi policy schema v2 is now a fact-complete, governed boundary.
Its 86-line entrance owns canonical candidate-set and decision-log joins, then
descends into `model`, `identity`, `codec`, and focused tests. The pass manager
has a separate 30-line policy entrance over context, validated feature
projection, independent manifest reconstruction, and exact replay. Requests
bind candidate cost, scheduled analyses, and canonical proof/ownership facts;
replay rejects drift before consuming a decision, and record-only execution
cannot alter baseline output. The architecture gate pins both meaningful
entrances and rejects restoration of the former flat schema.

[x] The 954-line post-allocation manifest catch-all is now a governed
register-allocation entrance. Its 100-line `mod.rs` owns the direct-home and
selected-lowering projection/validation joins; record shape, errors, canonical
identity, codec, reconstruction, validation, rendering, and focused tests
descend into named leaves, with no replacement above 315 lines. The source
organization gate pins the real projection join and rejects restoration of the
flat mixed file.

[x] The GVN total-scalar identity rung now follows the catalog literally.
Seven catalog rows descend into seven same-named folders; each small `mod.rs`
owns one exact rule contract and its proposal join, and each `laws.rs` owns
only that rule's closed semantic partition. The former mixed `rule.rs` and
`shapes.rs` catch-alls are gone. The architecture gate pins every folder,
requires both the proposal join and classifier, and rejects restoration of the
two catch-alls. This is the Squalr navigation shape at the exact-rule rung,
with Omega's independent validator remaining across the trust boundary.

[x] Global value numbering now keeps absorbing bitwise literals in their own
four-row obligation-free family: `0 & x`, `x & 0`, `all_ones | x`, and
`x | all_ones`. Candidate tags 23-26 and catalog row 15 are append-only under
GVN v14. The rule retains only its exact-width law-literal fact, maps to a
separate independently reconstructed validator identity, and deliberately has
no XOR row. Signed and unsigned 1/128-bit boundaries, canonical left-literal
ties, rule-domain isolation, fixed-point execution, and all neutral/absorbing
AND/OR overlaps are covered; catalog order gives the earlier neutral rule the
stable disposition where both laws apply.

[x] Global value numbering now has a separate six-row obligation-free bitwise
neutral-literal family: `all_ones & x`, `x & all_ones`, `0 | x`, `x | 0`,
`0 ^ x`, and `x ^ 0`. Candidate tags 17-22 and catalog row 14 are append-only
under GVN v13; the rule retains only its exact-width law-literal fact and has a
separate independently reconstructed validator identity. Signed and unsigned
1/128-bit carrier boundaries, canonical left-literal ties, rule-domain
isolation, disabled-by-default selection, and fixed-point execution are
covered. Absorbing bitwise literals remain a distinct adjacent rule rather
than being folded into this family.

[x] Global value numbering now has a separate two-row obligation-free
saturating multiplication annihilation family: `0 * x -> 0` and `x * 0 ->
0`. It appends candidate tags 15/16 and catalog row 13 under GVN v12, retains
only the typed zero-literal fact, and maps to a separate independently
reconstructed validator identity. Signed and unsigned 1/128-bit boundaries,
left-zero ties, rule-domain isolation, and the confluent `0*1`/`1*0` overlap
with the earlier multiply-one rule are covered.

[x] Same-block and dominating GVN now mirror the phi-translated taxonomy. Their
32- and 37-line entrances own traversal-specific analysis/invalidation
contracts and descend into separate obligation-free, proof-certified, and
compatible-policy leaves. The former 449- and 656-line mixed files are gone;
no replacement exceeds 212 lines, and the architecture gate pins both joins.

[x] Phi-translated GVN no longer combines three exact catalog rules in one
824-line leaf. Its 37-line entrance owns their shared analysis, invalidation,
pass, and version contract, then descends into obligation-free,
proof-certified, and compatible-policy leaves of at most 271 lines. The
source-organization gate pins that meaningful entrance.

[x] Producer-side GVN expression identity no longer lives in one 954-line
mixed leaf. A 33-line `expression_keys/mod.rs` entrance owns the canonical row
and operand-pair contracts and descends into named `model`, obligation-free
`total`, `proof_certified`, and asymmetric `compatible_policy` leaves. No
replacement exceeds 300 lines, and the source-organization gate pins the
meaningful entrance.

[x] Global value numbering now has a separate five-row obligation-free
saturating neutral-arithmetic family: add-zero left/right, subtract-zero right,
and multiply-one left/right. It appends candidate tags 10-14 and catalog row 12
under GVN v11, retains one independently typed law-literal fact, and maps to a
separate independently reconstructed validator identity. Signed one-bit
multiply-one is explicitly absent because that type cannot represent `+1`;
`0+0` and `1*1` choose their left rows deterministically.

[x] Global value numbering now has a separate two-row obligation-free wrapping
multiplication annihilation family: `0 * x -> 0` and `x * 0 -> 0`. It appends
candidate tags 8/9 and catalog row 11 under GVN v10, retains exactly the
law-defining zero-literal fact, and has an independently reconstructed
validator identity. The `0*1`/`1*0` overlap is confluent and the earlier
multiply-one rule wins deterministically; `0*0` chooses the left-zero row.

[x] Exact release rollback now has catalog-wide disabled-selection coverage.
Every source-visible `Optimization::ALL` name is independently subtracted from
the full suite, excluded from every phase projection, and reapplied to prove
idempotence. Both build preludes prove exact enable/disable name agreement;
the existing four-hosted-target golden test proves an empty effective suite
rejoins the byte-identical ordinary native path.

[x] Global value numbering now has a separate two-row obligation-free wrapping
shift family: `x << 0 -> x` and `x >> 0 -> x`. Its exact rule is adjacent to,
but not merged with, the five-row neutral-arithmetic rule. Producer and
validator independently reconstruct the value type and the potentially
different count type; candidates retain only the exact zero-literal fact and
consume no accepted obligation. Candidate tags 6/7 are append-only, the GVN
pass identity is v9, and the rule remains disabled with an empty exact-name
selection.

[x] Global value numbering now includes one closed obligation-free wrapping
neutral-arithmetic family: `0 + x`, `x + 0`, `x - 0`, `1 * x`, and `x * 1`.
The ordered producer leaf admits only those five typed rows; a separate
17-line validation entrance descends into independent classification, literal
evidence, accounting, and application leaves. Candidates retain the exact
scalar-constant fact, consume no accepted-obligation custody, and remain
disabled with an empty exact-name selection. Toolchain `emit_report()` is now
finalized as a reporting-only intrinsic distinct from optimization selection.

[x] The ordinary no-selection path now bypasses optimizer-only artifact
lowering, optimizer-unit construction, pass management, and optimized-plan
projection. A focused compiler canary runs on every supported host and checks
all four hosted native targets. It pins empty selection/reporting, exact
acceptance and diagnostics, interpreter output, two-build raw native-byte
determinism, and reviewed per-target artifact metadata/digests. UEFI remains
outside this direct-native matrix until its physical adapter/publication chain
exists.

[x] Psi rule coordination now has one small selection/application entrance,
one declarative pass table, and one local ordered `catalog.rs` in every exact
pass folder. Copy propagation no longer survives as a flat catch-all file. The
architecture test rejects a missing pass catalog or a named stage entrance
that regresses into a re-export wall.

[x] Every Omega pipeline root is now governed by the source-navigation gate;
small stage entrances retain their real joins and semantic leaves remain below
the production/test ceilings.

[x] Exact per-rule release rollback is now a typed, native-only, subtractive
overlay. Unknown and duplicate names reject; known unselected rules are visible
no-ops; publication retains the authored/requested/applied/effective receipt;
and rollback-to-empty matches the ordinary path byte-for-byte on all four
hosted native targets.

[x] Target-register-environment corruption coverage now spans System V AMD64,
Microsoft x64, AAPCS64, and Darwin AAPCS64 across all five supported native
targets. General selected-call lowering and live-across-call allocation remain
separate work because calls are not yet present in the selected CFG.

[x] Psi, selected-lowering/allocation-recovery, and post-allocation-machine
rule crates expose meaningful selection entrances immediately above their
ordered catalogs and named leaves. Cross-stage custody code consumes those
catalogs rather than owning proxy enable/order tables; the architecture gate
pins both the entrances and their tables.

[x] One global coverage test composes only the owning catalog views and proves
every `Optimization::ALL` name occurs exactly once in its declared phase. It is
a test, not a second production registry.

[x] Add explicit target-applicability dispositions to rule descriptors so
unsupported targets reject for a named reason rather than relying on leaf
failure.

[x] Mandatory legalization now has one ordered twelve-form catalog spanning
seven scalar, plain Unit, and four structural-Unit forms. Producer matching
and independent replay descend into separate typed leaves and share only
recipe, shape, and non-authoritative cost data. Removing a row disables that
form; missing and ambiguous recipe lookup fails closed.

[x] The legalized-operation representation no longer hides model, identity,
validation, and broad fixtures in a 2,098-line crate root. Its 17-line entrance
is a responsibility map into named semantic folders. Legalization source and
replay have separate sub-100-line roster entrances and mirrored structural
subtrees, all pinned by the source-organization gate.

[x] The exact ranked-`u32` countdown now crosses the ordinary object boundary
through target-owned x86-64/AArch64 decoders and independent image-owned
contract/fuel replay. Object custody retains the full ranked record; missing
custody, corrupted bytes/ABI/frontier/provenance/fuel, and mixed body evidence
reject. Final-image, installation, native-artifact, and callable
publication custody now replay that record independently.

[x] Layout-independent selected-form encoding now has an independent
validation rung. Its small entrance coordinates ordinary rows, structural
rows, and aggregate custody; candidate bytes descend into target-owned
decoders and cannot re-enter producer encoding helpers.

[x] Resolved selected-form layout now has one small construction/admission
entrance and mirrored semantic subtrees. Ordinary construction descends
through policy, canonical order, span planning, row handling, and target
branch encoding; independent validation separately derives those facts and
admits candidate branch bytes only through target decoders. Architecture and
corruption tests pin both the navigation shape and the producer boundary.

[x] Fragment admission now consumes one generic selected-lowering realization
instead of requiring the x86 rel8 rule. Exact add and subtract selected
lowering reach fragment, text, and object-container custody on both supported
ISAs, then object-artifact and callable custody, without rule-specific route
variants; v8 fragment/text manifests retain the generic source kind and exact
selection identity.

[x] Exact x86-64 zero-extended i64 materialization now selects canonical
five-byte low-register or six-byte extended-register `MOV r32, imm32` for
`0..=u32::MAX`. Its 29-line machine-rule entrance descends into separate model,
compute, replay, identity, codec, and test leaves; its 12-line pipeline entrance
descends into model/stage custody. A closed materialization adapter replaces
the former optional MOVN/XOR tuple, so another rule does not add another
parallel route or optional coordinator field.

## Completed foundation

- [x] Exact source-visible `Optimization` vocabulary and versioned canonical
  selection encoding/identity.
- [x] Empty selection bypasses optimizer construction; human report selection
  is separate.
- [x] Toolchain-provided `build.omg` exact enable surface with duplicate
  rejection.
- [x] Complete validated optimization unit and retained facts/provenance/fuel.
- [x] Rule, validator, candidate, pass, fact, ledger, report, and artifact
  identities/codecs.
- [x] Bounded deterministic pass manager with ordered catalog and analysis
  invalidation auditing.
- [x] Independent Psi candidate and complete-unit validation.
- [x] Psi CFG products: graph, dominators, postdominators, SCCs, loops, and call
  graph.
- [x] Psi exact pass families: control-flow cleanup, SCCP, copy propagation,
  GVN, dead pure scalar elimination, and proof-check elision.
- [x] Proof-certified exact scalar identities retain accepted-obligation
  custody through lowering.
- [x] Selected-lowering exact incoming u12 add/subtract folds.
- [x] Physical register model, liveness, live ranges, allocation legality,
  deterministic transition-free interference allocation, fixed-view recovery,
  and bounded rematerialization slices.
- [x] Symbolic post-allocation plan/effects and independent validation.
- [x] AArch64 compare-zero/branch-nonzero CBNZ fusion.
- [x] AArch64 shortest MOVN-seeded i64 materialization.
- [x] x86 zero-extended i64 materialization via canonical `MOV r32, imm32`.
- [x] x86 conditional-branch rel32-to-rel8 function-relative relaxation.
- [x] Encoding, layout, whole-function exit, realization, fragment, text,
  object, optimized artifact, and callable-entry custody slices.
- [x] Catalog-driven optimizer slices use analyses/planning/rules/stages with
  focused leaf modules and mirrored tests.

## Organization gate

- [x] Replace `post_allocation_machine_optimizations.rs` with a folder whose
  entrance consumes the machine-rule catalog into one typed result; descend to
  `aarch64/{cbnz,movn}` and `x86_64/xor_zero` custody leaves while the adjacent
  machine-rule entrance remains the only enable/order owner.
- [x] Split `post_allocation_selected_form_encoding.rs` by stage model,
  row/structural encoding, machine-rule disposition, identity, and independent
  validation. Its entrance must own the encode/validate join.
- [x] Remove optimization-name-specific variants from complete physical route
  results once the typed post-allocation result can carry them.
- [x] Derive or exhaustively cross-check build vocabulary and report mappings
  against `Optimization::ALL`; no hidden second registry.
- [x] Keep preferred entrance files below 100 lines. Any entrance above 200
  lines needs a documented semantic reason. No optimizer production file may
  exceed the 1,000-line default ceiling unless it is an exact pinned migration
  debt that cannot grow; no pinned debt may exceed 1,300 lines. Dedicated test
  fixtures retain a 1,500-line ceiling. Files may not mix catalog, unrelated
  rule mechanics, validator, codec, and broad fixtures.
- [x] Eliminate the pinned pre-ratchet production leaves by semantic split.
  Live-range validation is complete: its 34-line entrance owns liveness-custody
  replay followed by independent range replay, with receipt projection and
  focused tests below it; the former 1,294-line catch-all is gone. SCCP constant
  evaluation is also complete: its 35-line shared-contract entrance descends
  into boolean rules and an integer subtree for binary operations, exact casts,
  unary operations, and fact lookup; the former 1,276-line leaf is gone and the
  largest replacement is below 750 lines. Rewrite-candidate construction is
  complete too: its 76-line entrance owns decision derivation, common custody,
  exact patch validation, identity encoding, and immutable admission, with
  scalar/control-flow constructors and accessors in named leaves; the former
  1,253-line file is gone and the largest replacement is below 400 lines.
  Straight-line scalar lowering is complete as well: its 50-line entrance owns
  ordered evaluation and terminal sealing, then descends into exhaustive
  routing, integer arithmetic, integer conversion, and exit leaves; the former
  1,238-line file is gone and the largest replacement is below 600 lines.
  Shared conditional-scalar lowering is complete too: its 37-line entrance
  orders direct scalar handling before exhaustive integer routing, with binary
  semantics and shift semantics in separate shared leaves; the former
  1,111-line file is gone and the largest replacement is below 600 lines.
  Live-range computation is complete as well: its 62-line plan coordinator
  descends through function construction, constraints, fragments,
  architectural units, and focused tests; the former 1,071-line mixed file is
  gone and the largest replacement is below 450 lines.
  Psi-to-abstract machine lowering is complete too: its existing 57-line stage
  entrance descends through a 49-line payloadless/structural/ordinary route,
  then ordinary lifecycle, exact operation, and terminator projection leaves;
  the former 1,058-line file is gone and no replacement exceeds 700 lines.
  Abstract-to-target Unit lowering is complete as well: its ordered setup/loop
  now descends into separate boundary-realization and cleanup-return leaves;
  the former 1,034-line file is gone and no replacement exceeds 453 lines.
  The optimization-unit model is complete too: its 57-line aggregate/map owns
  `PsiOptimizationUnit` above graph, proof, range, ownership, and one-time
  attachment leaves; the former 1,023-line file is gone and no replacement
  exceeds 323 lines.
  Fixed-view-copy computation is complete as well: the existing rule entrance
  retains explicit policy selection and compute-to-validation custody, while
  its application loop descends into source preflight, shared-entry policy,
  CFG mutation, and focused tests; the former 1,022-line file is gone and no
  replacement exceeds 278 lines.
  Legalization leaf replay is complete too: its 95-line entrance owns source
  custody, recipe dispatch, return sealing, and edge-fuel replay, then descends
  into exhaustive recipe, exact-arithmetic, immediate, and fuel families; the
  former 1,022-line catch-all is gone and no replacement exceeds 464 lines.
  Liveness validation completes the migration: its 48-line entrance owns root
  custody, scalar replay/comparison, structural roster replay, and receipt
  admission above named constraint, replay, comparison, structural, receipt,
  shared-canonicalization, and test leaves. The former 1,021-line catch-all is
  gone, no replacement exceeds 225 lines, and the exact exception table is
  empty.
- [x] Clear the current production-file size violations by semantic split, not
  line shuffling. Pipeline `whole_function_exit_contract`,
  `resolved_selected_form_layout`, `x86_branch_relaxation`, and
  `function_fragment_emission` are split, as are validator GVN, SCCP, and
  proof-check-elision candidates, Psi semantic analyses, optimization-unit
  model/rewrite/identity ownership, and matching oversized test suites. A
  repository architecture test enforces file and entrance ceilings.
- [x] Split the architecture brief into a real entrance plus semantic, rule
  engine, physical pipeline, source organization, and rollout briefs.
- [x] Compact this file to an executable checklist; detailed design is not a
  task-list responsibility.
- [x] Replace the conditional Psi mega-registry with a declarative pass table;
  each exact pass owns its ordered rule catalog immediately below its folder
  entrance.
- [x] Split the flat optimized ordinary-callable-entry stage into a subfolder;
  its small entrance owns build/replay while model, reconstruction, and codec
  descend into named leaves.
- [x] Split the flat selected-lowering literal-fold stage; the regalloc rule
  entrance owns exact selection projection and catalog order while pipeline
  carriers, execution, replay, and work accounting descend into named leaves.
  The proxy schedule registry and hidden whole-catalog combined policy are
  removed; exact catalog rows now compose their own payloads append-safely.
- [x] Move selected-lowering, allocation-recovery, and post-allocation-machine
  enable/order tables to their rule-owning crate entrances. Remove the proxy
  pipeline catalogs and enforce each meaningful entrance plus adjacent catalog
  in the repository navigation test.
- [x] Split the regalloc rule root by executable phase. Selected lowering and
  allocation recovery each own a sub-100-line entrance, adjacent catalog,
  phase coverage tests, and only their named rule-family folders; the crate
  rule root is now a small responsibility map rather than a mixed coordinator.
- [x] Move the 800-line pressure-rematerialization fixture suite out of the
  production compute leaf; the exact rule entrance now descends separately to
  compute, identity, model, validation, and tests.
- [x] Move broad liveness and pre-allocation machine-effect codec fixtures out
  of production compute/codec leaves while preserving shared typed fixtures
  for independent validators.
- [x] Move home-assignment compute and fixed-view-copy codec fixtures into
  explicit path-bound sibling leaves without changing their private test scope
  or test names.
- [x] Replace flat register-home compute/replay leaves and their forwarding test
  bridge with mirrored domain/conflict/placement taxonomies. Their small
  producer and validator entrances own the complete-roster joins independently,
  and focused tests descend through constrained domains, ties, early clobbers,
  alias footprints, and determinism.
- [x] Split target register-environment custody into a small construction and
  validation entrance above explicit target catalog, validated model,
  validation mechanics, and tests.
- [x] Split selected-instruction staging into retained model, construction,
  fixed-input constraint projection, and independent replay leaves; its
  entrance owns environment-to-replayed-result custody.
- [x] Split optimized target-operation lowering into retained model and exact
  source-route leaves; its entrance owns every lowering-to-custody join and
  provider-installation retention.
- [x] Split bounded target-operation assignment into retained model, source
  lowering, assignment construction, and independent custody replay leaves;
  its entrance owns the construction-to-replay admission join.
- [x] Split selected-CFG liveness staging into model, analysis, independent
  replay, and custody projection leaves below one replay-gated entrance.
- [x] Split CFG-aware live-range staging into model, analysis, independent
  replay, and custody projection leaves below one replay-gated entrance.
- [x] Split exact fixed-view-copy recovery into model, materialization,
  independent replay, and custody projection leaves below a source-validated
  and replay-gated entrance.
- [x] Split transformed-selected reanalysis into complete recomputation,
  independent replay, transition invariant, custody, and model leaves below
  one source-validated entrance.
- [x] Split allocation-legality staging into explicit availability policies,
  analysis, independent replay, custody projection, and model leaves; its
  entrance owns policy selection and the shared replay-gated stage join.
- [x] Split baseline and post-copy register-home staging into construction,
  independent validation, custody projection, and model leaves; the shared
  entrance grants each source family custody only after complete replay.
- [x] Split post-fold and complete selected-lowering home staging into model,
  construction, independent validation, manifest projection, and custody
  leaves below one replay-gated entrance.
- [x] Split post-allocation machine analysis by source-route construction,
  replay/custody validation, and sealed plan model; its entrance owns the
  common effects-plus-machine custody join.
- [x] Split active-resident rematerialization into producer computation,
  independent replay validation, custody projection, and model leaves; its
  entrance alone grants stage custody after compute-to-validation replay.
- [x] Split machine-effect staging into an exact ISA catalog, analysis,
  source-route construction, independent replay, custody, and model leaves;
  its entrance replay-gates every supported selected-source lineage.
- [x] Split active-resident selected-form encoding into retained model,
  source-validated construction, custody projection, independent replay, and
  test-support leaves below one replay-gated entrance.
- [x] Split active-resident resolved-layout staging into retained model,
  policy-checked construction, aggregate custody projection, independent
  replay, and test-support leaves below one replay-gated entrance.
- [x] Split structural-Unit function-relative realization into model,
  construction, independent replay, source admission, manifest reconstruction,
  and custody leaves below one replay-gated entrance.
- [x] Split active-resident function-relative realization into model,
  construction, independent replay, source projection, manifest, custody, and
  test-support leaves below one replay-gated entrance.
- [x] Split receiver-free Unit function-relative realization into model,
  construction, independent replay, source admission, manifest reconstruction,
  and custody leaves below one replay-gated entrance.
- [x] Split the flat optimized object-artifact stage; its small entrance owns
  the terminal/object build-and-replay join while model, reconstruction, and
  canonical codec descend into named leaves.
- [x] Split the flat relocation-free object-container stage; its entrance owns
  construction/replay while model, object assembly, manifest codec, and tests
  descend independently.
- [x] Split the pre-physical manifest monolith into retained model, record
  projection, independent join validation, canonical codec/identity, human
  rendering, and focused test leaves below one projection-to-replay entrance.
- [x] Split complete-unit operation contracts into value flow, ordered node
  contracts, service/call, structural-access, claim-transfer, payloadless-case,
  boundary, and scalar-type leaves below one per-node validation entrance.
- [x] Split current-ownership validation into entry model, ordered CFG replay,
  frontier mutation, cleanup, structural-placement, and residual-affine leaves
  below one current-entry reconstruction-to-replay entrance.
- [x] Split complete-unit structural catalogs into ordered type/domain indexing,
  content projection, type declarations, function-local catalogs, witnesses,
  provider specialization, and path-resolution leaves below one catalog join.
- [x] Split independent rewrite accounting into adjacent/non-adjacent merge,
  terminal fusion, dead scalar, proof identity, common-subexpression,
  substitution, and threading leaves below shared custody/substitution contracts.
- [x] Split independent GVN candidate validation into exact rule classification,
  proof admission, expression keys, dominance reconstruction, local/dominating
  elimination, and phi-translated join synthesis below one custody-and-dispatch
  entrance.
- [x] Split independent dead-scalar validation into exact rule classification,
  an exhaustive operation-safety partition, and rewrite replay below one
  custody-and-analysis-contract entrance.
- [x] Split independent redundant-parameter validation into witness replay,
  closed-region observation normalization, outside-region comparison, and
  exhaustive operation rewriting below one custody-and-analysis entrance.
- [x] Split per-function unit validation into CFG, entry/parameter, result,
  structural-root, fact-index, and provenance/fuel/effect leaves below one
  ordered acceptance entrance.
- [x] Split derived operation metadata into dominance/control-flow, declared
  places, scalar values, provenance, successor edges, and ownership leaves
  below one place-and-claim admission entrance.
- [x] Split current-value-range validation into applicability, independent
  reconstruction, canonical proof-goal, and exact interval-algebra leaves
  below one fact-first validation entrance.
- [x] Split optimized abstract-plan projection into one meaningful stage
  entrance above retained models, typed errors, catalog-derived run replay,
  all-candidate decision custody, and source-shape projection. Run replay
  descends into rule-set, commit, candidate-decision, and record rungs; the
  candidate-decision entrance descends into manifest, retained-declaration, and
  baseline leaves. Source projection descends into roster and function-shape
  mechanics.
- [x] Split verified/transformed optimizer-context validation into immutable
  context projection, seed/fact replay, surviving frontier validation, and
  signature/roster custody below one revision-policy entrance.
- [x] Split complete-unit core validation into canonical identity/fact indexes,
  active/pruned machine and structural/service catalogs, retained affine
  authority, and final entry/frontier checks below one ordered entrance.
- [x] Replace the 5,000-line target-legalization/instruction-selection entrance
  with separate legalization and selection joins; construction, independent
  replay/validation, identities, constraints, structural/scalar families, and
  focused fixtures descend through named leaves, and the architecture gate now
  governs the entire lowering crate.
- [x] Replace the 4,930-line abstract-to-target lowering entrance and
  2,714-line fixture monolith with settlement/function coordination,
  per-result routing, scalar setup/special/conditional/straight-line families,
  structural routes/layout, Unit lowering, boundary settlement, cleanup, and
  mirrored test families. The architecture gate now governs this entire stage.
- [x] Replace the 3,133-line compatibility target-assignment entrance with
  plan and function coordinators, an exhaustive carrier-family router, and
  named cleanup, boundary, Unit, structural, scalar-control, placement,
  expression-frame, typed-expression, and parameter-discovery leaves. The
  architecture gate now governs the entire assignment stage.
- [x] Replace the 2,924-line Terminal-to-abstract entrance with separate
  artifact admission/replay, verified optimizer-unit construction,
  provider-installation custody, and machine-lowering joins. Proof-fact,
  proof-question, ownership-frontier, payloadless, ordinary-machine, and
  structural-machine mechanics descend through named leaves, and the
  architecture gate governs the entire stage.
- [x] Replace the 1,860-line optimized ProgramStorage semantic-wrapper object
  file with one small owning stage entrance above retained models, semantic
  validation, object composition/validation/manifest construction, custody,
  codecs, and focused fixtures. The architecture gate governs this slice and
  requires each real coordination seam to remain visible.
- [x] Replace the 775-line Terminal-Psi-to-native crate entrance and adjacent
  flat wrapper-encoding stage with a crate responsibility map plus source-entry
  settlement, native realization, provider admission, machine emission,
  artifact assembly, diagnostics, encoding projection, and replay leaves. The
  architecture gate now governs the entire crate and its real stage joins.
- [x] Replace remaining flat executable stages and mixed-responsibility files
  with semantic folders whose small `mod.rs` owns the real stage join. Tighten
  the production-file ceiling as each named legacy leaf is removed.

## P0 — Opt-in and compatibility firewall

- [x] Exact named build selections and canonical order.
- [x] Duplicate, unknown, noncanonical, trailing, and old-version rejection.
- [x] Full selection identity retained across phase projections.
- [x] Empty selection preserves ordinary compilation.
- [x] Add golden canaries comparing no-selection source acceptance,
  diagnostics, interpreter output, native bytes, and artifact metadata on every
  supported host/target pair.
- [x] Add an exact per-rule disable/rollback path to release tooling.

## P1 — Shared rule engine and analysis system

- [x] Stable rule contracts, safety classes, required analyses, invalidations,
  budgets, decisions, typed facts, and manifests.
- [x] Deterministic Psi catalog, scheduling, fixed point, and replay.
- [x] Revision-aware analysis cache with stale-analysis tests.
- [x] Add one obvious ordered post-allocation machine catalog at the
  machine-rule entrance and route its typed result through one complete
  physical realization carrier.
- [x] Unify common catalog descriptors without erasing representation-specific
  candidate and validator types.
- [x] Add catalog coverage tests proving every selected name is scheduled once
  or rejected for an explicit phase/target reason.

## P2 — Validation and publication

- [x] Representation and rule-level independent validators.
- [x] Identity-bound decisions, pass records, manifests, and work usage.
- [x] Source-to-optimized Psi projection and lower-stage custody checks.
- [x] Retain and rebind every validated Psi candidate, Applied or skipped, to
  its selected rule contract, independently replayed declaration, exact input
  revision, manifest analyses/facts, baseline cost, pass partition, commit
  disposition, and external-policy mirror across the complete six-pass Psi
  catalog.
- [>] Complete translation validation for all lowering and machine rule
  families. Selected-lowering incoming-u12 add/subtract folds,
  straight-line integer-immediate, Boolean-immediate, scalar-Crash,
  integer-parameter, Boolean-parameter, Boolean-not-parameter, and ordered
  Boolean-equal-parameters, typed integer-equal-parameters, and typed
  integer-less-than-parameters, integer-less-or-equal-parameters,
  integer-bitwise-not-parameter, integer-widen-parameter, and proof-bearing
  integer-exact-cast-parameter, integer-bitwise-and-parameters, and
  integer-bitwise-or-parameters
  abstract-to-target translation,
  layout-independent baseline, MOVN, XOR-zero, MOV-r32-imm32, CBNZ dispositions,
  structural-Unit encodings, and resolved function-relative layouts now replay
  independently. Structural-Unit selected validation also reconstructs ABI
  layout and call constraints without construction helpers. The ordinary
  ranked-`u32` machine carrier now has target-decoder-led ordinary object,
  final-image, installation, and native-artifact replay. Other lowering and
  publication routes still need closure.
- [ ] Add generated differential testing across interpreter/reference native
  execution for exact integer, float, trap, atomic, placed-memory, cleanup, and
  transition cases.
- [ ] Add end-to-end mutation tests for every manifest/custody field.

## P3 — Psi optimizer

- [x] Control-flow cleanup with independent graph reconstruction.
- [x] SCCP with exact range/constant facts.
- [x] Copy propagation with dominance, ownership, and effect barriers.
- [x] GVN for local, dominating, and phi-translated expressions.
- [x] Dead pure scalar elimination using a closed operation partition.
- [x] Proof-check elision and proof-certified exact integer identities.
- [x] Extend GVN with an exhaustive five-row obligation-free wrapping neutral
  arithmetic identity partition and independent validator reconstruction.
- [x] Extend GVN and scalar identities to further exact operation families only
  with exhaustive producer/validator partitions. The separate wrapping-shift
  zero-count, wrapping multiply-zero, saturating neutral-arithmetic, and
  saturating multiply-zero families and the bitwise neutral-literal family are
  complete. The separately named bitwise absorbing-literal family is also
  complete, including deterministic overlap coverage against the earlier
  neutral-literal family.
- [ ] Implement loop-invariant code motion after cyclic Terminal-Psi semantics
  are implemented by ordinary execution validation.
- [x] Keep suspension as an interprocedural state of the exact call rather than
  a second local CFG successor. CFG analyses retain the ordinary completion
  edge and consult `SuspensionCrossingId` plus the checked crossing frontier.
- [x] Admit finite cyclic control in Terminal Psi without a `Loop` terminator.
  The verifier derives SCC topology; block parameters and successor arguments
  carry loop state; ownership converges by fixed point; reducibility is only an
  optimization classification; and ranked, unranked, and quantitatively bounded
  components retain distinct progress authority.
- [ ] Generalize the exact unsigned-countdown carrier into ordinary cyclic
  execution. Replace DAG-only dominance and frontier walks with verified
  fixed-point algorithms, derive canonical `CycleComponentId` values, normalize
  source rankings into well-founded relation/decrease certificates, admit
  unranked productive components without termination authority, and report
  structured finite-work failures by component and directed cause. Keep
  fixed-fuel certificate construction a separate stricter gate.
- [ ] Retarget LICM and other loop-forest consumers to validated Terminal SCCs.
  A transform that changes component membership, loop-carried custody, or
  ranking edges must invalidate the old certificate and pass ordinary Terminal
  verification; unsupported and irreducible regions may be forwarded unchanged.

## P4 — Lowering optimizer

- [x] Target/legalized operation and selected-instruction validation.
- [x] Make mandatory selected construction catalog-driven: one visible ordered
  scalar family inventory produces each complete virtual-register-plus-block
  body after one exact-zero-or-one classification.
- [x] Exact incoming u12 add/subtract immediate folds.
- [x] Generalize legalization into one ordered declarative catalog of target
  forms, constraints, costs, producer matcher kinds, and independent validator
  kinds. Its twelve rows cover all current scalar, plain Unit, and structural
  Unit families; omission and ambiguity fail closed.
- [ ] Add exact address-mode folding, compare/branch selection, extension
  elimination, and constant materialization rules one named family at a time.
- [ ] Validate ABI operands, calls, clobbers, effects, traps, provenance, and
  logical fuel across every selected rule.

## P5 — Register allocation and frame assignment

- [x] Selected-CFG liveness and live-range fragments.
- [x] Register views/units, aliasing, availability, and allocation legality.
- [x] Transition-free home assignment and post-allocation manifest.
- [x] Exact fixed-view copy and active-resident rematerialization recovery
  slices.
- [x] Replace the remaining narrow allocator with a general deterministic
  interference allocator for the currently admitted transition-free,
  spill-free domain.
- [ ] Add spill choice, insertion, reload/store validation, and stack-slot
  coloring.
- [ ] Add coalescing, live-range splitting, fixed/precolored intervals, and
  rematerialization cost decisions.
- [ ] Implement frame layout, alignment, red-zone/shadow-space, unwind, probing,
  stable-address loans, and dynamic-allocation constraints.
- [x] Add x86-64 and AArch64 target-register-environment ABI/call-clobber
  corruption matrices.
- [ ] Extend ABI/call-clobber validation through general selected scalar calls
  and live-across-call allocation after general calls enter the selected CFG.

## P6 — Machine optimizer

- [x] Target-neutral post-allocation symbolic machine plan and effects.
- [x] AArch64 CBNZ fusion and MOVN materialization rules.
- [x] x86 rel8 layout relaxation.
- [x] x86 XOR-zero materialization with RFLAGS-dead proof.
- [x] x86 MOV-r32-imm32 zero-extended i64 materialization with exact encoded
  subview, write-semantics, and byte-count replay.
- [ ] Add declarative peephole matching over symbolic instructions, physical
  register units, effects, traps, memory, stack, and control flow.
- [ ] Add exact copy removal, redundant extension removal, address folding,
  compare/test selection, and scheduling rules where independently verifiable.
- [ ] Add target cost models as non-authoritative identities; semantic
  validation must not depend on cost estimates.
- [x] Generalize whole-function encoding/layout/realization so new form
  substitutions add one rule leaf and catalog entry, not a new route family.

## P7 — Proof-, ownership-, and state-aware optimizations

- [x] Accepted-obligation identities can authorize exact proof-check and scalar
  rewrites.
- [ ] Alias/borrow-aware load forwarding, dead-store elimination, and mutation
  motion.
- [ ] Field/variant relevance and invariant-window specialization.
- [ ] Cleanup and transition reachability pruning.
- [ ] State-argument/result specialization with edge provenance.
- [ ] Interprocedural service/call summaries and proof-bound inlining.
- [ ] Proof-directed loop bounds, induction simplification, and vectorization
  with exact lane semantics.

Each rule must name the exact proof/ownership facts consumed and retain their
identities in the decision and publication chain.

## P8 — Search and ML extensibility

- [x] Identity vocabulary for workload profile, decisions, cost model, rule
  set, selections, and ledger.
- [x] Versioned model input schema containing source/target/rule/fact features
  without raw pointers or unstable insertion order.
- [x] Versioned output schema naming existing candidate identities plus scores
  or decisions.
- [x] Record-only mode that cannot change baseline output.
- [x] Deterministic replay with exact identity mismatch rejection.
- [ ] Sandboxed external policy boundary with timeout/resource limits and an
  explicit fallback.
- [ ] Offline corpus capture, training, evaluation, and regression tooling.

ML may rank declared equal transformations. It cannot invent an unvalidated
rewrite or opt a program into lossy floating-point semantics.

## P9 — Testing, stabilization, and rollout

- [x] Repair the external-decision and projection exact division/remainder
  fixtures. Their proof bundles now derive verifier-reconstructed `/ 1`,
  zero-dividend, `% 1`, and signed `% -1` definedness propositions from exact
  constant semantic axioms through checked integer-bound substitution; proof
  admission remains unchanged.
- [>] Per-rule positive, negative, boundary, disabled, budget, determinism,
  idempotence, and corruption suites. Disabled-selection coverage is complete
  for every exact public name and phase projection. Applied and skipped
  decision declarations now retain and independently replay analyses, facts,
  predicted cost, pass, validator, input revision, verdict, and commit
  disposition; the complete six-pass Psi catalog and coordinated external-log
  mutations are covered. The remaining behavioral dimensions still need
  focused gap closure.
- [x] Cross-rule phase-composition matrix, including deliberate fail-closed
  unsupported combinations.
- [ ] Randomized valid-Psi and selected-machine differential corpus.
- [ ] Supported target/OS allocator, encoding, unwind, object, and callable
  matrix.
- [ ] Compile-time, memory, code-size, and runtime benchmarks with versioned
  non-authoritative evidence.
- [ ] Exact-rule release notes and rollback procedures.
- [ ] Owner-reviewed promotion criteria per rule; never implicit broad levels.

## Near-term execution order

1. [x] Finish the x86 XOR-zero leaf encoder and symbolic rule.
2. [x] Introduce the post-allocation stage catalog and typed result.
3. [x] Split the encoding stage monolith along that taxonomy (the machine stage
   split is complete).
4. [x] Carry the generic result through encoding, layout, realization, and
   artifact custody with exact byte-delta tests.
5. [x] Retain exact build opt-in and direct/selected XOR-zero coverage through
   publication and callable entry.
6. [x] Finish the remaining stage-entrance taxonomy migration and make the
   navigation contract executable for each migrated stage.
7. [x] Add the broader target-register-environment ABI corruption matrix.
8. [x] Move selected-lowering, allocation-recovery, and
   post-allocation-machine catalogs to their rule-owning crate entrances, and
   make the navigation gate enforce those ownership points.
9. [x] Add global exact-name-to-rule-stage disposition coverage.
10. [x] Add exact target-applicability dispositions at the owning catalogs.
11. [>] Finish workspace validation and rollout canaries before promoting any
   rule beyond explicit opt-in.
12. [x] Replace selected-form producer replay with an independent decoder-led
    validation rung and enforce the boundary architecturally.
13. [x] Replace the x86-rel8-only fragment admission carrier with one generic
    selected-lowering carrier; do not add add/subtract route variants.
14. [x] Unify fixed-view-copy and active-resident realization under one generic
    allocation-recovery carrier before extending either publication route.
    The shared carrier now owns tagged source custody, machine plan, generic
    encoding, resolved layout, whole-function exit, v9 realization manifest,
    and fragment/object/callable admission for both exact rules.
15. [x] Make resolved-layout validation independent before claiming complete
    translation validation for those generic publication routes.
16. [x] Add x86 MOV-r32-imm32 zero-extended i64 materialization as one exact
    catalog rule, and route it through the closed generic materialization and
    publication carriers without a new vertical pipeline.
17. [x] Centralize physical phase-composition policy and exhaustively classify
    all 210 exact-name pair/architecture cells, with higher-order composition
    anchors and full-Psi overlay invariance.
18. [x] Add the independent abstract-to-target validation entrance, retain its
    partial family receipt in optimized target custody, and close the exact
    straight-line integer-immediate translation family on all native targets.
19. [x] Add parameterless straight-line Boolean-immediate translation as a
    separate independent family row and retain its exact custody through
    optimized target lowering on all native targets.
20. [x] Make abstract-to-target catalog classification exact-zero-or-one, bind
    one typed disposition directly to every function-roster row, and add the
    parameterless straight-line scalar-Crash family across all native targets.
21. [x] Add straight-line integer-parameter returns as a disjoint family and
    independently replay native ABI register/stack placement across all five
    targets through optimized target custody.
22. [x] Add the exact Boolean-parameter sibling while moving shared
    source-envelope and native-ABI replay beneath one governed
    `straight_line_parameter` coordination entrance.
23. [x] Add exact Boolean-not-parameter replay, retaining operand/result and
    operation/edge identities, while descending source grammar and catalog
    adapters into explicit subfolders before either entrance could grow opaque.
24. [x] Replace selected construction's forwarding wall, mixed plan file, and
    duplicated 966-line scalar classifier with meaningful result-family
    entrances and one seven-row scalar construction catalog.
25. [x] Add exact Boolean-equality-of-parameters translation replay across all
    native targets, retaining ordered or identical operand identities and ABI
    locations through optimized target custody. Split common source envelopes,
    derived-expression coordination, and family error/receipt vocabulary into
    small semantic entrances while doing so.
26. [x] Replace the run-to-abstract projection forwarding wall and 422-line
    catchall with one meaningful stage entrance above replay and source
    taxonomies. Bind every Applied Psi decision to exact selected contracts,
    pass partitions, declaration facts, manifest analyses, and baseline costs
    with a six-pass coordinated-corruption matrix.
27. [x] Add typed integer-equality-of-parameters translation replay across all
    native targets and integer signs/widths. Retain ordered or identical
    operand identity, exact integer type, ABI placements, provenance, and
    optimized-target custody. Split the derived-expression rung into a small
    unary entrance and a named equality leaf before adding the family.
28. [x] Add typed integer-less-than-of-parameters as its own ninth translation
    row across all native targets and integer signs/widths. Retain ordered or
    identical operands, exact integer type, full-roster ABI placements,
    provenance, and optimized-target custody beneath a named ordering replay
    leaf; keep less-or-equal as the separate family completed in item 30.
29. [x] Retain every validated Psi candidate declaration and replace flat
    Applied-only publication replay with one small all-candidate coordinator
    over manifest, independent declaration, and baseline-policy custody.
30. [x] Add typed integer-less-or-equal-of-parameters as the tenth independent
    abstract-to-target family. Retain exact signedness/width, ordered or
    identical operand identity, register/stack ABI placements, provenance, and
    optimized-target custody through named inclusive-ordering leaves.
31. [x] Add integer-bitwise-not-of-one-parameter as the eleventh independent
    abstract-to-target family. Descend integer source grammar through one
    meaningful typed coordinator first, then retain exact integer type,
    operand/result identity, register/stack ABI placement, provenance, and
    optimized-target custody through the named unary leaf.
32. [x] Replace start-ordered transition-free register-home assignment with a
    deterministic constrained interference allocator. Quotient exact ties,
    derive interference and directional early-clobber edges, prioritize the
    most constrained domain, independently replay the policy, and split both
    sides plus focused fixtures into visible semantic rungs.
33. [x] Add `IntegerWiden(parameter)` as the twelfth independent
    abstract-to-target family. Cover all 18 native widening relations and both
    register and stack placement on all targets, retain distinct source/target
    type custody, and replace mixed parameter catchalls with the explicit
    direct/unary/comparison taxonomy before extending the catalog.
34. [x] Add proof-bearing `IntegerExactCast(parameter)` as the thirteenth
    independent abstract-to-target family. Cover all 38 native nonidentity,
    nonwiden exact-cast relations; retain obligation, source/target types,
    register/stack placement, and provenance; exercise optimized custody from
    a canonical Terminal proof artifact; and split mixed unary/fixture
    catchalls before adding the row to the sole enable/disable catalog.
35. [x] Add `IntegerBitwiseAnd(parameter, parameter)` as the fourteenth
    independent abstract-to-target family. Introduce a named `bitwise` taxonomy
    rung; cover every native fixed-width integer type and native target with
    register/stack rosters, reversed and identical operands; reject address and
    nonnative carriers; retain exact provenance, ordered operands, ABI
    locations, and optimized-target custody; and pin the entrances with
    architecture tests.
36. [x] Add `IntegerBitwiseOr(parameter, parameter)` as the fifteenth exact
    translation family. Reuse the named `bitwise` rung with distinct OR leaves,
    extract shared ABI/provenance replay below the entrance, split bitwise
    vocabulary and fixture maps into exact-family leaves, cover all native
    type/target/register-stack cells plus ordered and identical operands, and
    reject address/nonnative carriers and every source/target field mutation.
