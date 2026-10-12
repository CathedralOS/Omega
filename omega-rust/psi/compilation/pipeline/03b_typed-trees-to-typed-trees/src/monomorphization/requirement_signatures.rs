//! Demanded trait/member applications produce their complete public interface.
//!
//! This owner consumes the original declaration and retained typed tuple. The
//! shared signature copier owns all formal, type and proof-fact children; the
//! authored conformance roster stays unchanged. Both exact application payload
//! and original demand survive preparation. A generated public header is not a
//! selected supplier, conformance receipt, or implementation body.

use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::static_applications::{
    InstantiatedRequirementSignature, StaticApplicationDemand, StaticMachineBinding,
    StaticMachineBindingValue,
};
use typed_trees::trait_definition::AppliedTraitScope;

/// A selected closed interface supplies its outer tuple to every member that
/// has no independent invocation-static operands. Reuse the ordinary demand
/// producer for those headers; invocation-generic members need their actual
/// demanded application and are never filled with arbitrary operands.
pub(super) fn complete_interface_headers(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let headers = program
        .instantiated_requirement_signatures
        .iter()
        .filter(|(_, row)| {
            program
                .trait_definition_by_symbol(row.declaring_trait)
                .is_some_and(|definition| definition.is_boundary)
        })
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    for header in headers {
        let identity =
            typed_trees::static_applications::requirement_interface_application_identity(
                program, &header,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        let selected = program
            .trait_definition_by_symbol(header.selected_interface)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "closed interface lost its original declaration",
                )]
            })?;
        let members =
            typed_trees::static_applications::boundary_interface_members(program, selected.symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "closed interface lost its original member roster",
                    )]
                })?;
        let parameters = program
            .trait_type_parameters(selected)
            .iter()
            .map(|parameter| parameter.symbol)
            .chain(selected.conformance_bounds.iter().map(|bound| bound.binder))
            .collect::<Vec<_>>();
        let captured = selected.lifetime_parameters.len();
        let original = program
            .expression_table
            .static_machine_application(header.binding_application)
            .clone();
        let bindings = program
            .expression_table
            .static_machine_application_bindings(header.binding_application)
            .iter()
            .filter(|binding| parameters.contains(&binding.parameter))
            .cloned()
            .collect::<Vec<_>>();
        let lifetimes = program
            .expression_table
            .static_machine_application_lifetimes(header.binding_application)
            .get(..captured)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "closed interface lost its original captured regions",
                )]
            })?
            .to_vec();
        for (_, member) in members {
            let (_, signature) = program.trait_machine_signature(member).ok_or_else(|| {
                vec![Diagnostic::error(
                    "closed interface lost an original member",
                )]
            })?;
            if !program
                .state_signature_type_parameters(signature)
                .is_empty()
                || !signature.lifetime_parameters.is_empty()
                || typed_trees::static_applications::original_requirement_is_complete(
                    program,
                    header.selected_interface,
                    member,
                )
            {
                continue;
            }
            let mut present = false;
            for (_, sibling) in
                program
                    .instantiated_requirement_signatures
                    .iter()
                    .filter(|(_, row)| {
                        row.selected_interface == header.selected_interface
                            && row.requirement_template == member
                    })
            {
                if typed_trees::static_applications::requirement_interface_application_identity(
                    program, sibling,
                )
                .map_err(|diagnostic| vec![diagnostic])?
                    == identity
                {
                    present = true;
                    break;
                }
            }
            if present {
                continue;
            }
            // The payload retains the original source and lexical capture.
            // Only the explicitly selected interface's member coordinate
            // changes; no executable source occurrence or provider is added.
            let mut payload = program.clone();
            let (_, signature) = payload.trait_machine_signature(member).ok_or_else(|| {
                vec![Diagnostic::error(
                    "closed interface lost an original member",
                )]
            })?;
            let name = signature.name.clone();
            let application =
                payload
                    .expression_table
                    .retain_static_machine_application(member, name, &[]);
            let tuple = payload
                .expression_table
                .static_machine_application_mut(application);
            tuple.binding_declaration = member;
            tuple.selected_interface = header.selected_interface;
            tuple.lexical_owner = original.lexical_owner;
            tuple.captured_lexical_scope = original.captured_lexical_scope;
            payload
                .expression_table
                .set_static_machine_application_bindings(application, &bindings);
            payload
                .expression_table
                .set_static_machine_application_lifetimes(application, &lifetimes);
            super::demanded_applications::instantiate_one(
                program,
                &StaticApplicationDemand {
                    declaration: member,
                    owner: std::sync::Arc::new(payload.into()),
                    application,
                },
            )?;
        }
    }
    Ok(())
}

