# Bucket contract chain

Depend-free compile-scope probe: proof-obligated mutation as user code —
the first sample whose calls are gated by `requires` and chained by
`ensures`. A concrete 3-slot bucket hand-instantiates the core
`FixedVec<T, const N>` pattern (generic machine instantiation is not wired
yet, so concrete data + attached machines carry the contracts).

The discharge chain is pure contract chaining — no runtime checks:
`clear` ESTABLISHES spare room (`length < items.len`), `push` CONSUMES it
and GUARANTEES non-emptiness plus the predecessor bound, `get` and `pop`
CONSUME those guarantees. Mutating calls invalidate carried facts, so each
proof-obligated call is fed by the call before it; the reads go through
whole-window borrows (`&bucket.items[..]`, `&mut bucket.items[..]`), which
name the same storage as the array, so the array's index facts hold for the
view.

The machine pushes 7, reads slot 0, pops it, and accepts only `first == 7`
and `popped == 7`. An undischarged `push`/`pop`/`get` call is a compile
error (the fail/collections corpus twins pin that side).

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/bucket_contract_chain/main.omg
mbx run -p omega -- update --project samples/cli/probes/bucket_contract_chain
```

Both complete green on Linux x86-64; `omega.lock` publishes locally and is
ignored like every sample lock.
