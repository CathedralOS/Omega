# Design Brief: Boundary Provider Plans

Current as of 2026-07-16. Provider definition, admission, and selection use
ordinary data, machines, traits, capabilities, and build data. The dedicated
`provides` declaration is retired from language canon; its implemented surface
is a compatibility shape awaiting the migration recorded in `TASKS.md`.

## Bottom line

A boundary trait states a service contract. A provider policy produces a
candidate plan describing how some artifact realizes that contract:

```omega
trait BoundaryProvider<Service> {
    machine plan(service: ServiceSchema<Service>) -> ProviderPlan<Service>;
}

data LinuxConsole;

machine LinuxConsole::plan(
    service: ServiceSchema<Console>
) -> ProviderPlan<Console>
    satisfies BoundaryProvider<Console>::plan
{
    ...
}
```

The exact library type names are provisional. The architecture is not: this is
the third policy family beside `Layout::plan(Schema) -> DataPlan` and
`CallingConvention::plan(Signature) -> CallPlan`. They share build-time
evaluation, deterministic normalization, identities, certificates, reporting,
and compiler-owned derivation. Their plan types, closed vocabularies,
validators, and consumers remain distinct.

`ProviderPlan` replaces the semantic work formerly hidden behind:

```omega
linux_x64 provides Console {
    write -> Syscall(1);
}
```

That spelling fused provider definition to target selection and accumulated
unrelated target constants. It is not retained as a foundational construct.
Sugar may be reconsidered only after real provider packages demonstrate a
repeated ergonomic need.

## What a provider plan contains

A normalized plan maps boundary-operation identities to compiler-known binding
descriptions. The closed binding vocabulary includes mechanisms such as:

- imported symbol;
- syscall/supervisor entry;
- vtable or service-table field;
- compiler intrinsic; and
- component/provider slot.

Each entry cites a normalized `CallPlan` where physical invocation is required
and carries typed dispatch context, applicability, and availability information.
The provider plan does not invent instructions, registers, or new binding
species. A new composition of existing primitives is policy work; a genuinely
new primitive requires a compiler release.

The plan's claims are not self-authenticating. In particular, structural
validation cannot prove that a foreign symbol or syscall implements the
boundary operation's semantic contract.

## Four-stage lifecycle

Provider handling has four distinct stages:

1. **Construct.** Any package may construct an inert candidate plan.
2. **Validate.** Deterministic validation proves structural facts: operation
   coverage, uniqueness, signature and calling-plan compatibility, typed
   dispatch context, availability well-formedness, and normalized identity.
3. **Admit.** An authorized boundary accepts the candidate's semantic claims
   and produces trust/grant receipts. A package can never self-grant merely by
   authoring a plan.
4. **Select.** The owner of a service slot chooses one admitted provider.

This is the general component-admission protocol applied to a provider-plan
payload, not a parallel trust system. A provider need not be a loadable
component: built-in intrinsics, syscall tables, firmware tables, brokers, test
doubles, and hardware realizations use the same lifecycle through different
artifact sources.

Construction grants neither authority nor reach. Admission records what was
trusted; selection spends slot-selection authority; ordinary capability values
still govern what resources a selected service may operate on.

## Selection and defaults

Provider definition is independent of selection. `build.omg` owns static root
selection because it constructs the root build graph. Component managers, test
harnesses, or holders of attenuated selection capabilities may own narrower or
dynamic slots.

The common case uses a platform profile:

```omega
machine build(b: &mut Build) {
    b.platform = LinuxHosted;
}
```

`LinuxHosted` contributes an ordinary package-authored default bundle. Authors
override only exceptional slots:

```omega
b.override_provider(Clock, DeterministicClock);
b.override_provider(Writable, SandboxedFilesystem);
```

Exact method spelling is library design, not grammar. Resolution order is:

1. an explicit slot override;
2. the selected profile's applicable default; and
3. otherwise a missing-provider error.

Two equally applicable defaults are an ambiguity error. Normalization expands
defaults into the complete selected mapping, and artifacts record provider-plan
identities, admission receipts, overrides, and final slot assignments. Target
selection may supply convenience defaults; it never secretly changes service
meaning.

## Package ownership

The intended package split is:

- `omega::core::provider`: plan vocabulary, policy traits, validation-facing
  identities, and admission/selection contracts;
- `omega::std`: standard boundary traits such as `Console`, `Filesystem`,
  `Clock`, and `Process`;
- platform packages: concrete providers, calling/layout/format policies, and
  default profiles such as `LinuxHosted`; and
- applications/build packages: profile choice and scoped overrides.

The compiler owns only irreducible lowering and checking. Syscall numbers,
library/symbol tables, platform structure layouts, standard provider choices,
and expressible ABI policies do not remain floating Rust tables.

## Foreign constants leave provider tables

The implemented `provides` rows also carry target numbers such as structure
offsets and open-flag bits. They are not provider bindings.

- Foreign structure offsets derive from declared schemas and `DataPlan`s.
- Foreign flag words use declared format/encoding policies mapping semantic
  options to the platform representation. A bitfield policy may use named bit
  placements, but the model also permits non-bit and composite encodings.

Hand-numbered representation constants therefore dissolve into the appropriate
layout or encoding policy. No generic target-facts declaration is introduced.

## Compiler and artifact laws

- Internal Omega-to-Omega calls remain compiler-sovereign and never select a
  boundary provider or user-authored calling convention.
- Published provider-plan identity is deterministic and normalizer-owned; the
  prover and admission authority gate legality without redefining identity.
- Provider validation checks structure. Admission owns semantic trust.
- Selection consumes slot-owner authority and may choose only admitted plans.
- Call lowering and inbound stubs consume checked `ProviderPlan` + `CallPlan`
  artifacts, never source strings or target-name switches.
- Provider substitution must refine the slot's pinned machine contract,
  including effects, authority, progress, failure, and calling-plan surface.
- Reports distinguish service reach, authority flow, provider selection, and
  trust receipts.

## Acceptance tests

1. A hosted profile supplies standard services without per-service ceremony,
   and the normalized report lists every resulting slot assignment.
2. A test overrides only `Clock`; all other profile defaults remain selected.
3. A missing or ambiguous provider fails before lowering.
4. A structurally valid fake syscall plan remains inert until admitted and can
   never self-grant a receipt.
5. A plan with a signature/calling-plan mismatch fails validation.
6. A component manager replaces one admitted provider without changing the
   abstract boundary-trait contract or unrelated slots.
7. A platform package on an existing ISA introduces a new ABI composition with
   no Rust change when it uses existing binding, placement, and format
   primitives.
8. No normalized artifact contains a dependency on the retired `provides`
   syntax or hand-numbered foreign layout constants.

## Engineering boundary

The architecture and retirement decision are settled. Engineering still owes
the exact source schemas for `ServiceSchema`, `ProviderPlan`, binding entries,
availability, receipts, and selection capabilities; profile/override library
ergonomics; static and dynamic admission adapters; and migration of the current
compiler tables and corpus. Those are implementation slices tracked in
`TASKS.md`, not reasons to preserve the keyword.
