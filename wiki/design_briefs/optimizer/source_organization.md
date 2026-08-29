# Optimizer Source Organization

This brief is a navigability contract. The architecture entrance is
[optimizer_architecture.md](../optimizer_architecture.md).

## Entrance rule

A human entering any optimizer stage must find a small `lib.rs` or `mod.rs`
that explains the stage and exposes its real coordination point. It must name
the next semantic rungs and own the stage catalog or compute-to-validation
join. It must not contain rule mechanics, codecs, broad fixtures, or hundreds
of accessors. A pure re-export wall is also insufficient.

Preferred entrance size is below 100 lines. Crossing 200 lines requires a
specific reason in review. Production files have a 1,000-line hard ceiling;
dedicated test fixtures retain a 1,500-line ceiling. These are rejection
ceilings, not healthy targets: leaves approaching them should be split at
their next semantic rung before another rule grows them. The architecture
guard's legacy exception table is empty and new exceptions are forbidden.

## Taxonomy

Use folders for semantic responsibilities, then exact families:

```text
analyses/<fact>/{mod,model,compute,identity,validate,tests}.rs
planning/<plan>/{mod,model,compute,identity,validate,codec,tests}.rs
rules/<target>/<exact-rule>/{mod,model,compute,identity,validate,codec,tests}.rs
stages/<stage>/{mod,catalog,model,compute,validate,tests}.rs
```

Not every leaf needs every file. Do not create empty layers or one crate per
rule. Shared code belongs at the nearest semantic ancestor only when two or
more leaves use the same concept and the concept has one contract.

## Catalog rule

There is one visibly named catalog per rule stage. It owns:

- canonical rule order;
- exact source-visible selection mapping;
- rule descriptor construction; and
- coverage tests proving every declared rule is either scheduled or rejected
  for a stated target/phase reason.

Every catalog uses the same small generic name/payload descriptor from
`omega-optimization-core`. The payload stays stage-owned: Psi retains its rule
registry constructor, register allocation retains its lowering policy, and
machine/layout stages retain canonical architecture applicability. Ordered-name
arrays, where compatibility still needs them, are const projections of those
descriptor catalogs and are never edited independently.

No second match table may silently become an alternate registry. Build
vocabulary, reports, and codecs derive from or exhaustively test against the
closed `Optimization::ALL` vocabulary.

Catalog ownership follows rule mechanics. A cross-stage custody crate may
dispatch a rule-owned selection into its own carrier types, but it may not own
a proxy enable/order table. The rule entrance must project selections through
its adjacent catalog before custody code sees the result.

For Psi, `rules/mod.rs` is the selection/application entrance,
`rules/catalog.rs` is the complete ordered pass table, and every
`rules/passes/<exact-pass>/catalog.rs` owns only that pass's exact rule order.
Enabling or disabling a Psi pass therefore changes one visible descriptor
table; changing a rule's within-pass order changes one local catalog.

## Squalr pattern carried forward

The clearest concrete reference is Squalr's
`registries/scan_rules/pointer_scan_rule_registry.rs`: one short registry
visibly lists the built-in planning rules. The `rule_map_search_kernel.rs` leaf
owns the SIMD-linear/scalar-linear/scalar-binary choice, while
`pointer_scans/pointer_scan_dispatcher.rs` has one obvious application loop.
The element-scan registry follows the same shape for its parameter and filter
rule families.

Omega keeps that navigational shape: catalog, named leaves, one application
loop. Omega strengthens it with deterministic catalog order, exact typed
selection names, immutable candidate plans, independent validation, and
identity-bound receipts. It deliberately does not copy the global unsafe
singleton, string-keyed scheduling, or unordered `HashMap` iteration.

The current source-visible rule paths are:

