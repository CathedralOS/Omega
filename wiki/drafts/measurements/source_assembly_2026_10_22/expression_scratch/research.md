# Expression scratch storage research — 2026-10-22

Question: can expression-local stacks avoid allocator traffic without growing recursion or changing parser meaning? Reused [prior ownership research](../ingress_ownership/research.md); that addressed durable arena custody, not these temporary stacks.

Queries: `site.llvm.org SmallVector avoid heap allocation stack size` and `site.doc.rust-lang.org std array from_fn Option take`.

Read [LLVM Programmer’s Manual, SmallVector](https://llvm.org/docs/ProgrammersManual.html#llvm-adt-smallvector-h), its inline-storage tradeoff and default roughly 64-byte guidance; inspected [SmallVector.h](https://github.com/llvm/llvm-project/blob/main/llvm/include/llvm/ADT/SmallVector.h) growth/push/pop implementation and out-of-line growth rationale. Read [Rust array::from_fn](https://doc.rust-lang.org/std/array/fn.from_fn.html) initialization semantics. Main-branch pages are research inputs, not vendored dependencies.

LLVM confirms the general hypothesis: avoid allocation for small temporary sequences, but budget frame size. Do not port its unsafe storage or add a dependency. Binary pending operators have a stronger local guarantee: strictly increasing precedence bounds their count to four logical/comparison levels or six integer levels. Safe arrays can hold all possible pending entries. Unary chains have no such bound: retain their first 56-byte prefix directly and use existing dynamic vector growth for additional prefixes. The ordinary zero-prefix path returns directly.

Measured distributions on 463 and 1792 real files are linked in the report. Both have only zero/one unary prefixes, binary maxima two/three. The grammar guarantee, not observed maxima, sizes binary storage. Rejected for this experiment: large inline unary arrays (unnecessary frame expansion), fixed unary limits (changes accepted source), shared scratch in durable syntax (wrong lifetime/ownership), generic unsafe small-vector reimplementation (unneeded complexity). Future worker-owned scratch is only justified if real long unary chains make the remaining spill important.

Experiment: frozen baseline/candidate exact tree/error comparison and allocation counter, separate prelexed parse/destroy repetition timing with black_box and disassembly; preserve long chains, all precedence levels, nesting and runtime/refusal controls. See [result](README.md).
