# Bounded atomic residents

Owning outcome: **ATOMIC-MEMORY-MODEL** in TASKS.md. The full item remains open.

Fact: **A shared atomic store preserves its resident bound and ordering, and a load carries that bound into Exact subtraction.**

Every place preserves bound 100, stores 41 through a shared helper, and consumes
the observation in `100 - observed` before checking runtime result 59. Each twin
changes only the store helper's `requires value <= 100` to `<= 101`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

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
checks swap and decisive-exchange instruction-observed priors and resident bounds.

Controls additionally reject [a result goal as its own premise](controls/load_result_goal_is_not_premise/main.omg),
[a stronger receiving field](controls/load_field_goal_is_not_premise/main.omg),
[a cached stored constant as a fresh load value](controls/load_bound_is_not_current_value/main.omg),
[an observed prior treated as zero](controls/prior_is_not_zero/main.omg), and
[using an event result before completion](controls/prior_before_event/main.omg).
The row, composition and controls are pinned under `tests/omega/{pass,fail}/facts/atomic_bounded_resident`.

The initial validated producer rejects a sufficient bounded store. The preserved
integration producer `bf7a6ee94d1` additionally reproduces missing direct-load
construction and return-contract proofs with this approved spelling. The
`ba1756dbb15` optimized producer passes all 32 row observations and all five
soundness controls. Final qualification and independent review remain pending.
Earlier bracket-range witness observations are historical and superseded; the
queued PR was withdrawn before landing to correct those spellings. Legacy atomic
bracket ranges are not an acceptance requirement or new semantics.

Linux x86-64 is the available execution host; other matching-host executions are
not claimed. Serial agreement establishes no concurrent theorem. Bounded
single-attempt exchange, universal fetch preservation, non-observing exchange,
standalone atomic locations, relation/refinement checks, and the required
TR3–TR8 concurrent runtime witnesses remain internal work. The named shared-store
fixture executes at exit 70 in scoped corpus intake; its normal CLI project
review awaits broader capability decisions. The [concurrency spec](../../../../wiki/spec/language/concurrency.md)
explicitly leaves complete formal/global-order axioms unfinished; the exact
release-sequence and global-order owner question remains separate from these
compiler repairs.