| Phase | Entrance | Catalog | Next rung |
|---|---|---|---|
| Mandatory legalization | `omega-target-operations-to-selected-instructions/src/legalization/mod.rs` | adjacent `catalog.rs` | separate `source/` producer and `replay/` validator taxonomies |
| Psi | `omega-psi-optimizer/src/rules/mod.rs` | `rules/catalog.rs` | `rules/passes/<exact-pass>/catalog.rs` |
| Selected lowering | `omega-regalloc/src/rules/selected_lowering/mod.rs` | adjacent `catalog.rs` | `literal_fold/` |
| Allocation recovery | `omega-regalloc/src/rules/allocation_recovery/mod.rs` | adjacent `catalog.rs` | `fixed_view_copy/` and `pressure_rematerialization/` |
| Post-allocation machine | `omega-machine-optimizer/src/rules/mod.rs` | `rules/catalog.rs` | `rules/<isa>/<exact-rule>/` |
| Function-relative layout | `omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/mod.rs` | adjacent `catalog.rs` | `compute`, `validation`, and typed stage model |

The architecture gate requires every listed rule-stage entrance to retain its
selection/application function and adjacent primary descriptor catalog. It
also guards each migrated pipeline custody join independently, so making the
registry clear cannot turn the stage entrance into a re-export wall.

## Test placement

Focused tests live beside the responsibility they verify. Integration tests
mirror production taxonomy under `tests/coordination`, `tests/stages`, and
`tests/fixtures`. Large fixture catalogs are split by typed artifact family.

Every rule has positive, negative, boundary, disabled-selection, budget, and
corruption tests. Every rule entrance has a catalog-coverage test; custody
entrances have focused join and corruption tests instead.

## Current reference slices

- `omega-psi-optimizer`: analyses, pass manager, ordered rules, and pass
  families.
- `omega-optimization-validation`: candidate validators and complete-unit
  validators remain independent of producers.
- `omega-regalloc`: analyses, allocation decisions, and exact recovery rules.
- `omega-machine-optimizer`: analyses, post-allocation planning, and rules
  grouped by ISA and exact transformation.
- `omega-optimization-pipeline`: coordination separated from ordered custody
  stages and tests that mirror those stages.

## Refactor trigger

Refactor before adding a rule when any of these are true:

- the stage entrance needs rule-specific mechanics;
- enabling a rule requires edits to several unrelated route enums;
- a new rule copies an existing owning carrier across encoding/layout/
  realization;
- a file mixes model, compute, validation, codec, and broad tests; or
- the only way to locate built-ins is repository-wide search.

The x86 XOR-zero milestone exercised this trigger across the full physical
conveyor. The symbolic-machine rule crate now owns one catalog and the pipeline
consumes it into one typed result;
encoding, layout, whole-function exit, and realization consume that result;
and fragment publication has one generic post-allocation source. Adding
XOR-zero did not copy the former MOVN route, and the named CBNZ/MOVN owning
complete routes were removed.

Physical composition is likewise navigable rather than implicit. A 74-line
`routes/composition/mod.rs` entrance owns the catalog-backed phase/target
preflight and returns a closed route model from `model.rs`; its focused matrix
stays in `tests.rs`. The compiler-facing physical entrance dispatches only from
that result, while lower route leaves retain independent custody checks.

The organization gate is executable for the catalog-driven reference slices.
`omega-optimization-core` declares
each exact name, stable tag, build case, build counter, phase, and canonical
order once; its descriptor generates both `Optimization` and
`Optimization::ALL`. Both injected build preludes are parsed against those
generated views and every exact name is evaluated independently through both
preludes, so swapped name-to-counter mappings fail.

The mandatory target-legalization and instruction-selection crate is now a
governed reference slice as well. Its crate map points to a 66-line
legalization join and a 30-line selection join. The 92-line source entrance
owns one target-function roster loop; the 94-line replay entrance owns its
independent whole-plan comparison. Each descends through scalar, plain Unit,
and structural-Unit leaves, with meaningful 90-line producer and 47-line replay
entrances for the structural family. Selected-plan construction and validation
descend separately through constraints, roster construction,
function/block/register checks, integrity replay, and canonical identity.
Structural-Unit validation independently reconstructs its ABI layout and call
constraint instead of importing producer construction. Leaf replay has a
95-line source-custody/return-sealing entrance over exhaustive recipe dispatch,
exact-arithmetic validation, immediate validation, and fuel replay. The former
1,022-line replay catch-all is gone, and no replacement exceeds 464 lines.

