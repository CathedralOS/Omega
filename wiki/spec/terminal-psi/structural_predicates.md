# Structural predicates

Structural predicates retain exact parameter roots, typed paths, and operation
meaning. They are not opaque aggregate hashes or permission to inspect arbitrary
storage. [Structural access](structural_access.md) governs observation;
[verification](verification.md) governs premise availability and substitution.

## Paths and call substitution

Path segments distinguish field identities, exact fixed-array indices, and case
identities followed by payload-field identities. Each nested selection retains
the complete ordered path. The verifier traverses declared types independently,
checking relevance, intermediate types, bounds, case/payload ownership, and leaf
type. Missing, erased, truncated, mistyped, out-of-bounds, or redirected paths
reject even if a replacement happens to reach another same-typed leaf.

For a projected argument, prepend the caller's canonical argument path to every
callee-relative leaf path. Substitute both roots of comparisons independently,
including leaves inside arithmetic, bitwise, or logical terms. Required clauses
keep their positional obligations and exact assumption citations. Encoding an
index/case path does not establish executable projection or cleanup support.

## Equality expansion

Language-defined structural equality expands the selected `Equatable` structure;
it does not introduce an opaque aggregate term or license a user-written body.

| Shape | Retained equality |
| --- | --- |
| Record | Canonical conjunction of supported relevant field equalities. |
| Truly zero-member record | Boolean truth. An all-erased record is not this case. |
| Payload-less sum | Canonical conjunction of both case-membership implications for each declared case. |
| Payload-bearing sum | One case-ordered disjunction: both roots select the same case and all selected payload leaves compare equal. |
| Common fields plus cases | Common-field equalities in declaration order followed by the case disjunction. |

Nested records/sums preserve every enclosing field, case, and payload segment.
Aggregate inequality negates the complete equality predicate; it does not invent
an independently reordered leaf expansion. Canonical conjunction/disjunction
encoding rejects duplicate, nested noncanonical, or misordered rows.

Boolean and fixed-integer leaves use their typed comparisons. IEEE leaves retain
the exact format and IEEE comparison kind: NaN is not reflexive and signed zeros
compare equal. Do not replace them with mathematical equality. Direct IEEE `!=`
retains its complementary comparison; aggregate negation uses the complete
equality predicate implying falsehood.

Byte-sequence equality compares live lengths and live byte prefixes. Pointer
identity, storage capacity, and bytes outside the live prefix are irrelevant.
The carrier still distinguishes borrowed views from bounded owned storage and
retains capacity without imposing a native descriptor layout.

## Integer predicate terms

Each fixed-integer member/literal term retains its carrier and selected policy.
Bitwise terms are total; wrapping/saturating arithmetic retains its distinct
denotation. Wrapping shifts preserve the language's Euclidean count reduction
and independently typed count. Exact arithmetic and conversions retain their
representability requirements; exact shifts additionally need legal counts,
and exact left shifts need representable results.

Division/remainder require a nonzero divisor under every policy. Exact signed
operations also exclude the exceptional `MIN / -1` pair. Runtime operands
require the complete applicable requirement package, independently reconstructed
and substituted at calls. [Integer certificates](integer_certificates.md)
defines the canonical questions; a source sufficient-form matcher cannot replace
them or waive an earlier operation's obligation.

Direct Trapping arithmetic does not form predicate terms or create a proof-side
crash effect. Explicit `embed` produces a mathematical integer with exact source
carrier/range; same-carrier `as` selects Exact arithmetic and retains its proofs.
Executable Trapping operations instead carry their primitive denotation and
path-conditioned crash site, checked against the published same-cause ceiling.

This predicate vocabulary grants no ownership/content-transfer effect. Such
effects require the separate [custody contract](../resources/content_custody.md).
Source and runtime limits remain beside
[Terminal production](../../../omega-rust/psi/compiler/terminal-production/README.md#structural-predicate-production).
