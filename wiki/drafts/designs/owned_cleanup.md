# Owned cleanup consumers

Ratified 2026-11-10. The machine a type selects with `dropped by` received the
dying value as `&mut T` and had to hand back a whole valid value before the
compiler destroyed it. A cleanup machine could therefore never give a
component to an operation that consumes it, which is how the language models
release, and no machine at all could take such a value apart. The selected
machine now takes the value by value, and one explicit pattern is what ends a
value that has a cleanup:

```omega
data Lease [linear] dropped by end_lease {
    pool: PoolHandle;
    slot: SlotClaim;
}

machine end_lease(value: Lease) {
    let Lease { pool, slot } = move value;
    PoolHandle::give_back(move pool, move slot);
}

public machine into_parts(value: Lease) -> LeaseParts {
    let Lease { pool, slot } = move value;
    LeaseParts { pool: move pool, slot: move slot }
}
```

The normative owner is [consumers and cleanup](../../spec/language/ownership.md#consumers-and-cleanup),
with the pattern rule in [patterns](../../spec/language/patterns.md#irrefutable-data-bindings)
and the lowered form in [nominal cleanup](../../spec/terminal-psi/ownership.md#nominal-cleanup).
Track implementation under `OWNED-CLEANUP` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Why

Returning a whole valid value was the obstruction, not a safeguard. The
owning-packages decision had to plan a separate field-disposition mechanism
because a package's cleanup could not move the hidden owner into the stored
operation that consumes it. The guide told type owners to provide a consuming
decomposition operation that no rule let them write. Taking the value by value
also agrees with the rule that a typed place never leaves its default domain:
a consumed value does not leave the domain, it ends.

An earlier draft let the cleanup parameter lose fields while forbidding it to
move whole, and skipped the cleanup when that parameter died. That made an
owned `T` mean something different inside one machine. The pattern rule below
needs no such exception.

## Decisions

1. The selected cleanup machine has exactly one parameter, the subject, by
   value, and a unit result. The `dropped by` clause and how selections
   compose are unchanged.
2. A value with a selected cleanup is an ordinary owned `T` through every move
   and call. A complete consuming pattern ends it: the wrapper's cleanup
   obligation is satisfied there and its components become ordinary owned
   values. Components ignored with `_` or `..` keep their own disposal.
3. That pattern is accepted only in a machine declared in the carrier's owning
   package, or in the exact cleanup machine selected for the occurrence.
   Authority follows where the machine is declared, not where it is called,
   instantiated or inlined. No keyword or authorization list is added; other
   packages call what the owner makes public.
4. The permission ends the wrapper and grants nothing else. Access policies,
   qualifications, loans and backing rules on the components apply as usual.
5. Obligations held independently of the wrapper, such as a capacity claim on
   the whole occurrence, must be transferred to a resulting owner or settled.
   The pattern cannot erase them.
6. A projected move such as `move value.field` on a value with a selected
   cleanup rejects everywhere. The pattern is the one visible transition.
7. An intact value reaching a death edge invokes its selected cleanup,
   including inside the selected machine. The generated invocation is an
   ordinary call for termination, effect, reach and stack analysis, so a
   cleanup machine that never takes its subject apart rejects.
8. The selected machine is an ordinary machine that source may call.
9. Transfer and partition are of ownership, not bytes. A subject that may not
   be relocated stays in place through the transfer, the pattern and the
   disposal of its components.
10. Eligibility for automatic cleanup is unchanged and covers the whole
    selected machine. Explicit consumers such as `into_parts` have their
    ordinary contracts.
11. An owning `some` package gets automatic cleanup by selecting an ordinary
    machine that takes it apart and calls a stored consuming operation. The
    package stays `[linear]`, and the field-disposition mechanism planned in
    the owning-packages record is withdrawn.

## What an author must still get right

The checker accounts for every component and every independent obligation in
any machine that writes the pattern. It cannot check work that exists only as
a statement in the cleanup machine. A package that offers `into_parts` must do
that work there too, or hold it as an owned component, such as a linear token
only one operation can consume, so that every machine taking the value apart
has to deal with it.

## What changes for source

Each selected cleanup machine changes its parameter from `&mut T` to `T` and
gains a consuming pattern, even one that only borrows its subject. A machine
that used the fields through the reference then uses the bound components.
`source/library` selects no cleanup machine. A text search on the ratification
revision found `dropped by` in 576 files under `samples` and 1,088 under
`tests`, most of them fixtures; that is a file count from text search, not a
count of selections.

## Implementation order

1. Check a selected machine's signature as one by-value subject with a unit
   result.
2. Admit the complete consuming pattern on a subject with a selected cleanup
   by declaration package or exact selection, retire its nominal disposition
   there, and reject it elsewhere.
3. Lower the cleanup action as an ordinary owned-call transfer and include
   generated cleanup calls in termination, effect and stack analysis.
4. Keep an address-stable subject in place through transfer and partition.
5. Admit source calls to a selected machine.
6. Sweep `samples` and `tests/fixtures`.

The encoded Terminal Psi fields for cleanup actions and decomposition evidence
are settled with this implementation, as one coordinated semantic-format
change, so that a borrowed-cleanup row never acquires owned-transfer meaning.

## Fixtures and acceptance

- Automatic cleanup and a direct source call to the same machine dispose of
  each component exactly once.
- An owner-package `into_parts` returns the components without cleanup running.
- The same pattern in another package rejects, with the fields visible.
- A foreign generic helper instantiated with the type cannot write the pattern.
- A cleanup machine that lets its subject die whole rejects.
- A value constructed inside a cleanup machine keeps its normal cleanup.
- An ignored component is disposed of by its own cleanup.
- A capacity claim on the whole occurrence cannot vanish through an empty or
  ignoring pattern.
- An address-stable subject is consumed without relocation.
- An owning package with an owner-selected cleanup machine is disposed of
  automatically; one with only a stored operation stays linear.

Acceptance: these pass `omega run --both`, and no selected cleanup machine in
`samples` or `tests/fixtures` takes its subject by reference.

## Left open

- The encoded Terminal Psi rows, as noted above.
- `source/library/core/drop.omg` documents the cleanup target as one that
  source cannot call. The comment follows the implementation.
