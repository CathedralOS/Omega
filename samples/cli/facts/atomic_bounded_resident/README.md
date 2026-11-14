# Bounded atomic residents

Owning outcome: **ATOMIC-MEMORY-MODEL** in TASKS.md. The full item remains open.

Fact: **A shared atomic store preserves its resident bound and ordering, and a load carries that bound into Exact subtraction.**

Every place preserves bound 100, stores 41 through a shared helper, and consumes
the observation in `100 - observed` before checking runtime result 59. Each twin
permits 101 in the store helper and actually supplies 101, violating the same
Cell bound of 100 at the store. The former twins weakened only the private
helper contract while still supplying 41. Checked private-arrival inference now
proves those safe calls, so their old refusals are retained as historical
observations rather than invalidity evidence.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [invalid](invalid_local/main.omg) |
| Field | [program](field/main.omg) | [invalid](invalid_field/main.omg) |
| Parameter | [program](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| Call result | [program](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| Match join | [program](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| Const name | [program](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| Nested machine | [program](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| Other machine | [program](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| Target body | [program](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

Transport uses ordinary propositions under [domains](../../../../wiki/spec/language/domains.md):
field invariants use `where`, parameter bounds use `requires`, and returned
snapshots use `ensures result <= 100`. Receiving locals remain bare `u32`.
The owning machine's guarantee applies at its nested machine's normal exit;
nested signatures declare only arrival requirements. No caller guard or named
replacement domain re-establishes the fact.

Atomic resident validity has distinct establishment evidence. Ordinary scalar
store evidence supplies no atomic authority; initialization has its own validity
path. A load's declaration bound attaches to the exact completed snapshot.
Subsequent writes cannot reinterpret it, and no cached current-resident constant
or requested receiving predicate supplies its proof. The [composition](composition/main.omg)
checks swap and decisive-exchange instruction-observed priors and resident bounds,
including a swap result transported through a checked return guarantee. A reserved
result slot has no mathematical initializer: neither arithmetic nor return-contract
checking may treat the parser's placeholder zero as the observed prior.

Controls additionally reject [a result goal as its own premise](controls/load_result_goal_is_not_premise/main.omg),
[a stronger receiving field](controls/load_field_goal_is_not_premise/main.omg),
[a cached stored constant as a fresh load value](controls/load_bound_is_not_current_value/main.omg),
[an observed prior treated as zero](controls/prior_is_not_zero/main.omg),
[a return guarantee derived from the reserved zero](controls/prior_guarantee_is_not_zero/main.omg), and
[using an event result before completion](controls/prior_before_event/main.omg).
The row, composition and controls are pinned under `tests/omega/{pass,fail}/facts/atomic_bounded_resident`.

Serial execution agreement establishes no concurrent theorem. Bounded
single-attempt exchange, universal fetch preservation, non-observing exchange,
standalone atomic locations, relation/refinement checks, and the required
TR3–TR8 concurrent runtime witnesses remain internal work. The named shared-store
fixture executes at exit 70 in scoped corpus intake; its normal CLI project
review awaits broader capability decisions. The [concurrency spec](../../../../wiki/spec/language/concurrency.md)
explicitly leaves complete formal/global-order axioms unfinished; the exact
release-sequence and global-order owner question remains separate from these
compiler repairs.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Observed evidence is
Linux x86-64 native and Terminal Psi agreement for the positive and source
rejection for the twin; the other hosted targets keep their authored rows,
unexecuted here.
