//! Compiler supply belongs to an exact bundled declaration and its entire
//! signature. Derive that signature from the ordinary machine owner: a token
//! view or a matching operand count cannot stand in for its contract.

use crate::TypedTrees;
use crate::data::TypeParameterKind;
use crate::domain::{ProofFact, ProofMembershipFact};
use crate::machine::Machine;
use crate::signature::{SignatureContract, SignatureContractKind};
use crate::type_identity::constraint_identity::{
    atom, compound, normalized_compiler_domain_identity,
};
use crate::type_identity::identity_context::{
    TypeIdentityContext, TypeIdentityQualification, normalize_index_expression,
    normalize_type_reference,
};
use crate::type_identity::{LifetimeParameterIdentity, NormalizedTypeIdentity};
use diagnostics::Diagnostic;
use language_core::sealed_intrinsics::{CompilerPrimitive, SealedIntrinsic};
use language_semantics::type_identity::vocabulary::Spelled;
use std::cell::Cell;
use symbols::SymbolHandle;

impl TypedTrees {
    /// Source metadata only. This does not prove a requirement, select a
    /// provider, or authorize execution of an application. The caller must
    /// instantiate this declaration and check its whole contract at the use.
    pub fn compiler_primitive_schema(
        &self,
        symbol: SymbolHandle,
    ) -> Result<Option<(CompilerPrimitive, NormalizedTypeIdentity)>, Diagnostic> {
        self.compiler_primitive_schema_checked(symbol)
            .map_err(Diagnostic::error)
    }

    pub(crate) fn compiler_primitive_schema_checked(
        &self,
        symbol: SymbolHandle,
    ) -> Result<Option<(CompilerPrimitive, NormalizedTypeIdentity)>, &'static str> {
        let Some(machine) = self.machine_by_symbol(symbol) else {
            return Ok(None);
        };
        let custody = self
            .symbols
            .sealed_declaration(symbol, machine.name.as_str());
        let Some(SealedIntrinsic::CompilerPrimitive(operation)) = custody else {
            return if machine.supply_mode == language_semantics::MachineSupplyMode::CompilerProvided
            {
                Err("compiler primitive has no exact sealed declaration custody")
            } else {
                Ok(None)
            };
        };
        if machine.supply_mode != language_semantics::MachineSupplyMode::CompilerProvided
            || machine.body_is_present
        {
            return Err(
                "sealed compiler primitive has a different supply mode or an authored body",
            );
        }
        normalized_schema_checked(self, machine).map(|schema| Some((operation, schema)))
    }
}

// Schema coordinates use the existing canonical identity interner. They are
// derived descriptions of the one Machine, never another declaration carrier.
#[derive(Clone, Copy)]
enum Tag {
    Schema,
    Binders,
    Binder,
    Kind,
    Bounds,
    Lifetimes,
    Parameters,
    Parameter,
    Mode,
    Type,
    Result,
    Contracts,
    Contract,
    Requires,
    Ensures,
    Crash,
    Envelope,
    Termination,
    Premises,
    Premise,
    Profile,
    Subject,
    Projection,
    Attachment,
    Owner,
    Membership,
    Value,
    Domain,
}

impl Spelled for Tag {
    fn spelling(self) -> &'static str {
        match self {
            Self::Schema => "machine-schema",
            Self::Binders => "binders",
            Self::Binder => "binder",
            Self::Kind => "kind",
            Self::Bounds => "bounds",
            Self::Lifetimes => "lifetimes",
            Self::Parameters => "parameters",
            Self::Parameter => "parameter",
            Self::Mode => "mode",
            Self::Type => "type",
            Self::Result => "result",
            Self::Contracts => "contracts",
            Self::Contract => "contract",
            Self::Requires => "requires",
            Self::Ensures => "ensures",
            Self::Crash => "crash",
            Self::Envelope => "envelope",
            Self::Termination => "termination",
            Self::Premises => "premises",
            Self::Premise => "progress-premise",
            Self::Profile => "progress-profile",
            Self::Subject => "progress-subject",
            Self::Projection => "progress-projection",
            Self::Attachment => "attachment",
            Self::Owner => "owner",
            Self::Membership => "membership",
            Self::Value => "value",
            Self::Domain => "domain",
        }
    }
}

// Complete signatures must not silently lose a malformed retained roster.
// Storage span lookups validate nonempty ownership; only the canonical empty handle
// represents a genuinely absent roster here.
fn schema_span<T, Rows>(
    read: impl FnOnce(arena::HandleSpan<T>) -> Option<Rows>,
    span: arena::HandleSpan<T>,
) -> Option<Rows> {
    if span.is_empty() && span != arena::HandleSpan::empty() {
        return None;
    }
    read(span)
}

