# Route coverage shard 1

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`
Harness built at: `c2f7013b1017413b848817d9e7b13d7500e72948`
Rows completed: 2174 (shard size 2174)

## Execution

Fixtures run **serially** (one `route_one_comparison` process at a time),
per `tools/build_slot.md` on shared test VMs (one heavy corpus-style job
per machine). No parallel fixture processes.

Build:
```
cd /agent/repos/omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \
  mbx build -p compiler --example route_one_comparison < /dev/null
```

Per fixture (`fail/` uses `--every-machine`):
```
timeout 300 target/debug/examples/route_one_comparison \
  [--every-machine] <fixture>/main.omg < /dev/null
```

## Agreement definition

`current_verdict` is the harness `current=` field from the same
`route_one_comparison` run (the ordinary checker on the typed frontier),
not the corpus ledger or fixture tier. `route_verdict` buckets match
`--tally`: any closure `rejected` rejects the fixture; else any `accepted`
closure accepts (even if sibling closures are `outside`); else `outside`.
`agreement` is `n-a` unless both sides are `accepted` or `rejected`.

## first_refusal buckets

- Construct refusals use the harness detail after the generic reason
  (e.g. `type float`, not `Unsupported`).
- `meta:no-bound-entry`: compilation binds no entry and no machine root
  was elaborated (not a construct refusal).
- `meta:typed-frontier-stopped`: `current=stopped`, route never ran.

## Timeouts

No `installed_arrival_*` fixtures appear in this shard (index i%8==1).
Timeout bucket (300s): 0 rows.

## Totals

- route_verdict: {'outside': 1736, 'rejected': 65, 'accepted': 245, 'error': 128}
- agreement (judged=310): {'n-a': 1864, 'agree': 89, 'disagree': 221}
- timeouts: 0
- disagreements: 221
- route accepts / current rejects: 205

## Top first-refusal constructs (outside bucket)

1. `machine service reach` — 333
1. `parameter mutable parameter` — 257
1. `machine generic parameters` — 188
1. `type reference` — 152
1. `meta:no-bound-entry` — 139
1. `declaration Build selection that is not a machine or a data type` — 119
1. `type data with custody` — 102
1. `machine attached data` — 84
1. `type constrained` — 58
1. `type float` — 34

## Hand-check: five disagreements

Verified by re-reading `program` lines from a fresh harness run;
disagree rows match `current=` vs aggregated `route=` (tally rules).

1. `immutable_unsigned_mask_false_floor_rejected`: current=rejected;
   `machine:invalid` outside (contract term…); `machine:main` accepted
   with empty obligation set → fixture route=accepted, disagree.
2. `generic_operator_guard_argument_mismatch`: current=rejected;
   `machine:inspect` outside (type float); `main`/`done` accepted →
   route=accepted, disagree.
3. `indexed_divisor_wrong_collection`: current=rejected; `quotient`
   outside (type reference); `main` accepted → disagree.
4. `implicit_integer_policy_erasure_rejected`: current=rejected;
   `consume` accepted; `erase`/`main` outside (type constrained) →
   route=accepted, disagree.
5. `truncating_reconstruction_too_tight`: current=rejected; tally
   sees an accepted machine closure alongside outside siblings →
   route=accepted, disagree.

## Hand-check: five route-accepts / current-rejects

Same mechanism as above (not ledger drift): the lowered route accepts
a trivial or unobligationed helper closure while the product checker
rejected the defect on another machine still outside the subset.

- `tests/omega/fail/arithmetic/immutable_unsigned_mask_false_floor_rejected/main.omg` (1.30s)
- `tests/omega/fail/arithmetic/implicit_integer_policy_erasure_rejected/main.omg` (1.29s)
- `tests/omega/fail/arithmetic/indexed_divisor_wrong_collection/main.omg` (1.25s)
- `tests/omega/fail/arithmetic/truncating_reconstruction_too_tight/main.omg` (1.33s)
- `tests/omega/fail/asm/atomic_release_disposition_missing_store/main.omg` (1.44s)
