# Bootstrap construction variants

[Whole-chain minimization](MINIMIZATION.md) governs retention and engineering
comparisons; language and resource contracts remain with their rung owners.
[Edge contracts](CONTRACT.md) own source/evaluator composition, acceptance,
acceleration, and native execution requirements.

Variants pursue the common [construction goal](MINIMIZATION.md) as peers; no
variant is selected during exploration. The numbered directories currently own
the interpreted variant. Their layout is not an architectural requirement. New
variants may keep their own implementation beneath a descriptive directory.

The existing interpreted source owners are:

```text
0_alpha/    raw tape semantics and audited native VM seeds
1_beta/     trusted imperative tape-assembly language and compiler
2_gamma/    typed scalar/effect functional language and evaluator
3_delta/    typed pure functional language and compiler
4_epsilon/  fixed-storage compiler-host language and evaluator
5_omega/    Epsilon-written bootstrap compiler
proofs/     source-owned proof checker and artifact-specific theories
```

Numeric prefixes sort the retained rungs in Greek-letter order. They are folder
names, not language versions; source suffixes and language names are unchanged.
A compiler or evaluator owner is named by the language it accepts or executes;
its source suffix names the implementing language. `.beta`, `.gamma`, `.delta`,
`.epsilon`, and `.omg` identify source languages; `.tape` identifies raw Alpha
bytecode. Epsilon's evaluator retains the `epsilon_compiler.delta` entrance
name; its ordered `.sources` manifest selects the complete implementation.
[`proofs/`](proofs/README.md) holds optional proof research; it is not
another language rung. Alpha, Beta, and Gamma keep their small implementations
beside their contracts. Later rungs retain concept-owned source folders without
an enclosing `compiler/` directory.
Tool and test directories retain their language names. Audited seed source and
disassembly retain historical path annotations so relocation preserves their bytes.

The interpreted construction route is:

```text
Alpha -> Beta -> Gamma -> Delta -> Epsilon -> bootstrap compiler
      -> source/{psi,omega} as the first Omega compiler -> source/{psi,omega} as the self-compiled Omega compiler
```

In this variant, every rung is implemented in its immediate predecessor except Alpha's audited
native execution floor. `5_omega/` is the bootstrap implementation of the Omega
compiler; the final Omega-written compiler lives under [`../source/`](../source).
The maintained Rust implementation under [`../omega-rust/`](../omega-rust) is a
development comparator and grants no bootstrap authority.
Beta self-reconstruction binds the readable compiler source to its admitted
cold-start tape; no later intermediate rung is required to self-host.

Cross-rung scripts resolve these owners through
[`../tools/bootstrap/paths.sh`](../tools/bootstrap/paths.sh). Host tools that only invoke, stamp, compare and report supply no language
semantics. Any development compiler or accelerator is instead a disclosed
implementation with [explicit acceleration obligations](MINIMIZATION.md#explicit-acceleration).
