# NEW-DNMO-UNUSED-SELF-RECEIVER-PIN — record

Planner-scoped item: pin the member-call admission of a `self`-receiver
callee that never reads its receiver (`tests/omega/pass/calls/
member_call_unused_self_receiver` + `canary_suite/roster.rs` + this
record). Delete this record once the fixture and its
`CHECKED_ONLY_PASS_CANARIES` seat land.

## Scope verified at `d650f2e45a` (linux x86-64, `omega --check`)

The admission is live: a machine declared `machine Main::constant(&mut
self) -> u64 { 42 }` — `&mut self` receiver, body never naming `self` —
called through the member-call spelling `self.constant()` from
`Main::main` compiles under checked semantics with no diagnostic
(probe fixture, `omega --check`, rev `416e9dd7e6` binary).

No existing member pins the shape: `core/self_read_only_receiver_compile`
takes `&self` but reads receiver state (`room.exits`); the `calls/`
self-receiver members (`machine_self_call_recursion_compile`,
`runtime_selfcall_chain_second_receiver_exit`,
`runtime_enum_self_method_exit`, `sequential_self_field_rmw_exit`,
`value_call_sequential_self_capture_exit`,
`by_value_case_param_self_write_exit`,
`runtime_effectful_guard_local_and_self_terminal_exit`,
`runtime_value_call_self_field_enum_match_exit`) all consume `self` in
the body.

## Pin design (landable when the seat drains)

Fixture `tests/omega/pass/calls/member_call_unused_self_receiver/
main.omg`:

```omega
// An unused `self` receiver is admitted: a member call dispatches on the
// receiver's owner regardless of whether the callee body reads `self`.
// Pins the checked-semantics admission — the receiver is still a real
// boundary argument (dropping it would be a lowering question).
data Main {
}
machine Main::constant(&mut self) -> u64 {
    42
}
machine Main::main(&mut self) {
    let value: u64 = self.constant();
    transition value == 42 {
        true -> yes()
        false -> no()
    }
    state yes(&mut self) { }
    state no(&mut self) { }
}
```

Registration seat: `CHECKED_ONLY_PASS_CANARIES` in
`omega-rust/omega/compiler/compiler/tests/canary_suite.rs` — the same
array carrying `core/self_read_only_receiver_compile` and
`calls/nested_value_call_arg_compile` (checked-semantics compile
members). Note `canary_suite/roster.rs` is only the inventory harness
consuming those arrays; the planner's `roster.rs` path names the
registration file family, the array itself lives in `canary_suite.rs`.

## Fences at record time (do not land the fixture until both drain)

Landing the fixture without a seat fails
`registered_pass_canaries_have_source_on_every_host`
(`InventoryScope::CompleteCorpus` — every on-disk member must be
rostered).

- `omega-rust/omega/compiler/compiler/tests/canary_suite/roster.rs`:
  NEW-APR-TRAPPING-SHIFT-REFUSAL-PIN (Devin / z30), expires ~14:23Z.
- `omega-rust/omega/compiler/compiler/tests/canary_suite.rs`
  (the actual seat): PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION
  (Devin / devin-5389), expires ~16:19Z Sep 21.
- `tests/omega/pass/calls/member_call_unused_self_receiver`: unfenced.
- This draft: unfenced.

## Verdict

Record landed; fixture + roster seat deferred to the fence drain (or a
scope widened to `canary_suite.rs`). The pin is valid today — the
admission is the documented, verified behavior and no existing member
covers it.
