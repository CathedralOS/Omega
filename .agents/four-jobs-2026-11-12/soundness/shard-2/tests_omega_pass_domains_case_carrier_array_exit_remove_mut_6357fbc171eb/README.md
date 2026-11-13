# Case carriers through nested arrays

Squalr's nested-machine migration exposed declaration-contract publication
expanding case-dependent field bounds once per array element, per machine.
The field representations already retain and enforce those bounds.

This smaller witness keeps the same mechanism: a case-dependent default
domain, nested fixed arrays, and a mutable replacement through a call. It
checks both tags, the changed payload, an adjacent payload and enclosing
canaries in Terminal Psi and native execution. Its reject twin changes only
the replacement from 251 to 257, exceeding the declared 256 bound.

The `validation` declaration-predicate tests also pin the representation:
carrier-implied case coverage produces no expanded rows; coverage excluding
a possible case and independent payload couplings still produce rows.
This is a bounded iteration repair under `NESTED-MACHINE-UNIFICATION`, not
closure of that migration or of general quantified collection contracts.
