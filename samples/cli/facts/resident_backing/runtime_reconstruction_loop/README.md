# Reconstruct allocation descriptors in a loop

Parent: **BUMP-ALLOCATOR-CANARY / VEC-NATIVE-GROWTH**.

A granted vacant extent retains actual backing through transport,
initialization, permitted writes, and extraction.

The allocator's copy loop must rebuild descriptors while keeping ownership of
their backing. Unlike `runtime_helper_loop`, whose helper returns its input
unchanged, this helper destructures and reconstructs each extent under checked
content-conservation contracts. The direct exit returns input claims; the
recursive exit returns reconstructed claims. They need not share one static
claim identity. This is another production of the existing resident-backing
fact, not a new row or a larger-count variant.

`main.omg` obtains real zeroed backing, splits it into two distinct regions,
and exercises zero and repeated iterations. Different writes and cross-checks
make swapped or aliased results fail at runtime. Construction must establish
`Granted & Vacant`; every call must consume its inputs and return its complete
owned result. The same checks apply to the loop's backedge, without expanding
an infinite history of earlier invocations.

`invalid/main.omg` changes only the reconstructed length to zero. Its unchanged
content-conservation contracts must reject: returning a descriptor does not
authorize changing which backing it claims. `invalid_duplicate/main.omg`
instead changes only the second helper argument to the already consumed first
extent; reconstruction must not make a second transfer legal.

Run `omega run --both main.omg` and check `invalid/main.omg`. The positive
binds all four hosted targets; execution observations are host-specific. Both
engines must exit 0. Corpus copies retain the positive and invalid control.

This establishes custody through reconstruction, checked-to-Terminal lowering,
and native execution with separate live backing. It does not yet prove the
Vec copy loop's recursive descriptor-equality contracts or its complete
growth/release/reset lifecycle; those remain the parent outcome's acceptance.
