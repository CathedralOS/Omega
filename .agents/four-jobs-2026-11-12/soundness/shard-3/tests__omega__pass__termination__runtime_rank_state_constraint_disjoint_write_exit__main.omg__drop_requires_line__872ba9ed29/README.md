# Live dependent arrival after a disjoint write

The named state arrives with `scratch <= ceiling`. Assigning `unrelated` changes
neither observation, so the recurrent `scratch > ceiling` guard stays impossible.
The authored main checks that the returned value is 70; success exits zero in
native and Terminal execution.

```sh
omega run --both tests/omega/pass/termination/runtime_rank_state_constraint_disjoint_write_exit/main.omg
python3 tools/corpus_gate.py --both --filter termination/runtime_rank_state_constraint_disjoint_write_exit,termination/runtime_rank_arrival_field_premise_disjoint_write_exit
python3 tools/corpus_gate.py --filter termination/rank_state_dependent_bound_prefix_write
```

The public run requires ordinary local package/entry review. No broad acceptance
policy is included. The matching-host runtime qualification is macOS ARM64.

At `bd68c42a6ef`, both this program and its prefix-write invalid control have
four scalar suffix positions:

| Original position | Original suffix | Canonical arrival requirement |
| --- | --- | --- |
| `walk.remaining` | `u32 [0..=9]` | machine `remaining <= 9` |
| `step.current` | `u32 [0..=9]` | state `current <= 9` |
| `step.ceiling` | `u32 [0..=4]` | state `ceiling <= 4` |
| `step.scratch` | `u32 [0..=ceiling]` | state `scratch <= ceiling` |

All carriers remain `u32`, supplying the zero floors. The dependent upper bound
is an arrival proposition, not a nominal domain, const index or permanent store
restriction. The invalid control writes `ceiling = 0` before the guard. Its
former inequality no longer describes the current values; assuming it would
hide a nondecreasing infinite loop. It must reject the rank proof.

Named-state and call-component rank judgments share the exact observation
coordinates of their arrival premises. A prefix must preserve those observations
before the judgment imports them. Checked-body call variants of the unrelated
write and a nested call that clears `ceiling` retain the same acceptance and
refusal, respectively; the maintained sources keep their original direct stores.
