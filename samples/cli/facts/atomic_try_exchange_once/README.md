# Non-observing single-attempt exchange

Owning outcome: **ATOMIC-MEMORY-MODEL** in [TASKS.md](../../../../TASKS.md).

Fact: **A non-observing single attempt returns the proposed u32 on failure and the displaced u32 on commit.**

The nearest row, [atomic_compare_exchange_once](../atomic_compare_exchange_once/README.md),
observes a copy of the resident on failure and has no displaced payload on
success. This row must preserve the other canonical outcome family: mismatch
and uncommitted attempts return the proposal, while a committed exchange returns
the displaced resident. A missing non-observing result or a substituted observing
result cannot satisfy that requirement. This is a distinct custody/result rule,
not a larger width or count of the existing row.

The first [local program](local/main.omg) is a frozen baseline witness, with its
[result-family twin](local/invalid/main.omg). The positive keeps resident 10,
proposal 99 and untouched neighbor 17. It tests both a matching key 10 and a
mismatching key 9. A matched single attempt may lawfully be uncommitted; that
case must return proposal 99 and retain resident 10. A mismatch must return
proposal 99, never the resident 10. Success must return displaced 10 and leave
resident 99. The runtime checks occur after the operation and introduce no
static premise. The twin changes only the selected primitive to observing
compare_exchange_once while retaining the non-observing receiving type and all
oracles; it must reject the wrong canonical result family.

This first production uses dedicated AtomicU32 storage and its concrete u32
comparison representation. It does not select an arbitrary custom key law or
admit an affine/linear resident. The accepted [atomic outcome rule](../../../../wiki/spec/resources/access.md#atomic-compare-exchange-outcomes)
keeps Key and its encoding law in call evidence, outside the runtime outcome.
The concrete builtin admission must retain that correspondence rather than
infer a provider contract from a receiving type. Broader resident/key support
and non-observing-only placed permissions remain separate acceptance.

The local source and oracle are being measured against the exact published
compiler before any compiler edit. No repaired-source or runtime success is
claimed. All nine places, their pinned twins, the u64/representation boundary
where required, ordinary package review and all four matching hosts remain
unfinished. The row will not close at this first witness. Formal concurrency,
forced Uncommitted outcomes and target-refinement proofs also remain open.
