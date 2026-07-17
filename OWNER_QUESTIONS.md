# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`.

Last pruned: 2026-07-20.

## Machine parameters

1. **`<machine M>` generics.** N5/N6 schema axioms and `Seq` map/filter need
   machines as generic arguments, but the source and proof model are not yet
   settled.

   Needed ruling: the parameter/signature-constraint spelling; monomorphized
   versus dictionary instantiation; how an instantiated machine's contract is
   exposed to the proof judge; and whether accepted schema grants attach to the
   template or each instantiation. First-order proof/library work does not wait
   on this.