fn normalized_schema_checked(
    program: &TypedTrees,
    machine: &Machine,
) -> Result<NormalizedTypeIdentity, &'static str> {
    // Attribute the first refused phase before another normalization can set
    // the shared invalid-context bit. The complete schema owner and every
    // admission guard remain the same for diagnostics and prepared calls.
    let Machine {
        lexical_region_origin,
        assembly_target_requirement,
        symbol,
        name: _,
        attached_data,
        attached_data_symbol,
        attached_data_application,
        generic_data_template,
        spelling,
        is_public,
        interface_exposure,
        supply_mode: _,
        purpose,
        target,
        authored_target,
        target_family_interface,
        body_is_present: _,
        body_status: _,
        structural_type_equations_pending,
        termination_plan,
        service_reach_row,
        service_reach_is_installation_bound,
        admitted_same_stack_bytes,
        declares_no_crash_route,
        suspends_keyword_source_spans,
        blocks_keyword_source_spans,
        lifetime_parameters,
        type_parameters: _,
        declared_type_equations,
        owned_data,
        satisfies,
        conformance_bounds,
        invokes,
        suspends,
        blocks,
        contracts: _,
        states: _,
    } = machine;
    if lexical_region_origin.is_some()
        || generic_data_template.is_valid()
        || target.is_some()
        || authored_target.is_some()
        || *target_family_interface
        || *structural_type_equations_pending
        || *declared_type_equations != arena::HandleSpan::empty()
        || *owned_data != arena::HandleSpan::empty()
        || *satisfies != arena::HandleSpan::empty()
        || !conformance_bounds.is_empty()
        || *invokes != arena::HandleSpan::empty()
        || *service_reach_is_installation_bound
        || !symbol.is_valid()
        || termination_plan.implementation_witness.is_some()
        || termination_plan.checked_summary != language_semantics::TerminationGuarantee::NoGuarantee
    {
        return Err("compiler primitive schema: unsupported header");
    }
    let reach = if *service_reach_row == language_semantics::ServiceReachRowId::NULL {
        "inferred"
    } else if *service_reach_row == language_semantics::ServiceReachRowTable::EMPTY_ROW {
        "none"
    } else {
        return Err("compiler primitive schema: unsupported reach row");
    };
    let entry_rows = schema_span(|span| program.machine_states.span(span), machine.states)
        .ok_or("compiler primitive schema: stale entry roster")?;
    let [entry] = entry_rows
        .as_array()
        .ok_or("compiler primitive schema: requires one exact entry")?;
    if !entry.symbol.is_valid() {
        return Err("compiler primitive schema: unresolved entry declaration");
    }
    let types = schema_span(
        |span| program.data_type_parameters.span(span),
        machine.type_parameters,
    )
    .ok_or("compiler primitive schema: stale static binder roster")?;
    let parameters = schema_span(|span| program.state_parameters.span(span), entry.parameters)
        .ok_or("compiler primitive schema: stale parameter roster")?;
    let binders = types
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| (parameter.symbol, format!("$static{ordinal}")))
        .chain(
            parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| (parameter.symbol, format!("$value{ordinal}"))),
        )
        .collect::<Vec<_>>();
    if binders.iter().any(|(symbol, _)| !symbol.is_valid()) {
        return Err("compiler primitive schema: unresolved static or value binder");
    }
    let lifetimes = lifetime_parameters
        .iter()
        .enumerate()
        .map(|(ordinal, _)| {
            (
                LifetimeParameterIdentity {
                    declaration: *symbol,
                    ordinal,
                },
                ordinal,
            )
        })
        .collect::<Vec<_>>();
    let invalid = Cell::new(false);
    let context = TypeIdentityContext {
        binders: &binders,
        lifetime_binders: &lifetimes,
        application_owner: *symbol,
        application_argument_owner: *symbol,
        signature_owners: Some((*symbol, entry.symbol)),
        qualification: TypeIdentityQualification::PackageQualified,
        invalid_application: Some(&invalid),
        invalid_index_argument: Some(&invalid),
        encountered_elided_regions: Some(&invalid),
        ..Default::default()
    };
    let termination = termination_schema(program, &termination_plan.interface, &context)
        .ok_or("compiler primitive schema: unsupported termination premises")?;
    if invalid.get() {
        return Err("compiler primitive schema: open termination identity");
    }
    let type_identity = |reference, refusal| {
        if !program.contains_type_reference(reference) {
            return Err(refusal);
        }
        let identity = normalize_type_reference(program, reference, &context);
        if invalid.get() {
            return Err(refusal);
        }
        Ok(identity)
    };
    // Preserve the actual nominal owner and its application exactly as before.
    let attachment = match attached_data {
        None => {
            if attached_data_symbol.is_valid() || attached_data_application.is_valid() {
                return Err("compiler primitive schema: attachment metadata without an owner");
            }
            None
        }
        Some(_) => {
            if !program
                .symbols
                .symbols()
                .nodes()
                .is_valid(*attached_data_symbol)
                || program.symbols.get(*attached_data_symbol).kind != symbols::SymbolKind::Data
            {
                return Err("compiler primitive schema: unresolved attachment owner");
            }
            let declaration = program
                .unique_data_definition_by_symbol(*attached_data_symbol)
                .ok_or("compiler primitive schema: attachment has no unique data declaration")?;
            let application = if attached_data_application.is_valid() {
                if !program.contains_type_reference(*attached_data_application)
                    || program
                        .type_reference_table
                        .type_head_symbol(*attached_data_application)
                        != Some(*attached_data_symbol)
                {
                    return Err(
                        "compiler primitive schema: attachment application selects another owner",
                    );
                }
                compound(
                    Tag::Type,
                    [type_identity(
                        *attached_data_application,
                        "compiler primitive schema: attachment type is missing or open",
                    )?],
                )
            } else {
                if !declaration.type_parameters.is_empty() {
                    return Err("compiler primitive schema: generic attachment has no application");
                }
                compound(Tag::Type, [])
            };
            let owner = context.name(
                program,
                *attached_data_symbol,
                program.symbols.name(*attached_data_symbol),
            );
            if invalid.get() {
                return Err("compiler primitive schema: open attachment owner identity");
            }
            Some(compound(
                Tag::Attachment,
                [atom(Tag::Owner, &owner), application],
            ))
        }
    };
    let mut telescope = Vec::with_capacity(types.len());
    for parameter in types {
        let kind = match parameter.kind {
            TypeParameterKind::Type => atom(Tag::Kind, "type"),
            TypeParameterKind::Const { type_reference } => compound(
                Tag::Kind,
                [
                    atom(Tag::Kind, "const"),
                    type_identity(
                        type_reference,
                        "compiler primitive schema: static binder type is missing or open",
                    )?,
                ],
            ),
            _ => return Err("compiler primitive schema: unsupported static binder kind"),
        };
        telescope.push(compound(
            Tag::Binder,
            [kind, atom(Tag::Bounds, &format!("{:?}", parameter.bounds))],
        ));
    }
    let parameters = parameters
        .iter()
        .map(|parameter| {
            Ok(compound(
                Tag::Parameter,
                [
                    atom(
                        Tag::Mode,
                        &format!(
                            "const={};mutable={};relevance={:?}",
                            parameter.is_const, parameter.is_mutable, parameter.relevance
                        ),
                    ),
                    compound(
                        Tag::Type,
                        [type_identity(
                            parameter.type_reference,
                            "compiler primitive schema: parameter type is missing or open",
                        )?],
                    ),
                ],
            ))
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let result = type_identity(
        entry.return_type,
        "compiler primitive schema: result type is missing or open",
    )?;
    let contracts = contracts(
        program,
        schema_span(
            |span| program.signature_contracts.span(span),
            machine.contracts,
        )
        .ok_or("compiler primitive schema: stale machine contract roster")?
        .iter()
        .chain(
            schema_span(
                |span| program.signature_contracts.span(span),
                entry.contracts,
            )
            .ok_or("compiler primitive schema: stale entry contract roster")?,
        ),
        &context,
    )?;
    let identity = compound(Tag::Schema, [
        compound(Tag::Binders, telescope), atom(Tag::Lifetimes, &lifetimes.len().to_string()),
        compound(Tag::Parameters, parameters), compound(Tag::Result, [result]),
        compound(Tag::Contracts, contracts),
        atom(Tag::Envelope, &format!(
            "public={is_public};exposure={interface_exposure:?};purpose={purpose:?};token={spelling:?};assembly={assembly_target_requirement:?};reach={reach};suspends={suspends};suspends-written={};blocks={blocks};blocks-written={};crashes-none={declares_no_crash_route};termination={termination};stack={admitted_same_stack_bytes}",
            !suspends_keyword_source_spans.is_empty(), !blocks_keyword_source_spans.is_empty()
        )),
    ].into_iter().chain(attachment));
    Ok(program.type_identities.intern(&identity))
}

#[cfg(test)]
fn normalized_schema(program: &TypedTrees, machine: &Machine) -> Option<NormalizedTypeIdentity> {
    normalized_schema_checked(program, machine).ok()
}

fn contracts<'a>(
    program: &TypedTrees,
    contracts: impl Iterator<Item = &'a SignatureContract>,
    context: &TypeIdentityContext<'_>,
) -> Result<Vec<String>, &'static str> {
    contracts
        .map(|contract| {
            if contract.binding.is_some() {
                return Err("compiler primitive schema: unsupported bound contract premise");
            }
            let kind = match contract.kind {
                SignatureContractKind::Requires => atom(Tag::Requires, ""),
                SignatureContractKind::Ensures => atom(Tag::Ensures, ""),
                SignatureContractKind::Crashes { cause } => atom(Tag::Crash, &format!("{cause:?}")),
                SignatureContractKind::EnsuresForResultCase { .. } => {
                    return Err("compiler primitive schema: unsupported result-case contract");
                }
            };
            let facts = schema_span(|span| program.proof_facts.span(span), contract.facts)
                .ok_or("compiler primitive schema: stale contract fact roster")?
                .iter()
                .map(|fact| {
                    let expression = match fact {
                        ProofFact::Expression(expression) => {
                            if !expression.is_valid() {
                                return Err("compiler primitive schema: missing contract expression");
                            }
                            normalize_index_expression(program, *expression, context)
                        }
                        ProofFact::Membership(membership) => membership_schema(program, membership, context)?,
                        ProofFact::Proposition(_) => return Err("compiler primitive schema: unsupported proposition contract proof form"),
                    };
                    if context.invalid_application.is_some_and(Cell::get) {
                        return Err(
                            "compiler primitive schema: open or unserved contract expression",
                        );
                    }
                    Ok(expression)
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            Ok(compound(Tag::Contract, std::iter::once(kind).chain(facts)))
        })
        .collect()
}

// Qualified formals retain ordinary membership requirements on the entry.
// Describe their actual subject and domain application with the same owners
// used by expression and qualified-type normalization, without granting the
// membership, selecting an issuer, or treating its authored path as identity.
fn membership_schema(
    program: &TypedTrees,
    membership: &ProofMembershipFact,
    context: &TypeIdentityContext<'_>,
) -> Result<String, &'static str> {
    if !program
        .expression_table
        .expression_is_valid(membership.value)
    {
        return Err("compiler primitive schema: stale membership subject");
    }
    if !program
        .symbols
        .symbols()
        .nodes()
        .is_valid(membership.domain_symbol)
        || program.symbols.get(membership.domain_symbol).kind != symbols::SymbolKind::Domain
        || program
            .domain_definition(membership.domain_symbol)
            .is_none()
    {
        return Err("compiler primitive schema: unresolved membership domain declaration");
    }
    schema_span(
        |span| program.domain_path_members.span(span),
        membership.domain,
    )
    .ok_or("compiler primitive schema: stale membership spelling roster")?;
    let arguments = program
        .type_reference_table
        .type_reference_handles(membership.domain_arguments);
    if arguments.len() != membership.domain_arguments.len()
        || (membership.domain_arguments.is_empty()
            && membership.domain_arguments != arena::HandleSpan::empty())
    {
        return Err("compiler primitive schema: stale membership argument roster");
    }
    let retained = program
        .domain_arguments_for_instance(membership.domain_symbol, membership.semantic_domain)
        .ok_or("compiler primitive schema: membership lost its retained domain application")?;
    if retained.len() != arguments.len() {
        return Err("compiler primitive schema: membership changed its domain argument roster");
    }
    for (actual, retained) in arguments.iter().zip(retained) {
        if !program.contains_type_reference(*actual) || !program.contains_type_reference(*retained)
        {
            return Err("compiler primitive schema: stale membership argument type");
        }
        if normalize_type_reference(program, *actual, context)
            != normalize_type_reference(program, *retained, context)
        {
            return Err(
                "compiler primitive schema: membership changed its retained domain arguments",
            );
        }
    }
    let subject = normalize_index_expression(program, membership.value, context);
    let domain = normalized_compiler_domain_identity(
        program,
        &crate::types::DomainConstraint {
            // The resolved declaration and retained application supply meaning;
            // the spelling and authored-selection coordinates stay source metadata.
            name: crate::name::Identifier::from(program.symbols.name(membership.domain_symbol)),
            symbol: membership.domain_symbol,
            arguments: arguments.to_vec(),
            semantic_id: membership.semantic_domain,
            ..Default::default()
        },
        context,
    );
    if context.invalid_application.is_some_and(Cell::get) {
        return Err("compiler primitive schema: open membership subject or domain application");
    }
    Ok(compound(
        Tag::Membership,
        [
            compound(Tag::Value, [subject]),
            compound(Tag::Domain, [domain]),
        ],
    ))
}

