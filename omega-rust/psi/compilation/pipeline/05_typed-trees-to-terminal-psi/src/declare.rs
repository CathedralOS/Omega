/*
Each machine of a compilation's module is given the declaration identity
Terminal Psi requires of a source-produced body, so the module can be the
program a product is built from: the program-entry receipt finds the entry by
it, and native admission compares it with the entry the Build bound.

`wiki/spec/terminal-psi/product.md`: "Source-produced machine bodies retain a
portable checked declaration identity: exact-owner callable and initial
state, contract commitment, and optional specialization template/commitment."
`wiki/spec/terminal-psi/encoding.md` gives the row: the exact-owner machine,
initial state and semantic declaration, the parameter types, the callable and
the result type as identity-table entries, the 32-byte contract commitment,
and the published suspension and blocking Booleans.

Every identity is the one the typed trees fix for the declaration's symbol,
copied into the module's identity table in the text form that table holds,
exactly as a record type's is (`RecordType::identity`). These identities are
never formed from a name.

The contract commitment is a digest of the contract the lowered form
publishes for the machine: its callable identity, each formal's complete declared
type and binding mode, its complete result type (including lifetime relations),
the `requires` and `ensures` propositions and crash routes of its
`MachineContract`, and what its header says about
termination. Consumers compare this commitment only under its domain and
encoding; a digest from another contract representation is not interchangeable.

A machine this cannot state is left without a declaration and listed in
`Elaboration::undeclared` with the reason. That changes no verdict: the
module is judged as before, and `AcceptedClosure::publish` refuses a module
that lists one, because use it would publish a source-produced body with no
declaration identity.
*/

#[path = "declare_selected.rs"]
mod selected;

use diagnostics::Diagnostic;
use std::hash::{Hash, Hasher};

use optimization::digest_encoder::DomainDigestEncoder;
use semantic_vocabulary::{IdTable, MachineId};
use symbols::SymbolHandle;
use terminal_psi::{
    CheckedMachineCapture, CheckedMachineDeclaration, ClosedReachApplication, ClosedReachCapture,
    ClosedReachParameter, NominalDeclarationIdentity, TerminalIdentityTableBuilder,
    TerminalMachine,
};
use typed_trees::{MachineInstantiation, TypedTrees};

use crate::{Elaboration, Signature};

const COMMITMENT_DOMAIN: &[u8] = b"omega.lowered-machine-public-contract.v2\0";

/// Feeds the contract's structured fields into its domain-separated digest.
struct Commitment(DomainDigestEncoder);

impl Hasher for Commitment {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.raw(bytes);
    }
}

pub(crate) type BinderReach = Vec<(SymbolHandle, Option<Vec<semantic_vocabulary::ServiceId>>)>;

/// Bindings for the generated body or its enclosing map. Retain them until
/// the exact source template forms supply the application contract.
pub(crate) struct PreparedApplication {
    template: SymbolHandle,
    enclosing_capture: bool,
    fixed: Option<Vec<semantic_vocabulary::ServiceId>>,
    parameter_kinds: Vec<u8>,
    telescope: Vec<PreparedParameter>,
}

enum PreparedParameter {
    Ready(ClosedReachParameter),
    Machine {
        selected: terminal_psi::TerminalIdentityIndex,
        upper_bound: Option<Vec<semantic_vocabulary::ServiceId>>,
        upper_bound_may_suspend: bool,
        upper_bound_may_block: bool,
    },
}

