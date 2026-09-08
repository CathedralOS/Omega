# Authored selections and carried dependencies

[Package boundaries](../../../../wiki/spec/packages/boundaries.md) defines the
source rule. The compiler keeps two package-neutral records: authored declaration
selection and semantic dependencies of carried values. Omega qualifies and admits
them against the exact source graph; Psi does not interpret package aliases.

## Capture and finalization

The [selection vocabulary](../../foundation/language-semantics/src/declaration_selection.rs)
retains exact occurrence, source span, exposure, and declaration/intrinsic/late
target. [Resolution](../syntax-trees-to-symbol-resolved-trees/src/authored_selections.rs)
captures paths while they are still attached to source. Typing records exact
nominal selections and enclosing public/private position.
[Checked finalization](src/authored_selections.rs) settles late receiver calls,
overloads, operators, and inferred conformances after checking succeeds.

| Source use | Custody |
| --- | --- |
| Public data/domain/machine/trait type positions | Interface exposure; primitive types, lexical binders, locals, and compiler synthesis receive no fictional package owner. |
| Internal state, local annotation/cast, owned storage | Private exposure, even inside a public machine. |
| Trait parent / body requirement | Exact resolved trait under enclosing exposure. Its explanatory `trait_parent` span is not a separate grant. |
| Attached declaration head | Exact carrier type selection, including exported boundary supply. |
| Contract membership | Selected domain declaration; parameter/local roots stay lexical. |
| Qualified conformance bound | Exact carrier and conformance; enclosing declaration determines exposure. |
| Quotient formation | Every authored carrier/relation/trait/application coordinate; selected proof conformance alone uses private formation exposure. |
| Explicit static conformance argument | Exact source-backed declaration at its own token. |
| Inferred conformance argument | Exact unique specialization selection and qualified fingerprint, attached to the authored call; do not duplicate explicit arguments at the call token. |
| Unit/discarded-result statement call | Preserve the call and explicit static arguments before rebuilding statement tables; checked flow finalizes late targets and inferred conformances. |
| Nested static arguments | Retain each declaration path recursively. Conformances keep their evidence-specific kind; type/machine/forwarded binder paths share static-argument kind with exact symbol/telescope category. Integer literals name no declaration. |
| Named const | Static arguments join declaration and canonical value; expression substitution retains declaration provenance and occurrence without carrying const semantics into runtime trees. |

Compiler-owned build markers and lowered assembly use closed intrinsic variants,
not invented package declarations. Unresolved package static paths remain late
obligations and cannot be admitted without exact declaration custody.

Package-aware preliminary/final checking currently permits unresolved late rows
only with exact compiler-owned Toolchain source origin. They remain TCB input,
are not attributed to the requesting package, and do not bypass visibility.
Ordinary package late rows reject before build authority or evidence is issued.
A private toolchain declaration still rejects; make the intended API public.

## Carried semantics

[Carried dependency capture](src/flow/carried_semantic_dependencies.rs) assembles
a checked-flow sidecar only after successful checking. It joins machine-head,
checked result, and ownership-place types; promotes public exposure; and retains
automatic cleanup only for the exact attached nominal declaration. Same-spelled
cleanup on another owner does not match. Erased owned descriptors retain exact
movement/cleanup with payload custody; borrowed descriptors own no referent cleanup.

Review qualifies consumer and dependency declarations and emits exact
dependency-kind/exposure rows with both source anchors. Private flow affects
content identity, public flow also affects API identity. This does not replace
authored-selection admission or establish total package admission by itself.

## Early evaluation

[Build-time selection authority](../../semantics/build-time-evaluation/src/admission/selection_authority.rs)
checks concrete call closures and authored body selections against the reconciled
direct graph before const-generic/array/domain, layout, wire, or calling-policy
execution. Shared policy requires every application site to pass. Unresolved
choices fail closed unless the full compiler-derived candidate set is confined
to Toolchain, self, or direct dependencies. Operator fallback uses checked
intrinsic classification, never name matching. These joins may use private typed
or probe representations; they do not create a stable package format.
