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
exactly as a record type's is (`RecordType::identity`). Nothing here or in
the checker compares those texts, and none is formed from a name.

The contract commitment is this route's own. The deleted checker committed to
the contract as it publishes it in checked trees, which this crate may not
read. Here the commitment is a digest of the contract the lowered form
publishes for the machine: its callable identity, each formal's type and
binding mode, its result type, the `requires` and `ensures` propositions and
crash routes of its `MachineContract`, and what its header says about
termination. Two machines with one commitment publish one contract. A
commitment from this route and one from the deleted checker are never equal,
so a consumer that compares the two refuses instead of pairing them.

A machine this cannot state is left without a declaration and listed in
`Elaboration::undeclared` with the reason. That changes no verdict: the
module is judged as before, and `AcceptedClosure::publish` refuses a module
that lists one, because it would publish a source-produced body with no
declaration identity.
*/

use std::hash::{Hash, Hasher};

use optimization::digest_encoder::DomainDigestEncoder;
use semantic_vocabulary::{IdTable, MachineId};
use symbols::SymbolHandle;
use terminal_psi::{
    CheckedMachineDeclaration, NominalDeclarationIdentity, TerminalIdentityTableBuilder,
    TerminalMachine,
};
use typed_trees::TypedTrees;

use crate::{Elaboration, Signature};

const COMMITMENT_DOMAIN: &[u8] = b"omega.lowered-machine-public-contract.v1\0";
/// Domain of the commitment a generated machine makes to its application.
const APPLICATION_DOMAIN: &[u8] = b"omega.lowered-machine-static-application.v1\0";

/// Feeds what `Hash` writes into the commitment digest, as the current
/// checker's `ContractHasher` does for its own commitment.
struct Commitment(DomainDigestEncoder);

impl Hasher for Commitment {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.raw(bytes);
    }
}

/// States the declaration identity of every machine of `elaboration` that
/// `machines` gives a typed-tree declaration, in machine order.
pub(crate) fn declare(
    typed: &TypedTrees,
    machines: &IdTable<MachineId, SymbolHandle>,
    signatures: &IdTable<MachineId, Signature>,
    elaboration: &mut Elaboration,
) {
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
    let mut identities = std::mem::take(&mut elaboration.module.identities).into_builder();
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
                identities_of(&machine.published_service_ceiling).and_then(|reach| {
                    let entries = identities_of(&machine.published_synchronous_entries)?;
                    declaration(
                        typed,
                        sources,
                        *symbol,
                        signature,
                        machine,
                        Published {
                            reach: &reach,
                            entries: &entries,
                        },
                        &mut identities,
                    )
                })
            }
            (Err(_), _, _) => Err("a toolchain source has no exact identity"),
            _ => Err("the machine has no typed-tree declaration"),
        };
        match stated {
            Ok(declaration) => machine.declaration = Some(declaration),
            Err(reason) => elaboration.undeclared.push((machine.id, reason)),
        }
    }
    elaboration.module.identities = identities.build();
}

