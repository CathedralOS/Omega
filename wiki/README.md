# Omega documentation

| Directory | Read it for |
| --- | --- |
| [learn/](learn/index.html) | Illustrated field guide for reading Omega and supervising agent-written code. |
| [language_guide/](language_guide/language_guide.md) | How to write Omega: a short tutorial that starts from code. |
| spec/ | Current contracts, organized by subject. |
| [proposals/](proposals/README.md) | Proposed changes, not current rules. |
| [drafts/](drafts/README.md) | Temporary investigations and migration plans. |

The specification uses grammar, definitions, rules, tables, and examples where
useful. It is not claimed to be fully formalized. Implementation details belong
beside code; completed work belongs in Git, not current reference material.
The spec states the accepted language and required guarantees, independently of
compiler delivery. Put implementation gaps in the owning task on the
[execution boards](../TASKS.md), extending an existing task before adding another.
Keep normative limits, trust assumptions and required evidence in the spec;
do not replace them with progress reports or remove them as status.

## Guide and specification

The guide and the specification do different jobs, and text belongs in one of
them:

| | Guide | Specification |
| --- | --- | --- |
| Answers | What do I write? | What is true in every case? |
| Leads with | A code sample | The rule |
| Covers | What most code needs | Everything, including what rejects and why |
| When it is incomplete | It links to the rule | That is a defect |

A rule is stated in the specification. The guide shows it working and links to
it. A change to the language updates the specification, and touches the guide
only to correct an example the change made wrong or to add a sample for
something a newcomer would reach for. A guide chapter that needs a list of
what rejects, or a paragraph of exceptions, is restating the specification;
link to it.

## Public names

Use **Psi** for the portable IR, artifact, interpreter and user-facing commands.
Use **Terminal Psi** only to identify the internal compiler stage or its
representation, particularly when distinguishing it from checked or lowered
trees. Internal crate paths and wire/type identifiers retain their own names.
Do not turn the stage name into a public product label. Source-language terminal
transfers and terminal outcomes are unrelated and retain those terms.
The [command-line contract](spec/build/command_line.md) owns public spellings.

## Current specification subjects

