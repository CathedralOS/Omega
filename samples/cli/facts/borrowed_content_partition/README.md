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
All nine transport places are executable: `local`, `field`, `parameter`,
`call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, and `target_body`.
Each uses the same checked operation in `support`, keeps both detached owners
alive, and consumes the static 16/8/24 facts through `finish`. Named constants
provide request sizes, not a shared linear owner. Each `invalid` sibling changes
only the first request from 16 to 17; the unchanged consuming obligation rejects.

The `match_join` place
selects `carve(&mut strategy, 48, 16)` in both arms, then calls
`carve(&mut strategy, 32, 8)`. Each call restores the exact place with a
different authentic returned claim. Checking retains those alternatives and
lowering specializes the ordinary continuation's claim namespace without
relocating the storage or manufacturing common lineage. Separate fixtures
exercise a surviving sibling loan and its stable address, scalar results,
nested matches, borrowed ingress, returned owners and a nested-machine backedge.
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

The transport fixtures are pinned as
`tests/omega/pass/facts/borrowed_content_partition_through_<place>_exit` (nine
positives, each carrying its native and interpreter exit oracle) and
`tests/omega/fail/facts/borrowed_content_partition_through_<place>_rejected`
(nine twins with their refusing fragment).
Run each positive with `omega run --both <place>/main.omg`; both engines must
exit 0. Every positive binds all four hosted targets. Execution on this host
establishes macOS arm64 behavior, not other-host execution.

The requirement coverage is:

- Establishment: `issue` supplies the only 48-unit root and checked partition
  conservation supplies the detached and residual owners.
- Identity and lifetime: a brief exclusive loan restores the exact strategy;
  both detached results remain owned until `finish` consumes them.
- Composition: fields, parameters, nested call results, matches, constants, nested machines,
  forwarding machines and target bodies retain the same size obligations.
- Soundness boundaries: the 17-unit twins fail at unchanged call contracts;
  the original custody, restoration and conservation controls remain relevant.
- Psi 05–07: ordinary checked ownership and borrow restoration lower to Terminal
  execution, including detached results and explicit consumption.
- Omega 00–02: package imports, entry bindings, structural layout and calling
  plans must preserve the same values through each exercised route.
- Omega 03–06: native calls, live allocations, field stores and selected
  restoration exercise register and activation storage realization.
- Omega 07–09 and emission: actual native image execution must agree with
  Terminal execution and the authored exit oracle.

The nested-call repair applies each completed operation's own write frame, not
its operands' writes a second time. Independent scalar fixtures under
`tests/omega/{pass,fail}/contracts/nested_call_*` distinguish harmless enclosing
calls from later ordinary and opaque writes; old facts cannot survive the latter.
This does not establish physical backing, cross-host
execution, general allocator retirement, or reset.
