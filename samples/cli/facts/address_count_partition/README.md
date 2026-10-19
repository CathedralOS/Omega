# Address/count interval partition

The fact is that an address base and a count length partition into disjoint
intervals with exactly the original content. The operation is a checked split,
with the requested count bounded by the input length and the input endpoint
bounded by a separately formed address ceiling. No host address width is assumed.

Every place performs the same operation: split a `Region { base: 10, length: 80 }`
at a requested count of `20` under an address `ceiling` of `100`, verify that the
left piece is `{base: 10, length: 20}` and the right piece is `{base: 30,
length: 60}`, and exit 0 only when both hold. Only the transport of the operands
changes.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | the requested count is copied into a local, after an unrelated local store |
| field | `field/` | bounded record fields carry the base, length and requested count |
| parameter | `parameter/` | the split machine reads `length` and `ceiling` directly |
| call result | `call_result/` | a call with a declared result supplies the requested count |
| match join | `match_join/` | both arms keep the bounded count, so the joined count splits safely |
| const name | `const_name/` | `const LENGTH: u64 = 20` provides the named requested count |
| another state | `other_state/` | a successor state receives the operands and splits there |
| another machine | `other_machine/` | a forwarding machine transports operands to the split machine |
| another target body | `target_body/` | each of four target bodies splits the base and count |