- Source semantics: [invocation contracts and live facts](spec/language/state_contracts.md),
  [data and literals](spec/language/data_and_literals.md),
  [patterns and dispatch](spec/language/patterns.md),
  [modules and visibility](spec/language/modules.md),
  [ownership and multiplicity](spec/language/ownership.md),
  [lifetimes and carried borrows](spec/language/lifetimes.md),
  [concurrency and atomics](spec/language/concurrency.md),
  [machines and refinement](spec/language/machines.md),
  [semantic reflection and typed visitation](spec/language/reflection.md),
  [named conformances](spec/language/conformances.md),
  [generic applications](spec/language/generics.md),
  [termination and progress](spec/language/termination.md),
  [process exit and abandonment](spec/language/process_exit.md),
  [domains and qualification](spec/language/domains.md),
  [value-dependent facts and views](spec/language/dependent_values.md),
  [service reach and operational ceilings](spec/language/effects.md),
  [inline assembly](spec/language/assembly.md),
  [interpreted instructions and environments](spec/language/assembly.md#interpreted-execution),
  [semantic evaluation](spec/language/evaluation.md),
  [expressions and operators](spec/language/expressions.md),
  [constants](spec/language/constants.md), [numeric values and bounds](spec/language/numeric_values.md),
  and [counts, indices, and addresses](spec/language/counts_and_addresses.md).
- Representation policies: [layout plans](spec/layouts/plans.md),
  [size and alignment queries](spec/layouts/plans.md#size-and-alignment-queries),
  [native code regions](spec/language/counts_and_addresses.md#native-code-layout-queries),
  [recasts](spec/layouts/recasts.md), and [codecs](spec/layouts/codecs.md).
- Mathematical proofs: [foundation and checking judgments](spec/proofs/foundation.md),
  [mathematical bindings and logical hypotheses](spec/proofs/mathematical_bindings.md),
  [W-based induction and indexed encoding](spec/proofs/inductive_profile.md),
  [contracts, bundles, normalization, and trust](spec/proofs/contracts.md),
  [stable index identity and proved compatibility](spec/proofs/contracts.md#licensed-normalization)
  and [relations and quotients](spec/proofs/quotients.md).
- Runtime data/code interfaces: [machine values](spec/language/machine_values.md)
  and [some packages](spec/language/some_packages.md).
- Psi: [product](spec/terminal-psi/product.md),
  [calls and outcomes](spec/terminal-psi/calls_and_outcomes.md),
  [control flow and ranking](spec/terminal-psi/control_flow.md),
  [boundary calls](spec/terminal-psi/boundary_calls.md),
  [byte views](spec/terminal-psi/byte_views.md),
  [structural access and stores](spec/terminal-psi/structural_access.md),
  [structural predicates](spec/terminal-psi/structural_predicates.md),
  [loan resources and compatibility](spec/terminal-psi/loans.md),
  [structural claims and cleanup](spec/terminal-psi/ownership.md),
  [runtime callable dispatch](spec/terminal-psi/dynamic_dispatch.md),
  [observations](spec/terminal-psi/observations.md), and
  [encoding](spec/terminal-psi/encoding.md),
  [fact validity](spec/terminal-psi/fact_validity.md),
  [mathematical proof values](spec/terminal-psi/mathematical_values.md), and
  [integer certificates](spec/terminal-psi/integer_certificates.md).
- Resources: [logical work](spec/resources/logical_work.md) and
  [storage](spec/resources/storage.md),
  [external-entry stacks](spec/resources/entry_stacks.md),
  [content custody](spec/resources/content_custody.md),
  [dense owned runs](spec/resources/typed_runs.md),
  [authority and qualification](spec/resources/authority.md),
  [extents and mappings](spec/resources/extents.md),
  [access policies](spec/resources/access.md),
  [device custody and ordering](spec/resources/device_access.md),
  [carry demands](spec/resources/carry.md),
  [runtime-sized activation storage](spec/resources/activation_storage.md),
  [allocation strategies](spec/resources/allocation.md),
  [bounded byte input](spec/resources/bounded_input.md), and
  [bounded growth](spec/resources/bounded_growth.md).
- Build: [command-line tools](spec/build/command_line.md),
  [declarations and package identity](spec/build/declarations.md),
  [dependency scopes, product references, and isolated I/O](spec/build/scoped_execution.md),
  [configuration and targets](spec/build/configuration.md),
  [recorded proof-work policy](spec/build/configuration.md#proof-work-policy),
  [portable platform services](spec/build/platform_services.md),
  [requirement-based test groups](spec/build/testing.md),
  [selected-product behavior exclusions](spec/build/behavior_exclusions.md),
  [optimization selection and validation](spec/build/optimizations.md),
  [validation subjects](spec/build/validation_subjects.md),
  [target slots and entry roots](spec/build/entry_roots.md),
  [UEFI entry and handoff](spec/build/uefi_entry.md),
  [installed external roots](spec/build/external_roots.md),
  [boundary calling plans](spec/build/calling_plans.md),
  [boundary signature shapes](spec/build/boundary_shapes.md),
  [foreign binding values](spec/build/foreign_bindings.md),
  [foreign storage and lifetime](spec/build/foreign_storage.md),
  [machine-state evidence](spec/build/machine_state_evidence.md),
  [provider selection](spec/build/provider_selection.md),
  [root-assumed bindings (exceptional use only)](spec/build/assumed_binding_contracts.md),
  [embedding and interpreted components](spec/build/embedding.md),
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
  [checked boundary topology](spec/packages/topology.md),
  [project locks](spec/packages/locks.md),
  [declaration and carried-type boundaries](spec/packages/boundaries.md),
  [core and ordinary libraries](spec/packages/toolchain.md),
  [compiler-derived review](spec/packages/review.md),
  [acceptance](spec/packages/acceptance.md), and the
  [workflow guide](language_guide/packages.md).

Bootstrap language and proof contracts live beside the chain; start at
[edge contracts](../bootstrap/CONTRACT.md) and
[minimization](../bootstrap/MINIMIZATION.md). Alternative-chain tradeoffs are
[reference notes](drafts/reference/bootstrap_chain_alternatives.md), not a proposed replacement.

[Execution tasks](../TASKS.md) track implementation;
[owner questions](../OWNER_QUESTIONS.md) hold unresolved decisions.
