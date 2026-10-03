# Evaluated byte predicates in state contracts

Run on a supported host after ordinary package preparation:

```sh
omega update --project tests/omega/pass/proofs/runtime_byte_predicate_state_contract_exit --target macos_arm64
# Review the generated package decisions, then use omega update --resume.
omega run --both tests/omega/pass/proofs/runtime_byte_predicate_state_contract_exit/main.omg
```

Expected native and checked-interpreter exit: **70**. Exit 71 means an
incorrect UTF-8 verdict. The euro sign is valid; overlong encoding, a surrogate,
a value above U+10FFFF, and truncation are invalid.

Each call compares the evaluated predicate to an expected Boolean, then passes
the same view and Boolean through a state whose requirement retains that exact
relation. The extra integer formal checks authored-to-structural parameter
mapping. Source-to-Terminal tests independently reject a substituted predicate
or sibling field; verifier tests reject content facts after overlapping writes
and mutable ordinary or dynamic calls.
