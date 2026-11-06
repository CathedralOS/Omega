# Shared atomic storage

`Cell::publish` stores 10 through a shared reference. `Cell::mark` performs
fetch-or with 5 through that same storage and returns the prior value. The
caller checks prior value 10 and final stored value 15, then exits 70.

The compiler now uses the existing operand index when checking ownership
occurrences and prepares case observations from the relevant fact handles.
Both operations preserve complete handle identity, first occurrence and order.
The state-owner query also borrows matching coordinates from its existing
index instead of copying their list; parent priority and duplicate order stay
unchanged. These changes avoid unrelated scans and temporary allocations.
The program, its contracts and its expected exit are unchanged.

Actual matching-host observations agree in native and Terminal execution:

| Host | Actual compiler source | Corpus result | Ordinary CLI result |
| --- | --- | --- | --- |
| Linux x86-64 | `50564db4c096` | 70/70 in 89,532 ms | 70/70 in 76.317 s |
| macOS ARM64 | `f00661dbe2a1` | 70/70 in 112,286 ms | 70/70 in 105.892 s |
| Linux ARM64 | `f00661dbe2a1` | 70/70 in 118,662 ms | 70/70 in 109.790 s |
| Windows x86-64 | `153edee2ee7e` | 70/70 in 90,384 ms | 70/70 in 85.547 s |

These runs retain the original 120-second corpus, 600-second package update
and resume, and 240-second ordinary execution limits, with one worker and
profiling disabled. The matching-host `f006` producers include four separately
reviewed lexical-observation changes; these are their measured results, not
execution of the later integrated source. The existing ARM rejection record
is replaced by its retained actual `f006` row. Unchanged source-check, macOS and
Windows golden blocks keep their historical measurements.

The integrated Linux replay checks six source outcomes, two engine pairs
(atomic exit 70 and case-retirement exit 0), and ordinary atomic execution.
This covers incoming borrow/call handling and fused control; the other host
runs retain their actual identities under the reviewed applicability bounds.
Earlier Windows corpus timeouts and partial debugger failures remain in the
evidence ledger. No speed ratio or sole-cause claim is made. The earlier
`0aeed` Linux archive omitted its hidden owner marker; its runtime acceptance
is separate from the complete products retained by this integrated replay.

The negative `fail/atomics/shared_receiver_plain_field_write` keeps ordinary
shared writes forbidden. Snapshot authority, copied-argument invalidation and
case replacement controls remain enforced. Serial execution checks event
identity and caller-visible storage. Concurrent TR3–TR8 witnesses and the
remaining atomic memory-model acceptance stay open.

With a matching optimized `omega` executable on PATH:

```sh
omega update --project tests/omega/pass/atomics/shared_receiver_atomic_store --target macos_arm64
omega run --both tests/omega/pass/atomics/shared_receiver_atomic_store/main.omg
```

Use the matching declared target on Linux or Windows. A fresh checkout requires
the package review reported by `omega update` to be settled, followed by
`omega update --project <directory> --resume`. That local acceptance is not an
audit claim.