Selected construction now descends literally beneath that selection join. A
52-line `construction/mod.rs` owns scalar/plain-Unit/structural-Unit roster
assembly. The structural entrance joins `layout.rs`, optional `call.rs`, and
return construction. The scalar entrance reconstructs common condition input,
then walks the sole ordered `SCALAR_FAMILIES` catalog. Its seven exact rows
select immediate, entry-parameter, direct and widened add/subtract, or
active-resident add-chain bodies. Every leaf returns registers and blocks
together, so the source family cannot be classified independently for two
projections. The architecture gate requires all three real entrances, the
catalog, ambiguity rejection, and removal of the former flat `plan.rs` and
966-line `scalar.rs` files.

Mandatory legalization carries the Squalr-style registry shape directly: one
adjacent twelve-row `LEGALIZATION_FORMS` catalog owns precedence, typed recipe,
shape, and descriptive planning-cost data for seven scalar, one plain Unit, and
four structural-Unit forms. Producer matcher kinds and independent validator
kinds are distinct closed dispatches. Removing the sole row for a recipe
disables it, while missing and ambiguous lookup fails closed. The architecture
gate rejects alternate family catalogs, requires the four real source/replay
coordination entrances, and recursively prevents either side from reaching the
other's mechanics.

The legalized-operation representation follows the same downward navigation.
Its former 2,098-line crate root is now a 17-line responsibility map over
`model/`, `identity/`, `validation/`, and `tests/`; identity and structural
calling mechanics descend another rung into named leaves. Typed plain-Unit and
structural-Unit recipes participate in the V9 identity rather than being
inferred from an untyped roster after admission.

Ranked-`u32` object admission is governed as one exact validation subtree, not
as a rule catalog. Its sub-100-line `ranked_u32_countdown/mod.rs` entrance owns
classification and the layout/contract/fuel join; `layout.rs` consumes only
opaque target-decoder evidence, `contract.rs` reconstructs semantic, ABI, and
frontier custody, and `fuel.rs` reconstructs the nine exact attribution rows.
An architecture guard prevents this subtree from depending on machine emission
or calling either target encoder, and the entrance keeps final-image authority
explicitly fenced.

Native-fuel instrumentation and replay now follow the same downward navigation
on both sides of their trust boundary. Each `native_fuel/mod.rs` is a small real
join: machine emission classifies ranked custody before general two-pass
instrumentation, while image emission admits the semantic object before general
meter replay and ranked branch replay. Their `general.rs` leaves retain the
ordinary algorithm and focused fixtures. Image replay's 45-line
`ranked_u32_countdown/mod.rs` entrance owns classification plus the object and
final-publication joins, then descends into coordinate decoding, object
admission, and final-byte publication leaves. The architecture gate governs
both trees and prevents image replay from calling charge, cold-dispatch, or
ranked-branch producer encoders.

Psi SCCP constant evaluation now follows the same downward-navigation rule.
Its 35-line entrance owns the shared SCCP rule contract and names boolean and
integer families. The integer entrance descends into binary arithmetic/shifts,
exact casts, unary operations, and shared fact lookup. The ordered SCCP catalog
above it remains the sole enable/order table; no leaf in the constant-evaluation
subtree exceeds 750 lines.

Optimization-unit rewrite candidates now expose a 76-line admission entrance.
Scalar and control-flow constructors rejoin there for canonical decision-point
derivation, common custody checks, exact patch-family validation, identity
encoding, and immutable construction. Read-only access is a separate leaf.
The former 1,253-line construction/validation/access file is gone, and no leaf
in the candidate subtree exceeds 400 lines. The adjacent immutable model has a
57-line aggregate/map that owns `PsiOptimizationUnit` above executable graph,
proof custody, derived range, ownership-frontier, and one-time attachment
leaves. Its former 1,023-line mixed model is gone and no replacement exceeds
323 lines.

The layout-independent selected-form encoding stage applies the same
navigational rule to a custody join rather than a rule catalog. Its 57-line
entrance owns construction followed by independent admission. Validation then
descends through `ordinary`, `structural`, target-decoding `row`, and
`aggregate` leaves. The architecture gate rejects any validation dependency
on producer computation or encoder helpers, so the small entrance cannot hide
a producer-as-validator cycle.

