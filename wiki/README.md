# Omega documentation

| Directory | Read it for |
| --- | --- |
| [language_guide/](language_guide/language_guide.md) | How to write Omega. The existing guide is still being consolidated. |
| spec/ | Current contracts, organized by subject. |
| [proposals/](proposals/README.md) | Proposed changes, not current rules. |
| [drafts/](drafts/README.md) | Temporary investigations and migration plans. |
| [pre_migration/](pre_migration/README.md) | Existing documents that still need review and porting. |

The specification uses grammar, definitions, rules, tables, and examples where
useful. It is not claimed to be fully formalized. Implementation details belong
beside code; completed work belongs in Git, not current reference material.

## Current specification subjects

- Mathematical proofs: [contracts, bundles, normalization, and trust](spec/proofs/contracts.md).
- Terminal Psi: [product](spec/terminal-psi/product.md),
  [calls and outcomes](spec/terminal-psi/calls_and_outcomes.md),
  [control flow and ranking](spec/terminal-psi/control_flow.md),
  [boundary calls](spec/terminal-psi/boundary_calls.md),
  [byte views](spec/terminal-psi/byte_views.md),
  [structural access and stores](spec/terminal-psi/structural_access.md),
  [structural predicates](spec/terminal-psi/structural_predicates.md),
  [loan resources and compatibility](spec/terminal-psi/loans.md),
  [structural claims and cleanup](spec/terminal-psi/ownership.md),
  [dynamic dispatch](spec/terminal-psi/dynamic_dispatch.md),
  [observations](spec/terminal-psi/observations.md), and
  [encoding](spec/terminal-psi/encoding.md),
  [verification](spec/terminal-psi/verification.md),
  [mathematical proof values](spec/terminal-psi/mathematical_values.md), and
  [integer certificates](spec/terminal-psi/integer_certificates.md).
- Resources: [logical work](spec/resources/logical_work.md) and
  [storage](spec/resources/storage.md),
  [content custody](spec/resources/content_custody.md), and
  [placed access](spec/resources/placed_access.md).
- Build: [declarations and package identity](spec/build/declarations.md),
  [native products and component publication](spec/build/component_publication.md)
  and [private callbacks](spec/build/private_callbacks.md).
- Packages: [acceptance](spec/packages/acceptance.md) and the
  [workflow guide](language_guide/packages.md).

Only the migrated subjects above have specification owners so far.
The [migration index](pre_migration/README.md) identifies the remaining source
material. Moving a file there is not a review or an approval of its contents.

The [cleanup plan](drafts/documentation_cleanup.md) drives the migration.
[Execution tasks](../TASKS.md) track implementation;
[owner questions](../OWNER_QUESTIONS.md) hold unresolved decisions.
