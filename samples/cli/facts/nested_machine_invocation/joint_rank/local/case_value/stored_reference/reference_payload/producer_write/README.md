# Producer-established payload bounds

Owner: `NESTED-MACHINE-UNIFICATION` in `TASKS.md`. This is another production of
[the same reference-payload rank fact](../README.md), with the same bound and
counter operation, rather than a new fact family.

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends while advancing the explicit counter once.

The producer `input` starts with an unconstrained mutable `Leaf`, writes 7 or 0,
and returns `Payload::Pending` carrying that exact leaf. It owes both its selected
case and `result.value.count <= 7` at normal completion. No entry bound or caller
guard establishes this guarantee. The loop consumes that published guarantee
through `Remaining.child.value.count` in the nine sibling transport places.

Every original leaf starts at 55 with stamp 413. Seven decrements take the counter
from 3 to 10; the empty path preserves 10. Callers inspect the original leaf, the
payload guard 827 and canary 641. The match join also inspects its unselected
backing. Omitting or inverting a producer write, redirecting the reference, taking
the wrong cycle arm, or losing writeback causes `main` to trap.

Each `invalid/main.omg` changes only the producer's seven-value write to 8 while
retaining the public bound. It must refuse the producer's own Ensures, independently
of the callers' contracts. Sources are pinned under `tests/omega/{pass,fail}/facts`
as `nested_machine_joint_rank_producer_write_reference_payload_<place>_*`.

Run a positive with `omega run --both <main.omg>` and check its twin with
`omega --check <invalid/main.omg>`. Every positive binds all four hosted targets.

The fixed baseline is `28c4d04ce1a3c31177812a9c64493a80cd006c75`. Its verified optimized
compiler refuses all nine positives at `input` statement 1 with `cannot prove
ensures contract`. The field place additionally loses the nested call's public
bound, and the call-result place loses the moved carrier's completed case evidence.
Raw baseline outcomes and exact source hashes are in `build/producer-postcondition/`.

The repaired path retains joined scalar observations on the exact returned value,
keeps owned case observations when a move retires the former home, and consumes
completed scalar predicates through the same typed contract owner as descriptor
metadata. The own postcondition remains an obligation, never an input assumption.
The caller's record-field projection uses the existing ordered normal-completion
proof; it does not rebuild a nested call's guarantees from its syntax.

Focused controls additionally reset the referent after the bounded write, mutate
it in a later return operand, distinguish a saved reference from a reseated
reference, and distinguish a selected index operation's result from its backing
array element. The reject twins must each reach their own intended obligation.
The corresponding fixtures are named `completed_operand_*` under
`tests/omega/{pass,fail}/facts/`.

This does not establish joined bounds through an additional saved mutable
reference after an unrelated descriptor reseat, nor repair the independently
observed mutable-carrier duplicate-cleanup refusal. Those compositions remain
outside this completed producer transport row and inside the parent capability's
remaining acceptance.

Acceptance includes establishment from actual completed writes; exact result and
backing identities across calls, construction, joins and moves; independent loan,
access and case checks; Terminal execution and affine cleanup; ordinary provider,
layout and ABI realization; disjoint native homes and cycle preservation; and
native/Terminal agreement on the full caller-visible oracle. The existing long-cycle
rank witness retains physical-stack coverage. Other-host execution and the parent
capability's remaining obligations remain open; source admission alone closes none
of those runtime requirements.

[Saved-reference transport after descriptor reseating](saved_reference/README.md)
continues the remaining joined-bound and cleanup composition.