The resolved selected-form layout stage now makes the next rung equally
literal. Its stage entrance owns `compute -> independent validation` and names
`ordinary`, `structural`, and `validation` directly. Ordinary construction has
a tiny map into `policy`, `order`, `plan`, `function`, `row`, and `branch`;
ordinary validation has its own `roster`, `order`, `plan`, and `function`
subtree. Branches remain separate because this is the only layout concern that
creates new target bytes. The architecture gate rejects the former flat
`rules.rs` bucket, requires both rung maps, and prevents validation from
calling construction or target encoders.

The preceding abstract-to-target stage is governed by the same contract. Its
crate map points to a settlement-and-installation coordinator, then to one
per-result function route. Scalar lowering visibly orders setup, special
forms, conditional routing, and straight-line lowering; structural lowering
visibly chooses direct-call return or explicit structural return. Unit,
boundary-settlement, cleanup, structural-layout, and four focused fixture
families remain separate leaves. Straight-line scalar lowering now owns its
operation lifecycle in a 50-line entrance, then descends into exhaustive
operation routing, integer arithmetic, integer conversion, and terminal-edge
construction; its former 1,238-line match is gone and no replacement exceeds
600 lines. Conditional-scalar lowering has its own 37-line entrance that tries
direct call/boolean/comparison work before exhaustive integer operation
routing; binary-kind folding and exact/wrapping shift semantics are separate
shared leaves, and its former 1,111-line file is gone with no replacement over
562 lines. Unit lowering now keeps its ordered setup/application loop separate
from boundary settlement and admitted-provider realization, plus
cleanup-sensitive Unit return validation; its former 1,034-line leaf is gone
and no replacement exceeds 453 lines. The stage, function, scalar,
straight-line, conditional-scalar, and structural joins are mandatory
coordination entrances rather than re-export walls.

Abstract-to-target translation validation is a parallel rung, not another
branch inside producer lowering. Its sub-100-line `validation/mod.rs` entrance
owns target/root/roster custody and hands each function to the sub-100-line
ordered `catalog/mod.rs` enable/disable inventory. Descriptor model and typed
replay adapters descend into `catalog/model.rs` and
`catalog/dispatch/{immediate,parameter,terminal}.rs`; the
catalog rejects duplicate or overlapping matches before dispatch. Exact replay
then descends to the sibling
`straight_line_integer_immediate.rs` and
`straight_line_boolean_immediate.rs` literal leaves or
`straight_line_scalar_crash.rs` and
`straight_line_parameter/{integer,boolean,boolean_not,boolean_equal}.rs`. The
governed parameter coordinator visibly joins the `source/mod.rs` grammar map,
its shared `envelope.rs`, and the `direct.rs`, `boolean_not.rs`, or
`boolean_equal.rs` grammar leaf to whole-roster `abi.rs` calling-plan replay.
`derived.rs` then joins derived source identity to exact register/stack
placements and provenance before the typed target leaf. Boolean-not preserves
its operand and Boolean equality preserves both ordered operands, including
identity, alongside produced value, operation, and edge custody. Immutable
root and family vocabulary descend through small `model/error/` and
`model/receipt/` maps and semantic leaves. The optimizer's sub-100-line
target-operation entrance owns the visible
`lower -> independent validation -> retained carrier` join. An architecture
gate prevents validation from importing lowering helpers, and each complete
function-roster row carries either `Uncovered` or exactly one typed validated
family receipt.

The temporary target-to-assigned compatibility continuation is governed too.
Its 33-line stage entrance checks the entry roster and coordinates per-function
assignment; a 25-line function entrance retains identity and provenance around
a 43-line exhaustive carrier-family router. Cleanup, boundary, Unit,
structural-parameter, structural-call, scalar-control, placement,
expression-frame, typed-expression, and parameter-discovery mechanics descend
into named leaves. No production leaf exceeds 554 lines, so its temporary
status no longer excuses an opaque assignment monolith.

The Terminal-to-abstract boundary is likewise governed as four distinct
responsibilities. Artifact admission owns canonical decode/verify and replay;
optimizer-unit construction owns structural catalog retention followed by
accepted facts, proof questions, ownership frontiers, and sealing; provider
installation owns selection policy followed by exact plan and call replay;
machine lowering owns entry-roster coordination over payloadless, ordinary,
and structural families. Their entrances are 48, 65, 53, and 57 lines, and no
production leaf exceeds 700 lines. Ordinary-machine routing now keeps its
49-line payloadless/structural/ordinary choice separate from ordinary block
lifecycle, exhaustive operation projection, and terminator projection; the
former 1,058-line leaf is gone.

