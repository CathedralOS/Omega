# Computed subslice extent from a predicate

Given `can_window(items, low) == true`, the captured view
`items[low + 0 .. low + 2]` has extent 2, and reading offset 1 observes original
backing `items[low + 1]`.

`can_window` is an ordinary pure total predicate whose checked body states
`items.len == 4 && low <= 2`. Its name alone grants nothing. Its exact
application supplies the premises for the checked endpoint arithmetic and the
independent ordered range-formation obligation. The subsequent offset-1 read
still owes its own bound against the resulting descriptor. The target-family
head retains the whole predicate requirement: publishing interval observations
requires checked equivalence in both directions, rather than only necessary
consequences. Disconnected truth sets cannot be replaced by their interval hull.

The nine places are `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`. Each retains
that same fact, bound and read. Every runnable entry checks the view's measured
extent and reads from `[11, 23, 37, 53]` at both `low = 1` and the allowed
endpoint `low = 2`, expecting 37 and 53 respectively. The match place executes
both alternatives at both values. All four target bodies are checked; native
execution observes Windows x86-64 and Linux x86-64 on matching hosts.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) | the computed subslice view is bound in a local |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) | the computed subslice view is stored in a record field |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) | the computed subslice view arrives as an explicit parameter |
| call result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) | a helper call returns the computed subslice view with `result.len == 2` |
| match join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) | both match arms produce the computed subslice view |
| const name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) | named constants `ZERO` and `SPAN` supply the subslice endpoint offsets |
| other state | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) | a successor state receives the computed subslice view |
| other machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) | another machine receives and inspects the computed subslice view |
| target body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) | all four target-scoped machine bodies construct and read the subslice view |

Each place's `invalid/` (and `controls/insufficient_bound`) changes only the predicate's bound
from `low <= 2` to `low <= 3`. The original range and read remain intact. The
twin must reject the range call's `end <= items.len` obligation, because the
weaker premise permits an endpoint of 5 in backing of extent 4.

## Requirements and Evidence

- **Parent outcome:** `BORROW-PROOF-CONVERGENCE` (`TASKS.md`).
- **Establishment:** Checked pure predicate `can_window(items, low) == true` (whose body states `items.len == 4 && low <= 2`) supplies the exact collection length and index upper bound needed to prove exact `u64` endpoint additions `low + 0u64` and `low + 2u64`, ordered range formation `start <= end`, and backing inclusion `end <= items.len`, establishing that the resulting subslice descriptor `items[low + 0u64..low + 2u64]` has exact extent 2 without any post-hoc caller guard.
- **Identity and lifetime:** The borrowed subslice view `&[u8]` retains its provenance and exact extent fact (`view.len == 2`) across immutable local bindings, borrowed record fields (`Held<'a>`), parameters, helper return lifetimes, match joins, constant offset names, state transitions, and cross-machine calls while remaining tied to the live backing array `items`.
- **Composition:** Call contracts (`requires can_window(items, low) == true`, `requires view.len == 2`, `ensures result.len == 2`), match-arm joins, state transitions (`inspect(view)`), and target-qualified machine bodies compose the predicate premise with subslice construction so the consuming offset read `view[1]` is proved directly from the transported extent fact (`1 < 2`).
- **Soundness boundaries:** Each place's reject twin (`<place>/invalid/main.omg` and `<place>/controls/insufficient_bound/main.omg`) weakens only the predicate bound from `low <= 2` to `low <= 3` and is rejected during proof checking (`cannot prove requires contract for call range ... end <= items.len`), while additional corpus controls reject effectful actuals, saved stale predicates, and mutated fields.
- **Terminal Psi and carrier lowering:** Subslice range formation `items[low + 0u64..low + 2u64]`, descriptor length projection `view.len`, and indexed element read `view[1]` lower without loss into Terminal Psi slice-view descriptors (base pointer + element count), preserving borrow custody and exact byte-offset calculation across all nine places.
- **Native realization, frame/layout encoding, and image emission:** All four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) bind `ProgramEntry` in root `main.omg` and all nine positive places; native two-word slice descriptor realization, stack-frame array allocation, bounds-checked offset loads, and PE/ELF/Mach-O emission execute and agree with Terminal Psi (`omega run --both`, exit 0).

```sh
python3 tools/sample_native_gate.py --filter cli/facts/computed_subslice_extent
python3 tools/corpus_gate.py --both --filter computed_subslice_extent
```

Authority comes from [state contracts](../../../../wiki/spec/language/state_contracts.md),
[checked predicate consequences](../../../../wiki/spec/language/dependent_values.md#collection-invariants),
[exact subslice descriptors](../../../../wiki/spec/terminal-psi/byte_views.md), and
[live fact identity](../../../../wiki/spec/terminal-psi/fact_validity.md).
