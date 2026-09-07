# Bootstrap source owners

This tree contains the trust-minimizing compiler chain and no final product
implementation:

```text
0_alpha/    raw tape semantics and audited native VM seeds
1_beta/     trusted imperative tape-assembly language and compiler
2_gamma/    typed scalar/effect functional language and evaluator
3_delta/    typed pure functional language and compiler
4_epsilon/  fixed-storage compiler-host language and evaluator
5_omega/    Epsilon-written first Omega compiler D
```

Numeric prefixes sort the retained rungs in Greek-letter order. They are folder
names, not language versions; source suffixes and language names are unchanged.
Tool and test directories retain their language names. Audited seed source and
disassembly retain historical path annotations so relocation preserves their bytes.

The selected chain is:

```text
Alpha -> Beta -> Gamma -> Delta -> Epsilon -> Omega D
      -> source/{psi,omega} as omega0 -> source/{psi,omega} as omega
```

Every rung is implemented in its immediate predecessor except Alpha's audited
native execution floor. `5_omega/` is the bootstrap implementation of the Omega
compiler; the final Omega-written compiler lives under [`../source/`](../source/).
The maintained Rust implementation under [`../omega-rust/`](../omega-rust/) is a
development comparator and grants no bootstrap authority.

Cross-rung scripts resolve these owners through
[`../tools/bootstrap/paths.sh`](../tools/bootstrap/paths.sh). Host tools may
invoke, stamp, compare, and report; they do not implement a language stage.
Retained downgraded implementations remain nested under their owning rung and
are not selected chain edges.
