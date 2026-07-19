# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`.

Last pruned: 2026-07-18.

## 1. Admitted executable installation evidence

Omega has no general byte-to-code conversion, `ExecutableMemory` capability,
JIT facility, or self-modifying-code path. Executable eligibility is a sealed,
non-self-assertable fact over an immutable artifact, established by the normal
validation/admission spine and bound to normalized content, identity,
relocations, footprint, and placement plan. Installation may realize an
already-admitted artifact in executable mappings; mutation destroys eligibility.

Every executable-mapping route, including page-table construction and checked
assembly, must require this provenance. Otherwise those routes would recreate
the deleted conversion as a security bypass. Device firmware remains device
I/O, and template patching of already-live code remains component
replacement/quiescence rather than arbitrary code generation.

The remaining owner decision is the normalized evidence/API boundary:

- What exact state/domain records admitted-but-uninstalled and installed
  artifacts?
- Where is final validation performed after declared relocation,
  materialization, and placement?
- How is installation authority scoped to artifact identity, slot, placement,
  and audience?
- What evidence records W^X and local or cross-core instruction-fetch
  visibility, including targets where enforcement is convention-only?
- How do static images and dynamically loaded admitted artifacts share this
  discipline without pretending there is one universal final link step?

Recommendation: a linear installation transition accepting only an admitted
artifact, validating the final realization, and producing a scoped installed
executable plus any visibility-completion obligation. A dormant/local audience,
a future remote fetcher, and a possible current executor are distinct: the last
requires replacement/quiescence, never installation.

Detailed settled context and engineering residue are in
[`wiki/design_briefs/os_memory_and_hardware_foundation.md`](wiki/design_briefs/os_memory_and_hardware_foundation.md).
