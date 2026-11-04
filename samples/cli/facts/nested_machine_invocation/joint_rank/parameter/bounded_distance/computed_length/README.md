# Computed descriptor endpoint for an unranked helper

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../TASKS.md).
This is a smaller witness inside the parameter place of the
[bounded-distance row](../README.md). [main.omg](main.omg) now runs with its
authored `Expected exit: 0` under `omega run --both`.

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends toward the computed descriptor endpoint
`items.len - 1` while advancing the explicit counter once.

A cycle may stop one element before the end of a slice, so `count` ranks itself
by the distance from `cursor` to `items.len - 1`. Its private helper `forward`
states no ranking: it bumps the caller's counter and calls
`count(counter, view, position + 1)`. Starting at cursor 0 over eight elements,
the cycle runs seven times and raises the counter from 3 to 10; any other count
traps.

## What changed

Before this change the compiler refused the program with `no exact
private-coordinate assignment proves every edge and complete cycle` (recorded
on the rebased base `6672d7b68`, optimized build). `forward` could only measure
toward values it owns — `view.len`, its own position, or a constant — and its
distance to `view.len` is one larger than `count`'s distance to `items.len - 1`,
so the call from `count` to `forward` would have raised the rank.

`forward` may now measure toward the endpoint `count` authored, read through
`forward`'s own call to `count`. That call passes `view` as `items`, so in
`forward` the endpoint means `view.len - 1`. The rule is general: every formal
the authored endpoint reads must arrive as one of the helper's whole parameters
that the helper never reassigns, so the helper's entry already holds the value
the authored machine will see. The pairing only proposes a bound; each actual
edge and complete cycle is still proved by the existing component search.

The retained rank plan records this as an expression operand that keeps two
identities: the state that declares the expression (`count`) and the state that
evaluates it (`forward`), with a binding from `items` to `view`. Executable rank
observations read the endpoint through that binding, including the slice length
`items.len`, which now reads `view`'s descriptor.

## Reject twin

[invalid/main.omg](invalid/main.omg) differs only in the helper's call:
`count(counter, view, position)` does not advance, so no complete cycle
descends and the program is refused with the same component diagnostic.

Dropping `requires items.len > 0` is not a reject twin for this program: the
remaining `requires cursor < items.len` already implies `items.len >= 1`, so the
endpoint forms and the program is correctly accepted (native and Terminal exit 0).

## Requirements

- Establishment: `count`'s requires form the endpoint; `forward` establishes its
  own `position < view.len - 1`. No caller guard supplies either.
- Identity and lifetime: the helper's endpoint is read from its entry
  parameters, which it does not reassign; the counter stays caller-owned.
- Composition: both edges are proved against the transported endpoint, and only
  the `forward -> count` edge is strict.
- Checked-to-Terminal Psi lowering: the rank observation for `forward` evaluates
  `count`'s expression through the retained formal binding.
- Native realization and execution: linux_x86_64 native and Terminal Psi agree
  on exit 0; the other hosted targets are declared but not executed here.

## What remains

A helper that supplies an endpoint formal through a field projection or a
computed argument has no transported bound yet. The nine places that carry
this whole-parameter endpoint are parameter, local, field, call result,
match join, const name, nested machine, other machine, and target body.

## Pinned fixtures

- [pass](../../../../../../../../tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_parameter_exit/main.omg)
- [fail](../../../../../../../../tests/omega/fail/facts/nested_machine_joint_rank_bounded_computed_length_parameter_rejected/main.omg)
