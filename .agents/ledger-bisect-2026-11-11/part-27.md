# Ledger bisect part 27 (k=27, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`.

The earlier c01 result `3009cab5a41cba2b0491f3ef35a7b073c18986da` is withdrawn. It used a shared `target/` directory and a binary search for the good commit. This run uses a commit-scoped `/tmp/bisect-target-<sha>`, an exponential first-parent good walk, and `git bisect start --first-parent`.

Fresh measurement and cluster results are written here as each cluster finishes.