/// Callable headers retain exact source identities after source is released.
/// Prepare the source and service identities once, then stamp real bodies and
/// requirement descriptions without giving a requirement an executable body.
pub(crate) fn declare(
    typed: &TypedTrees,
    program: &crate::read::Program<'_>,
    outcomes: &semantic_vocabulary::IdTable<MachineId, crate::outcome::Outcome>,
    binder_reach: &semantic_vocabulary::IdTable<MachineId, BinderReach>,
    elaboration: &mut Elaboration,
) -> Result<Vec<(MachineId, PreparedApplication)>, &'static str> {
    let machines = &program.symbols;
    let signatures = &program.signatures;
    let requirement_sources = &program.declaration_sources;
    let requirement_signatures = &program.declaration_signatures;
    let sources =
        source::source_identity::toolchain_source_identities(typed.symbols.source_files());
    // A service is committed to by its portable identity, never by its
    // module-local number.
    let services: Vec<(semantic_vocabulary::ServiceId, String)> = elaboration
        .module
        .services
        .iter()
        .map(|service| {
            (
                service.id,
                elaboration
                    .module
                    .identities
                    .text(service.identity)
                    .unwrap_or_default()
                    .to_owned(),
            )
        })
        .collect();
    let identities_of = |row: &[semantic_vocabulary::ServiceId]| {
        row.iter()
            .map(|service| {
                services
                    .iter()
                    .find(|(id, _)| id == service)
                    .map(|(_, identity)| identity.as_str())
                    .filter(|identity| !identity.is_empty())
                    .ok_or("a published service has no identity")
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let mut identities = std::mem::take(&mut elaboration.module.identities).into_builder();
    // Anonymous invocation source has exact local declaration identities, not
    // portable toolchain ownership. Keep those declarations available to the
    // lowered authority judgment without exporting a fabricated source hash.
    let sources = match sources {
        Err(_) if identities.is_invocation_scoped() => Ok(Vec::new()),
        sources => sources,
    };
    let mut prepared = Vec::new();
    for machine in &mut elaboration.module.machines {
        // A requirement stands for a contract, not a source-produced body.
        if elaboration.requirements.contains(&machine.id) {
            continue;
        }
        let stated = match (
            &sources,
            machines.get(machine.id),
            signatures.get(machine.id),
        ) {
            (Ok(sources), Some(symbol), Some(signature)) => {
                identities_of(&machine.published_service_ceiling).and_then(|reach| {
                    let entries = identities_of(&machine.published_synchronous_entries)?;
                    let envelope = elaboration
                        .module
                        .callables
                        .machine_envelope(machine.id)
                        .ok_or("the machine has no completed operational envelope")?;
                    declaration(
                        typed,
                        sources,
                        *symbol,
                        signature,
                        machine,
                        (
                            Published {
                                reach: &reach,
                                entries: &entries,
                                operational: [envelope.suspends, envelope.blocks],
                            },
                            binder_reach
                                .get(machine.id)
                                .ok_or("a machine has no prepared binder reach row")?,
                        ),
                        &mut identities,
                    )
                })
            }
            (Err(_), _, _) => Err("a toolchain source has no exact identity"),
            _ => Err("the machine has no typed-tree declaration"),
        };
        match stated {
            Ok((declaration, application)) => {
                machine.declaration = Some(declaration);
                if let Some(mut application) = application {
                    application.fixed = outcomes
                        .get(machine.id)
                        .ok_or("a prepared application has no exact reach outcome")?
                        .fixed_service_reach
                        .clone();
                    prepared.push((machine.id, application));
                }
            }
            Err(reason) => elaboration.undeclared.push((machine.id, reason)),
        }
    }
    let result = (|| {
        if elaboration.module.declaration_contracts.is_empty() {
            return Ok(());
        }
        let sources = sources
            .as_ref()
            .map_err(|_| "a public requirement toolchain source has no exact identity")?;
        for contract in &mut elaboration.module.declaration_contracts {
            let original = *program
                .declaration_representatives
                .get(contract.id)
                .ok_or("public contract lost its source representative")?;
            let requirement = *requirement_sources
                .get(original)
                .ok_or("public contract lost its source declaration coordinate")?;
            let signature = requirement_signatures
                .get(original)
                .ok_or("public contract lost its exact source signature")?;
            let reach = identities_of(&contract.published_service_ceiling)?;
            let entries = identities_of(&contract.published_synchronous_entries)?;
            let (_, declaration, _) = trait_requirement(
                typed,
                sources,
                requirement,
                signature,
                Published {
                    reach: &reach,
                    entries: &entries,
                    operational: signature.operational.published([false; 2]),
                },
                &mut identities,
            )?;
            contract.declaration = Some(declaration);
        }
        let links = selected::publish(
            typed,
            program,
            sources,
            &mut elaboration.module,
            &mut identities,
        )?;
        for (required, application, mut dispatch, result_lifetimes) in links {
            let application = elaboration
                .module
                .closed_conformance_applications
                .get(application)
                .ok_or("selected public contract lost its emitted application")?;
            dispatch.conformance_application_report_fingerprint = application.report_fingerprint;
            dispatch.conformance_application_commitment = application.commitment;
            let contract = elaboration
                .module
                .declaration_contracts
                .iter_mut()
                .find(|contract| contract.id == required)
                .ok_or("selected application lost its exact public contract")?;
            if contract
                .selected_application
                .as_ref()
                .is_some_and(|known| known != &dispatch)
            {
                return Err("one public contract is associated with different closed selections");
            }
            if contract.selected_application.is_some()
                && contract.result_lifetimes != result_lifetimes
            {
                return Err("one public contract is published with different lifetime accounts");
            }
            contract.selected_application = Some(dispatch);
            contract.result_lifetimes = result_lifetimes;
        }
        elaboration
            .module
            .closed_conformance_applications
            .sort_unstable_by(|left, right| {
                (
                    left.owner,
                    identities.text(left.declaration_identity),
                    left.report_fingerprint,
                )
                    .cmp(&(
                        right.owner,
                        identities.text(right.declaration_identity),
                        right.report_fingerprint,
                    ))
            });
        elaboration.module.closed_conformance_applications.dedup();
        Ok(())
    })();
    elaboration.module.identities = identities.build();
    result.map(|()| prepared)
}

/// Prepares each exact supplied requirement's candidate intent. The complete
/// compilation still owes its body/refinement witnesses; receiving catalog
/// formation publishes these rows after those judgments. An identical retained
/// static-provider question owns its row and its independent proof instead.
///
/// A provider plan selects its checked adapters among these rows by the
/// provider's declaration identity and each requirement's identity
/// (`backend_plan::TerminalTargetProviderSelection::from_plans`), so a row
/// carries the identities this module already states: the boundary's
/// callable identity, the machine's callable identity and package, the
/// provider's exact-owner identity, and its path for reports. The machine is
/// the one `supplied_slots` reads as a root and retains a refinement witness
/// for. Final publication still requires that complete judgment. The rows state
/// no receiving application: the route declares only boundaries of traits
/// with no parameters (`read/calls.rs`, `boundary`).
pub(crate) fn provider_candidates(
    typed: &TypedTrees,
    machines: &IdTable<MachineId, SymbolHandle>,
    boundaries: &IdTable<semantic_vocabulary::BoundaryMachineId, SymbolHandle>,
    supplied: &[crate::SuppliedRequirement],
    static_conformances: &[terminal_check_tables::contracts::StaticConformanceQuestion],
    module: &mut terminal_psi::TerminalModule,
) -> Result<Vec<terminal_psi::ProviderCandidateConformance>, &'static str> {
    if supplied.is_empty() {
        return Ok(Vec::new());
    }
    let sources =
        source::source_identity::toolchain_source_identities(typed.symbols.source_files())
            .map_err(|_| "a provider toolchain source has no exact identity")?;
    let source_key = |provider: SymbolHandle, requirement: SymbolHandle| {
        (
            provider.arena_index(),
            provider.generation(),
            requirement.arena_index(),
            requirement.generation(),
        )
    };
    let mut static_providers = Vec::new();
    for (position, question) in static_conformances.iter().enumerate() {
        if question.provider.is_none() {
            continue;
        }
        if !typed
            .symbols
            .symbols()
            .nodes()
            .is_valid(question.declaration)
            || !typed
                .symbols
                .symbols()
                .nodes()
                .is_valid(question.requirement)
        {
            return Err("a static provider question has a stale source declaration");
        }
        static_providers.push((
            source_key(
                typed.implementation_value_head(question.declaration),
                question.requirement,
            ),
            position,
        ));
    }
    static_providers.sort_unstable_by_key(|(key, _)| *key);
    let mut identities = std::mem::take(&mut module.identities).into_builder();
    let rows = (|| {
        let mut rows = Vec::with_capacity(supplied.len());
        for row in supplied {
            let boundary = boundaries
                .iter()
                .find_map(|(boundary, symbol)| (*symbol == row.requirement).then_some(boundary))
                .ok_or("a supplied requirement has no boundary declaration")?;
            let requirement = module
                .boundary_machine(boundary)
                .ok_or("a supplied requirement's boundary was not published")?
                .identity;
            let machine = module
                .machine(row.machine)
                .ok_or("a provider's machine is not in the module")?;
            let callable = machine
                .declaration
                .as_ref()
                .ok_or("a provider's machine has no declaration identity")?
                .callable;
            let symbol = *machines
                .get(row.machine)
                .ok_or("a provider's machine has no typed-tree declaration")?;
            let path = typed.symbols.display_path(row.provider, "::");
            if path.is_empty() {
                return Err("a provider has no declaration path");
            }
            let provider_declaration = NominalDeclarationIdentity::new(nominal(
                typed,
                &sources,
                row.provider,
                &mut identities,
            )?);
            let key = source_key(
                typed.implementation_value_head(row.provider),
                row.requirement,
            );
            let begin = static_providers.partition_point(|(candidate, _)| *candidate < key);
            let end = begin
                + static_providers[begin..].partition_point(|(candidate, _)| *candidate == key);
            if begin != end {
                if end - begin != 1 {
                    return Err("a supplied candidate has competing static provider questions");
                }
                let question = &static_conformances[static_providers[begin].1];
                let stated = question
                    .provider
                    .as_ref()
                    .ok_or("a static provider question lost its prepared candidate")?;
                if stated.boundary != boundary
                    || stated.declaration != provider_declaration
                    || !matches!(
                        question.implementation,
                        terminal_check_tables::contracts::StaticImplementation::Machine(id)
                            if id == row.machine
                    )
                {
                    return Err("a static provider question changes its supplied candidate");
                }
                // The complete question retains this exact row and proves its
                // required header before the one final catalog publication.
                continue;
            }
            rows.push(terminal_psi::ProviderCandidateConformance {
                boundary,
                requirement_identity: requirement,
                source_requirement_identity: requirement,
                declaration_application: terminal_psi::TerminalIdentityIndex::ABSENT,
                receiving_application: terminal_psi::TerminalIdentityIndex::ABSENT,
                receiving_region_arguments: Vec::new(),
                provider_identity: identities.intern(&path),
                provider_declaration,
                candidate_identity: callable,
                source_candidate_identity: callable,
                source_candidate_package_identity: typed.symbols.symbol_package_identity(symbol),
                candidate: row.machine,
                signature: terminal_psi::ProviderSignature {
                    parameters: machine
                        .structural_parameters
                        .iter()
                        .map(|parameter| terminal_psi::ProviderSignatureParameter {
                            position: parameter.position,
                            structural_type: parameter.structural_type,
                            multiplicity: parameter.multiplicity,
                            access: parameter.access,
                            qualifications: parameter.qualifications.clone(),
                            projected_qualifications: parameter.projected_qualifications.clone(),
                        })
                        .collect(),
                },
                refinement: terminal_psi::ProviderRefinement {
                    realized_service_ceiling: machine.published_service_ceiling.clone(),
                    ..terminal_psi::ProviderRefinement::empty()
                },
            });
        }
        Ok(rows)
    })();
    module.identities = identities.build();
    let mut rows = rows?;
    let text = |identity: terminal_psi::TerminalIdentityIndex| {
        module
            .identities
            .text(identity)
            .unwrap_or_default()
            .to_owned()
    };
    rows.sort_by_cached_key(|row| {
        (
            row.boundary,
            text(row.provider_identity),
            text(row.provider_declaration.identity()),
            text(row.candidate_identity),
            row.candidate,
        )
    });
    Ok(rows)
}

fn declaration(
    typed: &TypedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    symbol: SymbolHandle,
    signature: &Signature,
    machine: &TerminalMachine,
    (published, binder_reach): (Published<'_>, &BinderReach),
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<(CheckedMachineDeclaration, Option<PreparedApplication>), &'static str> {
    let declared = typed
        .machine_by_symbol(symbol)
        .ok_or("the machine has no typed-tree declaration")?;
    let [state] = typed
        .machine_states
        .span(declared.states)
        .ok_or("the machine's state span does not resolve")?
    else {
        return Err("the machine does not have exactly one state");
    };
    let formals = typed
        .state_parameters
        .span(state.parameters)
        .ok_or("the machine's parameter span does not resolve")?;
    let callable = typed
        .machine_callable_identity_text(declared)
        .filter(|callable| !callable.is_empty())
        .ok_or("the machine has no callable identity")?;
    let semantic = semantic_identity(typed, symbol, identities)?;
    // A descriptive type identity erases reference lifetimes. Reuse the
    // complete signature normalization used by public requirement schemas so
    // these existing declaration fields retain the authored promise as well.
    let (parameter_types, result_type) = typed
        .machine_definition_types(declared)
        .ok_or("the machine has no complete declared signature identity")?;
    // A machine stage 04 generated for a static application also names the
    // generic machine it was generated from and commits to the application.
    let (specialization, prepared) = match typed.executable_application_template(symbol)? {
        Some(template) => {
            let (specialization, prepared) =
                application(typed, sources, symbol, template, binder_reach, identities)?;
            (Some(specialization), prepared)
        }
        None => (None, None),
    };
    let machine_identity = nominal(typed, sources, symbol, identities)?;
    let state_identity = nominal(typed, sources, state.symbol, identities)?;

    let contract = declaration_contract_commitment(
        &callable,
        formals,
        &parameter_types,
        &result_type,
        &machine.contract.requires,
        machine
            .contract
            .ensures
            .iter()
            .map(|clause| &clause.proposition),
        &machine.contract.crash_routes,
        signature,
        published,
    );

    Ok((
        CheckedMachineDeclaration {
            machine: NominalDeclarationIdentity::new(machine_identity),
            state: NominalDeclarationIdentity::new(state_identity),
            semantic_declaration: semantic,
            parameter_types: parameter_types
                .iter()
                .map(|text| identities.intern(text))
                .collect(),
            callable: identities.intern(&callable),
            result_type: identities.intern(&result_type),
            contract,
            published_may_suspend: machine.published_may_suspend,
            published_may_block: machine.published_may_block,
            specialization,
        },
        prepared,
    ))
}

/// What the declaration of `instance`, a machine stage 04 generated from the
/// generic machine `template`, states beside its own identity: the generic
/// machine's identities and a commitment to the application.
///
/// The commitment is this route's own, like the contract commitment: a
/// digest of the generic machine's callable identity and the identity of
/// each type and implementation the application binds, in parameter order.
/// Two machines generated from one generic machine at the same arguments
/// commit alike, and no other pair does. Type, implementation, callable
/// and scalar value arguments each retain their own identity.
fn application(
    typed: &TypedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    instance: SymbolHandle,
    template: SymbolHandle,
    binder_reach: &[(SymbolHandle, Option<Vec<semantic_vocabulary::ServiceId>>)],
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<
    (
        terminal_psi::CheckedMachineSpecialization,
        Option<PreparedApplication>,
    ),
    &'static str,
> {
    use typed_trees::data::TypeParameterKind;
    use typed_trees::static_applications::StaticMachineBindingValue;
    let declared = typed
        .machine_by_symbol(template)
        .ok_or("a generated machine's generic machine has no typed-tree declaration")?;
    let callable = typed
        .machine_callable_identity_text(declared)
        .filter(|callable| !callable.is_empty())
        .ok_or("a generated machine's generic machine has no callable identity")?;
    // A closed application retains the same identity scope as its actual
    // template and arguments. Anonymous invocation source does not acquire
    // a package owner merely because stage 04 generated a specialization.
    let semantic = semantic_identity(typed, template, identities)?;
    let row = typed
        .machine_instantiation(instance)?
        .filter(|row| row.template() == template)
        .ok_or("a generated machine does not have exactly one application")?;
    let binding_application = row.binding_application();
    if !typed
        .expression_table
        .static_machine_application_is_valid(binding_application)
    {
        return Err("a generated machine has a foreign binding application");
    }
    let bindings = typed
        .expression_table
        .static_machine_application_bindings(binding_application);
    if bindings.is_empty() {
        return Err("a generated machine's application binds nothing");
    }
    let arguments = typed
        .expression_table
        .static_machine_application_arguments(binding_application);
    if arguments.len() != bindings.len() {
        return Err(
            "a generated machine's application binds other than one argument per parameter",
        );
    }
    let (parameters, enclosing_capture) = match row {
        MachineInstantiation::Specialization(_) => (typed.machine_type_parameters(declared), false),
        MachineInstantiation::Capture(capture) => {
            let (owner, _) = typed
                .owned_conformance_realization(template)
                .filter(|(owner, _)| owner.symbol == capture.conformance_application.declaration)
                .ok_or("a captured application lost its exact enclosing map")?;
            if capture.normalized_template_identity != callable
                || binding_application != capture.conformance_application.callable_application
                || !declared.type_parameters.is_empty()
            {
                return Err("a captured application lost its exact source template");
            }
            (typed.conformance_type_parameters(owner), true)
        }
    };
    let mut prepared = (declared.conformance_bounds.is_empty()
        && parameters.iter().all(|parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Type | TypeParameterKind::Machine { .. }
            ) || parameter.kind.is_const_or_value()
        }))
    .then(|| PreparedApplication {
        template,
        enclosing_capture,
        fixed: None,
        parameter_kinds: Vec::with_capacity(bindings.len()),
        telescope: Vec::with_capacity(bindings.len()),
    });
    if prepared.is_some()
        && (parameters.len() != bindings.len()
            || !parameters.iter().zip(bindings).all(|(parameter, binding)| {
                parameter.symbol == binding.parameter
                    && match binding.value {
                        StaticMachineBindingValue::Type(_) => {
                            matches!(parameter.kind, TypeParameterKind::Type)
                        }
                        StaticMachineBindingValue::Const(_) => parameter.kind.is_const_or_value(),
                        StaticMachineBindingValue::Machine(_) => {
                            matches!(parameter.kind, TypeParameterKind::Machine { .. })
                        }
                        _ => false,
                    }
            }))
    {
        return Err("a checked specialization lost its complete template binder roster");
    }
    let commitment = application_bindings_commitment(
        typed,
        &callable,
        bindings,
        binder_reach,
        identities,
        &mut prepared,
    )?;
    Ok((
        terminal_psi::CheckedMachineSpecialization {
            capture: None,
            template_entry: NominalDeclarationIdentity::new(nominal(
                typed,
                sources,
                typed
                    .machine_states(declared)
                    .first()
                    .ok_or("a generated machine's template has no exact entry")?
                    .symbol,
                identities,
            )?),
            template: NominalDeclarationIdentity::new(nominal(
                typed, sources, template, identities,
            )?),
            template_callable: identities.intern(&callable),
            template_semantic_declaration: semantic,
            commitment,
        },
        prepared,
    ))
}

// Checked bodies and descriptions use the same complete argument calculation.
fn application_bindings_commitment(
    typed: &TypedTrees,
    callable: &str,
    bindings: &[typed_trees::static_applications::StaticMachineBinding],
    binder_reach: &[(SymbolHandle, Option<Vec<semantic_vocabulary::ServiceId>>)],
    identities: &mut TerminalIdentityTableBuilder,
    prepared: &mut Option<PreparedApplication>,
) -> Result<[u8; 32], &'static str> {
    use typed_trees::static_applications::StaticMachineBindingValue;
    let mut commitment =
        language_semantics::conformance_identity::MachineApplicationCommitment::new(callable)?;
    let mut binder_reach = binder_reach.iter();
    for binding in bindings {
        match &binding.value {
            StaticMachineBindingValue::Type(type_reference) => {
                let argument = type_argument(typed, *type_reference)?;
                commitment.type_argument(&argument)?;
                if let Some(prepared) = prepared.as_mut() {
                    prepared.parameter_kinds.push(0);
                    prepared
                        .telescope
                        .push(PreparedParameter::Ready(ClosedReachParameter::Type {
                            argument,
                        }));
                }
            }
            // conformances.md, "Resulting-value identity": a name "creates
            // no additional identity", so the implementation is stated by
            // the declaration that stands for its value, whichever name the
            // application wrote. The kind is hashed with it, so a type and
            // an implementation never commit alike.
            StaticMachineBindingValue::Conformance(argument) => {
                let named = argument.symbol;
                if !named.is_valid()
                    || !typed.symbols.symbols().nodes().is_valid(named)
                    || typed.symbols.get(named).kind != symbols::SymbolKind::Conformance
                {
                    return Err("an application's implementation argument names no implementation");
                }
                let identity =
                    semantic_identity(typed, typed.implementation_value_head(named), identities)?;
                commitment.conformance_argument(identities.text(identity).ok_or(
                    "an application's implementation identity is absent from its table",
                )?)?;
            }
            // generics.md, "Closed applications and canonical identity":
            // a value argument is part of the application's identity. It is
            // stated by the exact binder's type and the prepared value's
            // canonical typed encoding, so `7` given to a `u8` and to a `u64`
            // parameter, or a value and a type, never commit alike. A
            // Boolean is stated the same way, by `true` or `false`: its
            // parameter's type is `bool`, which no integer literal lands in.
            StaticMachineBindingValue::Const(value_reference) => {
                let argument = const_argument(typed, binding.parameter, *value_reference)?;
                commitment.const_argument(&argument)?;
                if let Some(prepared) = prepared.as_mut() {
                    prepared.parameter_kinds.push(1);
                    prepared.telescope.push(PreparedParameter::Ready(
                        ClosedReachParameter::Const { argument },
                    ));
                }
            }
            StaticMachineBindingValue::Machine(argument) => {
                if argument.application.is_some() {
                    return Err(
                        "an application's callable argument must select its declared machine",
                    );
                }
                let machine = typed
                    .machine_by_symbol(argument.symbol)
                    .or_else(|| {
                        typed
                            .unique_machine_by_entry_state_symbol(argument.symbol)
                            .map(|(machine, _)| machine)
                    })
                    .ok_or("an application's callable argument has no exact named machine")?;
                let identity = semantic_identity(typed, machine.symbol, identities)?;
                commitment.machine_argument(
                    identities
                        .text(identity)
                        .ok_or("an application's callable identity is absent from its table")?,
                )?;
                if let Some(prepared) = prepared.as_mut() {
                    let contract = typed
                        .retained_static_machine_contract(binding.parameter)
                        .ok_or("an application's callable binder lost its requirement")?;
                    let (_, upper_bound) = binder_reach
                        .next()
                        .filter(|(parameter, _)| *parameter == binding.parameter)
                        .ok_or("an application's callable binder lost its reach ceiling")?;
                    prepared.parameter_kinds.push(3);
                    prepared.telescope.push(PreparedParameter::Machine {
                        selected: identity,
                        upper_bound: upper_bound.clone(),
                        upper_bound_may_suspend: contract.signature().suspends,
                        upper_bound_may_block: contract.signature().blocks,
                    });
                }
            }
            StaticMachineBindingValue::RuntimeValue { .. } => {
                return Err("a generated machine's application binds a runtime value");
            }
        }
    }
    Ok(commitment.finish())
}