/// Token calls demand the same original public header as named requirement
/// calls. Runtime operands propose static substitutions through the ordinary
/// inference owner; they never select a realization or contribute body facts.
pub(crate) fn retain_requirement_operand_application(
    program: &mut TypedTrees,
    requirement: SymbolHandle,
    lexical_owner: SymbolHandle,
    operand_types: &[Option<typed_trees::types::TypeReferenceHandle>],
) -> Result<typed_trees::static_applications::StaticMachineApplicationHandle, Diagnostic> {
    let application = retain_requirement_application(
        program,
        requirement,
        lexical_owner,
        Default::default(),
        &[],
        operand_types,
    )?;
    if !application.is_valid() {
        let declaration =
            typed_trees::static_applications::StaticApplicationDeclaration::for_symbol(
                program,
                requirement,
            )
            .map_err(Diagnostic::error)?;
        if !declaration.parameters().is_empty() || !declaration.conformance_bounds().is_empty() {
            return Err(Diagnostic::error(
                "selected boundary token has no complete coherent static application",
            ));
        }
    }
    Ok(application)
}

/// Named calls and token calls derive the same original public tuple. Explicit
/// operands stay fixed; only omitted operands receive structural proposals.
/// This retains a declaration application, never a selected implementation.
/// Zero means no application was derived in this round: an ordinary inner
/// specialization or endpoint fold may still supply a missing operand. Source
/// admission continues to own an incomplete or incompatible original call.
pub(super) fn retain_requirement_application(
    program: &mut TypedTrees,
    requirement: SymbolHandle,
    lexical_owner: SymbolHandle,
    original_application: typed_trees::static_applications::StaticMachineApplicationHandle,
    arguments: &[typed_trees::expression::StaticMachineArgument],
    operand_types: &[Option<typed_trees::types::TypeReferenceHandle>],
) -> Result<typed_trees::static_applications::StaticMachineApplicationHandle, Diagnostic> {
    let (definition, signature) = program
        .trait_machine_signature(requirement)
        .ok_or_else(|| Diagnostic::error("public requirement call lost its original signature"))?;
    let selected_interface = program
        .expression_table
        .static_machine_application(original_application)
        .selected_interface;
    let selected_interface = if selected_interface.is_valid() {
        selected_interface
    } else {
        definition.symbol
    };
    let declaration =
        typed_trees::static_applications::StaticApplicationDeclaration::for_selected_interface(
            program,
            requirement,
            selected_interface,
        )
        .map_err(Diagnostic::error)?;
    let parameters = declaration
        .parameters()
        .iter()
        .map(|parameter| (**parameter).clone())
        .collect::<Vec<_>>();
    let conformance_bounds = declaration.conformance_bounds().to_vec();
    if !definition.lifetime_parameters.is_empty() && !original_application.is_valid() {
        return Err(Diagnostic::error(
            "public requirement call has no retained captured region application",
        ));
    }
    if parameters.is_empty() && conformance_bounds.is_empty() {
        // The original zero-static telescope is already a complete public
        // signature. It needs neither a generated header nor a snapshot.
        return Ok(Default::default());
    }
    let signature = signature.clone();
    let name = signature.name.clone();
    let mut arguments = if original_application.is_valid() {
        program
            .expression_table
            .static_machine_application_arguments(original_application)
            .to_vec()
    } else {
        arguments.to_vec()
    };
    for (parameter, argument) in parameters.iter().zip(&mut arguments) {
        if matches!(parameter.kind, TypeParameterKind::Type)
            && !argument.type_reference.is_valid()
            && argument.symbol.is_valid()
            && matches!(
                program.symbols.get(argument.symbol).kind,
                symbols::SymbolKind::BuiltinType
                    | symbols::SymbolKind::Data
                    | symbols::SymbolKind::TypeParameter
            )
        {
            argument.type_reference =
                super::application_arguments::static_argument_type_reference(program, argument)
                    .unwrap_or_default();
        }
    }
    let previous = program
        .expression_table
        .static_machine_application_bindings(original_application)
        .to_vec();
    let Some(explicit) = validation::static_arguments::derive_explicit_static_binding_prefix(
        program,
        &parameters,
        &conformance_bounds,
        &arguments,
    ) else {
        return Ok(Default::default());
    };
    let type_parameters = parameters
        .iter()
        .filter(|parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Type
                    | TypeParameterKind::Machine { .. }
                    | TypeParameterKind::Implementation
            )
        })
        .map(|parameter| (parameter.symbol, parameter.name.as_str().to_owned()))
        .collect::<Vec<_>>();
    let const_parameters = parameters
        .iter()
        .filter_map(|parameter| match parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => Some((
                parameter.symbol,
                parameter.name.as_str().to_owned(),
                type_reference,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut type_proposals = Vec::new();
    let mut const_proposals = Vec::new();
    for (position, (parameter, actual)) in program
        .state_signature_parameters(&signature)
        .iter()
        .zip(operand_types)
        .enumerate()
    {
        let Some(actual) = actual else {
            continue;
        };
        let (actual, expected) = typed_trees::operator::operator_operand_types(
            program,
            *actual,
            parameter.type_reference,
            position == 0
                && matches!(
                    signature.spelling,
                    Some(
                        language_core::OperatorSpelling::Index
                            | language_core::OperatorSpelling::Range
                    )
                ),
        );
        super::selection::infer_static_bindings(
            program,
            expected,
            actual,
            &type_parameters,
            &const_parameters,
            None,
            0,
            &mut type_proposals,
            &mut const_proposals,
        );
    }
    let mut bindings = Vec::new();
    for parameter in &parameters {
        if let Some(binding) = explicit
            .iter()
            .find(|binding| binding.parameter == parameter.symbol)
        {
            bindings.push(binding.clone());
            continue;
        }
        let (ordinal, proposals) = match parameter.kind {
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => (
                const_parameters
                    .iter()
                    .position(|(symbol, _, _)| *symbol == parameter.symbol),
                &const_proposals,
            ),
            _ => (
                type_parameters
                    .iter()
                    .position(|(symbol, _)| *symbol == parameter.symbol),
                &type_proposals,
            ),
        };
        let proposals = proposals
            .iter()
            .filter(|(_, index, _)| Some(*index) == ordinal)
            .map(|(_, _, actual)| *actual)
            .collect::<Vec<_>>();
        if proposals.is_empty()
            && let Some(binding) = previous
                .iter()
                .find(|binding| binding.parameter == parameter.symbol)
        {
            bindings.push(binding.clone());
            continue;
        }
        let Some(actual) = super::selection::coherent_type_proposal(program, &proposals) else {
            return Ok(Default::default());
        };
        let value = match parameter.kind {
            TypeParameterKind::Type => StaticMachineBindingValue::Type(actual),
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                StaticMachineBindingValue::Const(actual)
            }
            TypeParameterKind::Machine { .. } => StaticMachineBindingValue::Machine(
                super::application_arguments::static_argument_from_type_reference(program, actual)
                    .ok_or_else(|| {
                        Diagnostic::error("public requirement call lost its machine application")
                    })?,
            ),
            TypeParameterKind::Implementation => StaticMachineBindingValue::Conformance(
                super::application_arguments::static_argument_from_type_reference(program, actual)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "public requirement call lost its implementation application",
                        )
                    })?,
            ),
            TypeParameterKind::Proposition { .. } => {
                return Err(Diagnostic::error(
                    "public requirement call has no retained proposition application",
                ));
            }
        };
        bindings.push(StaticMachineBinding {
            parameter: parameter.symbol,
            value,
        });
    }
    for bound in &conformance_bounds {
        if bindings
            .iter()
            .any(|binding| binding.parameter == bound.binder)
        {
            continue;
        }
        let Some(binding) = explicit
            .iter()
            .chain(&previous)
            .find(|binding| binding.parameter == bound.binder)
        else {
            return Ok(Default::default());
        };
        bindings.push(binding.clone());
    }
    for binding in &bindings {
        if let StaticMachineBindingValue::Const(reference) = &binding.value {
            super::application_arguments::materialize_const_type_argument(program, *reference);
        }
    }
    // A raw inner declaration result can still contain a foreign const
    // binder. Its portable index identity does not establish availability in
    // this caller. Keep that readiness judgment separate from tuple identity;
    // the ordinary specialization round supplies the completed result later.
    let identity_owner = if original_application.is_valid() {
        program
            .expression_table
            .static_machine_application(original_application)
            .type_identity_owner(program)
    } else {
        Some(typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(lexical_owner))
    };
    let Some(identity_owner) = identity_owner else {
        return Err(Diagnostic::error(
            "public requirement call lost its original lexical capture",
        ));
    };
    if !super::application_arguments::bindings_are_in_scope(program, identity_owner, &bindings) {
        return Ok(Default::default());
    }
    if typed_trees::static_applications::static_binding_identity(
        program,
        selected_interface,
        &bindings,
        identity_owner,
        program
            .expression_table
            .static_machine_application_lifetimes(original_application)
            .iter()
            .map(|binding| &binding.argument),
    )
    .is_err()
    {
        return Ok(Default::default());
    }
    let application = if original_application.is_valid() {
        original_application
    } else {
        program
            .expression_table
            .retain_static_machine_application(requirement, name, &arguments)
    };
    let row = program
        .expression_table
        .static_machine_application_mut(application);
    row.binding_declaration = requirement;
    row.selected_interface = selected_interface;
    row.lexical_owner = lexical_owner;
    program
        .expression_table
        .set_static_machine_application_bindings(application, &bindings);
    // Static operand proposals do not supply the receiver's captured regions.
    // A public checking view can retain a universally quantified invocation
    // tail, but its enclosing application must already have its complete
    // captured correspondence. Leave unfinished source tuples to the ordinary
    // application producer rather than publishing a partially scoped contract.
    let declaration =
        typed_trees::static_applications::StaticApplicationDeclaration::for_application(
            program,
            program
                .expression_table
                .static_machine_application(application),
        )
        .map_err(Diagnostic::error)?;
    if declaration.bind_lifetimes(program, application).is_err()
        && declaration
            .bind_captured_lifetimes(program, application)
            .is_err()
    {
        return Ok(Default::default());
    }
    Ok(application)
}

