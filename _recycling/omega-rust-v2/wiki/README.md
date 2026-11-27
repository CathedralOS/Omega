# Omega documentation

| Directory | Read it for |
| --- | --- |
| [learn/](learn/index.html) | Illustrated field guide for reading Omega and supervising agent-written code. |
| [language_guide/](language_guide/language_guide.md) | How to write Omega: a tutorial starting from code. |
| [wiki/spec](../../wiki/spec/) | Current contracts, organized by subject. |
| [proposals/](proposals/README.md) | Proposed changes, not current rules. |
| [drafts/](drafts/README.md) | Temporary investigations and implementation plans. |

The specification uses grammar, definitions, rules, tables and examples where
useful; it is not claimed to be fully formalized. Implementation details belong
beside code. Accepted language and required guarantees are independent of compiler
delivery. Put gaps in the existing owning task on [TASKS.md](../TASKS.md), not a
parallel progress ledger. Keep normative limits, trust assumptions and evidence
requirements in the spec; they are not implementation status to delete.

## Current rules, not historical layers

State the current rule once at its owner and link to it. Correct contradictory
examples directly. Keep a design's reason when it prevents a recurring mistake;
remove superseded rulings, completed migrations, obsolete fallback instructions,
stale pass totals and chronology. Git already retains that history. A recorded
run establishes only its exact revision, host and tested behavior.

Before release, obsolete source spellings and internal formats need no
compatibility aliases, parallel decoders or migration dialects merely to keep
development artifacts working. This does not remove the current format
discriminator or exact schema identity: incompatible bytes must reject rather
than be interpreted under a new layout. Package pins, producer authorization,
selected-provider identity and contracted runtime replacement are live correctness
mechanisms, not historical baggage. The [encoding contract](../../source-rust/subsystems/compiler/psi/artifact/wire/encoding.md)
owns these requirements.

A missing implementation, test or difficult repair stays with its owning task.
Escalate a genuinely unresolved language, architecture or trust choice to
[OWNER_QUESTIONS.md](../OWNER_QUESTIONS.md), naming the customer and the contracts
checked. Do not reopen a settled rule because its implementation is unfinished.

## Guide and specification

| | Guide | Specification |
| --- | --- | --- |
| Answers | What do I write? | What is true in every case? |
| Leads with | A code sample | The rule |
| Covers | What most code needs | Complete behavior, including rejection and why |
| When incomplete | Link to the rule | Repair the contract or escalate the actual choice |

