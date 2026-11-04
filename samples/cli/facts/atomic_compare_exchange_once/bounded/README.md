# Bounded single-attempt production

These programs keep the selected atomic resident at or below 100 after every
single-attempt exchange outcome. They extend the existing **ATOMIC-MEMORY-MODEL**
fact; they do not introduce another capability row.

Replacement 99 fits the standing bound 100. Each standard twin changes only
that bound to 98, so the same replacement cannot establish a valid resident.
Zero initialization and stores 10 and 17 still fit. A caller guard does not
supply the missing obligation.

The same canonical outcome, exact cell, observed failure values, ordering and
sibling checks cross all nine existing transport places. The current focused
qualification includes all 14 positive bounded fixtures and their 14 controls,
as part of the parent row's recorded scope. See the
[parent validation](../README.md#validation-and-remaining-scope) for actual
producer revisions, hosts and execution results. The earlier note describing a
resident-decision lowering refusal is not a current result; this milestone's
fixed-baseline run covered the nine unbounded places rather than this production.

The operand-effect helper retains its explicit `ensures result == 99`, and its
atomic write still makes the expected value mismatch. Dynamic replacements use
`requires replacement <= 100`. Ordered replacements retain the earlier expected
operand's mutation and restored referent invariant. The left/right snapshot
programs distinguish an earlier read of 99 from a sibling's later write of 101,
and a later read of 70; their twins compute 101 and reject. The compiler must
consume those completed values rather than reevaluate the original expression.

Fresh serial agreement does not establish concurrent publication, contention,
the memory-model axioms or the **TR3-TR8** task-runtime acceptance. Those
requirements remain open with the parent task.
