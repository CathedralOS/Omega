# case-field-store-nonparam-roots — attribution (blocked on live claim)

Leg asked to fix `structural field store: destination parameter` declines.
Diagnosed the failing shape precisely; the entire fix surface is inside
repair-local-r2's live claim LOCAL-DATA-CLUSTER-REPAIR (ticket
3ab40d642ff149a6b35cd0db5b31fd06, expires 2026-09-28T01:07Z, note:
"widened: add control/call_results.rs result-type gate + types/mod.rs
borrowed-view classifiers for &mut view/call-result locals" — the same fix
family). No unfenced sub-path exists; this doc is the resume state.

## The failing shape

Stores whose destination roots at a **`&mut` view local bound to a call
result** (or any non-`Borrow` view initializer):

```omega
let entry_slice: &mut [Entry] = self.entries.as_mut_slice();
entry_slice[0].value = 7;          // decline: destination parameter

let cell: &mut Cell = self.bag.pick();
cell.value = 7;                    // same wall
```

## Mechanism

`structural_scalar_store/destination.rs::resolve`:

1. `canonical_place_from_expression_in_state` roots the target at the local
   symbol (`entry_slice`, `cell`).
2. `rejoin_loan_local` (destination.rs:608) only rejoins locals whose
   `initial_value` is `ExpressionNode::Borrow` — `let x = &mut place`.
   Call results (`as_mut_slice()`, `pick()`) leave the local as root.
3. Param roster match fails (it is a local).
4. `owned_local_root` (destination.rs:249) requires
   `has_plain_owned_contents_with_numeric_constraints` — a reference type
   never satisfies it → `None` → trace phase `destination parameter`.

Contrast that pins it as a field-store-lane gap, not a general view-local
gap: `pass/slices/requires_field_count_alias_index_compile` is `checked`
today — `let exits: &mut [Exit] = room.exits.as_mut_slice()` then
`exits[index] = Exit { ... }` — an **element** store through the identical
call-result view local works. Only **field** stores
(`view[i].field = v`, `view.field = v`) lack the root case.

## Reproduced declines (binary @105dff2a43e, pre-b0d0cc20494)

- `pass/slices/runtime_mutable_slice_element_write_exit` — state 0 stmt 1
  (`entry_slice[0].value = 7`), `destination parameter`.
- `pass/borrow/runtime_method_view_write_after_last_use_exit` — state 0
  stmt 1 (`cell.value = 7` after `let cell = self.bag.pick()`), same.
- `pass/calls/runtime_reference_returned_slice_element_write_exit` — callee
  `Main::fill` (`cell` from `self.pick(bag)`), same.
- `pass/calls/runtime_fixed_vec_round_trip_exit` — callee
  `FixedVecI32x4::push`, same (per corpus row adjacency).
- Squalr machines (per the earlier sweep):
  `pointer_scan_results_{sort_u64,dedup_u64,copy_cached_ids}`.

Minimal reproducer (probe fixture, not committed — corpus_outcomes.txt is
claude-ambient-claimed; whoever resumes should add it as a corpus row):

```omega
data Entry { value: i32; }
data Main {
    console: Binding<Console>;
    entries: [Entry; 4];
}

machine Main::main(&mut self) reaches Console {
    let view: &mut [Entry] = self.entries.as_mut_slice();
    view[0].value = 7;
    transition self.entries[0].value == 7 {
        true -> good()
        false -> bad()
    }
    state good(&mut self) { self.console.exit_process(21); }
    state bad(&mut self) { self.console.exit_process(22); }
}
```

→ `the machine's unit plan was omitted at local construction at
"structural field store: destination parameter" (state 0, statement 1)`.

## Adjacent upstream walls in the same claim (not this store lane)

- Passing a view local as a **state argument** declines earlier:
  `statement sequence: local data: structural result shape` /
  `structural call binding` — e.g.
  `pass/recast/runtime_interior_slice_view_mutable_write_exit` fails at
  `structural call binding` before any store resolves. Both live in
  `control/statement_sequence/local_data.rs`, also claimed by r2.

## Fence map (all under LOCAL-DATA-CLUSTER-REPAIR till 2026-09-28T01:07Z)

- `terminal_unit/structural_scalar_store/` — resolve + Root enum
- `terminal_unit/borrowed_windows.rs` — view-local admission downstream
- `terminal_unit/control/*` (local_data.rs, call_results.rs,
  statement_sequence/*, checked_machine.rs) — binding + forwarding walls
- `terminal_unit/types/mod.rs` — borrowed-view classifiers
- `terminal_unit/calls/{call_operations,call_results}.rs`,
  `receiver_calls/observations.rs`, `atomic_operations.rs`
- `flow/reference_places/result_candidates.rs`,
  `flow/mutation/local_origins.rs`, `flow/transfers{,/projected}.rs`,
  `flow/ownership/{type_references,place_types}.rs`
- `psi/compilation/checking/src/machine_calls/reference_result_custody.rs`,
  `psi/compilation/checking/src/borrow/loans.rs`

## Fix sketch for the resumer (if r2's land leaves the gap)

`destination.rs` needs a borrowed-local root: unique earlier `LocalData`
with a `&mut`/`&`-write reference type and an exclusive-access initializer
(Borrow or call result — the initializer's provenance decides authority).
Downstream, `CheckedStoreRoot::Local` currently
resolves through the structural-homes roster (owned homes); a view local
must instead write *through* the view's stored place — likely a distinct
destination variant or the borrowed-windows root vocabulary from
`owned-local-window-roots` (StructuralWindowRoot) extended with a
local-view case. Element stores already prove the downstream write path
exists for view locals; the field-store resolve is the missing arm.
