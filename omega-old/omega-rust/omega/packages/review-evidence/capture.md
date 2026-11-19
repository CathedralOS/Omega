# Review capture joins

The [review contract](../../../../wiki/spec/packages/review.md) defines review's
meaning. This note maps it to where the compiler keeps that data. Start at [src/capture.rs](src/capture.rs);
[record](src/record) does not depend on compiler state. Supported forms and
private size limits live in [EVIDENCE_SCHEMA.md](EVIDENCE_SCHEMA.md).

## Immutable custody and supplied semantics

[PackageReviewInput](src/capture/input.rs) borrows the semantic candidate and
the original compilation's read-only source and selection records
separately. Normal callers pass `&CheckedCompilation`; adversarial
reconstruction may pass raw trees without forging or changing a checked
result. Projected data is inert: it runs nothing and admits no package.
Checks match the candidate's types and facts to retained records;
dispatch source restoration checks the candidate against the original edits.

Signature instantiation builds temporary `TypedTrees`, never a mutable
compilation, and reads temporary parameter spans there. Proof-expression
links, original signature scopes and source commitments stay on the review
input. A temporary type handle is never read against an unrelated source or
proof arena.

## Source roles

[Source capture](src/capture/source) keeps exact symbols and authored spans
beside semantic rows before canonical sorting. Paths are relative to
exact package/toolchain owners. Package-less user source, a missing owner
link or a changed owner never counts as exact toolchain origin.

| Role | Retained origin and checked relationship |
| --- | --- |
| Declaration | Exact row declaration; dangerous authority also retains the service and exposing callables. |
| Provider schema/requirement/realization | Distinct exact declarations alongside the plan, plus nominal provider when present. Require package ownership or exact authored toolchain source. Never keep only the implementation anchor. |
| Provider selection/grant | Authored build/default site or closed implicit-choice reason. Grant joins the exact selected plan, not its compact report fingerprint. |
| `trait_parent` | Exact typed parent application with its authored identifier span. |
| `contract_clause` | Clause keyword retained through syntax, resolution, typing, synthesis, and specialization, including nested structural static-machine contracts. Accepted claims use their callable/ensures custody. |
| `body_call` | Authored call-selection occurrence joined to checked flow before provider settlement rewrites identity. Check target, receiver, shape, and operational acknowledgement. A legitimate late-bound target keeps the authored span. |
| `synchronous_invocation` | Authored target name joined once to exact parameter symbol/ordinal or boundary trait; checked published/inferred targets remain available before settlement. |
| `service_reach` | Authored keyword and member spans joined to exact boundary symbols and checked parent-closed reach, including invocation contributions. Empty authored clauses use the keyword span; inferred closure entries get no invented location. |
| `suspension`, `blocking` | Separate authored keywords, booleans, and checked operational interfaces. Omission/inference has no fabricated source site. |
| `external_binding` | Evaluated package-local native-binding intent site and original Build custody joined to exact product requirement/application, target, supplier, normalized locator and pinned calling policy; require intent/row/span parity, not a fabricated forwarding declaration. |
| `const_initializer` | Parsed initializer extent retained before value substitution, beside the const declaration. |
| `proof_fact` | Full semantic-token extent of every authored public domain/data fact and fact under a public contract. Source-free synthesis gets no invented span. |
| `trait_requirement`, `data_member` | Exact requirement, field, case, and payload-field declarations, or real authored derivation origin. |
| `callable_parameter` | Value-parameter declarations on reviewed callables/operators/requirements, recursively including structural static-machine contracts. |

Statement/transition calls keep explicit selection occurrences;
expressions reuse their attached one. Compiler-generated calls have no
authored location. Keep a span a late stage lacks from earlier; never
reparse source text during projection.

The `external_binding` row describes the accepted Build-owned contract.
Capture still pairs legacy `via` keywords with declaration bindings;
`BUILD-OWNED-NATIVE-BINDINGS` in [the task board](../../../../TASKS.md#pipeline-route)
replaces that. An untaken Build call has no evaluated binding record, and
product selections grant no call authority while Build runs. A schema change
keeps exact old/new identities rather than passing a legacy declaration
span off as a new Build intent.

An occurrence's identity is its authored token plus its declaration's
exposure and selection kind, shared by compiler-derived copies. An exact
target replaces an unresolved provisional copy; two different resolved
targets reject. Ranking records keep typed expression roots apart from
rendered witnesses. Nominal static-machine contracts keep the full exact
trait/requirement path, nested contracts included. A requirement's own
`satisfies` edges keep their exact trait/application or operator overload
before supply-policy admission; a rejected link cannot substitute
another selected declaration. Domain establishment paths keep each authored
occurrence after unique subject-authorized, signature-free resolution, even
when equal semantic routes deduplicate.

## Structural identity

[Semantic capture](src/capture/semantics) links authored nominal types
through exact package ownership or private source identity. Only the
canonical source commitment enters toolchain review bytes. Generic binders
get no invented owner. Root builtin-type slots are chosen by compiler
position and kind; a same-spelled package/generated declaration is not a
builtin.

Declared, carry, value-domain and layout subjects use separate tags. Layout
keeps its closed grammar and exact structural schema declaration. A compiler
canonical const decodes to a typed value term, not its diagnostic display,
and is legal only under an exact const parameter. Array/open-expression
binders must match one alpha-normalized telescope.
Leftover calls, unrelated source leaves, legacy layouts and incomplete index
selections reject.

Each public domain fact links to exactly one checked definition, one
fact-keyed ownership record, and checked dependency places for nested paths.
A domain role record names its declaration's own typed semantic identity;
evidence stores the qualified domain and closed role, not a private
semantic-domain ID. An establishment route keeps a closed kind with exact
trait/requirement identities or an exact machine identity. Machine routes use
package ownership and the full normalized callable declaration identity, not a display path.
Alternatives are sorted and deduplicated. Domain operators stay separate
public-operator rows.

Only domain-route normalization records `DomainIssuerAuthorization` occurrences.
A public catalog keeps public-interface exposure and exact private
issuer identity; ordinary references to that declaration keep their
visibility checks. The author still needs declaration-selection
authority; this metadata lets no consumer call or conform to it. Machine
identities include the exact declaring module and overload, so equal leaf
names in one package stay distinct. Recovery is inert; local reconstruction
matches the retained identity to checked source. A retained private
admission claim is not proof of a call: listing an exact boundary machine and
its result annotation does not grant an admitted boundary requirement's
authority.

## Realization joins

[Callable capture](src/capture/callables.rs) cross-checks supply mode, exact
`satisfies` edge, structural binding and requirement declaration. An external
leaf has one complete application even when private and absent from public
rows. A table-field binding requires the exact data declaration its
leaf's first parameter borrows. An authored alias stays separate from
requirement/operator identity.

Checked operator realizations keep machine/operator symbols, admission
form, overload shape, exact lifetime-bearing types, complete contracts and
typed contract snapshots. Review rederives signature selection and contract
refinement: typed and retained copies that drifted together still agree. Only the selected-provider set decides the active
selection.

Each selected top-level foreign calling row links to the exact
requirement overload and plan. The requirement's semantic receiver matches the
satisfier's first explicit carrier and stays in the foreign signature.
Extraction neither calls installed code nor replays final code. The
compiler's private baseline does not stop a trusted component that
deliberately rewrites all its inputs.