The Terminal-Psi-to-native crate now follows the same navigational contract as
a complete governed tree. Its crate root is only a responsibility map.
Source-entry settlement owns calling-plan pairing plus canonical Terminal
replay; native realization visibly sequences exact input selection, provider
admission, machine emission, and artifact assembly. Provider coordination
descends into external settlements, checked-adapter projection, and admitted
installation. The ProgramStorage encoding entrance owns projection, encoding,
and independent replay, while the wrapper-object entrance owns the actual
settlement, semantic-contract, object, manifest, and custody join. Object and
validation work descend again into named semantic leaves. The architecture
gate requires all of these real joins rather than accepting re-export walls.

Psi, selected-lowering, allocation-recovery, post-allocation, and
function-relative-layout stages expose ordered catalogs with phase coverage
tests. Validator candidates, semantic analyses, optimization-unit identity and
rewrite machinery, projection tests, and physical custody tests descend through
small named entrances rather than monoliths. The repository architecture test
enforces a 1,000-line default production ceiling, pinned non-growing legacy
exceptions no higher than 1,300 lines, and a 1,500-line dedicated-test ceiling,
plus the entrance exception contract over the governed optimizer roots. It
additionally names the coordination marker
that must remain in each migrated executable-stage entrance and requires one
local rule catalog for every Psi pass; a small re-export wall no longer passes
that check. The optimized ordinary-callable-entry stage is a physical example:
its `mod.rs` owns build/replay, with records in `model.rs`, semantic
reconstruction in `reconstruction.rs`, and wire format in `codec.rs`. The
External Psi policy follows the same navigation rule. The policy crate enters
through `external_schema/mod.rs`, where candidate canonicalization and point/log
identity joins are visible; `model`, `identity`, and `codec` are separate
leaves. Pass-manager policy coordination enters through
`pass_manager/external_policy/mod.rs`, which owns the sole validated-feature
projection boundary before descending into context, candidate-feature,
recording, and replay leaves. The former flat schema and mixed execution-policy
coordination are not alternative routes.
The selected-lowering literal-fold stage follows the same rule: its phase entrance
owns exact selection projection through the adjacent catalog, and the
`literal_fold/mod.rs` entrance owns the proposal-to-independent-validation
join. Proposal and
validation descend through separate roots, constraint, work-accounting,
action-reconstruction, and selected-plan reconstruction leaves; validation
cannot import producer transformation mechanics. The architecture gate names
this entrance and rejects any producer dependency from its complete validation
subtree. Pipeline custody then descends through `model`, `execution`, and
`accounting`. It retains the owning catalog's selected-rule identity and must
not introduce a second schedule enum or a special whole-catalog combination.
Each catalog row contributes one composable policy payload, so adding a rule
cannot silently reuse an older combination's behavior.
Fragment admission consumes the resulting phase carrier through one
`SelectedLowering` source kind. The exact add/subtract rules and optional rel8
layout result remain below that carrier; none owns a parallel publication
route.
Register-allocation rule folders use the same
shape; pressure rematerialization keeps its production computation and broad
fixtures in separate leaves below its real compute/validate entrance. The
optimized object-artifact boundary likewise exposes one build/replay entrance
above separate model, reconstruction, and codec leaves. The preceding
relocation-free object-container boundary mirrors it and keeps codec tests out
of production leaves. Target register-environment custody exposes one
build/validation entrance above the exact target catalog, validated custody
model, validation mechanics, and tests; the target/ABI matrix is therefore
visible without burying the stage join in that catalog. Selected-instruction
staging separates retained model, construction, fixed-input constraint
projection, and independent replay; its entrance owns the exact environment
through replayed-result custody join. Selected-CFG liveness follows with a
single analysis-to-independent-replay entrance over separate model, analysis,
validation, and custody leaves. CFG-aware live ranges repeat that explicit
analysis/replay custody shape before allocation legality. Live-range
validation has its own 34-line entrance: it first replays liveness custody,
then descends into independent reconstruction, receipt projection, and focused
corruption tests. The former 1,294-line validation catch-all is gone; its
largest leaf is below the new default ceiling. Computation has a separate
62-line plan-assembly coordinator over function construction,
tied/early-clobber constraints, virtual fragments, architectural register
units, and focused tests. Its former 1,071-line mixed compute/fixture file is
gone and no replacement exceeds 416 lines. Allocation-legality staging
puts each exact availability policy in one visible leaf and keeps
analysis, independent replay, custody projection, and the retained model
separate; its entrance owns policy selection plus the replay-gated stage join.
Exact fixed-view-copy recovery then separates model, materialization,
independent replay, and custody projection below one source-validated
entrance. Its compute rung retains one visible application loop, then descends
through source preflight/work accounting, shared-entry policy mechanics,
selected-CFG mutation, and focused fixtures. The former 1,022-line mixed leaf
is gone and no replacement exceeds 278 lines. Transformed-selected reanalysis
recomputes liveness, ranges, and
legality without source-fact reuse and keeps replay, transition invariants,
custody, and model in named leaves.
Register-home staging preserves baseline-legality and post-copy-reanalysis as
explicit source families while sharing construction, independent validation,
custody projection, and model leaves below one replay-gated entrance.
Post-fold home staging applies the same shape to one-step literal-fold chains
and complete selected-lowering runs, with manifest projection separated from
construction and replay.
Post-allocation machine analysis likewise separates source-route construction
from replay validation and the sealed model, while its entrance owns the
common effects-plus-machine custody join. Active-resident rematerialization
keeps producer computation and independent replay validation in separate
leaves; its entrance alone grants stage custody after that reconstruction.
Pre-allocation machine-effect staging keeps its exact ISA catalog, analysis,
source-route construction, independent replay, custody projection, and model
in named leaves; its entrance replay-gates every supported selected-source
lineage.
The final allocation-recovery vertical does not fork by rule. Its 34-line
entrance owns one construction-to-independent-validation join; `source/`
descends into `fixed_view`, `active_resident`, and shared `projection` leaves,
while sibling `construction`, `validation`, `manifest`, `custody`, and `model`
files own the common function-relative carrier. Both source leaves use the
generic selected-form encoder and resolved-layout stages.
Structural-Unit function-relative realization separates its retained model,
physical-stage construction, independent replay, source-shape admission,
manifest reconstruction, and custody projection below one replay-gated
entrance.
Allocation-recovery function-relative realization retains tagged fixed-view or
active-resident source custody, then binds the common machine, encoding,
layout, exit, and manifest identities. Corruption helpers remain in a
test-support leaf rather than production mechanics.
Receiver-free Unit function-relative realization likewise keeps construction,
replay, Unit-shape admission, manifest reconstruction, custody, and model
behind one small entrance.
Bounded target-operation assignment now separates retained model, source
lowering routes, assignment construction, and independent custody replay; its
entrance alone admits the constructed assignment after that replay succeeds.
The preceding optimized target-operation boundary likewise keeps its retained
owning carrier separate from exact lowering mechanics, while its entrance owns
all lowering-to-custody joins and checked-provider-installation retention.
Pre-physical manifest custody now has the same visible shape: its entrance owns
candidate projection followed by independent replay, while model, projection,
validation, canonical codec/identity, human rendering, and focused rendering
tests descend into named leaves.
Complete-unit operation-contract validation now exposes one per-node entrance
that preserves value-use, node-contract, and binding order, then descends into
value flow, scalar typing, service/call, structural-access, claim-transfer,
payloadless-case, and boundary leaves.
Current-ownership validation exposes one current-signature entry reconstruction
to ordered CFG-replay entrance; its model, frontier mutation, cleanup,
structural-placement, and residual-affine mechanics descend independently.
Complete-unit structural catalogs expose one ordered type-index to
domain-index join, then descend into content projection, type declarations,
function-local catalogs, witnesses, provider specialization, and path
resolution.
Independent rewrite accounting keeps common edge-custody preservation and
scalar-substitution contracts in one entrance, with adjacent/non-adjacent
merge, terminal fusion, dead scalar, proof identity, common-subexpression,
substitution, and threading mechanics in exact family leaves.
Independent GVN validation now follows the same navigational contract. Its
entrance validates candidate custody and selects local/dominating elimination
or phi-translated join synthesis; `rule_catalog` exposes the exact admitted
rule identities, while proof admission, expression keys, dominance replay,
and the two rewrite protocols descend into named leaves.
Producer-side GVN expression identity has the same shape. Its former flat
`expression_keys.rs` leaf is a 33-line entrance that owns the common canonical
row and operand-pair contracts, then names separate `model`, obligation-free
`total`, `proof_certified`, and asymmetric `compatible_policy` rungs. No leaf
exceeds 300 lines, and the architecture gate pins the entrance contract.
Phi-translated GVN no longer hides three exact catalog rules in one 824-line
file. Its 37-line entrance owns their common analysis, invalidation, pass, and
version contract, then descends into separate obligation-free,
proof-certified, and compatible-policy rule leaves. Each leaf retains only its
own expression/evidence mechanics, and none exceeds 275 lines.
Same-block and dominating GVN now mirror that descent. Their 32- and 37-line
entrances own the analysis/invalidation contract specific to their traversal,
then expose obligation-free, proof-certified, and compatible-policy leaves.
The former 449- and 656-line mixed files are gone; all six replacements are at
most 212 lines. A catalog row therefore leads through one traversal entrance
to one exact semantic family at every GVN scope.
Obligation-free wrapping neutral arithmetic is a separate closed family. The
producer's `identities/mod.rs` entrance exposes its rule and five-row shape
partition; the validator's `total_scalar_identity/mod.rs` entrance descends
independently into classification, literal evidence, accounting, application,
and corruption tests. The two sides share typed candidate data, not matching
logic or an authoritative shape table.
Wrapping zero-count shifts are an adjacent two-row rule, not a widening of the
neutral-arithmetic rule identity. The producer's common proposal conveyor
accepts a rule-local shape classifier; the independent validator maps each
closed identity kind back to exactly one rule and validator. Both sides retain
the shifted value type separately from the shift-count literal type, so a
mixed-width shift cannot be admitted through same-type assumptions.
Wrapping multiply-zero annihilation is a third adjacent rule with its own
identity and a focused `multiply_zero.rs` test leaf. The shared conveyor calls
its proving constant the law-defining literal rather than assuming every law
uses a neutral element. Catalog order makes its overlap with multiply-one
explicit and deterministic without merging their rule custody.
Saturating neutral arithmetic is a fourth exact rule and a named
`saturating.rs` classifier leaf beside the wrapping partitions. Its five rows
retain saturating policy in both candidate kind and independent validator
mapping; the shared conveyor does not reclassify them as wrapping operations.
The focused `saturating_neutral.rs` test leaf owns width boundaries, canonical
ties, policy isolation, and invalid-fact rejection. Bitwise work remains split
into future neutral-literal and absorbing-literal families rather than one
vague identity bucket.
Saturating multiply-zero is a fifth exact rule with a separate
`saturating_multiply_zero.rs` classifier and focused test leaf. It reuses the
common immutable candidate conveyor but has distinct rule/validator domains;
catalog order explicitly resolves its confluent overlap with saturating
multiply-one without merging their custody.
Bitwise neutral literals are a sixth exact rule with a six-row
`bitwise_neutral/laws.rs` producer leaf: AND with exact-width all-ones, OR with
zero, and XOR with zero, each in both operand positions. Bitwise absorbing
literals are a seventh exact rule with a separate four-row
`bitwise_absorbing/laws.rs` leaf: AND with zero and OR with exact-width
all-ones, each in both operand positions.
There is deliberately no XOR absorbing row. Independent classification no
longer grows in one mixed file: its small exhaustive entrance dispatches to
`wrapping`, `saturating`, `bitwise_neutral`, and `bitwise_absorbing` semantic
leaves above a shared reconstructed-shape model, and the architecture gate
pins that dispatch seam. Focused overlap tests establish that the earlier
neutral rule wins whenever a neutral and absorbing law both apply.

