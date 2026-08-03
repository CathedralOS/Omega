# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-08-02.

## Q1 — Who owns entry-provisioned program-storage root identities?

The typed process/firmware handoff must introduce a small number of authority
roots for the loaded image and initial stack/storage, after which the compiler
derives sections and statics as subextents. Which semantic shape is canonical?

- one core-owned stable entry requirement with core-owned image/storage Extent
  domains, inherited by `UefiApplication::entry` and other target entry traits;
  or
- target-owned entry requirements and root domains, with some separate generic
  relationship by which Omega recognizes their image/storage roles?

Core `Extent::Granted` cannot directly cite a Cathedral/UEFI-specific route,
and recognizing friendly target domain names would violate the ownership
firewall. This decision fixes provider-schema identity, route ownership, the
typed handoff shape, and the generic derivation key used for sections/statics.
