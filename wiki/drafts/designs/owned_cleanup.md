# Owned cleanup consumers

Ratified 2026-11-10. The machine a type selects with `dropped by` received the
dying value as `&mut T` and had to hand back a whole valid value before the
compiler destroyed it. A cleanup machine could therefore never give a field to
an operation that consumes it, which is how the language models release. The
selected machine now takes the value by value and is an ordinary consumer:

```omega
data Lease [linear] dropped by end_lease {
    pool: PoolHandle;
    slot: SlotClaim;
}

machine end_lease(value: Lease) {
    PoolHandle::give_back(move value.pool, move value.slot);
}
```

The normative owner is [consumers and cleanup](../../spec/language/ownership.md#consumers-and-cleanup),
with the lowered form in [nominal cleanup](../../spec/terminal-psi/ownership.md#nominal-cleanup).
Track implementation under `OWNED-CLEANUP` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Why

Returning a whole valid value was the obstruction, not a safeguard. The
owning-packages decision had to plan a separate field-disposition mechanism
because a package's cleanup could not move the hidden owner into the stored
operation that consumes it. Taking the value by value also agrees with the
rule that a typed place never leaves its default domain: a consumed value does
not leave the domain, it ends.

## Decisions

1. The selected cleanup machine has exactly one parameter, the subject, taken
   by value. The `dropped by` clause and how selections compose are unchanged.
2. Inside the selected machine the subject may be borrowed and its fields may
   be moved out. The subject may not be moved whole to a local, an argument, a
   field or a result. The parameter is therefore the only occurrence the
   machine answers for, and no tracking through other bindings is needed.
3. When the selected machine returns, each remaining field is disposed of by
   its own cleanup. The subject's selected machine is not invoked again. A
   remaining field with no legal automatic disposition rejects there.
4. Outside its selected machine a value with a selected cleanup cannot be
   partially moved, as before. Nothing else may dismantle it and skip the
   selected machine.
5. The selected machine is an ordinary machine. Source may call it with an
   owned argument; the rule that source may not name the cleanup target is
   removed.
6. The transfer is of ownership, not bytes. A subject that may not be
   relocated is consumed where it sits.
7. Eligibility is unchanged and covers the whole selected machine: it returns
   normally and is infallible, nonblocking, non-suspending, crash-free and
   needs no authority beyond its parameter.
8. An owning `some` package gets automatic cleanup by selecting an ordinary
   machine that opens it and calls a stored consuming operation. The package
   stays `[linear]`, and the field-disposition mechanism planned in the
   owning-packages record is withdrawn.

## What changes for source

Each selected cleanup machine changes its parameter from `&mut T` to `T`. A
machine that moves nothing behaves as before. `source/library` selects no
cleanup machine. A text search on the ratification revision found `dropped by`
in 576 files under `samples` and 1,088 under `tests`, most of them fixtures;
that is a file count from text search, not a count of selections.

## Implementation order

1. Check a selected machine's signature as one by-value subject and apply
   decision 2 inside it.
2. Lower the cleanup action as a transfer of the subject to the target, and
   emit typed residual disposal on the target's own return edge without naming
   the nominal target again.
3. Realize the transfer in place for a subject that may not be relocated.
4. Admit source calls to a selected machine.
5. Sweep `samples` and `tests/omega`, re-recording touched golden rows.

## Fixtures and acceptance

- A cleanup machine that moves a field into a by-value release operation
  passes; the same body under a `&mut` subject rejects.
- A cleanup machine that moves its subject whole rejects.
- A field left unmoved is disposed of once, after the machine returns, and the
  machine runs once per value.
- A partial move outside the selected machine rejects.
- An address-stable subject is consumed without relocation.
- An owning package with an owner-selected cleanup machine that calls its
  stored consuming operation is disposed of automatically; one with only the
  stored operation stays linear.

Acceptance: these pass `omega run --both`, and no selected cleanup machine in
`samples` or `tests/omega` takes its subject by reference.

## Left open

- An explicit consuming operation that takes a value with a selected cleanup
  apart outside its cleanup machine, such as returning one field to the
  caller. The guide tells type owners to provide one and the rules do not say
  how it is written.
- The Terminal Psi encoding rows for nominal cleanup actions carry a receiver
  place. Whether their fields change under an ownership transfer was not
  reviewed for this record.
- `source/library/core/drop.omg` documents the cleanup target as one that
  source cannot call. The comment follows the implementation.
