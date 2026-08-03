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

## Q2 — What is the final artifact-footprint certificate boundary?

Final machine-state admission must independently validate the exact placed
bytes of compiler functions, format-owned thunks, relaxation products,
veneers, generated stubs, and admitted leaves against the selected
`StatePlan`. Which evidence boundary is canonical for both statically linked
and dynamically loaded admitted artifacts?

- a self-describing, versioned instruction/region certificate whose normalized
  semantic rows are replayed against exact final bytes by the admission
  checker; or
- an independent target decoder which derives the complete footprint directly
  from final executable regions, with admitted leaves joined through a
  separate receipt vocabulary?

The choice fixes the certificate format, the trusted decoder surface, how
relocation and generated-region identities bind to decoded instructions, and
where admitted leaf claims enter transitive composition. The current exact
relocation envelope, checked-assembly validators, import-thunk validators, and
complete executable-region inventory are sound precursors, but none may claim
complete final-footprint validation until this boundary is settled.

## Q3 — Where are width-varying foreign record fields normalized?

The portable filesystem metadata surface needs one semantic record, but the
native `struct stat` fields are not representation-identical. Linux x86-64
uses 64-bit `st_nlink` and `st_blksize`, while the AArch64 asm-generic ABI uses
32-bit fields; Darwin differs again. The current `FieldPlan::At` only relocates
one representation-identical field, and `Bits` requires complete source-bit
tiling, so neither can honestly project and extend these target-sized integers.
Which boundary is canonical?

- extend the closed layout-plan vocabulary with a checked integer placement
  that names stored width and signed/zero extension into the semantic carrier,
  including explicit rules for whether and how mutable views may write it; or
- keep layout plans representation-preserving and require target-owned checked
  adapter machines to decode raw foreign bytes into one canonical semantic
  metadata record before portable code observes it?

The choice determines whether width conversion is layout semantics or ordinary
target-policy computation, whether direct foreign-record views remain the
filesystem mechanism, and what read/write guarantees a future width-adapting
placement would carry. Correct Linux `StatLayout`, path metadata, and decoded
descriptor metadata must not claim completion until this is settled.

## Q4 — What is the canonical terminal-Psi conditional edge?

The live terminal-Psi vocabulary has only total unconditional jumps and
returns. Its architecture requires future guards and branch-created blocks to
be explicit, but it does not yet choose the semantic shape that makes one
conditional transition independently verifiable, executable, serializable,
meterable, and lowerable. Which edge form is canonical?

- one conditional edge with an exact Boolean guard and ordered true/false
  successors, each carrying its own typed block-parameter bindings and edge
  actions; or
- separate guarded edges whose mutual exclusivity and exhaustiveness are
  reconstructed as a block-level obligation?

The choice fixes successor ordering and identity, whether the guard is an
already-defined Boolean value or may contain a closed predicate form, where
branch exclusivity/exhaustiveness is proven, which selected edge is charged,
and how exact-path and safe-point fuel certificates identify the untaken arm.
Do not freeze semantic v13, extend the codec, or publish branch certificates
until this shape is settled as one reviewed vertical slice.
