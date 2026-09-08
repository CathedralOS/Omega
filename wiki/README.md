# Omega documentation

| Directory | Read it for |
| --- | --- |
| [language_guide/](language_guide/language_guide.md) | How to write Omega. The existing guide is still being consolidated. |
| spec/ | Current contracts, organized by subject. |
| [proposals/](proposals/README.md) | Proposed changes, not current rules. |
| [drafts/](drafts/README.md) | Temporary investigations and migration plans. |

The specification uses grammar, definitions, rules, tables, and examples where
useful. It is not claimed to be fully formalized. Implementation details belong
beside code; completed work belongs in Git, not current reference material.

## Current specification subjects

- Source semantics: [state contracts and live facts](spec/language/state_contracts.md),
  [modules and visibility](spec/language/modules.md),
  [ownership and multiplicity](spec/language/ownership.md),
  [lifetimes and carried borrows](spec/language/lifetimes.md),
  [concurrency and atomics](spec/language/concurrency.md),
  [machines and refinement](spec/language/machines.md),
  [named conformances](spec/language/conformances.md),
  [termination and progress](spec/language/termination.md),
  [domains and qualification](spec/language/domains.md),
  [value-dependent facts and views](spec/language/dependent_values.md),
  [service reach and operational ceilings](spec/language/effects.md),
  [semantic evaluation](spec/language/evaluation.md),
  [expressions and operators](spec/language/expressions.md),
  [constants](spec/language/constants.md), [numeric values and bounds](spec/language/numeric_values.md),
  and [counts, indices, and addresses](spec/language/counts_and_addresses.md).
- Representation policies: [layout plans](spec/layouts/plans.md),
  [recasts](spec/layouts/recasts.md), and [codecs](spec/layouts/codecs.md).
- Mathematical proofs: [contracts, bundles, normalization, and trust](spec/proofs/contracts.md)
  and [relations and quotients](spec/proofs/quotients.md).
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
  [external-entry stacks](spec/resources/entry_stacks.md),
  [content custody](spec/resources/content_custody.md),
  [authority and qualification](spec/resources/authority.md),
  [extents and mappings](spec/resources/extents.md),
  [placed access](spec/resources/placed_access.md),
  [device custody and ordering](spec/resources/device_access.md),
  [carry demands](spec/resources/carry.md),
  [allocation strategies](spec/resources/allocation.md), and
  [bounded growth](spec/resources/bounded_growth.md).
- Build: [declarations and package identity](spec/build/declarations.md),
  [configuration and targets](spec/build/configuration.md),
  [optimization selection and validation](spec/build/optimizations.md),
  [target slots and entry roots](spec/build/entry_roots.md),
  [UEFI entry and handoff](spec/build/uefi_entry.md),
  [installed external roots](spec/build/external_roots.md),
  [boundary calling plans](spec/build/calling_plans.md),
  [boundary signature shapes](spec/build/boundary_shapes.md),
  [foreign binding values](spec/build/foreign_bindings.md),
  [foreign storage and lifetime](spec/build/foreign_storage.md),
  [machine-state evidence](spec/build/machine_state_evidence.md),
  [provider selection](spec/build/provider_selection.md),
  [task activation and lifecycle](spec/build/task_runtime.md),
  [service permissions](spec/build/permissions.md),
  [opaque representations](spec/build/opaque_representations.md),
  [execution and generated source](spec/build/execution.md),
  [observations and reproducibility](spec/build/observations.md),
  [standalone compiler request](spec/build/compiler_request.md),
  [native products and component publication](spec/build/component_publication.md),
  [macOS applications](spec/build/macos_application.md),
  [private callbacks](spec/build/private_callbacks.md),
  [interrupt obligations](spec/build/interrupt_obligations.md),
  [hardware materialization](spec/build/hardware_materialization.md), and
  [executable installation](spec/build/executable_installation.md).
- Packages: [source selection](spec/packages/sources.md),
  [project locks](spec/packages/locks.md),
  [declaration and carried-type boundaries](spec/packages/boundaries.md),
  [core and ordinary libraries](spec/packages/toolchain.md),
  [compiler-derived review](spec/packages/review.md),
  [acceptance](spec/packages/acceptance.md), and the
  [workflow guide](language_guide/packages.md).

Bootstrap language and proof contracts live beside the chain; start at
[edge contracts](../bootstrap/CONTRACT.md) and
[minimization](../bootstrap/MINIMIZATION.md). Alternative chains remain an
[open proposal](proposals/bootstrap_chain_alternatives.md).

The [cleanup plan](drafts/documentation_cleanup.md) drives the migration.
[Execution tasks](../TASKS.md) track implementation;
[owner questions](../OWNER_QUESTIONS.md) hold unresolved decisions.