pub(crate) fn access_caller_route(
    typed: &TypedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    selected: &typed_trees::expression::StaticMachineArgument,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<terminal_psi::StructuralEstablishmentRoute, &'static str> {
    use terminal_psi::{CheckedMachineDeclaration, StructuralEstablishmentRoute};
    if let Some((owner, requirement)) = typed.trait_machine_signature(selected.symbol) {
        let callable = typed.trait_requirement_callable_identity_text(owner, requirement);
        if callable.is_empty() {
            return Err("access caller requirement lost its callable identity");
        }
        let specialization = typed
            .closed_requirement_application(requirement.symbol)
            .map(|closed| requirement_application(typed, sources, &closed, identities))
            .transpose()?
            .map(|(application, _)| application);
        let identity = CheckedMachineDeclaration::callable_evidence_identity(
            &callable,
            specialization
                .as_ref()
                .map(|application| &application.commitment),
        );
        let declaration = NominalDeclarationIdentity::new(nominal(
            typed,
            sources,
            requirement.symbol,
            identities,
        )?);
        let requirement_identity = identities.intern(&identity);
        return Ok(if owner.is_boundary {
            StructuralEstablishmentRoute::BoundaryRequirement {
                declaration,
                requirement_identity,
            }
        } else {
            StructuralEstablishmentRoute::CheckedRequirement {
                declaration,
                requirement_identity,
            }
        });
    }
    let machine = typed
        .machine_by_symbol(selected.symbol)
        .or_else(|| typed.machine_holding_state(selected.symbol))
        .ok_or("access caller has no retained machine declaration")?;
    let callable = typed
        .machine_callable_identity_text(machine)
        .filter(|value| !value.is_empty())
        .ok_or("access caller has no callable identity")?;
    let (declaration, callable) = if let Some(template) =
        typed.executable_application_template(machine.symbol)?
    {
        let (application, _) =
            application(typed, sources, machine.symbol, template, &[], identities)?;
        let template_callable = identities
            .text(application.template_callable)
            .ok_or("access caller lost its template callable identity")?;
        let identity = CheckedMachineDeclaration::callable_evidence_identity(
            template_callable,
            Some(&application.commitment),
        );
        (application.template, identity)
    } else {
        use typed_trees::data::TypeParameterKind;
        use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
        use typed_trees::types::TypeReferenceNode;
        let parameters = typed.machine_type_parameters(machine);
        if !typed
            .type_reference_table
            .contains_type_reference(selected.type_reference)
        {
            return Err("access caller lost its prepared declaration application");
        }
        let arguments = match typed
            .type_reference_table
            .type_reference(selected.type_reference)
        {
            TypeReferenceNode::Named { symbol, .. } if *symbol == machine.symbol => &[][..],
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if *base_symbol == machine.symbol => typed
                .type_reference_table
                .type_reference_handles(*arguments),
            _ => return Err("access caller lost its prepared declaration application"),
        };
        if arguments.len() != parameters.len() + machine.conformance_bounds.len() {
            return Err(
                "access caller needs its complete type, value and implementation arguments",
            );
        }
        let mut bindings = Vec::with_capacity(arguments.len());
        for (parameter, reference) in parameters.iter().zip(arguments) {
            let value = match parameter.kind {
                TypeParameterKind::Type => StaticMachineBindingValue::Type(*reference),
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                    StaticMachineBindingValue::Const(*reference)
                }
                _ => return Err("access caller has an unsupported static binder"),
            };
            bindings.push(StaticMachineBinding {
                parameter: parameter.symbol,
                value,
            });
        }
        for (bound, reference) in machine
            .conformance_bounds
            .iter()
            .zip(&arguments[parameters.len()..])
        {
            let argument = typed.static_operand_from_type_reference(*reference)?;
            bindings.push(StaticMachineBinding {
                parameter: bound.binder,
                value: StaticMachineBindingValue::Conformance(argument),
            });
        }
        let identity = if bindings.is_empty() {
            callable
        } else {
            let commitment = application_bindings_commitment(
                typed,
                &callable,
                &bindings,
                &[],
                identities,
                &mut None,
            )?;
            CheckedMachineDeclaration::callable_evidence_identity(&callable, Some(&commitment))
        };
        (
            NominalDeclarationIdentity::new(nominal(typed, sources, machine.symbol, identities)?),
            identity,
        )
    };
    Ok(StructuralEstablishmentRoute::ExactMachine {
        declaration,
        machine_identity: identities.intern(&callable),
    })
}