// This is an interface description, never termination evidence. The caller
// still refuses implementation witnesses and checked body summaries; neither
// may disappear from a source-only compiler primitive schema.
fn termination_schema(
    program: &TypedTrees,
    interface: &language_semantics::TerminationInterface,
    context: &TypeIdentityContext<'_>,
) -> Option<String> {
    use language_semantics::{TerminationGuarantee, TerminationInterface};
    match interface {
        TerminationInterface::InternalDerived => Some("inferred".to_owned()),
        TerminationInterface::Published(TerminationGuarantee::NoGuarantee) => {
            Some("no-guarantee".to_owned())
        }
        TerminationInterface::Published(TerminationGuarantee::Terminates { premises }) => {
            let premises = premises
                .iter()
                .map(|premise| {
                    let language_semantics::ProgressPremise { profile, subject } = premise;
                    let language_semantics::ProgressSubject { root, projections } = subject;
                    let profile = program.semantic_domains.name(*profile)?;
                    let mut roots = context.binders.iter().filter(|(symbol, _)| symbol == root);
                    let (_, root) = roots.next()?;
                    if roots.next().is_some() {
                        return None;
                    }
                    let projections = projections
                        .iter()
                        .map(|symbol| {
                            if !program.symbols.symbols().nodes().is_valid(*symbol)
                                || program.symbols.get(*symbol).kind != symbols::SymbolKind::Field
                            {
                                return None;
                            }
                            Some(atom(
                                Tag::Projection,
                                &context.name(program, *symbol, program.symbols.name(*symbol)),
                            ))
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Some(compound(
                        Tag::Premise,
                        [
                            atom(Tag::Profile, profile),
                            compound(
                                Tag::Subject,
                                std::iter::once(atom(Tag::Subject, root)).chain(projections),
                            ),
                        ],
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(compound(
                Tag::Termination,
                [
                    atom(Tag::Termination, "terminates"),
                    compound(Tag::Premises, premises),
                ],
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::TypeParameter;
    use crate::expression::{
        BinaryOperator, CollectionMeasure, Expression, ExpressionHandle, ExpressionNode,
        MemberSelection, NamePath, TableBinaryExpression, TableMemberExpression, TableNamePath,
    };
    use crate::name::Identifier;
    use crate::signature::StateParameter;
    use crate::state::State;
    use crate::types::{FixedArrayLength, TypeReferenceNode};
    use symbols::{BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder};

    struct Fixture {
        program: TypedTrees,
        owner: SymbolHandle,
        other_owner: SymbolHandle,
        domains: [SymbolHandle; 2],
        element: arena::Handle<TypeParameter>,
        extent: arena::Handle<TypeParameter>,
        result: ExpressionHandle,
        measure: ExpressionHandle,
    }

    impl Fixture {
        // The typed header of an array view, including a const extent and a
        // result loan tied to the first of two distinct declared lifetimes.
        fn new(names: [&'static str; 5]) -> Self {
            let [element_name, extent_name, life, other_life, value] = names;
            let mut builder = SymbolTableBuilder::default();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let roots = builder.insert_children(
                root,
                symbols::builtin_type_symbols().into_iter().chain([
                    (SymbolKind::Machine, SymbolNameRef::Static("view")),
                    (SymbolKind::Data, SymbolNameRef::Static("Owner")),
                    (SymbolKind::Data, SymbolNameRef::Static("OtherOwner")),
                    (
                        SymbolKind::Domain,
                        SymbolNameRef::Static("MembershipDomain"),
                    ),
                    (
                        SymbolKind::Domain,
                        SymbolNameRef::Static("OtherMembershipDomain"),
                    ),
                ]),
            );
            let u64_symbol = roots.handle_at(BuiltinTypeAtom::U64.ordinal() as u32);
            let machine = roots.handle_at(BuiltinTypeAtom::ALL.len() as u32);
            let owner = roots.handle_at(BuiltinTypeAtom::ALL.len() as u32 + 1);
            let other_owner = roots.handle_at(BuiltinTypeAtom::ALL.len() as u32 + 2);
            let domains = [
                roots.handle_at(BuiltinTypeAtom::ALL.len() as u32 + 3),
                roots.handle_at(BuiltinTypeAtom::ALL.len() as u32 + 4),
            ];
            let domain_parameters = domains.map(|domain| {
                builder.insert_children(
                    domain,
                    [
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("Left")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("Right")),
                    ],
                )
            });
            let children = builder.insert_children(
                machine,
                [
                    (
                        SymbolKind::TypeParameter,
                        SymbolNameRef::Static(element_name),
                    ),
                    (
                        SymbolKind::TypeParameter,
                        SymbolNameRef::Static(extent_name),
                    ),
                    (SymbolKind::State, SymbolNameRef::Static("entry")),
                ],
            );
            let (element_symbol, extent_symbol, entry) = (
                children.handle_at(0),
                children.handle_at(1),
                children.handle_at(2),
            );
            let value_symbol = builder
                .insert_children(
                    entry,
                    [(SymbolKind::Parameter, SymbolNameRef::Static(value))],
                )
                .handle_at(0);
            let mut program = TypedTrees {
                symbols: builder.finish(),
                ..Default::default()
            };
            for (symbol, name) in [(owner, "Owner"), (other_owner, "OtherOwner")] {
                program.push_data_definition(crate::data::DataDefinition {
                    symbol,
                    name: Identifier::generated(name),
                    ..Default::default()
                });
            }
            let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
            for (domain, parameters) in domains.into_iter().zip(domain_parameters) {
                let name = Identifier::from(program.symbols.name(domain));
                let semantic_id = program.semantic_domains.intern(name.as_str());
                let parameters = parameters
                    .handles()
                    .map(|symbol| TypeParameter {
                        symbol,
                        name: Identifier::from(program.symbols.name(symbol)),
                        kind: TypeParameterKind::Type,
                        ..Default::default()
                    })
                    .collect::<Vec<_>>();
                let type_parameters = program.data_type_parameters.insert_many(parameters);
                program.push_domain_definition(crate::domain::DomainDefinition {
                    symbol: domain,
                    name,
                    type_parameters,
                    target_type: unit,
                    semantic_id,
                    ..Default::default()
                });
            }
            let u64_type = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: u64_symbol,
                    name: Identifier::generated("u64"),
                });
            let element_type = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: element_symbol,
                    name: Identifier::generated(element_name),
                });
            let type_parameters = program.data_type_parameters.insert_many([
                TypeParameter {
                    symbol: element_symbol,
                    name: Identifier::generated(element_name),
                    kind: TypeParameterKind::Type,
                    ..Default::default()
                },
                TypeParameter {
                    symbol: extent_symbol,
                    name: Identifier::generated(extent_name),
                    kind: TypeParameterKind::Const {
                        type_reference: u64_type,
                    },
                    ..Default::default()
                },
            ]);
            let array = program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::ConstParameter {
                        symbol: extent_symbol,
                        name: Identifier::generated(extent_name),
                    },
                });
            let input = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: array,
                    access: language_core::ReferenceAccess::Shared,
                    lifetime: Some(Identifier::generated(life)),
                });
            let slice = program
                .type_reference_table
                .insert(TypeReferenceNode::Slice { element_type });
            let output = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: slice,
                    access: language_core::ReferenceAccess::Shared,
                    lifetime: Some(Identifier::generated(life)),
                });
            let parameters = program.state_parameters.insert_many([StateParameter {
                symbol: value_symbol,
                name: Identifier::generated(value),
                type_reference: input,
                ..Default::default()
            }]);
            let result = program.expression_table.insert_tree(&Expression::Name(
                NamePath::unresolved(vec![Identifier::generated("result")])
                    .denoting_reserved_result_of(entry),
            ));
            let measure =
                program
                    .expression_table
                    .insert(ExpressionNode::Member(TableMemberExpression {
                        receiver: result,
                        selection: MemberSelection::CollectionMeasure(CollectionMeasure::Length),
                        member: Identifier::generated("len"),
                        case_variant: None,
                    }));
            let mut members = arena::HandleSpan::empty();
            program
                .expression_table
                .push_name_path_member(&mut members, Identifier::generated(extent_name));
            let extent = program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    symbol: extent_symbol,
                    head_symbol: extent_symbol,
                    ..Default::default()
                }));
            let equality =
                program
                    .expression_table
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        left: measure,
                        operator: BinaryOperator::Equal,
                        right: extent,
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    }));
            let facts = program
                .proof_facts
                .insert_many([ProofFact::Expression(equality)]);
            let contracts = program.signature_contracts.insert_many([SignatureContract {
                kind: SignatureContractKind::Ensures,
                facts,
                ..Default::default()
            }]);
            let states = program.machine_states.insert_many([State {
                symbol: entry,
                name: Identifier::generated("entry"),
                parameters,
                return_type: output,
                contracts,
                ..Default::default()
            }]);
            program.push_machine(Machine {
                symbol: machine,
                name: Identifier::generated("view"),
                lifetime_parameters: vec![
                    Identifier::generated(life),
                    Identifier::generated(other_life),
                ],
                type_parameters,
                states,
                supply_mode: language_semantics::MachineSupplyMode::CompilerProvided,
                ..Default::default()
            });
            Self {
                program,
                owner,
                other_owner,
                domains,
                element: type_parameters.handle_at(0),
                extent: type_parameters.handle_at(1),
                result,
                measure,
            }
        }

        fn ordinary() -> Self {
            Self::new(["Element", "Count", "backing", "other", "items"])
        }

        fn memberships(names: [&'static str; 5]) -> (Self, arena::HandleSpan<ProofFact>) {
            let mut fixture = Self::new(names);
            let program = &mut fixture.program;
            let entry = program.machines()[0].states.start();
            let formal = program.state_parameters(program.machine_states.get(entry))[0].clone();
            let parameter = program.data_type_parameters.get(fixture.element).clone();
            let element = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: parameter.symbol,
                    name: parameter.name.clone(),
                });
            let TypeParameterKind::Const {
                type_reference: u64_type,
            } = program.data_type_parameters.get(fixture.extent).kind
            else {
                unreachable!()
            };
            let arguments = vec![element, u64_type];
            let mut facts = Vec::new();
            for domain in fixture.domains {
                let semantic = program
                    .retain_domain_application(domain, formal.type_reference, &arguments)
                    .unwrap();
                let contract = program.parameter_domain_membership_contract(
                    formal.symbol,
                    formal.name.clone(),
                    domain,
                    "DiagnosticAlias",
                    arguments.clone(),
                    semantic,
                );
                facts.extend(
                    program
                        .proof_facts
                        .span(contract.facts)
                        .unwrap()
                        .iter()
                        .cloned(),
                );
            }
            let facts = program.proof_facts.insert_many(facts);
            let state_contracts = program.machine_states.get(entry).contracts;
            let mut contracts = program
                .signature_contracts
                .span(state_contracts)
                .unwrap()
                .to_vec();
            contracts.push(SignatureContract {
                kind: SignatureContractKind::Requires,
                facts,
                ..Default::default()
            });
            program.machine_states.get_mut(entry).contracts =
                program.signature_contracts.insert_many(contracts);
            (fixture, facts)
        }

        fn text(&self) -> Option<String> {
            let schema = normalized_schema(&self.program, &self.program.machines()[0])?;
            self.program.type_identity_text(schema)
        }
    }

    #[test]
    fn membership_schemas_keep_the_subject_owner_arguments_and_fact_order() {
        let names = ["Element", "Count", "backing", "other", "items"];
        let (fixture, _) = Fixture::memberships(names);
        let original = fixture
            .text()
            .expect("implicit qualified-formal memberships normalize");
        assert_eq!(
            Some(original.clone()),
            Fixture::memberships(["T", "N", "scope", "spare", "values"])
                .0
                .text()
        );
        for fault in 0..5 {
            let (mut changed, facts) = Fixture::memberships(names);
            let ProofFact::Membership(mut first) =
                changed.program.proof_facts.get(facts.handle_at(0)).clone()
            else {
                unreachable!()
            };
            let ProofFact::Membership(second) =
                changed.program.proof_facts.get(facts.handle_at(1)).clone()
            else {
                unreachable!()
            };
            match fault {
                0 => {
                    first.domain_symbol = second.domain_symbol;
                    first.semantic_domain = second.semantic_domain;
                }
                1 => first.value = changed.result,
                2 => {
                    let mut arguments = changed
                        .program
                        .type_reference_table
                        .type_reference_handles(first.domain_arguments)
                        .to_vec();
                    arguments.reverse();
                    let entry = changed
                        .program
                        .machine_states(&changed.program.machines()[0])[0]
                        .clone();
                    let carrier = changed.program.state_parameters(&entry)[0].type_reference;
                    first.semantic_domain = changed
                        .program
                        .retain_domain_application(first.domain_symbol, carrier, &arguments)
                        .unwrap();
                    first.domain_arguments = changed
                        .program
                        .type_reference_table
                        .insert_type_reference_handles(arguments);
                }
                3 => {
                    *changed.program.proof_facts.get_mut(facts.handle_at(0)) =
                        ProofFact::Membership(second);
                    *changed.program.proof_facts.get_mut(facts.handle_at(1)) =
                        ProofFact::Membership(first);
                    assert_ne!(
                        Some(&original),
                        changed.text().as_ref(),
                        "membership fact order"
                    );
                    continue;
                }
                4 => {
                    let mut arguments = changed
                        .program
                        .type_reference_table
                        .type_reference_handles(first.domain_arguments)
                        .to_vec();
                    arguments.reverse();
                    first.domain_arguments = changed
                        .program
                        .type_reference_table
                        .insert_type_reference_handles(arguments);
                }
                _ => unreachable!(),
            }
            *changed.program.proof_facts.get_mut(facts.handle_at(0)) = ProofFact::Membership(first);
            if fault == 4 {
                assert!(
                    changed.text().is_none(),
                    "changed explicit arguments cannot borrow a retained application"
                );
            } else {
                assert_ne!(
                    Some(&original),
                    changed.text().as_ref(),
                    "membership mutation {fault}"
                );
            }
        }
    }

    #[test]
    fn complete_schema_keeps_the_nominal_attachment_and_refuses_a_substituted_application() {
        let mut fixture = Fixture::ordinary();
        let plain = fixture.text().unwrap();
        let mut attach = |owner, name| {
            fixture
                .program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: owner,
                    name: Identifier::generated(name),
                })
        };
        let application = attach(fixture.owner, "Owner");
        let other_application = attach(fixture.other_owner, "OtherOwner");
        let machine = fixture.program.machine_mut(0);
        machine.attached_data = Some(Identifier::generated("Owner"));
        machine.attached_data_symbol = fixture.owner;
        machine.attached_data_application = application;
        let original = fixture
            .text()
            .expect("a complete nominal attachment normalizes");
        assert_ne!(plain, original);
        fixture.program.machine_mut(0).attached_data_application = other_application;
        assert!(
            fixture.text().is_none(),
            "the application must belong to its declared owner"
        );
        fixture.program.machine_mut(0).attached_data_symbol = fixture.other_owner;
        let other = fixture
            .text()
            .expect("the other complete owner also normalizes");
        assert_ne!(original, other);
        fixture.program.machine_mut(0).attached_data = None;
        assert!(
            fixture.text().is_none(),
            "an attachment cannot disappear while retaining its application"
        );
    }

    #[test]
    fn complete_schema_renames_type_const_lifetime_and_value_binders() {
        let original = Fixture::ordinary()
            .text()
            .expect("the complete header must normalize");
        let renamed = Fixture::new(["T", "N", "scope", "unused", "source"])
            .text()
            .unwrap();
        assert_eq!(original, renamed);
    }

    #[test]
    fn binder_kinds_bounds_result_lifetimes_and_operational_clauses_change_schema() {
        let original = Fixture::ordinary().text().unwrap();
        let mut kind = Fixture::ordinary();
        kind.program.data_type_parameters.get_mut(kind.extent).kind = TypeParameterKind::Type;
        assert_ne!(Some(&original), kind.text().as_ref());
        let mut bound = Fixture::ordinary();
        bound
            .program
            .data_type_parameters
            .get_mut(bound.element)
            .bounds
            .multiplicity = language_semantics::Multiplicity::Unrestricted;
        assert_ne!(Some(&original), bound.text().as_ref());
        let mut lifetime = Fixture::ordinary();
        let output = lifetime
            .program
            .machine_states(lifetime.program.machines().first().unwrap())[0]
            .return_type;
        let mut replacement = lifetime
            .program
            .type_reference_table
            .type_reference(output)
            .clone();
        let TypeReferenceNode::Reference {
            lifetime: result_life,
            ..
        } = &mut replacement
        else {
            panic!("borrowed result")
        };
        *result_life = Some(Identifier::generated("other"));
        lifetime
            .program
            .type_reference_table
            .substitute_node(output, replacement);
        assert_ne!(Some(&original), lifetime.text().as_ref());
        let mut ceiling = Fixture::ordinary();
        ceiling.program.machine_mut(0).declares_no_crash_route = true;
        assert_ne!(Some(&original), ceiling.text().as_ref());
    }

    #[test]
    fn published_termination_keeps_its_complete_conditional_subjects() {
        use language_semantics::{
            ProgressPremise, ProgressSubject, TerminationGuarantee, TerminationInterface,
        };
        let original = Fixture::ordinary().text().unwrap();
        let mut unconditional = Fixture::ordinary();
        unconditional
            .program
            .machine_mut(0)
            .termination_plan
            .interface =
            TerminationInterface::Published(TerminationGuarantee::Terminates { premises: vec![] });
        let terminates = unconditional
            .text()
            .expect("published unconditional promise normalizes");
        assert_ne!(original, terminates);
        let conditional = |names, profile_name: &str| {
            let mut fixture = Fixture::new(names);
            let machine = &fixture.program.machines()[0];
            let entry = &fixture.program.machine_states(machine)[0];
            let subject = fixture.program.state_parameters(entry)[0].symbol;
            let profile = fixture.program.semantic_domains.intern(profile_name);
            fixture.program.machine_mut(0).termination_plan.interface =
                TerminationInterface::Published(TerminationGuarantee::Terminates {
                    premises: vec![ProgressPremise {
                        profile,
                        subject: ProgressSubject {
                            root: subject,
                            projections: vec![],
                        },
                    }],
                });
            fixture
        };
        let mut first = conditional(
            ["Element", "Count", "backing", "other", "items"],
            "package::Ready",
        );
        let first_text = first.text().unwrap();
        assert_ne!(terminates, first_text);
        assert_eq!(
            Some(first_text.clone()),
            conditional(["T", "N", "scope", "spare", "bytes"], "package::Ready").text()
        );
        assert_ne!(
            Some(first_text),
            conditional(
                ["Element", "Count", "backing", "other", "items"],
                "other::Ready"
            )
            .text()
        );
        let TerminationInterface::Published(TerminationGuarantee::Terminates { premises }) =
            &mut first.program.machine_mut(0).termination_plan.interface
        else {
            unreachable!()
        };
        premises[0]
            .subject
            .projections
            .push(SymbolHandle::invalid());
        assert!(
            first.text().is_none(),
            "unknown projections cannot disappear"
        );
    }

    #[test]
    fn implementation_rankings_and_checked_summaries_are_not_silently_erased() {
        let mut ranking = Fixture::ordinary();
        ranking
            .program
            .machine_mut(0)
            .termination_plan
            .implementation_witness = Some(language_semantics::RankingWitness::default());
        assert!(ranking.text().is_none());
        let mut checked = Fixture::ordinary();
        checked
            .program
            .machine_mut(0)
            .termination_plan
            .checked_summary =
            language_semantics::TerminationGuarantee::Terminates { premises: vec![] };
        assert!(checked.text().is_none());
    }

    #[test]
    fn every_added_requirement_is_part_of_the_same_schema() {
        let original = Fixture::ordinary().text().unwrap();
        let mut extra = Fixture::ordinary();
        let expression = extra
            .program
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let facts = extra
            .program
            .proof_facts
            .insert_many([ProofFact::Expression(expression)]);
        let contract = SignatureContract {
            kind: SignatureContractKind::Requires,
            facts,
            ..Default::default()
        };
        let contracts = extra.program.signature_contracts.insert_many([contract]);
        extra.program.machine_mut(0).contracts = contracts;
        assert_ne!(Some(&original), extra.text().as_ref());
    }

    #[test]
    fn arithmetic_policy_is_part_of_the_complete_contract() {
        let mut exact = Fixture::ordinary();
        let expression = exact
            .program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: exact.measure,
                operator: BinaryOperator::Add,
                right: exact.measure,
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
            }));
        let equality = exact
            .program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: expression,
                operator: BinaryOperator::Equal,
                right: exact.measure,
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
            }));
        let facts = exact
            .program
            .proof_facts
            .insert_many([ProofFact::Expression(equality)]);
        let contracts = exact
            .program
            .signature_contracts
            .insert_many([SignatureContract {
                kind: SignatureContractKind::Ensures,
                facts,
                ..Default::default()
            }]);
        exact.program.machine_mut(0).contracts = contracts;
        let original = exact.text().unwrap();
        let ExpressionNode::Binary(binary) =
            exact.program.expression_table.expression_mut(expression)
        else {
            panic!("arithmetic")
        };
        binary.policy = numerics::arithmetic::ArithmeticDomain::Wrapping;
        assert_ne!(Some(&original), exact.text().as_ref());
    }

    #[test]
    fn unresolved_members_foreign_results_and_unserved_binders_are_unavailable() {
        let mut member = Fixture::ordinary();
        let ExpressionNode::Member(selected) = member
            .program
            .expression_table
            .expression_mut(member.measure)
        else {
            panic!("measurement")
        };
        selected.selection = MemberSelection::Unresolved;
        assert!(member.text().is_none());
        let mut result = Fixture::ordinary();
        let ExpressionNode::Name(path) = result
            .program
            .expression_table
            .expression_mut(result.result)
        else {
            panic!("result")
        };
        path.reserved_result_of = SymbolHandle::from_arena_index(0);
        assert!(result.text().is_none());
        let mut binder = Fixture::ordinary();
        binder
            .program
            .data_type_parameters
            .get_mut(binder.extent)
            .kind = TypeParameterKind::Implementation;
        assert!(binder.text().is_none());
    }

    fn broken_span<T>(count: u32) -> arena::HandleSpan<T> {
        arena::HandleSpan::from_parts(arena::Handle::from_parts(u32::MAX, 1), count)
    }

    #[test]
    fn a_broken_retained_fact_span_cannot_become_an_empty_contract() {
        let mut broken = Fixture::ordinary();
        let entry = broken.program.machines()[0].states.start();
        let contract = broken.program.machine_states.get(entry).contracts.start();
        broken.program.signature_contracts.get_mut(contract).facts = broken_span(1);
        assert!(broken.text().is_none());

        // A genuinely empty fact roster is still a representable contract.
        broken.program.signature_contracts.get_mut(contract).facts = arena::HandleSpan::empty();
        assert!(broken.text().is_some());
        broken.program.signature_contracts.get_mut(contract).facts = broken_span(0);
        assert!(broken.text().is_none());
    }

    #[test]
    fn malformed_signature_rosters_are_unavailable_instead_of_disappearing() {
        for count in [0, 1] {
            let mut states = Fixture::ordinary();
            states.program.machine_mut(0).states = broken_span(count);
            assert!(states.text().is_none(), "states, count={count}");

            let mut types = Fixture::ordinary();
            types.program.machine_mut(0).type_parameters = broken_span(count);
            assert!(types.text().is_none(), "type parameters, count={count}");

            let mut parameters = Fixture::ordinary();
            let entry = parameters.program.machines()[0].states.start();
            parameters.program.machine_states.get_mut(entry).parameters = broken_span(count);
            assert!(parameters.text().is_none(), "parameters, count={count}");

            let mut machine_contracts = Fixture::ordinary();
            machine_contracts.program.machine_mut(0).contracts = broken_span(count);
            assert!(
                machine_contracts.text().is_none(),
                "machine contracts, count={count}"
            );

            let mut entry_contracts = Fixture::ordinary();
            let entry = entry_contracts.program.machines()[0].states.start();
            entry_contracts
                .program
                .machine_states
                .get_mut(entry)
                .contracts = broken_span(count);
            assert!(
                entry_contracts.text().is_none(),
                "entry contracts, count={count}"
            );
        }
    }

    #[test]
    fn a_complete_header_does_not_grant_catalog_custody() {
        let fixture = Fixture::ordinary();
        assert!(fixture.text().is_some());
        assert!(
            fixture
                .program
                .compiler_primitive_schema(fixture.program.machines()[0].symbol)
                .is_err()
        );
    }
}
