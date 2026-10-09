A nonconstant checked index reads a constrained scalar field through a shared call. The selected value, neighboring element and out-of-range branch must all retain their authored values; native and Terminal execution must exit 0.

Run `omega run --both tests/omega/pending/layouts/runtime_indexed_restricted_field_read/main.omg` after ordinary package preparation.

On baseline `b3bc01f500a792db6d178bbfeb78343ded3339ca`, native legalization rejects machine 2, operation 18 with `SourceCustodyMismatch` at `runtime_indices.rs:105`. The same refusal remains after the selected-reference binding repair. The next dependency is access-aware runtime projection for a restricted shared endpoint; do not substitute an owned projection or rewrite the index into a constant.
