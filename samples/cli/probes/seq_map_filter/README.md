# Seq map/filter accumulator

Depend-free compile-scope probe: static machine parameters. `map<F>` and
`filter<Predicate>` take `machine` arguments whose calls specialize the
recursive templates into direct calls — no runtime callable or capture
object. First sample exercising that `where machine F(...)` surface; the
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

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/seq_map_filter/main.omg
mbx run -p omega -- update --project samples/cli/probes/seq_map_filter
```

Both complete green on Linux x86-64; `omega.lock` publishes locally and is
ignored like every sample lock.