fn declaration(
    typed: &TypedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    symbol: SymbolHandle,
    signature: &Signature,
    machine: &TerminalMachine,
    published: Published<'_>,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<CheckedMachineDeclaration, &'static str> {
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
    let type_text = |type_reference| {
        typed
            .type_identity_text(typed.package_qualified_type_identity(type_reference))
            .filter(|text| !text.is_empty())
            .ok_or("a signature type has no identity")
    };
    let parameter_types = formals
        .iter()
        .map(|formal| type_text(formal.type_reference))
        .collect::<Result<Vec<_>, _>>()?;
    let result_type = type_text(state.return_type)?;
    // A machine stage 04 generated for a static application also names the
    // generic machine it was generated from and commits to the application.
    let specialization = match typed.executable_application_template(symbol)? {
        Some(template) => Some(application(typed, sources, symbol, template, identities)?),
        None => None,
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
        signature.termination,
        published,
    );

    Ok(CheckedMachineDeclaration {
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
        // The reader refuses a machine that declares `suspends` or `blocks`
        // (`read/machine.rs`, `machine_shape`), so every machine here
        // publishes neither.
        published_may_suspend: false,
        published_may_block: false,
        specialization,
    })
}

/// What the declaration of `instance`, a machine stage 04 generated from the
/// generic machine `template`, states beside its own identity: the generic
/// machine's identities and a commitment to the application.
///
/// The commitment is this route's own, like the contract commitment: a
/// digest of the generic machine's callable identity and the identity of
/// each type and implementation the application binds, in parameter order.
/// Two machines generated from one generic machine at the same arguments
/// commit alike, and no other pair does. Type, implementation and value
/// arguments are stated; an application that binds anything else is left
/// undeclared.
fn application(
    typed: &TypedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    instance: SymbolHandle,
    template: SymbolHandle,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<terminal_psi::CheckedMachineSpecialization, &'static str> {
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
    let mut rows = typed
        .machine_specializations
        .iter()
        .filter(|row| row.instance == instance);
    let (Some(row), None) = (rows.next(), rows.next()) else {
        return Err("a generated machine does not have exactly one application");
    };
    let mut commitment = Commitment(DomainDigestEncoder::new(APPLICATION_DOMAIN));
    callable.hash(&mut commitment);
    let bindings = typed
        .expression_table
        .static_machine_application_bindings(row.binding_application);
    if bindings.is_empty() {
        return Err("a generated machine's application binds nothing");
    }
    let arguments = typed
        .expression_table
        .static_machine_application_arguments(row.binding_application);
    if arguments.len() != bindings.len() {
        return Err(
            "a generated machine's application binds other than one argument per parameter",
        );
    }
    for (binding, argument) in bindings.iter().zip(arguments) {
        match &binding.value {
            StaticMachineBindingValue::Type(type_reference) => typed
                .type_identity_text(typed.package_qualified_type_identity(*type_reference))
                .filter(|text| !text.is_empty())
                .ok_or("an application's type argument has no identity")?
                .hash(&mut commitment),
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
                (
                    "implementation",
                    identities.text(identity).ok_or(
                        "an application's implementation identity is absent from its table",
                    )?,
                )
                    .hash(&mut commitment);
            }
            // generics.md, "Closed applications and canonical identity":
            // a value argument is part of the application's identity. It is
            // stated by the parameter's type and the literal's canonical
            // spelling, with its kind, so `7` given to a `u8` and to a `u64`
            // parameter, or a value and a type, never commit alike.
            StaticMachineBindingValue::Const(type_reference) => {
                let literal = argument
                    .const_literal
                    .as_ref()
                    .ok_or("an application's value argument is not a literal")?;
                let carrier = typed
                    .type_identity_text(typed.package_qualified_type_identity(*type_reference))
                    .filter(|text| !text.is_empty())
                    .ok_or("an application's value parameter type has no identity")?;
                ("value", carrier, literal.canonical_spelling().as_str()).hash(&mut commitment);
            }
            StaticMachineBindingValue::RuntimeValue { .. }
            | StaticMachineBindingValue::Machine(_) => {
                return Err(
                    "a generated machine's application binds something other than a type, an implementation or a value",
                );
            }
        }
    }
    Ok(terminal_psi::CheckedMachineSpecialization {
        capture: None,
        template: NominalDeclarationIdentity::new(nominal(typed, sources, template, identities)?),
        template_callable: identities.intern(&callable),
        template_semantic_declaration: semantic,
        commitment: commitment.0.finish(),
    })
}