/// Pass-local scratch. All handles belong to the live owner before its cohort
/// freezes; no borrowed declaration or second compiler object is retained.
pub(super) struct PreparedPublicContract {
    requirement: SymbolHandle,
    scope: typed_trees::trait_definition::AppliedTraitScopePayload,
    lifetimes: Vec<(
        typed_trees::name::Identifier,
        typed_trees::static_applications::StaticMachineLifetimeArgument,
    )>,
}

/// Requires, normal-return guarantees and result inference consume one complete
/// original applied scope. A symbolic caller tuple supplies a checking view,
/// independently of concrete header or provider body production.
pub(super) fn prepare_public_contract(
    program: &mut TypedTrees,
    requirement: SymbolHandle,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
) -> Result<PreparedPublicContract, Diagnostic> {
    let selected_interface = program
        .expression_table
        .static_machine_application(application)
        .selected_interface;
    let bindings = program
        .expression_table
        .static_machine_application_bindings(application)
        .to_vec();
    let (_, signature) = program
        .trait_machine_signature(requirement)
        .ok_or_else(|| Diagnostic::error("public result lost its original requirement"))?;
    let selected = program
        .trait_definition_by_symbol(selected_interface)
        .ok_or_else(|| Diagnostic::error("public result lost its selected interface"))?;
    let parameters = program
        .trait_type_parameters(selected)
        .iter()
        .chain(program.state_signature_type_parameters(signature))
        .map(|parameter| parameter.symbol)
        .collect::<Vec<_>>();
    // Independent conformance evidence stays in the original binding roster.
    // Declared static operands supply simultaneous type/value substitutions;
    // the complete binding roster also rewrites selected calls in the facts.
    let mut references = Vec::new();
    for binding in bindings
        .iter()
        .filter(|binding| parameters.contains(&binding.parameter))
    {
        let reference = match &binding.value {
            StaticMachineBindingValue::Type(reference)
            | StaticMachineBindingValue::Const(reference) => *reference,
            StaticMachineBindingValue::Machine(argument)
            | StaticMachineBindingValue::Conformance(argument) => {
                super::application_arguments::static_argument_type_reference(program, argument)
                    .ok_or_else(|| {
                        Diagnostic::error("public result operand has no original typed graph")
                    })?
            }
            StaticMachineBindingValue::RuntimeValue { .. } => {
                return Err(Diagnostic::error(
                    "public result cannot capture a runtime static subject",
                ));
            }
        };
        references.push((binding.parameter, reference));
    }
    let (declaring, signature) = program
        .trait_machine_signature(requirement)
        .ok_or_else(|| Diagnostic::error("public result lost its original requirement"))?;
    let selected = program
        .trait_definition_by_symbol(selected_interface)
        .ok_or_else(|| Diagnostic::error("public result lost its selected interface"))?;
    let arguments = program
        .trait_type_parameters(selected)
        .iter()
        .map(|parameter| {
            references
                .iter()
                .find(|(symbol, _)| *symbol == parameter.symbol)
                .map(|(_, reference)| *reference)
                .ok_or_else(|| {
                    Diagnostic::error("public result lost its original interface operand")
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if std::env::var_os("OMEGA_MAIN21_SCOPE_TRACE").is_some() {
        let row = program
            .expression_table
            .static_machine_application(application);
        eprintln!(
            "main21-public-scope requirement={requirement:?} selected={selected_interface:?} application={application:?} lexical={:?} captured={:?} formal_regions={:?} regions={:?} arguments={arguments:?}",
            row.lexical_owner,
            row.type_identity_owner(program).map(|owner| match owner {
                typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(symbol) => symbol,
                typed_trees::type_identity::AppliedTypeIdentityOwner::Captured(capture) =>
                    capture.symbol(),
            }),
            selected.lifetime_parameters,
            program
                .expression_table
                .static_machine_application_lifetimes(application)
        );
    }
    let root =
        AppliedTraitScope::from_static_application(program, selected, application, &arguments)
            .ok_or_else(|| Diagnostic::error("public result lost its complete selected scope"))?;
    let region_ordinals = root
        .declaring_lifetime_ordinals(program, declaring.symbol)
        .ok_or_else(|| Diagnostic::error("public result lost its exact inherited region map"))?;
    let mut scope = root
        .declaring_scope(program, declaring.symbol)
        .ok_or_else(|| Diagnostic::error("public result lost its exact declaring scope"))?;
    let member_arguments = program
        .state_signature_type_parameters(signature)
        .iter()
        .map(|parameter| {
            references
                .iter()
                .find(|(symbol, _)| *symbol == parameter.symbol)
                .copied()
                .ok_or_else(|| {
                    Diagnostic::error("public result lost its original invocation operand")
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let lifetimes = super::requirement_scopes::project_lifetimes(
        program,
        application,
        selected,
        scope.declaration,
        &region_ordinals,
    )?
    .into_iter()
    .map(|binding| (binding.parameter, binding.argument))
    .collect::<Vec<_>>();
    let member_lifetimes = lifetimes
        .iter()
        .skip(scope.declaration.lifetime_parameters.len())
        .map(|(_, actual)| actual.clone())
        .collect::<Vec<_>>();
    scope
        .extend_member(program, signature, &member_arguments, &member_lifetimes)
        .ok_or_else(|| Diagnostic::error("public result lost its complete member scope"))?;
    Ok(PreparedPublicContract {
        requirement,
        scope: scope.retain_payload(program),
        lifetimes,
    })
}

/// The cohort snapshot is issued once after every application's static operands,
/// bindings and regions have been prepared. Appends cannot alter its inputs.
pub(super) fn copy_public_contract(
    source: &TypedTrees,
    program: &mut TypedTrees,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
    prepared: &PreparedPublicContract,
) -> Result<typed_trees::types::TypeReferenceHandle, Diagnostic> {
    let scope = prepared
        .scope
        .bind(source)
        .ok_or_else(|| Diagnostic::error("public result lost its original cohort scope owner"))?;
    let (_, signature) = source
        .trait_machine_signature(prepared.requirement)
        .ok_or_else(|| Diagnostic::error("public contract lost its original cohort requirement"))?;
    let bindings = source
        .expression_table
        .static_machine_application_bindings(application);
    let mut copied = super::signature_cloning::copy_public_contract(
        source, program, signature, &scope, bindings,
    )?;
    program
        .apply_public_contract_regions(
            &mut copied.signature,
            copied.contract_fact_copies,
            &prepared.lifetimes,
        )
        .map_err(Diagnostic::error)?;
    let result = copied.signature.return_type;
    program
        .expression_table
        .static_machine_application_mut(application)
        .public_result_type = result;
    program
        .retain_public_requirement_contract(
            application,
            copied.signature,
            copied.contract_fact_copies,
        )
        .map_err(Diagnostic::error)?;
    Ok(result)
}

pub(super) fn produce(
    program: &mut TypedTrees,
    payload: &TypedTrees,
    demand: &StaticApplicationDemand,
    bindings: &[StaticMachineBinding],
) -> Result<SymbolHandle, Diagnostic> {
    // Product operand handles index the retained payload owner. Copy once into
    // the original source namespace before any public declaration elaboration.
    let mut source = program.clone();
    let (bindings, application) = super::demanded_applications::copy_application_payload(
        &mut source,
        payload,
        demand.declaration,
        demand.application,
        bindings,
    )?;
    let (definition, signature) = source
        .trait_machine_signature(demand.declaration)
        .ok_or_else(|| {
            Diagnostic::error("demanded application has no original public requirement")
        })?;
    let declaring_trait = definition.symbol;
    let selected_interface = source
        .expression_table
        .static_machine_application(application)
        .selected_interface;
    let selected_interface = if selected_interface.is_valid() {
        selected_interface
    } else {
        declaring_trait
    };
    let selected = source
        .trait_definition_by_symbol(selected_interface)
        .ok_or_else(|| Diagnostic::error("demanded requirement lost its selected interface"))?;
    let trait_parameters = source.trait_type_parameters(selected);
    let member_parameters = source.state_signature_type_parameters(signature);
    let parameters = trait_parameters
        .iter()
        .chain(member_parameters)
        .collect::<Vec<_>>();
    if parameters
        .iter()
        .any(|parameter| !bindings.iter().any(|row| row.parameter == parameter.symbol))
        || selected
            .conformance_bounds
            .iter()
            .any(|bound| !bindings.iter().any(|row| row.parameter == bound.binder))
        || bindings.iter().any(|row| {
            !parameters
                .iter()
                .any(|parameter| parameter.symbol == row.parameter)
                && !selected
                    .conformance_bounds
                    .iter()
                    .any(|bound| bound.binder == row.parameter)
        })
    {
        return Err(Diagnostic::error(
            "demanded requirement lost its complete static binder roster",
        ));
    }
    if parameters.iter().any(|parameter| {
        let binding = bindings
            .iter()
            .find(|row| row.parameter == parameter.symbol);
        !matches!(
            (&parameter.kind, binding.map(|row| &row.value)),
            (
                TypeParameterKind::Type,
                Some(StaticMachineBindingValue::Type(_))
            ) | (
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. },
                Some(StaticMachineBindingValue::Const(_))
            ) | (
                TypeParameterKind::Machine { .. },
                Some(StaticMachineBindingValue::Machine(_))
            ) | (
                TypeParameterKind::Implementation,
                Some(StaticMachineBindingValue::Conformance(_))
            )
        )
    }) {
        return Err(Diagnostic::error(
            "demanded requirement changed its declared static parameter category",
        ));
    }
    let lifetimes = source
        .expression_table
        .static_machine_application_lifetimes(application);
    let captured_count = selected.lifetime_parameters.len();
    if !(lifetimes.len() == captured_count
        || lifetimes.len() == captured_count + signature.lifetime_parameters.len())
        || selected
            .lifetime_parameters
            .iter()
            .chain(&signature.lifetime_parameters)
            .zip(lifetimes)
            .any(|(parameter, row)| parameter != &row.parameter)
    {
        return Err(Diagnostic::error(
            "demanded requirement lost its captured/invocation region correspondence",
        ));
    }
    // Every original static category can be represented in the existing typed
    // application graph. Build the receiving scope from those exact operands,
    // then let the shared inherited traversal project its declaring telescope.
    let trait_symbols = trait_parameters
        .iter()
        .map(|parameter| parameter.symbol)
        .collect::<Vec<_>>();
    let member_symbols = member_parameters
        .iter()
        .map(|parameter| parameter.symbol)
        .collect::<Vec<_>>();
    let mut references = Vec::new();
    for parameter in trait_symbols.iter().chain(&member_symbols) {
        let binding = bindings
            .iter()
            .find(|row| row.parameter == *parameter)
            .ok_or_else(|| {
                Diagnostic::error("demanded interface lost an original static binder")
            })?;
        let reference = match &binding.value {
            StaticMachineBindingValue::Type(reference)
            | StaticMachineBindingValue::Const(reference) => *reference,
            StaticMachineBindingValue::Machine(argument)
            | StaticMachineBindingValue::Conformance(argument) => {
                super::application_arguments::static_argument_type_reference(&mut source, argument)
                    .ok_or_else(|| {
                        Diagnostic::error("demanded interface operand has no original typed graph")
                    })?
            }
            StaticMachineBindingValue::RuntimeValue { .. } => {
                return Err(Diagnostic::error(
                    "demanded interface cannot capture a runtime static subject",
                ));
            }
        };
        references.push((binding.parameter, reference));
    }
    let find_reference = |parameter| {
        references
            .iter()
            .find(|(symbol, _)| *symbol == parameter)
            .map(|(_, reference)| *reference)
            .ok_or_else(|| Diagnostic::error("demanded interface lost an original static binder"))
    };
    let arguments = trait_symbols
        .iter()
        .map(|parameter| find_reference(*parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let scopes = super::requirement_scopes::project(
        &mut source,
        application,
        declaring_trait,
        selected_interface,
        &arguments,
        &bindings,
    )?;
    let declaring_application = scopes.declaring_application;
    let declaring_bindings = source
        .expression_table
        .static_machine_application_bindings(declaring_application)
        .to_vec();
    let selected = source
        .trait_definition_by_symbol(selected_interface)
        .ok_or_else(|| {
            Diagnostic::error("demanded requirement lost its original selected interface")
        })?;
    let selected_scope =
        AppliedTraitScope::from_static_application(&source, selected, application, &arguments)
            .ok_or_else(|| {
                Diagnostic::error("demanded requirement lost its complete interface scope")
            })?;
    let mut scope = selected_scope
        .declaring_scope(&source, declaring_trait)
        .ok_or_else(|| {
            Diagnostic::error("inherited requirement has no one exact declaring application")
        })?;
    // The member's operands belong to the original caller, independently of
    // each simultaneous inherited trait telescope and its matching region row.
    let member_arguments = member_symbols
        .into_iter()
        .map(|parameter| Ok((parameter, find_reference(parameter)?)))
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let (_, signature) = source
        .trait_machine_signature(demand.declaration)
        .ok_or_else(|| Diagnostic::error("demanded requirement lost its original member"))?;
    let lifetimes = source
        .expression_table
        .static_machine_application_lifetimes(declaring_application)
        .iter()
        .skip(scope.declaration.lifetime_parameters.len())
        .map(|binding| binding.argument.clone())
        .collect::<Vec<_>>();
    scope
        .extend_member(&source, signature, &member_arguments, &lifetimes)
        .ok_or_else(|| Diagnostic::error("demanded requirement lost its complete member scope"))?;
    let ordinal = program
        .instantiated_requirement_signatures
        .iter()
        .filter(|(_, row)| row.requirement_template == demand.declaration)
        .count();
    let copied_header = super::signature_cloning::copy_applied_signature(
        &source,
        program,
        signature,
        &scope,
        &declaring_bindings,
        &[],
        &format!("{}$application{ordinal}", signature.name),
    )?;
    let contract_fact_copies = copied_header.contract_fact_copies;
    let mut copied = copied_header.signature;
    // Static operands have been consumed. The original binder declarations
    // remain in the source arena for admission/provenance; only this header's
    // live open span closes, just as ordinary machine specialization does.
    copied.type_parameters = HandleSpan::empty();
    let instance = copied.symbol;
    let signature = program.trait_machine_signatures.insert(copied);
    // Retain the exact copied tuple in the receiving program's arena. This
    // relocation does not reinterpret its captured caller scope as a Product
    // declaration, or instantiate the member's universal invocation lifetime.
    let original_application = application;
    let (_, application) = super::demanded_applications::copy_application_payload(
        program,
        &source,
        demand.declaration,
        application,
        &bindings,
    )?;
    let declaring_application = if declaring_application == original_application {
        application
    } else {
        super::demanded_applications::copy_application_payload(
            program,
            &source,
            demand.declaration,
            declaring_application,
            &declaring_bindings,
        )?
        .1
    };
    let mut interfaces = Vec::new();
    for mut interface in scopes.applications {
        interface.binding_application = if !interface.binding_application.is_valid() {
            interface.binding_application
        } else if interface.binding_application == original_application {
            application
        } else if interface.interface == declaring_trait {
            declaring_application
        } else {
            let bindings = source
                .expression_table
                .static_machine_application_bindings(interface.binding_application);
            super::demanded_applications::copy_application_payload(
                program,
                &source,
                demand.declaration,
                interface.binding_application,
                bindings,
            )?
            .1
        };
        interfaces.push(interface);
    }
    let interface_applications = program
        .requirement_interface_applications
        .insert_many(interfaces);
    program
        .instantiated_requirement_signatures
        .append(InstantiatedRequirementSignature {
            declaring_trait,
            selected_interface,
            requirement_template: demand.declaration,
            signature,
            contract_fact_copies,
            binding_application: application,
            declaring_binding_application: declaring_application,
            interface_applications,
            captured_lifetime_count: u32::try_from(captured_count).map_err(|_| {
                Diagnostic::error("demanded requirement captured region roster is too large")
            })?,
        });
    Ok(instance)
}
