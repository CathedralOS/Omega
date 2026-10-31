# Moving upper distance

Parent: [NESTED-MACHINE-UNIFICATION](../../../../../../../../TASKS.md). This extends the existing bounded-distance production of the joint private-rank fact:

A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

Every place starts with the same seven-element view and `(cursor, stop) = (0, 7)`. Three steps advance cursor to 3; four steps reduce stop to 3. Each step reduces the actual distance `stop - cursor` by one and increments the caller counter, taking 3 to 10. An already-complete `(7, 7)` leaves 10 unchanged. Neither endpoint alone ranks both branches. Each reject twin changes only `ceiling - 1` to `ceiling`, leaving a preserving cycle even though the other branch strictly decreases.

[Joint ranking](../../../../../../../../wiki/spec/language/termination.md#ranking) requires the selected relation and strict descent on every complete cycle. [Invocation contracts](../../../../../../../../wiki/spec/language/state_contracts.md) require each helper's own premises and simultaneous actual substitution. The [product checker](../../../../../../../../source/psi/checking/ranking.omg) already permits a falling upper endpoint with a nondecreasing lower endpoint. This needs no new view or syntax. The named-bound/clamped `IncreasingTo` theory retains its own obligations.

The fixed optimized baseline is `02d6d92b37d4cee20861321d9ad0dfc58a0bb3d8`. All nine formed positives refuse with `no exact private-coordinate assignment proves every edge and complete cycle`. The current Rust producer discards the upper operand and emits `MAX - cursor`, justified only by an invariant upper. One initial other-machine wrapper also omitted its own seven-element premise; both twins now state that independent entry requirement, with the earlier authoring observation retained separately. Acceptance is unfinished: the repair must retain both exact operand observations, independently form both distances before comparison, carry the selected scalar value through checked plans and Terminal/native execution, complete all nine places and pins, and keep nonadvancement, invalid endpoint formation and existing clamped-view controls rejecting.

| Place | Positive | Twin |
| --- | --- | --- |
| local | [source](../../../local/bounded_distance/moving_upper/main.omg) | [source](../../../local/bounded_distance/moving_upper/invalid/main.omg) |
| field | [source](../../../field/bounded_distance/moving_upper/main.omg) | [source](../../../field/bounded_distance/moving_upper/invalid/main.omg) |
| parameter | [source](main.omg) | [source](invalid/main.omg) |
| call_result | [source](../../../call_result/bounded_distance/moving_upper/main.omg) | [source](../../../call_result/bounded_distance/moving_upper/invalid/main.omg) |
| match_join | [source](../../../match_join/bounded_distance/moving_upper/main.omg) | [source](../../../match_join/bounded_distance/moving_upper/invalid/main.omg) |
| const_name | [source](../../../const_name/bounded_distance/moving_upper/main.omg) | [source](../../../const_name/bounded_distance/moving_upper/invalid/main.omg) |
| other_state | [source](../../../other_state/bounded_distance/moving_upper/main.omg) | [source](../../../other_state/bounded_distance/moving_upper/invalid/main.omg) |
| other_machine | [source](../../../other_machine/bounded_distance/moving_upper/main.omg) | [source](../../../other_machine/bounded_distance/moving_upper/invalid/main.omg) |
| target_body | [source](../../../target_body/bounded_distance/moving_upper/main.omg) | [source](../../../target_body/bounded_distance/moving_upper/invalid/main.omg) |

Raw baseline sources, compiler identity and outcomes remain in `build/moving-upper/`. Broader invocation contracts, other orders and product migration remain open. No runtime success is claimed by this checkpoint.


The first repaired producer observes native/Terminal agreement at exit 0 for all nine moving-upper places, while all nine twins reject. Fixed-upper distance, clamped overshoot and its nonprogress controls, signed ranks and tuple ranks retain their own behavior. Input/retention plans now preserve both operand observations and their coordinate/index receipts, and the ordinary scalar DAG computes exact `upper - lower` as one private word. Source comparisons form both endpoint orders independently before comparing those values. Scoped expression inputs retain their exact declaration telescope without importing its facts. The former primitive source query now uses a transient record rather than dummy executable coordinates.

[Narrow endpoints](controls/narrow_endpoints/main.omg) exercise the narrow input-to-word rank boundary. Its ordinary entry condition uncovered a shared integer-predicate cast omission; widening and independently formed exact casts now lower through the common converter. [Exact requirement](controls/exact_requirement/main.omg) proves its narrowing operand from a prior independent bound; [its missing-bound control](controls/invalid_exact_requirement/main.omg) rejects representability rather than assuming its own clause. [Invalid falling interval](controls/invalid_falling_interval/main.omg) rejects the arriving `ceiling <= ceiling - 1` requirement before a negative distance can supply progress. These checks retain their actual producer/source identities in `build/moving-upper/`. Final exact-source corpus acceptance and review are in progress; the PR remains a draft.