fn type_argument(
    typed: &TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Result<String, &'static str> {
    typed
        .type_identity_text(typed.package_qualified_type_identity(reference))
        .filter(|text| !text.is_empty())
        .ok_or("an application's type argument has no identity")
}

fn const_argument(
    typed: &TypedTrees,
    binder: SymbolHandle,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Result<language_semantics::const_value::CanonicalConstValue, &'static str> {
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    use typed_trees::data::TypeParameterKind;
    use typed_trees::expression::ExpressionNode;

    let parameter = typed
        .data_type_parameter_by_symbol(binder)
        .ok_or("an application's value binder has no declaration")?;
    let (TypeParameterKind::Const { type_reference } | TypeParameterKind::Value { type_reference }) =
        parameter.kind
    else {
        return Err("an application's value binding belongs to another binder kind");
    };
    let carrier = typed
        .type_identity_text(typed.package_qualified_type_identity(type_reference))
        .filter(|text| !text.is_empty())
        .ok_or("an application's value parameter type has no identity")?;
    let primitive = typed
        .primitive_type_reference(type_reference)
        .ok_or("an application's value binder needs its admitted scalar carrier")?;
    let value = typed.static_operand_from_type_reference(reference)?;
    let decoded = match typed.prepare_callable_scalar_argument(primitive, &value)? {
        ExpressionNode::Integer(literal) => DecodedCanonicalConstValue::Integer {
            integer_type: primitive
                .landed_landing_fixed()
                .ok_or("an application's integer binding has no admitted carrier")?,
            value: literal
                .value_bignum()
                .ok_or("an application's integer binding has no canonical value")?,
        },
        ExpressionNode::Boolean(value) => DecodedCanonicalConstValue::Boolean(value),
        _ => return Err("an application's value binding has no admitted scalar value"),
    };
    let mut value = CanonicalConstValue::from_decoded(&decoded, "")
        .ok_or("an application's value binding has no canonical encoding")?;
    // The outer type names the exact source binder. The encoding retains the
    // landed scalar type and value checked against that binder above.
    value.type_name = carrier;
    Ok(value)
}

/// Close a generated body against the template contract this compilation
/// judges. Type and const applications preserve the generated body's fixed
/// service and operational rows.
pub(crate) fn close_specializations(
    generic: &[terminal_check_tables::compilation::GenericForms],
    applications: Vec<(MachineId, PreparedApplication)>,
    closure: &mut Elaboration,
) -> Result<(), (MachineId, &'static str)> {
    if applications.is_empty() {
        return Ok(());
    }
    let bases: symbols::SymbolMap<_> = generic
        .iter()
        .map(|generic| (generic.template, template_commitment(generic)))
        .collect();
    let mut chosen = applications
        .iter()
        .flat_map(|(_, application)| {
            application
                .telescope
                .iter()
                .filter_map(|parameter| match parameter {
                    PreparedParameter::Machine { selected, .. } => Some(*selected),
                    _ => None,
                })
        })
        .collect::<Vec<_>>();
    chosen.sort_unstable();
    chosen.dedup();
    let mut selections = if chosen.is_empty() {
        Vec::new()
    } else {
        closure
            .module
            .machines
            .iter()
            .filter_map(|machine| {
                let declaration = machine.declaration.as_ref()?;
                chosen
                    .binary_search(&declaration.semantic_declaration)
                    .ok()?;
                Some((
                    declaration.semantic_declaration,
                    ReachSelection {
                        callee: machine.id,
                        contract: declaration.contract,
                        reach: machine.published_service_ceiling.clone(),
                        may_suspend: declaration.published_may_suspend,
                        may_block: declaration.published_may_block,
                    },
                ))
            })
            .collect::<Vec<_>>()
    };
    selections.sort_unstable_by_key(|(identity, _)| *identity);
    let service_identity = |id| {
        let services = &closure.module.services;
        let position = services
            .binary_search_by_key(&id, |service| service.id)
            .ok()?;
        closure.module.identities.text(services[position].identity)
    };
    let mut machines = closure.module.machines.iter_mut();
    for (owner, application) in applications {
        let result = (|| {
            // Both rosters were produced in machine order by declaration.
            let machine = machines
                .find(|machine| machine.id == owner)
                .ok_or("a prepared application has no exact emitted owner")?;
            let Some(template_base_commitment) = bases
                .get(&application.template)
                .copied()
                .ok_or("a checked specialization has no prepared template contract")??
            else {
                // The template's own failed check form reports its reason and
                // prevents the compilation from reaching publication.
                return Ok(());
            };
            let suspension_dependency = machine.published_may_suspend.into();
            let blocking_dependency = machine.published_may_block.into();
            let has_static_calls = application.fixed.is_some();
            let fixed = application
                .fixed
                .unwrap_or_else(|| machine.published_service_ceiling.clone());
            let specialization = machine
                .declaration
                .as_mut()
                .and_then(|declaration| declaration.specialization.as_mut())
                .ok_or("a prepared application has no checked declaration")?;
            let mut parameter_kinds = application.parameter_kinds;
            let mut telescope = application
                .telescope
                .into_iter()
                .map(|parameter| parameter.close(&selections))
                .collect::<Result<Vec<_>, _>>()?;
            let template_commitment = terminal_psi::closed_reach_template_commitment(
                template_base_commitment,
                &telescope,
                &suspension_dependency,
                &blocking_dependency,
                &[],
                service_identity,
            )?;
            let capture = application.enclosing_capture.then(|| {
                let template_identity = specialization.template_callable;
                // The map supplied all the bindings. The owned method has
                // no separate static arguments at this call.
                specialization.capture = Some(CheckedMachineCapture {
                    template_identity,
                    template_commitment,
                    specialization_commitment: specialization.commitment,
                });
                ClosedReachCapture {
                    template_identity,
                    template_base_commitment,
                    template_commitment,
                    suspension_dependency: suspension_dependency.clone(),
                    blocking_dependency: blocking_dependency.clone(),
                    conditional_carry: Vec::new(),
                    specialization_commitment: specialization.commitment,
                    parameter_kinds: std::mem::take(&mut parameter_kinds),
                    telescope: std::mem::take(&mut telescope),
                }
            });
            let template_identity = specialization.template_semantic_declaration;
            let specialization_commitment = specialization.commitment;
            let mut calls = Vec::new();
            for invocation in machine.module_invocations() {
                let Some(binder) = invocation.static_reach_binding else {
                    continue;
                };
                let Some(ClosedReachParameter::Machine(binding)) = telescope.get(binder as usize)
                else {
                    return Err("a static callback invocation lost its exact application binder");
                };
                if binding.callee != Some(invocation.inputs.callee) {
                    return Err("a static callback invocation changed its selected machine");
                }
                calls.push(terminal_psi::ClosedReachCall {
                    site: invocation.site,
                    binder,
                    application: None,
                });
            }
            calls.sort_unstable_by_key(|call| call.site);
            if has_static_calls == calls.is_empty() {
                return Err("a static callback reach dependency lost its invocation");
            }
            let mut dependencies = calls.iter().map(|call| call.binder).collect::<Vec<_>>();
            dependencies.sort_unstable();
            dependencies.dedup();
            let mut derived = fixed.clone();
            for position in &dependencies {
                let ClosedReachParameter::Machine(binding) = &telescope[*position as usize] else {
                    return Err("a static reach dependency is not a machine binder");
                };
                derived.extend(&binding.selected_reach);
            }
            derived.sort_unstable();
            derived.dedup();
            if derived != machine.published_service_ceiling {
                return Err(
                    "static callback reach differs from the checked machine's published reach",
                );
            }
            machine.closed_reach_application = Some(ClosedReachApplication {
                template_identity,
                template_base_commitment,
                template_commitment,
                suspension_dependency,
                blocking_dependency,
                conditional_carry: Vec::new(),
                specialization_commitment,
                parameter_kinds,
                telescope,
                capture,
                fixed,
                dependencies,
                calls,
            });
            Ok(())
        })();
        result.map_err(|reason| (owner, reason))?;
    }
    Ok(())
}