Language changes update the specification and examples they invalidate. Guides
illustrate the rule rather than maintain another exception list. Proof failures
also need precise terminology: unsupported implementation, a missing required
automatic discharge, an unestablished goal and resource exhaustion are distinct
outcomes under [proof contracts](../../wiki/spec/proofs/contracts.md#required-automatic-discharge).

Release-facing examples need owning tests for their advertised behavior.
Deliberately invalid examples must reach the intended rejection, not merely
fail. Prefer maintained source and its tests to copied implementations or old
pass stamps. Label illustrative or unexecuted code honestly; documentation edits
are not execution evidence. Native/Psi agreement covers the tested execution,
not source-meaning errors already shared by both routes. See
[verification placement](../omega-rust/pipeline.md#verification-placement).

## Public names

Use **Psi** for the portable IR, artifact, interpreter and public commands.
Use **Terminal Psi** only for the internal compiler stage or representation,
particularly when distinguishing it from typed source forms. Internal crate
paths and wire/type identifiers keep their names. Do not make the stage name a
public product label. Source terminal transfers and outcomes are unrelated and
keep those terms. The [command-line contract](../../wiki/spec/build/command_line.md) owns
public spellings.

## Current specification subjects

- Source semantics: [invocation contracts and live facts](../../wiki/spec/language/state_contracts.md),
  [data and literals](../../wiki/spec/language/data_and_literals.md),
  [patterns and dispatch](../../wiki/spec/language/patterns.md),
  [modules and visibility](../../wiki/spec/language/modules.md),
  [ownership and multiplicity](../../wiki/spec/language/ownership.md),
  [lifetimes and carried borrows](../../wiki/spec/language/lifetimes.md),
  [concurrency and atomics](../../wiki/spec/language/concurrency.md),
  [machines and refinement](../../wiki/spec/language/machines.md),
  [semantic reflection and typed visitation](../../wiki/spec/language/reflection.md),
  [named conformances](../../wiki/spec/language/conformances.md),
  [generic applications](../../wiki/spec/language/generics.md),
  [termination and progress](../../wiki/spec/language/termination.md),
  [process exit and abandonment](../../wiki/spec/language/process_exit.md),
  [domains and qualification](../../wiki/spec/language/domains.md),
  [value-dependent facts and views](../../wiki/spec/language/dependent_values.md),
  [service reach and operational ceilings](../../wiki/spec/language/effects.md),
  [inline assembly](../../wiki/spec/language/assembly.md),
  [interpreted instructions and environments](../../wiki/spec/language/assembly.md#interpreted-execution),
  [semantic evaluation](../../wiki/spec/language/evaluation.md),
  [expressions and operators](../../wiki/spec/language/expressions.md),
  [constants](../../wiki/spec/language/constants.md), [numeric values and bounds](../../wiki/spec/language/numeric_values.md),
  and [counts, indices and addresses](../../wiki/spec/language/counts_and_addresses.md).
- Representation policies: [layout plans](../../wiki/spec/layouts/plans.md),
  [size and alignment queries](../../wiki/spec/layouts/plans.md#size-and-alignment-queries),
  [native code regions](../../wiki/spec/language/counts_and_addresses.md#native-code-layout-queries),
  [recasts](../../wiki/spec/layouts/recasts.md), and [codecs](../../wiki/spec/layouts/codecs.md).
- Mathematical proofs: [foundation and checking judgments](../../wiki/spec/proofs/foundation.md),
  [mathematical bindings and logical hypotheses](../../wiki/spec/proofs/mathematical_bindings.md),
  [W-based induction and indexed encoding](../../wiki/spec/proofs/inductive_profile.md),
  [contracts, bundles, normalization and trust](../../wiki/spec/proofs/contracts.md),
  [stable index identity and proved compatibility](../../wiki/spec/proofs/contracts.md#licensed-normalization),
  and [relations and quotients](../../wiki/spec/proofs/quotients.md).
- Runtime data/code interfaces: [machine values](../../wiki/spec/language/machine_values.md)
  and [some packages](../../wiki/spec/language/some_packages.md).
- Psi: [product](../../wiki/spec/terminal-psi/product.md),
  [calls and outcomes](../../wiki/spec/terminal-psi/calls_and_outcomes.md),
  [control flow and ranking](../../wiki/spec/terminal-psi/control_flow.md),
  [boundary calls](../../wiki/spec/terminal-psi/boundary_calls.md),
  [byte views](../../wiki/spec/terminal-psi/byte_views.md),
  [structural access and stores](../../wiki/spec/terminal-psi/structural_access.md),
  [structural predicates](../../wiki/spec/terminal-psi/structural_predicates.md),
  [loan resources and compatibility](../../wiki/spec/terminal-psi/loans.md),
  [structural claims and cleanup](../../wiki/spec/terminal-psi/ownership.md),
  [runtime callable dispatch](../../wiki/spec/terminal-psi/dynamic_dispatch.md),
  [observations](../../wiki/spec/terminal-psi/observations.md),
  [encoding](../../source-rust/subsystems/compiler/psi/artifact/wire/encoding.md),
  [fact validity](../../wiki/spec/terminal-psi/fact_validity.md),
  [mathematical proof values](../../wiki/spec/terminal-psi/mathematical_values.md), and
  [integer certificates](../../wiki/spec/terminal-psi/integer_certificates.md).
- Resources: [logical work](../../wiki/spec/resources/logical_work.md) and
  [storage](../../wiki/spec/resources/storage.md),
  [external-entry stacks](../../wiki/spec/resources/entry_stacks.md),
  [content custody](../../wiki/spec/resources/content_custody.md),
  [dense owned runs](../../wiki/spec/resources/typed_runs.md),
  [authority and qualification](../../wiki/spec/resources/authority.md),
  [extents and mappings](../../wiki/spec/resources/extents.md),
  [access policies](../../wiki/spec/resources/access.md),
  [device custody and ordering](../../wiki/spec/resources/device_access.md),
  [resources across suspension](../../wiki/spec/resources/suspension.md),
  [runtime-sized activation storage](../../wiki/spec/resources/activation_storage.md),
  [allocation strategies](../../wiki/spec/resources/allocation.md),
  [bounded byte input](../../wiki/spec/resources/bounded_input.md), and
  [bounded growth](../../wiki/spec/resources/bounded_growth.md).
- Build: [command-line tools](../../wiki/spec/build/command_line.md),
  [declarations and package identity](../../wiki/spec/build/declarations.md),
  [dependency scopes, product references and isolated I/O](../../wiki/spec/build/scoped_execution.md),
  [configuration and targets](../../wiki/spec/build/configuration.md),
  [recorded proof-work policy](../../wiki/spec/build/configuration.md#proof-work-policy),
  [portable platform services](../../wiki/spec/build/platform_services.md),
  [requirement-based test groups](../../wiki/spec/build/testing.md),
  [selected-product behavior exclusions](../../wiki/spec/build/behavior_exclusions.md),
  [optimization selection and validation](../../wiki/spec/build/optimizations.md),
  [validation subjects](../../wiki/spec/build/validation_subjects.md),
  [target slots and entry roots](../../wiki/spec/build/entry_roots.md),
  [UEFI entry and handoff](../../wiki/spec/build/uefi_entry.md),
  [installed external roots](../../wiki/spec/build/external_roots.md),
  [boundary calling plans](../../wiki/spec/build/calling_plans.md),
  [boundary signature shapes](../../wiki/spec/build/boundary_shapes.md),
  [foreign binding values](../../wiki/spec/build/foreign_bindings.md),
  [foreign storage and lifetime](../../wiki/spec/build/foreign_storage.md),
  [machine-state evidence](../../wiki/spec/build/machine_state_evidence.md),
  [provider selection](../../wiki/spec/build/provider_selection.md),
  [root-assumed bindings (exceptional use only)](../../wiki/spec/build/assumed_binding_contracts.md),
  [embedding and interpreted components](../../wiki/spec/build/embedding.md),
  [task activation and lifecycle](../../wiki/spec/build/task_runtime.md),
  [service permissions](../../wiki/spec/build/permissions.md),
  [opaque representations](../../wiki/spec/build/opaque_representations.md),
  [execution and generated source](../../wiki/spec/build/execution.md),
  [observations and reproducibility](../../wiki/spec/build/observations.md),
  [compiler storage](../../wiki/spec/build/compiler_storage.md),
  [standalone compiler request](../../wiki/spec/build/compiler_request.md),
  [native products and component publication](../../wiki/spec/build/component_publication.md),
  [macOS applications](../../wiki/spec/build/macos_application.md),
  [private callbacks](../../wiki/spec/build/private_callbacks.md),
  [interrupt obligations](../../wiki/spec/build/interrupt_obligations.md),
  [hardware materialization](../../wiki/spec/build/hardware_materialization.md), and
  [executable installation](../../wiki/spec/build/executable_installation.md).
- Packages: [source selection](../../wiki/spec/packages/sources.md),
  [checked boundary topology](../../wiki/spec/packages/topology.md),
  [project locks](../../wiki/spec/packages/locks.md),
  [declaration and carried-type boundaries](../../wiki/spec/packages/boundaries.md),
  [core and ordinary libraries](../../wiki/spec/packages/toolchain.md),
  [compiler-derived review](../../wiki/spec/packages/review.md),
  [acceptance](../../wiki/spec/packages/acceptance.md), and the
  [workflow guide](language_guide/packages.md).

Bootstrap contracts live beside the chain; start at
[edge contracts](../bootstrap/CONTRACT.md) and
[minimization](../bootstrap/MINIMIZATION.md). Alternative-chain tradeoffs are
[reference notes](drafts/reference/bootstrap_chain_alternatives.md), not a replacement proposal.

[Execution tasks](../TASKS.md) track implementation;
[owner questions](../OWNER_QUESTIONS.md) hold unresolved decisions.