The complete GVN total-scalar identity rung now mirrors the catalog rather
than collapsing below it. `identities/mod.rs` is the responsibility map and
shared-candidate entrance. From there, `wrapping_neutral/`,
`wrapping_shift_zero_count/`, `wrapping_multiply_zero/`,
`saturating_neutral/`, `saturating_multiply_zero/`, `bitwise_neutral/`, and
`bitwise_absorbing/` each own one exact catalog rule. Each folder's `mod.rs`
owns the versioned contract and proposal join; its `laws.rs` owns only the
closed operation partition. The former mixed `rule.rs` and `shapes.rs` files
are forbidden by the architecture gate. Thus the review path is catalog row
to same-named rule folder to contract/proposal and laws, without a repository
search or a generic catch-all.

The register-allocation post-allocation manifest now follows the same rule for
a custody boundary rather than a rewrite catalog. Its 100-line
`post_allocation_manifest/mod.rs` owns both direct-home and
selected-lowering projection/validation joins. `model`, `error`, `identity`,
`codec`, `reconstruction`, `projection`, `validation`, `rendering`, and
focused-test leaves own the subordinate responsibilities. The former 954-line
mixed file is gone, no replacement exceeds 315 lines, and the architecture
gate pins the projection join while forbidding the flat path.
Independent dead-scalar validation exposes one custody-and-analysis-contract
entrance above its exact rule catalog, exhaustive closed operation partition,
and rewrite replay. A new operation or dead-scalar rule therefore has one
obvious classification rung without growing the public coordination file.
Independent redundant-parameter validation likewise keeps candidate custody
and its structural-identity analysis contract in one entrance, then descends
through witness validation, closed-region observation normalization,
outside-region comparison, and exhaustive operation rewriting.
Per-function complete-unit validation now exposes its exact acceptance order in
one entrance, then descends through CFG indexing and totality, entry/parameter
metadata, result signatures, structural roots, fact reconstruction, and
provenance/fuel/effect replay. The entrance retains the joins to value,
ownership, service, and structural-catalog validation rather than becoming a
re-export wall.
Its derived-metadata evidence service also has an explicit place-and-claim
admission entrance above dominance/control-flow, declared-place, scalar-value,
provenance, successor-edge, and ownership reconstruction leaves. This is a
validation taxonomy, not a second rule catalog.
Current-value-range validation exposes one fact-first entrance that preserves
the validation-before-applicability order. Independent fact reconstruction,
current-operation availability, canonical proof-goal mapping, and exact
integer interval algebra descend into named evidence leaves.
Optimized abstract-plan projection exposes its acceptance order directly:
transformed unit and ledger, identity bundle, pass manifests, then
reconstructible projection shape. Receipt/error models, source custody,
manifest replay, source-roster partitioning, and equivalence checks descend
into named leaves, with broad custody fixtures isolated in `tests.rs`.
Verified and transformed optimizer-context validation shares one entrance that
owns the initial-revision policy and preserves complete-unit, immutable-context,
seed/fact, and surviving-frontier validation order. Proof-question projection,
ownership-frontier projection, immutable signature/roster custody, and seed
reconstruction descend into named leaves.
Complete-unit core validation exposes canonical identity/fact indexes,
active/pruned machine plus structural/service catalogs, retained affine
authority, and final frontier/entry/root-service checks as its ordered entrance.
Edge-cleanup and hidden-establishment transition mechanics descend into the
affine-authority leaf.
Liveness computation and pre-allocation machine-effect encoding also keep
their broad fixtures in sibling test leaves, so production file size measures
production responsibility. Independent liveness validation has a 48-line
root-custody/replay/comparison/receipt entrance above structural replay,
scalar/block replay, supported operand constraints, exact function comparison,
receipt projection, canonical collection helpers, and focused tests. The
former 1,021-line validator is gone and no replacement exceeds 225 lines.

When fixtures legitimately need a production module's private helpers, keep
the logical child module and use an explicit sibling `#[path]`; do not retain
hundreds of test lines at the bottom of `compute.rs` or `codec.rs` merely for
privacy.

Migration is not complete merely because every file is under the hard ceiling.
Remaining flat executable-stage leaves are tracked in `TASKS_OPTIMIZER.md` and
must move behind semantic folder entrances before those areas gain new rules.
