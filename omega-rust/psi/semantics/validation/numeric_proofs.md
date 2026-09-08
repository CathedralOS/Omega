# Numeric proof production

[Proof contracts](../../../../wiki/spec/proofs/contracts.md) and
[Terminal mathematical values](../../../../wiki/spec/terminal-psi/mathematical_values.md)
own meaning. Source acceptance and a private analysis bound are not independently
reconstructed Terminal evidence.

## Integer embeddings

[proof_embeddings.rs](src/proof_embeddings.rs) recognizes the compiler-installed
`embed`, not same-spelled package calls. Its
[call adapter](src/proof_embeddings/calls.rs) requires exact checked root entry
and telescope with matching declared arguments or representable integer/Boolean
literals. Preconditions, unresolved specialization, and computed arguments
needing caller-context type/range derivation remain unsupported there.

Computed fixed-carrier embeddings retain opaque source-and-carrier terms with
range bounds. Wrapping/Saturating computations do not become mathematical
addition. Complete computed denotation bridges and general target-relative
address projection remain further work. The older raw machine-arithmetic
induction producer needs its own policy-aware migration; the embedding/coercion
checks do not silently repair that separate path.

Exact cast and ordered affine/cast witnesses consume independently reconstructed
carrier bounds. Exact shift-left/add/subtract/multiply canonical projection and
legacy reduction must not be confused with a fully supplied independent
certificate. [Integer certificate rules](../../../../wiki/spec/terminal-psi/integer_certificates.md)
own canonical goals, ordered definition replay, and allowed premise-free total
images. Producer search depth and preferences do not add proof rules.

## Float meaning

[float_projection_invocations.rs](src/float_projection_invocations.rs) validates
exact source projection operations. The checked
[proof-row producer](../../pipeline/typed-trees-to-checked-trees/src/proof/float_meaning.rs)
retains direct machine parameters, reserved machine results, and direct
structural leaves separately from transitional typed-expression custody.
The [lowering join](../../pipeline/checked-trees-to-lowered-psi/src/float_meaning_projection.rs)
rejoins exact owner/parameter/result/path identities with emitted Terminal rows.

The direct structural source path currently supports nonempty field/case paths
below a direct top-level machine structural parameter. Fixed-index paths,
nested-state contracts, locals, and computed structural sources remain
transitional. Reserved result is the machine result unless shadowed by a real
entry parameter. There is no separate nested-state result source.

Terminal's operation-result, block-parameter, call-result, and fixed-index
structural source classes can be independently validated without implying that
the source producer retains the expression-to-Terminal correspondence for them.
Missing owners or joins stay unsupported, never guessed from names or coincident
value numbers. Every class remains proof-only metadata and broadens neither
native execution nor floating operation support.