struct ReachSelection {
    callee: MachineId,
    contract: [u8; 32],
    reach: Vec<semantic_vocabulary::ServiceId>,
    may_suspend: bool,
    may_block: bool,
}

impl PreparedParameter {
    fn close(
        self,
        selections: &[(terminal_psi::TerminalIdentityIndex, ReachSelection)],
    ) -> Result<ClosedReachParameter, &'static str> {
        let (selected, upper_bound, upper_bound_may_suspend, upper_bound_may_block) = match self {
            Self::Ready(parameter) => return Ok(parameter),
            Self::Machine {
                selected,
                upper_bound,
                upper_bound_may_suspend,
                upper_bound_may_block,
            } => (
                selected,
                upper_bound,
                upper_bound_may_suspend,
                upper_bound_may_block,
            ),
        };
        let first = selections.partition_point(|(identity, _)| *identity < selected);
        let Some((_, callee)) = selections
            .get(first)
            .filter(|(identity, _)| *identity == selected)
        else {
            return Err("a selected static callback has no admitted executable declaration");
        };
        if selections
            .get(first + 1)
            .is_some_and(|(identity, _)| *identity == selected)
        {
            return Err("a selected static callback has multiple admitted executable declarations");
        }
        if upper_bound.as_ref().is_some_and(|upper_bound| {
            callee
                .reach
                .iter()
                .any(|service| upper_bound.binary_search(service).is_err())
        }) {
            return Err("a selected static callback exceeds its authored reach bound");
        }
        if (callee.may_suspend && !upper_bound_may_suspend)
            || (callee.may_block && !upper_bound_may_block)
        {
            return Err("a selected static callback exceeds its operational bound");
        }
        Ok(ClosedReachParameter::Machine(
            terminal_psi::ClosedReachMachineBinding {
                nominal_requirement: None,
                requirement_parameter_kinds: Vec::new(),
                requirement_arguments: Vec::new(),
                upper_bound,
                upper_bound_may_suspend,
                upper_bound_may_block,
                selected_may_suspend: callee.may_suspend,
                selected_may_block: callee.may_block,
                selected_identity: selected,
                selected_contract_commitment: callee.contract,
                selected_reach: callee.reach.clone(),
                callee: Some(callee.callee),
                schema: None,
            },
        ))
    }
}

/// Rigid and finite check forms share the same template owner. Commit the
/// complete ordered contract roster so no representative can hide a form.
fn template_commitment(
    generic: &terminal_check_tables::compilation::GenericForms,
) -> Result<Option<[u8; 32]>, &'static str> {
    use terminal_psi::TargetMachineCallable;

    let Ok(forms) = &generic.forms else {
        return Ok(None);
    };
    if forms.is_empty() {
        return Err("a specialization's template has no check form");
    }
    let mut digest =
        DomainDigestEncoder::new(b"omega.lowered-machine-template-contract-forms.v1\0");
    digest.len(forms.len());
    for form in forms {
        let Some(TargetMachineCallable::Machine(entry)) = form.module.entry else {
            return Err("a specialization's template check form has no machine entry");
        };
        if form.machines.get(entry) != Some(&generic.template) {
            return Err("a specialization's template check form belongs to another declaration");
        }
        let declaration = form
            .module
            .machine(entry)
            .and_then(|machine| machine.declaration.as_ref())
            .ok_or("a specialization's template check form has no exact contract")?;
        digest.raw(&declaration.contract);
    }
    Ok(Some(digest.finish()))
}

pub(crate) fn service_declaration_scope(
    typed: &TypedTrees,
    owner: SymbolHandle,
) -> Result<source::DependencyScope, &'static str> {
    if typed.symbols.source_files().next().is_none() {
        // Without source files there are no scoped source instances to choose.
        return Ok(source::DependencyScope::default());
    }
    let span = typed
        .symbols
        .symbol_provenance_source_span(owner)
        .ok_or("boundary declaration lost its retained source provenance")?;
    let file = typed
        .symbols
        .source_file(span)
        .ok_or("boundary declaration provenance has no retained source file")?;
    Ok(file.dependency_scope)
}

pub(crate) fn service_declaration_commitment(
    typed: &TypedTrees,
    owner: &typed_trees::trait_definition::TraitDefinition,
) -> Result<[u8; 32], Diagnostic> {
    // The declaration's actual source instance selects its checking context.
    // This describes the complete service; it grants no execution permission.
    let scope = service_declaration_scope(typed, owner.symbol).map_err(Diagnostic::error)?;
    Ok(typed_trees::service_declarations::projection::from_typed(
        typed_trees::provider_declarations::ProviderPlanningContext::new(typed, scope),
        owner,
    )?
    .declaration
    .declaration_commitment())
}

