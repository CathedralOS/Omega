# Ledger bisect part 27 (k=27, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`.

Re-measured with a commit-scoped target directory `/tmp/bisect-target-<sha>`. Bisect oracle: exit 0 matches the fixed PIN ledger expectation, exit 1 is the PIN status plus the PIN diagnostic class, exit 125 is any other outcome, a build failure, or a ledger-only commit. Good commits come from an exponential first-parent walk (1, 2, 4, 8, …) with no binary search. `git bisect start --first-parent` runs between that good commit and the PIN.

## Counts

| | Count |
| --- | ---: |
| Rows re-measured | 13 |
| Fixed on PIN | 5 |
| Still broken on PIN | 8 |
| Clusters with a finished result | 3 |

## Clusters

| id | rows | witness | good | breaking | PR | parent 3x | bad 3x | status |
| --- | ---: | --- | --- | --- | --- | --- | --- | --- |
| c01 | 2 | `pass/targets/private_target_supplies_remain_disclosed` | `a48219f5cf97b2d7d766d07c48e1d8946a2b6477` | `eacdc3114349460f12c4a923026824330a8381d3` | 1617 | 0,0,0 | 1,1,1 | bisected |
| c02 | 1 | `pass/targets/retained_target_family_bodies` | `c417f61c95ba2b5078d36f367dc9b3c8e2ea0e30` | `6f0848b9beab89da25edf4054e9f32c5df962a90` | 2012 | 0,0,0 | 1,1,1 | bisected |
| c03 | 1 | `pass/borrows/runtime_stored_reference_case_residual_retirement_exit` | `a48219f5cf97b2d7d766d07c48e1d8946a2b6477` | `` |  |  |  | bisect-failed |
| c04 |  |  |  |  |  |  |  | pending |
| c05 |  |  |  |  |  |  |  | pending |
| c06 |  |  |  |  |  |  |  | pending |
| c07 |  |  |  |  |  |  |  | pending |

### c01

Witness: `pass/targets/private_target_supplies_remain_disclosed`

Members: `pass/targets/private_target_supplies_remain_disclosed` `pass/targets/retained_target_provider_family_exit`

retained_target_provider_family_exit first read exit 127 at the breaking commit because the witness script was edited while that run was in flight. Recheck on the same commit-scoped runner: exit 1, rejected, unknown canonical target profile `reserved_target`. It flips with the witness (parent 0, breaking 1).

Mechanism:

```
mechanism not established
```

Member checks at the parent and at the breaking commit:

- `pass/targets/private_target_supplies_remain_disclosed` parent=0 breaking=1
- `pass/targets/retained_target_provider_family_exit` parent=0 breaking=1

### c02

Witness: `pass/targets/retained_target_family_bodies`

Members: `pass/targets/retained_target_family_bodies`

Mechanism:

```
mechanism not established
```

Member checks at the parent and at the breaking commit:

- `pass/targets/retained_target_family_bodies` parent=0 breaking=1

### c03

Witness: `pass/borrows/runtime_stored_reference_case_residual_retirement_exit`

Members: `pass/borrows/runtime_stored_reference_case_residual_retirement_exit`

git bisect run did not name a first bad commit (exit 2). See c03-bisect.log.

Mechanism:

```
mechanism not established
```

## Fixed on PIN

- `fail/contracts/target_family_recursive_guarantee`
- `fail/contracts/target_family_unshared_result`
- `fail/targets/sibling_body_type_error_rejected`
- `fail/targets/unrealized_conformance_rejected`
- `fail/borrows/aliased_forwarded_placed_views`

## Still broken on PIN

- `pass/targets/private_target_supplies_remain_disclosed` — unknown canonical target profile `reserved_target`
- `pass/targets/retained_target_provider_family_exit` — unknown canonical target profile `reserved_target`
- `pass/targets/retained_target_family_bodies` — unknown canonical target profile `archive_target`
- `pass/borrows/runtime_stored_reference_case_residual_retirement_exit` — state `selected` receives `main` while local borrow `pair` is still active (borrowed at statement 2; its last use is at statement 9)
- `run/borrows/terminal_case_completed_observation` — state `finish` receives `pool` while local borrow `field` is still active (borrowed at statement 0; its last use is at statement 2)
- `pass/traits/prepared_callable_nested_borrow_paths_exit` — `main` has no executable body: this compiler does not lower a state of it yet (statement sequence: completion stored reference dependencies); in state `main::entry`
- `pass/facts/saved_observation_snapshot_indexed_handback_other_machine_exit` — cannot prove requires contract for call agree from read_kept: saved == holder.items[0] && saved == 7 (needs fact `observed == current && observed == 7` here; establish it before the call via a prior contract guarantee, domain membership, or guard) in state `entry`
- `pass/facts/case_payload_exclusive_loan_through_other_machine_exit` — machine `apply` state `entry` statement 0: cannot prove selected case for payload access `chosen.value`: captured source `chosen::Present.value` has no live case evidence: the current source does not select its required case

## Commands

```bash
cd /agent/repos/omega
# per commit: CARGO_TARGET_DIR=/tmp/bisect-target-<40-char-sha>
bash /tmp/ledger-bisect-part27/tools/bisect_witness.sh
```

Host: cloud VM. One `mbx` build at a time. `OMEGA_CORPUS_JOBS=1`.
