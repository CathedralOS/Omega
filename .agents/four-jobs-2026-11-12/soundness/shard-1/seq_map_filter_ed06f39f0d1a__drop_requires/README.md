# Seq map/filter accumulator

Depend-free compile-scope probe: static machine parameters. `map<F>` and
`filter<Predicate>` take `machine` arguments whose calls specialize the
recursive templates into direct calls — no runtime callable or capture
object. First sample exercising that `requires machine F(...)` surface; the
only prior exercise is a corpus fixture.

The machine builds `Seq::Cons` 1..5, maps through `triple` (3,6,9,12,15),
keeps the even terms via `keep_even` (6,12), then folds to 18 through a
state-machine recursion carrying the Seq value and a `u64 in Wrapping`
running sum.

`use omega::language::core::seq` is currently NOT importable: the shipped
module's `reverse_snoc` ensures contract is rejected at admission
("speaks about proof-only Seq, which no entailment tier judges yet"). The
sample therefore carries the corpus-spellable local `Seq`/`map`/`filter`
definitions; swap to the core import once the module is admitted.

`filter<keep_even>(map<triple>(input()))` derives `filter`'s `T` from the
destination and, once `map<triple>` is specialized, from its applied
`Seq<u64>` result; the unapplied call's declared `Seq<U>` schema supplies no
binding.

Compile-checked only (no std dependency, no ProcessExit provider). `Seq<T>`
is recursive by value, so it has no finite runtime representation and stays
proof-only ("Recursive mathematical data without a finite runtime
representation remain proof-only", `wiki/spec/proofs/contracts.md`);
`build.omg` binds no program entry, and the corpus gate checks it as a
fixture:

```sh
mbx run -p omega -- --check tests/omega/pass/generics/seq_map_filter/main.omg
```
