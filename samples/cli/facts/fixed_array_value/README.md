# Fixed array value carrier and in-bounds index preservation

A fixed-extent array `[u64; 2]` preserves its in-bounds index contract `index < 2`
and independent element values across all nine places.

Owning Task: `CONSERVATION-CONTRACT` / `TERMINAL-CONTENT-CLAIMS`
Reserved Claim Ticket: `26cce69dcf0e413baec3f321436e739b`

## Requirements and Evidence

Measured on macos_arm64 with an `opt-level 1` producer at `303d8b0d161` (base
`2d783d092d9d`). Producer identity: `omega --build-profile` reports
`{"schema":"omega-compiler-build-profile-v1","opt_level":"1"}`.

- **Establishment**: Fixed-extent array `[u64; 2]` construction `[19, 42]` and entry precondition
  `requires index < 2` (or record constraint `where index < 2` / named constant `LIMIT = 2`) establish
  proven in-bounds access before indexing. `omega run --both` reports native exit 0 and interpreter
  exit 0 agreeing for all nine places; `sample_native_gate --facts` reports 9/9 meeting the documented
  oracle, 0 unscored.
- **Identity and Lifetime**: The independent element values `values[0] == 19` and `values[1] == 42` are
  conserved across all value transfers and transitions. Discrimination was proved before recording the
  oracle: corrupting the authored element to `[19, 43]` makes **both** engines trap in every place
  (native signal 5, interpreter `crash Trap`, exit 255), so the recorded exit 0 is earned by the program
  rather than assumed by a vacuous oracle.
- **Composition**: Machine calls, pattern match joins, and state transitions preserve exact preconditions
  and actual values without narrowing or corruption. Each `invalid/main.omg` differs from its positive in
  only the bound carrying the fact and its explanatory comment — no caller guard, no unrelated edit
  (`diff` per place). Restoring only that bound makes all nine twins `--check` exit 0, so each rejection
  is caused solely by the construct's own contract.
- **Soundness Boundaries**: Weakened preconditions (`index <= 2` or widened limits) are rejected at
  compile time with `cannot prove index ... is within length 2`, proving that out-of-bounds indexing is
  strictly prevented across all nine places. Each fail fixture pins the exact per-place index expression
  (`local_index`, `container.index`, `index`, `picked_index`, `joined_index`, `passed_index`), and
  `corpus_pin_audit.py` lists none of the nine among its 369 weak pins.
- **Terminal Psi and carrier lowering / native realization**: `omega run --both` executes each place on
  both engines with agreeing exits, and `corpus_gate --native` builds and runs all nine
  (`9 built, 9 executed, exit codes {0: 9}`). The macos_arm64 native golden records the nine rows.
  **Open**: the linux/Windows native goldens carry no rows for this fact, and the fact is not yet
  exercised by a runtime conformance for those hosts.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | The array and bounded index are copied into locals before indexing. |
| field | `field/` | A record field carries the bounded index alongside the fixed-extent array. |
| parameter | `parameter/` | The reading machine receives the bounded index and array directly as parameters. |
| call result | `call_result/` | The index arrives from a helper call with an ensures result < 2 contract. |
| match join | `match_join/` | Both branches of a match expression produce in-bounds indices. |
| const name | `const_name/` | A named constant bounds the index contract for the fixed-extent array. |
| other state | `other_state/` | A transition to a successor state forwards the bounded index and array. |
| other machine | `other_machine/` | A forwarding machine transports the bounded index and array to the reader. |
| target body | `target_body/` | Each of the four target bodies reads the array element using the bounded index. |

Each place is an application that binds `ProgramEntry` for `windows_x86_64`,
`linux_x86_64`, `macos_arm64` and `linux_arm64` in its `build.omg`, declares
`Expected exit: 0`, carries a measured `expected_native_exit.txt`, and passes
`omega run --both`:

```sh
omega run --both samples/cli/facts/fixed_array_value/local/main.omg
omega run --both samples/cli/facts/fixed_array_value/field/main.omg
omega run --both samples/cli/facts/fixed_array_value/parameter/main.omg
omega run --both samples/cli/facts/fixed_array_value/call_result/main.omg
omega run --both samples/cli/facts/fixed_array_value/match_join/main.omg
omega run --both samples/cli/facts/fixed_array_value/const_name/main.omg
omega run --both samples/cli/facts/fixed_array_value/other_state/main.omg
omega run --both samples/cli/facts/fixed_array_value/other_machine/main.omg
omega run --both samples/cli/facts/fixed_array_value/target_body/main.omg
```

Each place has an `invalid/` twin that weakens the index bound (e.g. to `index <= 2` or `LIMIT = 3`),
so the array index access is no longer proved in bounds. Every twin rejects at compile time with
`cannot prove index ... is within length 2`. Check a twin with `omega --check <place>/invalid/main.omg`;
`sample_native_gate --facts` reports 9/9 intended-invalid controls matching their authored rejection.

The row is pinned by `tests/omega/{pass,fail}/facts/fixed_array_value_through_*` fixtures; every
positive records native exit 0. `corpus_gate --filter facts/fixed_array_value` is clean:
18 fixtures — 9 checked, 9 rejected — with 0 rejected without their expected fragment and 0 fail
fixtures failing to reject.
