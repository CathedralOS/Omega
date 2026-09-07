# Omega documentation

Documentation is being consolidated into four roles. A specification is the
current contract; the guide explains it; proposals suggest changes; work notes track
unfinished implementation. No Rust implementation or accepted proposal silently
overrides a current contract.

- [Language guide](language_guide/language_guide.md): explanations and examples.
- Specification: [Terminal product](spec/terminal-psi/product.md),
  [boundary calls and realization](spec/terminal-psi/boundary_calls.md), and
  [immutable byte views](spec/terminal-psi/byte_views.md), and
  [observations](spec/terminal-psi/observations.md). Resource contracts:
  [logical work and response](spec/resources/logical_work.md) and
  [provisioned storage](spec/resources/storage.md). Artifact details:
  [canonical encoding](spec/terminal-psi/encoding.md).
- Proposals: substantive designs will move to `proposals/`; the
  [owner-question inbox](../OWNER_QUESTIONS.md) holds unresolved decisions.
- [Work plan](work/documentation_cleanup.md) and [execution board](../TASKS.md):
  unfinished work, deleted when complete.

## Subjects still awaiting consolidation

These remain the current references for their unmigrated subjects, not competing
owners of the migrated Terminal subjects listed above:

- The [guide](language_guide/language_guide.md) still carries language rules
  awaiting extraction into the specification.
- [Terminal vocabulary and verification](architecture/pipeline/terminal_psi.md)
  retains the remaining operation and proof contracts.
- [Build/package behavior](design_briefs/build_and_package_model.md) and
  [calling plans](design_briefs/calling_plans.md) retain their respective contracts.
- [Architecture overview](architecture/architecture.md) navigates implementation
  and the other legacy subject references.
- The [bootstrap decision record](architecture/bootstrap_chain/decisions.md)
  remains protected pending explicit owner authorization of its transition.

The specification is not claimed to be fully formalized, and documented design
does not imply implemented support. A real unresolved conflict is marked at its
subject and raised in the owner inbox rather than resolved by choosing whichever
document is newest.
