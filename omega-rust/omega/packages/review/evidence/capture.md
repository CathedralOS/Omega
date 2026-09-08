# Review capture joins

The [review contract](../../../../../wiki/spec/packages/review.md) owns public
meaning. This note maps that contract to current compiler custody. Start at
[src/capture/mod.rs](src/capture/mod.rs); [record](src/record) remains independent
of compiler state. Canonical support and private bounds live in
[EVIDENCE_SCHEMA.md](EVIDENCE_SCHEMA.md).

## Source roles

[Source capture](src/capture/source) retains exact symbols and authored spans
beside semantic rows before canonical sorting. Paths are relative to exact
package/toolchain owners. Package-less user source, missing owner joins, and
owner drift cannot become exact toolchain provenance.

| Role | Retained origin and checked relationship |
| --- | --- |
| Declaration | Exact row declaration; dangerous authority also retains the service and exposing callables. |
| Provider schema/requirement/realization | Distinct exact declarations alongside the plan, plus nominal provider when present. Require package ownership or exact authored toolchain source. Never keep only the implementation anchor. |
| Provider selection/grant | Authored build/default site or closed implicit-choice reason. Grant joins the exact selected plan, not its compact report fingerprint. |
| `trait_parent` | Exact typed parent application with its authored identifier span. |
| `contract_clause` | Clause keyword retained through syntax, resolution, typing, synthesis, and specialization, including nested structural static-machine contracts. Accepted claims use their callable/ensures custody. |
| `body_call` | Authored call-selection occurrence joined to checked flow before provider settlement rewrites identity. Check target, receiver, shape, and operational acknowledgement. A legitimate late-bound target does not erase the authored span. |
| `synchronous_invocation` | Authored target name joined once to exact parameter symbol/ordinal or boundary trait; checked published/inferred targets remain available before settlement. |
| `service_reach` | Authored keyword and member spans joined to exact boundary symbols and checked parent-closed reach, including invocation contributions. Empty authored clauses use the keyword span; inferred closure entries get no invented location. |
| `suspension`, `blocking` | Separate authored keywords, booleans, and checked operational interfaces. Omission/inference has no fabricated source site. |
| `external_binding` | Exact `via` keyword paired with the normalized binding on the same conformance; require binding/span parity. |
| `const_initializer` | Parsed initializer extent retained before value substitution, beside the const declaration. |
| `proof_fact` | Full semantic-token extent of every authored public domain/data fact and fact under a public contract. Source-free synthesis gets no invented span. |
| `trait_requirement`, `data_member` | Exact requirement, field, case, and payload-field declarations, or real authored derivation origin. |
| `callable_parameter` | Value-parameter declarations on reviewed callables/operators/requirements, recursively including structural static-machine contracts. |

Statement/transition calls retain explicit selection occurrences; expressions
reuse their attached occurrence. Compiler-generated calls have no authored
location. Retaining spans earlier is the repair for missing late-stage custody,
not reparsing source text during projection.

Occurrence identity is the authored token plus declaration-owned exposure and
selection kind. Compiler-derived copies share it; an exact target supersedes an
unresolved provisional copy, while two different resolved targets reject. Ranking
custody retains typed expression roots separately from rendered witnesses.
Nominal static-machine contracts retain the complete exact trait/requirement path,
including nested contracts. Requirement-local satisfies edges retain their exact
trait/application or operator overload before supply-policy admission; a rejected
association cannot substitute another selected declaration. Domain establishment
paths likewise retain each authored occurrence after unique subject-authorized
signature-free resolution, even when equal semantic routes deduplicate.

## Structural identity

[Semantic capture](src/capture/semantics) joins authored nominals through exact
package ownership or private source identity. Only the canonical source
commitment crosses into toolchain review bytes. Generic binders receive no
invented owner. Root builtin-type slots are selected by compiler position and
kind; same-spelled package/generated declarations do not become builtin atoms.

Declared, carry, value-domain, and layout subjects use distinct tags. Layout
retains its closed grammar and exact structural schema declaration. Compiler
canonical-const transport decodes to a typed value term and excludes diagnostic
display; it is legal only under an exact const parameter. Array/open-expression
binders must rejoin one alpha-normalized telescope. Residual calls, unrelated
source leaves, legacy layouts, and incomplete index selections reject.

Public domain facts join exactly one checked definition, one fact-keyed ownership
record, and checked dependency places for nested paths. Domain role records must
name the declaration's own typed semantic identity; evidence stores its qualified
domain and closed role, not a private semantic-domain ID. Establishment routes
retain exact kind, trait, and requirement and normalize alternatives by sorting
and deduplication. Domain operators remain separate public-operator rows.

## Realization joins

[Callable capture](src/capture/callables) cross-checks supply mode, exact
`satisfies` edge, structural binding, and requirement declaration. External
leaves have one complete application even when private and absent from public
callable rows. Table-field bindings require the exact attached data declaration.
An authored alias remains separate from requirement/operator identity.

Checked operator realizations retain machine/operator symbols, admission form,
overload shape, exact lifetime-bearing types, complete contracts, and typed
contract snapshots. Rederive signature selection and contract refinement;
coordinated typed/retained drift does not pass merely because two cached copies
agree. Active selection still belongs exclusively to the selected-provider set.

Selected top-level foreign calling rows join the exact requirement overload and
plan. The requirement's semantic receiver corresponds to the satisfier's first
explicit carrier and stays in the foreign signature. Extraction is not installed
invocation or final-code replay. Compiler-private baseline custody is not a
defense against a trusted component deliberately rewriting all of its inputs.
