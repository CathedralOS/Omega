# Owned content returned through a brief borrow

Parent: BUMP-ALLOCATOR-CANARY. A brief exclusive borrow returns detached owned
content and restores its source. The caller allocates 16 and 8 counted units from
48, keeps both results alive, and observes the residual 24. No runtime guard
supplies the next allocation's static capacity premise.

Run `omega run --both samples/cli/facts/borrowed_content_partition/main.omg`.
The oracle is exit 0 from both engines. This is numeric custody, not physical
backing, resident placement, or a complete allocator.

The initial root isolates the checked-body custody and aggregate-store gap.
Its baseline refused owned return solely because the strategy parameter was
borrowed; after that repair the aggregate field replacement failed to lower.
The coupled strategy invariant `remaining == tail.length` is restored from
the two completed stores, including the partition call's returned-value facts.
The full nine-place row remains unfinished.
Cathedral's retained physical backing and monotonic retirement/reset remain
parent acceptance, not claims established by this program.

The matching positive fixture is
`tests/omega/pass/ownership/runtime_borrowed_content_partition_exit`.
Negative controls under `tests/omega/fail/ownership/borrowed_content_partition_*`
reject fabricated qualification, an unclosed restoration window, duplicate
transfer, incorrect partition conservation, a stale count and restoration of
the wrong receiver. They preserve the positive's content domain and partition
operation. The nested-field/reversed-store fixture exercises the same operation
without depending on field placement or store order.

The exercised requirements are owned establishment and exact transfer identity,
brief-borrow lifetime and restoration, composition through a checked partition
call, and rejection of unsatisfied custody/predicate obligations. Native and
Terminal execution cover aggregate lowering, layout and call placement, stores,
and emitted image execution on macOS arm64. This does not establish physical
backing, other-host execution, the remaining transport places, or allocator reset.
