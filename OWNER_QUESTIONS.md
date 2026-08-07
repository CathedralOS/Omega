# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-08-07.

## Q1 — Where does a content namespace declare its origin policy?

Every `IntervalSet<CoordinateSpace>` coordinate space and
`CountedQuantity<Unit>` unit has one closed origin policy. `ProgramLocal`
permits owner-authorized declared capacity; `ProviderBacked` permits fresh
roots only through selected admitted issuance. The compiler currently retains
only the normalized identity of the generic argument. An ordinary declaration
such as `data PhysicalMemory {}` says neither which policy applies nor which
package owns that decision. Which source surface is canonical?

- introduce a dedicated sealed content-namespace declaration that names the
  coordinate-space or unit identity and its origin policy; or
- attach a content-origin clause to the owner-controlled nominal data
  declaration used as the algebra parameter, with the clause becoming active
  when that type is selected by `IntervalSet` or `CountedQuantity`.

The choice fixes declaration ownership, imports and visibility, duplicate or
conflicting policy diagnostics, normalized identity and fingerprints, artifact
provenance, and where declared `ProgramLocal` capacity is authorized. Omission
must reject once a type is used as a content namespace; the compiler must not
infer the policy from a name, constructor, content projection, or provider use.
Relating a program-local namespace to external reality remains a separate
admitted correspondence and must not be implied by either spelling.
