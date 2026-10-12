//! Portable projection of the original checked application view.
//!
//! Static operands stay in their owning typed payload namespace. The shared
//! original-declaration view supplies binder order and category; the existing
//! type normalizer supplies exact source owners and original region subjects.
//! This query copies retained meanings, never executes a selected body or
//! substitutes runtime arithmetic for a missing producer receipt.
//! The caller supplies the module's one identity builder. Every projected type
//! and original declaration joins that same owner after exact normalization;
//! this query never creates an independent table or portable index digest.

use checked_trees::CheckedTrees;
use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
use terminal_psi::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryApplicationLifetimeBinding,
    BoundaryDeclarationArgumentKind, BoundaryLifetimeArgument, BoundaryLifetimeParameter,
    BoundaryTypeIdentity, NominalDeclarationIdentity, TerminalIdentityTableBuilder,
};
use typed_trees::static_applications::{
    StaticApplicationDeclaration, StaticApplicationView, StaticMachineApplicationHandle,
    StaticMachineBindingValue,
};
use typed_trees::type_identity::{AppliedTypeIdentityOwner, ExactOwnerTypeIdentityRequest};
use typed_trees::{
    TypedTrees,
    data::TypeParameterKind,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

pub(crate) fn project_checked_static_application(
    identities: &mut TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    handle: StaticMachineApplicationHandle,
) -> Result<BoundaryApplication, &'static str> {
    let program: &TypedTrees = &checked.typed;
    let view = StaticApplicationView::new(program, handle)?;
    let lexical_owner = view
        .application()
        .type_identity_owner(program)
        .ok_or("portable application lost its original lexical capture")?;
    let substitutions = view
        .bindings()
        .iter()
        .filter_map(|binding| match &binding.value {
            StaticMachineBindingValue::Type(reference) => Some((binding.parameter, *reference)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut arguments = Vec::with_capacity(view.bindings().len());
    for (ordinal, binding) in view.bindings().iter().enumerate() {
        let ordinal =
            u32::try_from(ordinal).map_err(|_| "portable application binder count exceeds u32")?;
        arguments.push(match &binding.value {
            StaticMachineBindingValue::Type(reference) => {
                BoundaryApplicationArgument::type_argument(ordinal, project_type_identity(identities, program, *reference, &[], lexical_owner, exact_toolchain_sources)?)
            }
            StaticMachineBindingValue::Const(reference) => {
                let parameter = view.declaration().parameters().get(ordinal as usize)
                    .ok_or("portable value application lost its original carrier binder")?;
                let (carrier, value_binder) = match parameter.kind {
                    TypeParameterKind::Const { type_reference } => (type_reference, false),
                    TypeParameterKind::Value { type_reference } => (type_reference, true),
                    _ => return Err("portable value application changed its original binder category"),
                };
                let value = closed_value(program, *reference, carrier)?;
                let carrier = project_type_identity(identities, program, carrier, &substitutions, lexical_owner, exact_toolchain_sources)?;
                if value_binder {
                    BoundaryApplicationArgument::value_argument(ordinal, carrier, value.type_name, value.encoding)?
                } else {
                    BoundaryApplicationArgument::const_argument(ordinal, carrier, value.type_name, value.encoding)?
                }
            }
            StaticMachineBindingValue::Machine(argument)
            | StaticMachineBindingValue::Conformance(argument) => {
                if argument.type_reference.is_valid() || argument.const_literal.is_some()
                    || argument.evidence_projection.is_some()
                {
                    return Err("portable declaration operand mixes original static categories");
                }
                let machine = matches!(&binding.value, StaticMachineBindingValue::Machine(_));
                let callable = program.machine(argument.symbol).or_else(|| program.machine_holding_state(argument.symbol)).is_some()
                    || program.trait_machine_signature(argument.symbol).is_some()
                    || typed_trees::operator::declaration_by_symbol(program, argument.symbol).is_some();
                if (machine && !callable)
                    || (!machine && program.conformance_by_symbol(argument.symbol).is_none())
                {
                    return Err("portable declaration operand lost its original machine or conformance owner");
                }
                let declaration = StaticApplicationDeclaration::for_symbol(program, argument.symbol)?;
                if program.machine_specializations.iter().any(|specialization| {
                    specialization.instance == declaration.symbol()
                        && specialization.template != declaration.symbol()
                }) {
                    return Err("nested machine instance lost its original declaration application association");
                }
                if !declaration.parameters().is_empty()
                    || !declaration.conformance_bounds().is_empty()
                    || !declaration.lifetimes().is_empty()
                    || argument.application.as_ref().is_some_and(|application| {
                        !application.arguments.is_empty() || !application.lifetime_arguments.is_empty()
                    })
                {
                    return Err("nested declaration application has no retained original typed binding association");
                }
                let kind = if matches!(&binding.value, StaticMachineBindingValue::Machine(_)) {
                    BoundaryDeclarationArgumentKind::Machine
                } else { BoundaryDeclarationArgumentKind::Conformance };
                BoundaryApplicationArgument::Declaration {
                    binder_ordinal: ordinal, kind,
                    declaration: nominal_identity(identities, program, argument.symbol, exact_toolchain_sources)?,
                    application: BoundaryApplication::Empty,
                }
            }
            StaticMachineBindingValue::RuntimeValue { .. } => {
                return Err("portable closed application has no produced static value for its runtime subject");
            }
        });
    }
    let mut lifetimes = Vec::with_capacity(view.lifetimes().len());
    let (region_program, region_owner) = match lexical_owner {
        AppliedTypeIdentityOwner::Declaration(symbol) => (program, symbol),
        AppliedTypeIdentityOwner::Captured(capture) => (capture.owner(), capture.symbol()),
    };
    for (ordinal, (parameter, binding)) in view.lifetimes().iter().enumerate() {
        let argument = match &binding.argument {
            typed_trees::static_applications::StaticMachineLifetimeArgument::Named(argument) => {
                let (argument_owner, argument_ordinal) = region_program
                    .original_declaration_lifetime_parameter(region_owner, argument.as_str())
                    .ok_or("portable lifetime argument has no retained original declaration correspondence")?;
                BoundaryLifetimeArgument::Named(BoundaryLifetimeParameter {
                    declaration: lifetime_declaration_identity(
                        identities,
                        region_program,
                        argument_owner,
                        exact_toolchain_sources,
                    )?,
                    parameter_ordinal: u32::try_from(argument_ordinal)
                        .map_err(|_| "portable lifetime argument ordinal exceeds u32")?,
                })
            }
            typed_trees::static_applications::StaticMachineLifetimeArgument::Anonymous => {
                if !parameter.invocation {
                    return Err(
                        "portable captured lifetime cannot use anonymous invocation erasure",
                    );
                }
                BoundaryLifetimeArgument::AnonymousInvocation
            }
        };
        lifetimes.push(BoundaryApplicationLifetimeBinding {
            binder_ordinal: u32::try_from(ordinal)
                .map_err(|_| "portable application lifetime count exceeds u32")?,
            parameter: BoundaryLifetimeParameter {
                declaration: lifetime_declaration_identity(
                    identities,
                    program,
                    parameter.declaration,
                    exact_toolchain_sources,
                )?,
                parameter_ordinal: u32::try_from(parameter.ordinal)
                    .map_err(|_| "portable lifetime formal ordinal exceeds u32")?,
            },
            argument,
        });
    }
    if arguments.is_empty() && lifetimes.is_empty() {
        Ok(BoundaryApplication::Empty)
    } else {
        BoundaryApplication::exact_with_lifetimes(arguments, lifetimes)
    }
}

fn project_type_identity(
    identities: &mut TerminalIdentityTableBuilder,
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    lexical_owner: AppliedTypeIdentityOwner<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<BoundaryTypeIdentity, &'static str> {
    let identity = program
        .exact_owner_applied_type_identity(
            ExactOwnerTypeIdentityRequest {
                type_reference: reference,
                binders: &[],
                substitutions,
                exact_toolchain_sources,
            },
            lexical_owner,
        )
        .ok_or(
            "portable application type has no complete original owner and region correspondence",
        )?;
    let text = program
        .type_identity_text(identity)
        .ok_or("portable identity lost its original interner owner")?;
    if text.is_empty() {
        return Err("portable application type has an empty original identity");
    }
    Ok(BoundaryTypeIdentity::new(identities.intern(&text)))
}

fn nominal_identity(
    identities: &mut TerminalIdentityTableBuilder,
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<NominalDeclarationIdentity, &'static str> {
    let identity = program
        .exact_owner_nominal_identity(symbol, exact_toolchain_sources)
        .ok_or("portable static declaration has no exact original package or toolchain owner")?;
    let text = program
        .type_identity_text(identity)
        .ok_or("portable static declaration lost its original interner owner")?;
    if text.is_empty() {
        return Err("portable static declaration has an empty original identity");
    }
    Ok(NominalDeclarationIdentity::new(identities.intern(&text)))
}

/// The source declaration, rather than an application or rendered identity,
/// owns its complete formal-role roster. Both formal and named actual region
/// coordinates publish that roster into the canonical Terminal identity owner.
fn lifetime_declaration_identity(
    identities: &mut TerminalIdentityTableBuilder,
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<NominalDeclarationIdentity, &'static str> {
    use terminal_psi::TerminalLifetimeFormalRole::{Captured, Invocation};
    let (count, role) = if let Some(machine) = program
        .machine(symbol)
        .or_else(|| program.machine_holding_state(symbol))
    {
        (machine.lifetime_parameters.len(), Invocation)
    } else if let Some((_, signature)) = program.trait_machine_signature(symbol) {
        if program
            .public_trait_requirement_signature(symbol)
            .is_some_and(|row| row.application.is_valid())
        {
            return Err("portable lifetime roster must name its original requirement declaration");
        }
        (signature.lifetime_parameters.len(), Invocation)
    } else if let Some(operator) = typed_trees::operator::declaration_by_symbol(program, symbol) {
        (operator.lifetime_parameters.len(), Invocation)
    } else if let Some(declaration) = program.trait_definition_by_symbol(symbol) {
        (declaration.lifetime_parameters.len(), Captured)
    } else if let Some(declaration) = program.conformance_by_symbol(symbol) {
        (declaration.lifetime_parameters.len(), Captured)
    } else if let Some(declaration) = program.data_definition_by_symbol(symbol) {
        (declaration.lifetime_parameters.len(), Captured)
    } else {
        return Err("portable lifetime roster has no original source declaration");
    };
    let declaration = nominal_identity(identities, program, symbol, exact_toolchain_sources)?;
    identities.retain_lifetime_declaration(declaration.identity(), vec![role; count])?;
    Ok(declaration)
}

fn closed_value(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    carrier: TypeReferenceHandle,
) -> Result<CanonicalConstIdentity, &'static str> {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(reference)
    else {
        return Err("portable static value has no retained canonical constant meaning");
    };
    if symbol.is_valid() {
        return Err("portable static value remains an open declaration operand");
    }
    if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
        return Ok(CanonicalConstIdentity {
            type_name: value.type_name,
            encoding: value.encoding,
        });
    }
    let value = name
        .as_str()
        .parse::<i128>()
        .map_err(|_| "portable static value has no retained canonical constant encoding")?;
    let primitive = program
        .primitive_type_reference(carrier)
        .ok_or("portable integer value has no retained declared primitive carrier")?;
    Ok(CanonicalConstIdentity::integer(primitive.name(), value))
}