/// A bodyless trait method retains its actual nominal owner and signature.
/// There is no executable state to invent: the owning trait and requirement
/// occupy the existing owner/entry coordinates of the declaration record.
/// Provider checking and host installation still establish execution supply.
pub(crate) fn boundary(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    execution: terminal_psi::CheckedBoundaryExecution,
    signature: &Signature,
    published: Published<'_>,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<
    (
        NominalDeclarationIdentity,
        terminal_psi::CheckedBoundaryCallable,
    ),
    Diagnostic,
> {
    if execution == terminal_psi::CheckedBoundaryExecution::NumericPrimitive {
        if typed.symbols.sealed_intrinsic(symbol)
            != Some(language_core::sealed_intrinsics::SealedIntrinsic::FloatOperationRequirement)
        {
            return Err(Diagnostic::error(
                "numeric primitive publisher lost its exact catalog custody",
            ));
        }
        return top_level_boundary(
            typed, symbol, execution, signature, published, None, identities,
        );
    }
    if matches!(
        execution,
        terminal_psi::CheckedBoundaryExecution::HostedPrimitive { .. }
    ) {
        return primitive_boundary(typed, symbol, execution, signature, published, identities);
    }
    let (owner, requirement) = typed
        .trait_machine_signature(symbol)
        .filter(|(owner, requirement)| owner.is_boundary && requirement.symbol == symbol)
        .ok_or_else(|| {
            Diagnostic::error("boundary declaration has no exact owning trait and requirement")
        })?;
    if !owner.type_parameters.is_empty()
        || !owner.lifetime_parameters.is_empty()
        || !requirement.type_parameters.is_empty()
        || !requirement.lifetime_parameters.is_empty()
    {
        return Err(Diagnostic::error(
            "boundary declaration needs its complete applied telescope",
        ));
    }
    if signature.declared_crash_routes().is_none() {
        return Err(Diagnostic::error(
            "boundary declaration has no published crash ceiling",
        ));
    }
    let sources =
        source::source_identity::toolchain_source_identities(typed.symbols.source_files())
            .map_err(|_| Diagnostic::error("a boundary toolchain source has no exact identity"))?;
    let service_declaration = service_declaration_commitment(typed, owner)?;
    let (owner, source, method_arguments) = trait_requirement(
        typed,
        &sources,
        requirement,
        signature,
        published,
        identities,
    )
    .map_err(Diagnostic::error)?;
    Ok((
        owner,
        terminal_psi::CheckedBoundaryCallable {
            service_declaration,
            source,
            execution: terminal_psi::CheckedBoundaryExecution::Provider,
            method_arguments,
        },
    ))
}

// A primitive has a source declaration and a contract, but no source body.
// State its actual machine identities without constructing a fictitious machine.
fn primitive_boundary(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    execution: terminal_psi::CheckedBoundaryExecution,
    signature: &Signature,
    published: Published<'_>,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<
    (
        NominalDeclarationIdentity,
        terminal_psi::CheckedBoundaryCallable,
    ),
    Diagnostic,
> {
    let machine = typed
        .machine_by_symbol(symbol)
        .ok_or_else(|| Diagnostic::error("primitive has no machine declaration"))?;
    let terminal_psi::CheckedBoundaryExecution::HostedPrimitive { operation, .. } = execution
    else {
        return Err(Diagnostic::error(
            "primitive publisher has no exact hosted operation",
        ));
    };
    if typed.symbols.sealed_intrinsic(symbol)
        != Some(language_core::sealed_intrinsics::SealedIntrinsic::HostedPrimitive(operation))
    {
        return Err(Diagnostic::error(
            "primitive publisher lost its exact catalog custody",
        ));
    }
    let mut requirements = typed
        .machine_trait_conformances(machine)
        .iter()
        .map(|edge| typed_trees::machine::resolve_satisfied_declaration(typed, machine, edge));
    let requirement = requirements
        .next()
        .flatten()
        .ok_or_else(|| Diagnostic::error("primitive has no exact satisfied requirement"))?;
    if requirements.next().is_some() {
        return Err(Diagnostic::error(
            "primitive has multiple satisfied requirements",
        ));
    }
    let (service, declared) = typed
        .trait_machine_signature(requirement.symbol())
        .filter(|(owner, required)| owner.is_boundary && required.symbol == requirement.symbol())
        .ok_or_else(|| Diagnostic::error("primitive requirement has no exact declaring service"))?;
    if typed
        .symbols
        .sealed_member_intrinsic(service.symbol, declared.name.as_str())
        != Some(language_core::sealed_intrinsics::SealedIntrinsic::HostedRequirement(operation))
    {
        return Err(Diagnostic::error(
            "primitive operation differs from its declared service requirement",
        ));
    }
    let service_declaration = service_declaration_commitment(typed, service)?;
    top_level_boundary(
        typed,
        symbol,
        execution,
        signature,
        published,
        Some(service_declaration),
        identities,
    )
}

/// Publish source callable metadata for hosted and numeric primitives here.
fn top_level_boundary(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    execution: terminal_psi::CheckedBoundaryExecution,
    signature: &Signature,
    published: Published<'_>,
    service_declaration: Option<[u8; 32]>,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<
    (
        NominalDeclarationIdentity,
        terminal_psi::CheckedBoundaryCallable,
    ),
    Diagnostic,
> {
    // Named operators have their own declaration, not an executable entry.
    // Read that actual source form before sharing the metadata publication.
    let (entry, formals, return_type, callable, operational) = if let Some(machine) =
        typed.machine_by_symbol(symbol)
    {
        let [state] = typed.machine_states(machine) else {
            return Err(Diagnostic::error("primitive has no exact entry state"));
        };
        let callable = typed
            .machine_callable_identity_text(machine)
            .filter(|text| !text.is_empty())
            .ok_or_else(|| Diagnostic::error("primitive has no callable identity"))?;
        (
            state.symbol,
            typed.state_parameters(state),
            state.return_type,
            callable,
            [machine.suspends, machine.blocks],
        )
    } else {
        if execution != terminal_psi::CheckedBoundaryExecution::NumericPrimitive {
            return Err(Diagnostic::error("primitive has no machine declaration"));
        }
        let requirement = typed_trees::declarations::intrinsic_requirements::IntrinsicRequirement::by_symbol(typed, symbol)
                .filter(|requirement| requirement.kind == typed_trees::declarations::intrinsic_requirements::IntrinsicRequirementKind::Operator)
                .ok_or_else(|| Diagnostic::error("primitive has no exact operator declaration"))?;
        (
            requirement.symbol,
            requirement.parameters,
            requirement.return_type,
            requirement.requirement_identity,
            [false; 2],
        )
    };
    let sources =
        source::source_identity::toolchain_source_identities(typed.symbols.source_files())
            .map_err(|_| Diagnostic::error("primitive source has no exact identity"))?;
    let owner = NominalDeclarationIdentity::new(
        nominal(typed, &sources, symbol, identities).map_err(Diagnostic::error)?,
    );
    let type_text = |reference| {
        typed
            .type_identity_text(typed.package_qualified_type_identity(reference))
            .filter(|text| !text.is_empty())
            .ok_or_else(|| Diagnostic::error("primitive signature type has no identity"))
    };
    let parameter_types = formals
        .iter()
        .map(|formal| type_text(formal.type_reference))
        .collect::<Result<Vec<_>, _>>()?;
    let result_type = type_text(return_type)?;
    let contract = declaration_contract_commitment(
        &callable,
        formals,
        &parameter_types,
        &result_type,
        &signature.requires,
        signature.ensures.iter(),
        signature
            .declared_crash_routes()
            .ok_or_else(|| Diagnostic::error("primitive has no crash ceiling"))?,
        signature,
        published,
    );
    let source = CheckedMachineDeclaration {
        machine: owner.clone(),
        state: NominalDeclarationIdentity::new(
            nominal(typed, &sources, entry, identities).map_err(Diagnostic::error)?,
        ),
        semantic_declaration: semantic_identity(typed, symbol, identities)
            .map_err(Diagnostic::error)?,
        parameter_types: parameter_types
            .iter()
            .map(|text| identities.intern(text))
            .collect(),
        callable: identities.intern(&callable),
        result_type: identities.intern(&result_type),
        contract,
        published_may_suspend: operational[0],
        published_may_block: operational[1],
        specialization: None,
    };
    Ok((
        owner,
        terminal_psi::CheckedBoundaryCallable {
            service_declaration: service_declaration.unwrap_or(contract),
            source,
            execution,
            method_arguments: Vec::new(),
        },
    ))
}

// A public requirement names its trait and declared member. Its source
// identity must not be borrowed from the body that happens to implement it.
pub(crate) fn trait_requirement(
    typed: &TypedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    requirement: &typed_trees::signature::StateSignature,
    signature: &Signature,
    published: Published<'_>,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<
    (
        NominalDeclarationIdentity,
        CheckedMachineDeclaration,
        Vec<terminal_psi::ClosedReachArgument>,
    ),
    &'static str,
> {
    let (owner, declared) = typed
        .trait_machine_signature(requirement.symbol)
        .filter(|(_, declared)| declared.symbol == requirement.symbol)
        .ok_or("requirement declaration has no exact owning trait and requirement")?;
    // Invocation lifetimes remain binders of this public callable, not
    // static arguments an application must close. Its callable and type
    // identities retain their relations; type/value arguments still need
    // the existing complete application before declaration publication.
    if !requirement.type_parameters.is_empty() {
        return Err("requirement declaration needs its complete applied arguments");
    }
    let owner_identity = nominal(typed, sources, owner.symbol, identities)?;
    let requirement_identity = nominal(typed, sources, declared.symbol, identities)?;
    let semantic = semantic_identity(typed, requirement.symbol, identities)?;
    let formals = typed
        .state_parameters
        .span(requirement.parameters)
        .ok_or("boundary requirement lost its complete parameter span")?;
    let type_text = |reference| {
        typed
            .type_identity_text(typed.package_qualified_type_identity(reference))
            .filter(|text| !text.is_empty())
            .ok_or("boundary signature type has no exact identity")
    };
    let parameter_types = formals
        .iter()
        .map(|formal| type_text(formal.type_reference))
        .collect::<Result<Vec<_>, _>>()?;
    let result_type = type_text(requirement.return_type)?;
    let callable = typed.trait_requirement_callable_identity_text(owner, requirement);
    if callable.is_empty() {
        return Err("boundary requirement has no canonical callable identity");
    }
    let contract = declaration_contract_commitment(
        &callable,
        formals,
        &parameter_types,
        &result_type,
        &signature.requires,
        signature.ensures.iter(),
        signature.declared_crash_routes().unwrap_or_default(),
        signature,
        published,
    );
    let (specialization, method_arguments) =
        match typed.closed_requirement_application(requirement.symbol) {
            Some(closed) => {
                let (specialization, arguments) =
                    requirement_application(typed, sources, &closed, identities)?;
                (Some(specialization), arguments)
            }
            None => (None, Vec::new()),
        };
    let source = CheckedMachineDeclaration {
        machine: NominalDeclarationIdentity::new(owner_identity),
        state: NominalDeclarationIdentity::new(requirement_identity),
        semantic_declaration: semantic,
        parameter_types: parameter_types
            .iter()
            .map(|identity| identities.intern(identity))
            .collect(),
        callable: identities.intern(&callable),
        result_type: identities.intern(&result_type),
        contract,
        published_may_suspend: requirement.suspends,
        published_may_block: requirement.blocks,
        specialization,
    };
    Ok((
        NominalDeclarationIdentity::new(owner_identity),
        source,
        method_arguments,
    ))
}

/// What the declaration of a closed requirement states beside its own
/// identity: the requirement it was closed from, the arguments its
/// parameters were given, in parameter order, and a commitment to that
/// application. The commitment is formed as a generated machine's is
/// (`application`): the generic declaration's callable identity, then each
/// argument's kind and identity. Type and value arguments are stated; an
/// application that binds anything else is left undeclared.
fn requirement_application(
    typed: &TypedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    closed: &typed_trees::ClosedRequirementApplication<'_>,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<
    (
        terminal_psi::CheckedMachineSpecialization,
        Vec<terminal_psi::ClosedReachArgument>,
    ),
    &'static str,
> {
    let application = RequirementApplicationSource::read(typed, closed)?;
    // Keep the existing invocation-local owner when this is a private call.
    let semantic = match invocation_local(typed, closed.template, identities)? {
        Some(identity) => identity,
        None => identities.intern(
            &typed
                .normalized_hermetic_symbol_identity(closed.template)
                .map_err(|_| "a closed requirement's requirement has no hermetic identity")?,
        ),
    };
    let arguments = application.publish_arguments(identities);
    let template = nominal(typed, sources, closed.template, identities)?;
    Ok((
        application.publish_specialization(template, semantic, identities),
        arguments,
    ))
}

#[derive(Debug, Clone)]
enum RequirementArgumentSource {
    Type(String),
    Const(String),
}

/// Normalize an application once while its typed source is available. The
/// publishing route interns these same identities into its own module table.
#[derive(Debug, Clone)]
struct RequirementApplicationSource {
    callable: String,
    arguments: Vec<RequirementArgumentSource>,
    commitment: [u8; 32],
}

impl RequirementApplicationSource {
    fn read(
        typed: &TypedTrees,
        closed: &typed_trees::ClosedRequirementApplication<'_>,
    ) -> Result<Self, &'static str> {
        use typed_trees::static_applications::StaticMachineBindingValue;
        let (owner, template) = typed
            .trait_machine_signature(closed.template)
            .filter(|(owner, _)| owner.symbol == closed.declaring_trait)
            .ok_or("a closed requirement's requirement has no typed-tree declaration")?;
        let callable = typed.trait_requirement_callable_identity_text(owner, template);
        if callable.is_empty() {
            return Err("a closed requirement's requirement has no callable identity");
        }
        let bindings = typed
            .expression_table
            .static_machine_application_bindings(closed.application);
        if bindings.is_empty() || bindings.len() != closed.portable_local_arguments.len() {
            return Err("a closed requirement binds other than one argument per parameter");
        }
        let mut selected = Vec::with_capacity(bindings.len());
        let mut arguments = Vec::with_capacity(bindings.len());
        for (binding, identity) in bindings.iter().zip(closed.portable_local_arguments) {
            let text = typed
                .type_identity_text(*identity)
                .filter(|text| !text.is_empty())
                .ok_or("a closed requirement's argument has no identity")?;
            match &binding.value {
                StaticMachineBindingValue::Type(_) => {
                    selected.push(
                        language_semantics::conformance_identity::MachineOperandSelectionIdentity::Type(text.clone()),
                    );
                    arguments.push(RequirementArgumentSource::Type(text));
                }
                StaticMachineBindingValue::Const(_) => {
                    selected.push(
                        language_semantics::conformance_identity::MachineOperandSelectionIdentity::Const(text.clone()),
                    );
                    arguments.push(RequirementArgumentSource::Const(text));
                }
                StaticMachineBindingValue::Conformance(_)
                | StaticMachineBindingValue::RuntimeValue { .. }
                | StaticMachineBindingValue::Machine(_) => {
                    return Err(
                        "a closed requirement's application binds something other than a type or a value",
                    );
                }
            }
        }
        let commitment =
            language_semantics::conformance_identity::requirement_application_commitment(
                &callable, &selected,
            )?;
        Ok(Self {
            callable,
            arguments,
            commitment,
        })
    }

    fn publish_arguments(
        &self,
        identities: &mut TerminalIdentityTableBuilder,
    ) -> Vec<terminal_psi::ClosedReachArgument> {
        self.arguments
            .iter()
            .map(|argument| match argument {
                RequirementArgumentSource::Type(text) => {
                    terminal_psi::ClosedReachArgument::Type(identities.intern(text))
                }
                RequirementArgumentSource::Const(text) => {
                    terminal_psi::ClosedReachArgument::Const(identities.intern(text))
                }
            })
            .collect()
    }

    fn publish_specialization(
        &self,
        template: terminal_psi::TerminalIdentityIndex,
        semantic: terminal_psi::TerminalIdentityIndex,
        identities: &mut TerminalIdentityTableBuilder,
    ) -> terminal_psi::CheckedMachineSpecialization {
        terminal_psi::CheckedMachineSpecialization {
            capture: None,
            template_entry: NominalDeclarationIdentity::new(template),
            template: NominalDeclarationIdentity::new(template),
            template_callable: identities.intern(&self.callable),
            template_semantic_declaration: semantic,
            commitment: self.commitment,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn declaration_contract_commitment<'a>(
    callable: &str,
    formals: &[typed_trees::signature::StateParameter],
    parameter_types: &[String],
    result_type: &str,
    requires: &[semantic_vocabulary::Proposition],
    ensures: impl IntoIterator<Item = &'a semantic_vocabulary::Proposition>,
    crash_routes: &[terminal_psi::CrashRouteBucket],
    signature: &Signature,
    published: Published<'_>,
) -> [u8; 32] {
    let mut commitment = Commitment(DomainDigestEncoder::new(COMMITMENT_DOMAIN));
    callable.hash(&mut commitment);
    for (formal, type_text) in formals.iter().zip(parameter_types) {
        (
            formal.is_mutable,
            formal.is_const,
            formal.relevance,
            type_text,
        )
            .hash(&mut commitment);
    }
    result_type.hash(&mut commitment);
    requires.hash(&mut commitment);
    for proposition in ensures {
        proposition.hash(&mut commitment);
    }
    crash_routes.hash(&mut commitment);
    // A caller relies on the promise, never on how it is discharged:
    // termination.md, "Ranking subjects, view, rank range, cyclic-component
    // mapping, and proof certificate remain private. Changing a valid
    // witness revalidates the provider and its proof cache, not
    // caller/requirement identity". So a witness commits as its promise.
    signature.termination.hash(&mut commitment);
    // Open requirements and explicit empty promises must not share identity.
    (
        matches!(signature.reaches, crate::Ceiling::Authored(_)),
        matches!(signature.invokes, crate::Ceiling::Authored(_)),
        signature.declared_crash_routes().is_some(),
    )
        .hash(&mut commitment);
    // The published service reach is part of what a caller relies on. An
    // empty row adds nothing, so a contract with no reach commits as it
    // did before reach was read; a nonempty one is hashed with its length.
    if !published.reach.is_empty() {
        published.reach.hash(&mut commitment);
    }
    // So are the published synchronous entries. The axis is hashed before
    // the row, so `invokes A` never commits as `reaches A`.
    if !published.entries.is_empty() {
        ("synchronous entries", published.entries).hash(&mut commitment);
    }
    published.operational.hash(&mut commitment);
    commitment.0.finish()
}

/// The identities of the rows a declaration publishes on the two ceiling
/// axes, in row order.
#[derive(Clone, Copy)]
pub(crate) struct Published<'a> {
    pub reach: &'a [&'a str],
    pub entries: &'a [&'a str],
    pub operational: [bool; 2],
}

/// Reuse the declaration owner's existing scoped identity for application
/// constituents too; portable publication still requires a hermetic owner.
pub(crate) fn semantic_identity(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<terminal_psi::TerminalIdentityIndex, &'static str> {
    if let Some(identity) = invocation_local(typed, symbol, identities)? {
        return Ok(identity);
    }
    let identity = typed
        .normalized_hermetic_symbol_identity(symbol)
        .map_err(|_| "the declaration has no hermetic identity")?;
    Ok(identities.intern(&identity))
}

/// The exact-owner identity of `symbol`: its package's, or its toolchain
/// source's. A declaration neither owns has no portable identity.
pub(crate) fn nominal(
    typed: &TypedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    symbol: SymbolHandle,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<terminal_psi::TerminalIdentityIndex, &'static str> {
    if let Some(identity) = invocation_local(typed, symbol, identities)? {
        return Ok(identity);
    }
    let text = typed
        .exact_owner_nominal_identity(symbol, sources)
        .and_then(|identity| typed.type_identity_text(identity))
        .filter(|text| !text.is_empty())
        .ok_or("the declaration has no exact package or toolchain owner")?;
    Ok(identities.intern(&text))
}

/// Anonymous source and generated invocation declarations belong to this one
/// activation. A generated call over concrete arguments must not acquire a
/// portable identity from its temporary name. Its exact retained derivation
/// must still end at a captured source token before local identity is minted.
fn invocation_local(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<Option<terminal_psi::TerminalIdentityIndex>, &'static str> {
    if !symbol.is_valid() || !identities.is_invocation_scoped() {
        return Ok(None);
    }
    if !typed.symbols.symbols().nodes().is_valid(symbol) {
        return Err("an invocation declaration is foreign to its retained symbols");
    }
    // An implicit source entry has no name token of its own. Its lexical
    // owner's retained source still gives it an ordinary nominal identity;
    // only an actual generated declaration requires a derivation chain.
    if typed.symbols.get(symbol).generated_from.is_valid() {
        let mut origin = symbol;
        loop {
            let declaration = typed.symbols.get(origin);
            // Generated initial states can derive from an authored implicit
            // state, whose retained token belongs to its lexical machine.
            // The first link remains the mandatory generated_from edge;
            // subsequent lexical ancestry uses the same provenance owner as
            // ordinary implicit source entries.
            let previous = if declaration.generated_from.is_valid() {
                declaration.generated_from
            } else {
                declaration.parent
            };
            if !previous.is_valid() {
                return Err("a generated invocation declaration has no derivation origin");
            }
            if !typed.symbols.symbols().nodes().is_valid(previous) {
                return Err("a generated invocation declaration has a foreign derivation origin");
            }
            if previous.arena_index() >= origin.arena_index() {
                return Err(
                    "a generated invocation declaration has a nonpreceding derivation origin",
                );
            }
            origin = previous;
            let Some(span) = typed.symbols.symbol_source_span(origin) else {
                continue;
            };
            let source = typed
                .symbols
                .source_file(span)
                .ok_or("a generated invocation declaration names a foreign source")?;
            if span.span.start >= span.span.end
                || source.source.get(span.span.start..span.span.end).is_none()
            {
                return Err("a generated invocation declaration has an invalid source token");
            }
            break;
        }
    } else {
        let span = typed
            .symbols
            .symbol_provenance_source_span(symbol)
            .ok_or("an invocation declaration has no retained source provenance")?;
        let source = typed
            .symbols
            .source_file(span)
            .ok_or("an invocation declaration names a foreign source")?;
        if span.span.start >= span.span.end
            || source.source.get(span.span.start..span.span.end).is_none()
        {
            return Err("an invocation declaration has an invalid source token");
        }
        if typed.symbols.symbol_package_identity(symbol).is_some()
            || typed.symbols.symbol_source_origin(symbol) != Some(source::SourceOrigin::User)
        {
            return Ok(None);
        }
    }
    identities
        .intern_invocation_local(&format!(
            "invocation-declaration({}:{})",
            symbol.arena_index(),
            symbol.generation()
        ))
        .map(Some)
}

/// A bodyless requirement keeps the declaration identities and formal types the
/// reader resolved. Publication only adds the lowered public contract digest.
#[derive(Debug, Clone)]
pub(crate) struct BoundaryDeclarationSource {
    pub callable: String,
    pub owner: String,
    machine: String,
    state: String,
    semantic: String,
    parameter_types: Vec<String>,
    parameter_modes: Vec<(bool, bool, language_core::BindingRelevance)>,
    result_type: String,
    may_suspend: bool,
    may_block: bool,
    application: Option<BoundaryApplicationSource>,
}

#[derive(Debug, Clone)]
struct BoundaryApplicationSource {
    template: String,
    semantic: String,
    application: RequirementApplicationSource,
}

impl BoundaryDeclarationSource {
    pub(crate) fn method_arguments(
        &self,
        identities: &mut TerminalIdentityTableBuilder,
    ) -> Vec<terminal_psi::ClosedReachArgument> {
        self.application.as_ref().map_or_else(Vec::new, |source| {
            source.application.publish_arguments(identities)
        })
    }
}

pub(crate) fn boundary_source(
    typed: &TypedTrees,
    source: SymbolHandle,
    owner: SymbolHandle,
    signature: &typed_trees::signature::StateSignature,
    callable: String,
) -> Result<BoundaryDeclarationSource, &'static str> {
    let sources =
        source::source_identity::toolchain_source_identities(typed.symbols.source_files())
            .map_err(|_| "a boundary toolchain source has no exact identity")?;
    if callable.is_empty() {
        return Err("the boundary has no callable identity");
    }
    let formals = typed
        .state_parameters
        .span(signature.parameters)
        .ok_or("the boundary parameter span does not resolve")?;
    let type_text = |reference| {
        typed
            .type_identity_text(typed.package_qualified_type_identity(reference))
            .filter(|text| !text.is_empty())
            .ok_or("a boundary signature type has no exact identity")
    };
    let nominal_text = |symbol| {
        typed
            .exact_owner_nominal_identity(symbol, &sources)
            .and_then(|identity| typed.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or("the boundary declaration has no exact package or toolchain authority")
    };
    Ok(BoundaryDeclarationSource {
        callable,
        owner: nominal_text(owner)?,
        machine: nominal_text(source)?,
        state: nominal_text(signature.symbol)?,
        semantic: typed
            .normalized_hermetic_symbol_identity(source)
            .map_err(|_| "the boundary has no hermetic declaration identity")?,
        parameter_types: formals
            .iter()
            .map(|formal| type_text(formal.type_reference))
            .collect::<Result<Vec<_>, _>>()?,
        parameter_modes: formals
            .iter()
            .map(|formal| (formal.is_mutable, formal.is_const, formal.relevance))
            .collect(),
        result_type: type_text(signature.return_type)?,
        may_suspend: signature.suspends,
        may_block: signature.blocks,
        application: typed
            .closed_requirement_application(signature.symbol)
            .map(|closed| -> Result<_, &'static str> {
                Ok(BoundaryApplicationSource {
                    template: nominal_text(closed.template)?,
                    semantic: typed
                        .normalized_hermetic_symbol_identity(closed.template)
                        .map_err(
                            |_| "a closed requirement's requirement has no hermetic identity",
                        )?,
                    application: RequirementApplicationSource::read(typed, &closed)?,
                })
            })
            .transpose()?,
    })
}

pub(crate) fn boundary_declaration(
    source: &BoundaryDeclarationSource,
    signature: &Signature,
    proof: &terminal_psi::MachineContract,
    identities: &mut TerminalIdentityTableBuilder,
) -> CheckedMachineDeclaration {
    let mut commitment = Commitment(DomainDigestEncoder::new(COMMITMENT_DOMAIN));
    source.callable.hash(&mut commitment);
    for (mode, type_text) in source.parameter_modes.iter().zip(&source.parameter_types) {
        (mode.0, mode.1, mode.2, type_text).hash(&mut commitment);
    }
    source.result_type.hash(&mut commitment);
    proof.requires.hash(&mut commitment);
    for clause in &proof.ensures {
        clause.proposition.hash(&mut commitment);
    }
    proof.crash_routes.hash(&mut commitment);
    signature.termination.hash(&mut commitment);
    let specialization = source.application.as_ref().map(|source| {
        let template = identities.intern(&source.template);
        let semantic = identities.intern(&source.semantic);
        source
            .application
            .publish_specialization(template, semantic, identities)
    });
    CheckedMachineDeclaration {
        machine: NominalDeclarationIdentity::new(identities.intern(&source.machine)),
        state: NominalDeclarationIdentity::new(identities.intern(&source.state)),
        semantic_declaration: identities.intern(&source.semantic),
        parameter_types: source
            .parameter_types
            .iter()
            .map(|identity| identities.intern(identity))
            .collect(),
        callable: identities.intern(&source.callable),
        result_type: identities.intern(&source.result_type),
        contract: commitment.0.finish(),
        published_may_suspend: source.may_suspend,
        published_may_block: source.may_block,
        specialization,
    }
}

/// A bodyless callable commits to its complete retained operational ceiling.
/// Service identities are portable declaration identities, never module slots.
pub(crate) fn provider_declaration(
    source: &BoundaryDeclarationSource,
    signature: &Signature,
    proof: &terminal_psi::MachineContract,
    envelope: &terminal_psi::TerminalCallableEnvelope,
    services: &crate::reach::ServiceMap,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<CheckedMachineDeclaration, &'static str> {
    let mut declaration = boundary_declaration(source, signature, proof, identities);
    declaration.published_may_suspend = envelope.suspends;
    declaration.published_may_block = envelope.blocks;
    let mut commitment = Commitment(DomainDigestEncoder::new(
        b"omega.lowered-boundary-contract.v1\0",
    ));
    declaration.contract.hash(&mut commitment);
    for service in &envelope.service_reach {
        services
            .identity(*service)
            .ok_or("native contract service is outside its canonical map")?
            .hash(&mut commitment);
    }
    0xffu8.hash(&mut commitment);
    for invocation in &envelope.invocations {
        match invocation {
            terminal_psi::TerminalCallableInvocation::Parameter(position) => {
                0u8.hash(&mut commitment);
                position.hash(&mut commitment);
            }
            terminal_psi::TerminalCallableInvocation::Service(service) => {
                1u8.hash(&mut commitment);
                services
                    .identity(*service)
                    .ok_or("native invocation is outside its canonical service map")?
                    .hash(&mut commitment);
            }
        }
    }
    envelope.suspends.hash(&mut commitment);
    envelope.blocks.hash(&mut commitment);
    match &envelope.termination {
        terminal_psi::TerminalCallableTermination::NotGuaranteed => 0u8.hash(&mut commitment),
        terminal_psi::TerminalCallableTermination::Terminates { premises } => {
            1u8.hash(&mut commitment);
            premises.len().hash(&mut commitment);
            for premise in premises {
                premise.parameter.hash(&mut commitment);
                premise.path.hash(&mut commitment);
                premise.domain.hash(&mut commitment);
            }
        }
    }
    declaration.contract = commitment.0.finish();
    Ok(declaration)
}

/// Retain the exact intrinsic source header before the publication phase.
pub(crate) fn intrinsic_boundary_source(
    typed: &TypedTrees,
    symbol: SymbolHandle,
) -> Result<BoundaryDeclarationSource, &'static str> {
    if let Some(machine) = typed.machine_by_symbol(symbol) {
        let [state] = typed.machine_states(machine) else {
            return Err("intrinsic has no exact entry");
        };
        let (_, signature) = typed
            .bodyless_declaration(symbol)?
            .ok_or("intrinsic has no bodyless signature")?;
        if signature.symbol != state.symbol {
            return Err("intrinsic bodyless signature has another entry declaration");
        }
        return boundary_source(
            typed,
            symbol,
            symbol,
            &signature,
            typed
                .machine_callable_identity_text(machine)
                .ok_or("intrinsic has no callable identity")?,
        );
    }
    let intrinsic =
        typed_trees::declarations::intrinsic_requirements::IntrinsicRequirement::by_symbol(
            typed, symbol,
        )
        .ok_or("intrinsic has no exact source declaration")?;
    let sources =
        source::source_identity::toolchain_source_identities(typed.symbols.source_files())
            .map_err(|_| "intrinsic source has no exact identity")?;
    let nominal = typed
        .exact_owner_nominal_identity(symbol, &sources)
        .and_then(|id| typed.type_identity_text(id))
        .ok_or("intrinsic declaration has no exact owner")?;
    let type_text = |reference| {
        typed
            .type_identity_text(typed.package_qualified_type_identity(reference))
            .filter(|text| !text.is_empty())
            .ok_or("intrinsic type has no exact identity")
    };
    Ok(BoundaryDeclarationSource {
        callable: intrinsic.requirement_identity,
        owner: nominal.clone(),
        machine: nominal.clone(),
        state: nominal,
        semantic: typed
            .normalized_hermetic_symbol_identity(symbol)
            .map_err(|_| "intrinsic has no hermetic identity")?,
        parameter_types: intrinsic
            .parameters
            .iter()
            .map(|formal| type_text(formal.type_reference))
            .collect::<Result<Vec<_>, _>>()?,
        parameter_modes: intrinsic
            .parameters
            .iter()
            .map(|formal| (formal.is_mutable, formal.is_const, formal.relevance))
            .collect(),
        result_type: type_text(intrinsic.return_type)?,
        may_suspend: false,
        may_block: false,
        application: None,
    })
}
