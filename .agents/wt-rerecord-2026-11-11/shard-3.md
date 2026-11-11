# wt re-record shard 3 (of 6)

Pinned base: `3bbe5c131a15666f68a700f499783f04910e8143`  
Branch: `work/wt-rerecord-shard-3`  
Host: linux/x86_64, 4 logical CPUs  
Jobs: `OMEGA_CORPUS_JOBS=2`, `OMEGA_CORPUS_FIXTURE_SECS=900`, `MBX_SCHEDULER_CPUS=4`, `CARGO_BUILD_JOBS=4`  
Batches: 1 (187 fixtures, under 200 cap)

## Row list

187 fixtures from sorted `+wt` ledger split (`/tmp/shard2` = shard index 2 = human shard 3). Canonical list: `shard-3-row-list.txt` in this directory.

## Commands

```bash
git checkout 3bbe5c131a15666f68a700f499783f04910e8143
git checkout -b work/wt-rerecord-shard-3
bash tools/agent_setup.sh && mbx xtask doctor

grep -E '^(pass|fail|run)/.*@[0-9a-f]{12}\+wt' tests/omega/corpus_outcomes.txt \
  | cut -d' ' -f1 | LC_ALL=C sort -u > /tmp/all.txt
split -n l/6 -d -a1 /tmp/all.txt /tmp/shard
# shard file: /tmp/shard2

CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null

FILTER=$(paste -sd, /tmp/shard2)
nice -n 19 mbx xtask corpus --record --filter "$FILTER" < /dev/null

# verification (post-record, clean golden)
nice -n 19 mbx xtask corpus --filter "$FILTER" < /dev/null
```

(`tools/swarm/safe_record.sh` was not used on this Linux host: its free-memory gate is macOS `vm_stat` only and aborted spuriously.)

## Outcomes

| Category | Count | Notes |
| --- | ---: | --- |
| Re-recorded (shard slice) | 187 | all former `+wt` stamps cleared to `@3bbe5c131a15` |
| Unchanged verdict and diagnostics (stamp/millis only) | 157 | dirty stamp replaced; prior millis kept where semantics matched |
| Changed | 30 | settled below |
| Unsettled | 0 | full shard `corpus: clean — 188 fixture(s) verified` (filter comma list counts 188 names; slice list 187 unique paths) |

## Changed rows by cause

### Main-line verdict: `expected` → `unexpected` (2)

Recorded regression already true at `3bbe5c131a15` (ledger-only PR allowed per `tools/testing.md` “Who may record a regression”). Re-verified stable on this host.

- `fail/facts/size_align_layout_geometry_through_match_join_rejected`
- `fail/facts/static_callback_reach_requirement_telescope_requirement`

### Diagnostic call-site display names demangled (9)

`$specialized$…` callee labels in requires-contract errors → stable `forward` / `observe` (and target-body probe labels unchanged in meaning). Verdict stays `rejected expected`.

- `fail/facts/runtime_index_application_nested_forwarded_through_call_result_rejected`
- `fail/facts/runtime_index_application_nested_forwarded_through_const_name_rejected`
- `fail/facts/runtime_index_application_nested_forwarded_through_field_rejected`
- `fail/facts/runtime_index_application_nested_forwarded_through_local_rejected`
- `fail/facts/runtime_index_application_nested_forwarded_through_match_join_rejected`
- `fail/facts/runtime_index_application_nested_forwarded_through_other_machine_rejected`
- `fail/facts/runtime_index_application_nested_forwarded_through_other_state_rejected`
- `fail/facts/runtime_index_application_nested_forwarded_through_parameter_rejected`
- `fail/facts/runtime_index_application_nested_forwarded_through_target_body_rejected`

### Additional pinned rejection line(s) (13)

Compiler at base now emits an extra diagnostic line (e.g. unreachable-branch guard contradiction) while retaining the original obligation; verdict stays `rejected expected`.

- `fail/facts/runtime_index_application_through_call_result_rejected`
- `fail/facts/runtime_index_application_through_const_name_rejected`
- `fail/facts/runtime_index_application_through_field_rejected`
- `fail/facts/runtime_index_application_through_local_rejected`
- `fail/facts/runtime_index_application_through_match_join_rejected`
- `fail/facts/runtime_index_application_through_other_machine_rejected`
- `fail/facts/runtime_index_application_through_other_state_rejected`
- `fail/facts/runtime_index_application_through_parameter_rejected`
- `fail/facts/unsigned_byte_to_signed_case_domain_through_const_name_rejected`
- `fail/generics/const_index_active_case_false`
- `fail/generics/inferred_equation_deferred_callee_requires_rejected`
- `fail/generics/inferred_equation_explicit_forward_conflict_rejected`
- `fail/generics/runtime_index_predicate_global_shadow`

### Diagnostic wording / ordering (6)

Same rejection obligation; message prefix or spelling aligned with current emitter (e.g. `cannot prove requires contract:` prefix, minor identifier spelling). Verdict stays `rejected expected`.

- `fail/facts/static_callback_reach_requirement_telescope_proposition`
- `fail/facts/static_callback_reach_requirement_telescope_shadow`
- `fail/facts/static_evidence_premise_through_target_body_rejected`
- `fail/generics/inferred_machine_array_equation_result_argument_count_rejected`
- `fail/generics/named_reference_equation_wrong_result_source`
- `fail/generics/open_value_case_constructor_false`

## Unsettled

None.