/// A bodyless trait method retains its actual nominal owner and signature.
/// There is no executable state to invent: the owning trait and requirement
/// occupy the existing owner/entry coordinates of the declaration record.
/// Provider checking and host installation still establish execution supply.
pub(crate) fn boundary(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    signature: &Signature,
    published: Published<'_>,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<
    (
        NominalDeclarationIdentity,
        terminal_psi::CheckedBoundaryCallable,
    ),
    &'static str,
> {
    let (owner, requirement) = typed
        .trait_machine_signature(symbol)
        .filter(|(owner, requirement)| owner.is_boundary && requirement.symbol == symbol)
        .ok_or("boundary declaration has no exact owning trait and requirement")?;
    if !owner.type_parameters.is_empty()
        || !owner.lifetime_parameters.is_empty()
        || !requirement.type_parameters.is_empty()
        || !requirement.lifetime_parameters.is_empty()
    {
        return Err("boundary declaration needs its complete applied telescope");
    }
    let sources =
        source::source_identity::toolchain_source_identities(typed.symbols.source_files())
            .map_err(|_| "a boundary toolchain source has no exact identity")?;
    let owner_identity = nominal(typed, &sources, owner.symbol, identities)?;
    let requirement_identity = nominal(typed, &sources, symbol, identities)?;
    let semantic = semantic_identity(typed, symbol, identities)?;
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
        signature
            .declared_crash_routes()
            .ok_or("boundary declaration has no published crash ceiling")?,
        signature.termination,
        published,
    );
    let (specialization, method_arguments) = match typed.closed_requirement_application(symbol) {
        Some(closed) => {
            let (specialization, arguments) =
                requirement_application(typed, &sources, &closed, identities)?;
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
        terminal_psi::CheckedBoundaryCallable {
            source,
            execution: terminal_psi::CheckedBoundaryExecution::Provider,
            method_arguments,
        },
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
    use typed_trees::static_applications::StaticMachineBindingValue;
    let (owner, template) = typed
        .trait_machine_signature(closed.template)
        .filter(|(owner, _)| owner.symbol == closed.declaring_trait)
        .ok_or("a closed requirement's requirement has no typed-tree declaration")?;
    let callable = typed.trait_requirement_callable_identity_text(owner, template);
    if callable.is_empty() {
        return Err("a closed requirement's requirement has no callable identity");
    }
    // Stated as a boundary's own semantic declaration is (`boundary`).
    let semantic = match invocation_local(typed, closed.template, identities)? {
        Some(identity) => identity,
        None => identities.intern(
            &typed
                .normalized_hermetic_symbol_identity(closed.template)
                .map_err(|_| "a closed requirement's requirement has no hermetic identity")?,
        ),
    };
    let bindings = typed
        .expression_table
        .static_machine_application_bindings(closed.application);
    if bindings.is_empty() || bindings.len() != closed.portable_local_arguments.len() {
        return Err("a closed requirement binds other than one argument per parameter");
    }
    let mut commitment = Commitment(DomainDigestEncoder::new(APPLICATION_DOMAIN));
    callable.hash(&mut commitment);
    let mut arguments = Vec::with_capacity(bindings.len());
    for (binding, identity) in bindings.iter().zip(closed.portable_local_arguments) {
        let text = typed
            .type_identity_text(*identity)
            .filter(|text| !text.is_empty())
            .ok_or("a closed requirement's argument has no identity")?;
        match &binding.value {
            StaticMachineBindingValue::Type(_) => {
                text.hash(&mut commitment);
                arguments.push(terminal_psi::ClosedReachArgument::Type(
                    identities.intern(&text),
                ));
            }
            StaticMachineBindingValue::Const(_) => {
                ("value", text.as_str()).hash(&mut commitment);
                arguments.push(terminal_psi::ClosedReachArgument::Const(
                    identities.intern(&text),
                ));
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
    Ok((
        terminal_psi::CheckedMachineSpecialization {
            capture: None,
            template: NominalDeclarationIdentity::new(nominal(
                typed,
                sources,
                closed.template,
                identities,
            )?),
            template_callable: identities.intern(&callable),
            template_semantic_declaration: semantic,
            commitment: commitment.0.finish(),
        },
        arguments,
    ))
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
    termination: crate::Termination,
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
    termination.hash(&mut commitment);
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
    commitment.0.finish()
}

/// The identities of the rows a declaration publishes on the two ceiling
/// axes, in row order.
#[derive(Clone, Copy)]
pub(crate) struct Published<'a> {
    pub reach: &'a [&'a str],
    pub entries: &'a [&'a str],
}

/// Reuse the declaration owner's existing scoped identity for application
/// constituents too; portable publication still requires a hermetic owner.
fn semantic_identity(
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
fn nominal(
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