Each positive place and the root `main.omg` binds `ProgramEntry` for all four
hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`),
declares `Expected exit: 0`, and passes `omega run --both` — Terminal Psi and
native execution agreeing:

```sh
omega run --both samples/cli/facts/address_count_partition/main.omg
omega run --both samples/cli/facts/address_count_partition/local/main.omg
omega run --both samples/cli/facts/address_count_partition/field/main.omg
omega run --both samples/cli/facts/address_count_partition/parameter/main.omg
omega run --both samples/cli/facts/address_count_partition/call_result/main.omg
omega run --both samples/cli/facts/address_count_partition/match_join/main.omg
omega run --both samples/cli/facts/address_count_partition/const_name/main.omg
omega run --both samples/cli/facts/address_count_partition/other_state/main.omg
omega run --both samples/cli/facts/address_count_partition/other_machine/main.omg
omega run --both samples/cli/facts/address_count_partition/target_body/main.omg
```

Each place has an `invalid/` child that breaks the partition contract by dropping
the `length <= whole.length` bound from the `split` machine's own `requires`
clause; the requested count is then unbounded relative to the input, and the
residual `whole.length - requested` is no longer provably in `u64` range. The
twin differs from its positive only in the construct's own contract — never in a
caller guard. It carries its authored rejection expectation in
`invalid/sample.json` (`rejection_contains`) and `invalid/expected.txt`, so the
control rejects with `exact arithmetic` rather than for an unrelated reason.
Check a twin with `omega --check <place>/invalid/main.omg`.

The local place writes an unrelated local before splitting. The completed
return theorem must survive that assignment; it is not an entry-only fact.
The separate unrelated-store regression under
`tests/omega/pass/core/address_interval_partition_unrelated_store_exit` pins
the same distinction without changing any partition operand. Neither test
authorizes storage: live backing and parent-to-child custody remain unfinished.

## Governance and Evidence

- Owning requirement: `wiki/spec/language/counts_and_addresses.md`
- Parent task: `BUMP-ALLOCATOR-CANARY` (`TASKS.md`)
- Reserved ticket: `3a4e4800ae2040059680d6c228f5683f`

This is the numeric prerequisite of `BUMP-ALLOCATOR-CANARY` and Cathedral's
backed allocator, not an authority issuer or a functioning allocator. The local
`Region::Owned` qualification grants no storage rights. Real `Extent` partition
still needs checked custody transfer and retirement of the parent.

### Measured Observations

Producer `target/debug/omega` built at `opt-level: 1` from base `c4fed470f35f`,
host `macos_arm64`.

- `omega run --both` on the root and all nine places: `native exit 0` and
  `interp exit: 0` for every one of the ten positives.
- `omega run --both <place>/invalid/main.omg` on all nine twins: exit 200 with
  the pinned `exact arithmetic in machine `split` state `entry` local `right` may
  overflow `u64`` diagnostic (decision 17).
- Discrimination, two independent probes. Corrupting the requested partition
  count (`20` -> `25`, so the left piece is no longer `{base: 10, length: 20}`)
  produces `native signal 5` together with
  `interp crash: Trap (host presentation agrees)`. Corrupting the address
  `ceiling` (`100` -> `89`, below the interval end `90`) is refused at the call
  site with `cannot prove requires contract for call split from main ...`. The
  oracle therefore discriminates, both engines trap on the corrupted partition,
  and the address-ceiling half is enforced statically.
- `tools/sample_native_gate.py --filter cli/facts/address_count_partition`: 10/10
  samples meet their documented oracle; 9/9 intended-invalid controls match their
  authored rejection.
- `tools/corpus_gate.py --native --filter facts/address_count_partition`: executed
  9, exit codes `{'0': 9}`; the nine `built ... exit:0` rows are recorded in
  `tests/omega/corpus_native_macos_arm64.txt`.
- `tools/corpus_gate.py --filter facts/address_count_partition`: 0 structural
  moves, 18 fixtures (`checked` 9, `rejected` 9), 0 fail fixtures did not reject.

### Acceptance Demonstrated

1. **Establishment and identity**: the `split` machine's own `requires` clause
   (`length <= whole.length && embed(whole.base) + embed(whole.length) <=
   embed(ceiling)`) establishes both the count bound and the address fit before
   the residual is formed, so the returned `Pieces` carries an exact value.
   Neither engine re-establishes the fact with a post-hoc caller equality guard,
   and the local place stores an unrelated local before splitting to show the
   completed return theorem survives that store.
2. **Composition**: each `invalid/` twin differs from its positive only in the
   construct's own contract — the dropped `length <= whole.length` bound in
   `split`'s `requires` clause — and never in a caller guard, so the reject twins
   exercise the fact itself.
3. **Transport across nine places**: the base, length, requested count and
   ceiling survive transport across local bindings, struct fields, machine
   parameters, call return results, match join bindings, named constants, state
   transitions, machine forwarding, and target-scoped bodies (`linux_x86_64`,
   `linux_arm64`, `macos_arm64`, `windows_x86_64`).
4. **Soundness boundaries**: the checked split preserves `Region` linearity and
   affine cleanup — `drop(whole)` retires the parent after the pieces are formed,
   and `main` destructures and drops both pieces — while the erased `IntervalSet`
   content obligation (`Owned::content(old(&whole)) == separate(...)`) stays
   mathematical and never becomes an executable value.
5. **Checked-to-Terminal Psi lowering (`Psi 05-07`)**: each place lowers to
   Terminal Psi and executes in the interpreter; the observed `interp exit: 0`
   agrees with the native exit for all ten positives, and the affine cleanup and
   state transitions (`done`/`bad`, `crash Trap`) lower without loss.
6. **Provider settlement, layout geometry and ABI calling plans (`Omega 00-02`)**:
   each positive binds `ProgramEntry` for all four hosted targets and settles the
   provider rows for the `Region`/`Pieces` structural carriers.
7. **Instruction selection, register homes and activation frames (`Omega 03-06`)**:
   the nine `_exit` fixtures in `tests/omega/pass/facts/` are built and executed
   for `macos_arm64`, recording `built ... exit:0` with the split and its
   `Pieces` return realized in a frame.
8. **Form encoding, branch relaxation, relocations and image emission (`Omega
   07-09` + emission)**: the `target_body` place additionally carries the split
   in all four target-scoped bodies; the emitted `macos_arm64` image executes and
   agrees with Terminal Psi.

### What Remains Open

- Real `Extent` partition: checked custody transfer from parent to children and
  retirement of the parent under **CONSERVATION-CONTRACT /
  TERMINAL-CONTENT-CLAIMS**.
- Aligned request geometry and the counted-residual/tail relationship required by
  **BUMP-ALLOCATOR-CANARY**.
- Live backing and resident-storage establishment for the allocator itself; this
  row grants no storage rights.
- Native execution rows are recorded for `macos_arm64` only; the three other
  hosted targets are bound and checked but not executed on this host.