# Qualified reference recasts

This unchanged program observes a `Narrow` field through a shared `Wide` view
and exits 70. The mutable domain and range siblings exercise equivalent facts;
the range program also writes through its byte view and observes the original
signed field.

The public command is:

```sh
omega run --both tests/omega/pass/recast/runtime_shared_domain_weakening_recast_exit/main.omg
```

Ordinary package-review acceptance is separate from compiler acceptance. The
repository harness supplies its reviewed fixture policy:

```sh
python tools/corpus_gate.py --both --filter runtime_shared_domain_weakening_recast_exit,runtime_mutable_equivalent_domain_recast_exit,runtime_mutable_equivalent_range_recast_exit
```

A fresh CLI built on main `e459ca044a2cd6254868fe719a147d28511b5660` with
this repair completes package preparation and returns exit 200: ordinary
package acceptance is missing or its current requirements need review. Its
generated review remains pending; the compiler fix does not accept that policy.

On Linux x86-64 base `f64782b891147a98314a566696a5a5e044378c43`, all three
reject at stage 05 with `Unit graph dropped or added a body effect`, while the
checked interpreter returns 70. With the statement-custody repair, all three
compile and execute with native/interpreter exit 70.

`unit/attached_unit/statement_custody.rs` uses the existing declared-carrier
comparison to account for an alias declaration. The checked recast judgment
owns qualification legality; storage accounting preserves carrier identity and
independently rejoins loan geometry, access and activation. A domain or range
qualification does not allocate another runtime value.

`python tools/corpus_gate.py --filter recast/` checks the shared-strengthening,
mutable range mismatch and access-escalation refusal controls alongside the
positive variations. The sources retain their authored qualification and
borrowing arrangements.

Strict crate Clippy (`mbx clippy -p checked-trees-to-lowered-psi --lib --no-deps
-- -D warnings`) exits 101 on both the base compiler source above and the
candidate: the same 12 diagnostics point to untouched files. The recast checked
gate also retains a golden mismatch for aggregate-slice loan liveness, diagnosed
by checking before stage 05. Neither baseline issue is suppressed by this fix.
