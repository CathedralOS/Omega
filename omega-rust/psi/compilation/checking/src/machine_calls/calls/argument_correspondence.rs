//! Selected calls have a public contract and a separately realized body.
//! Admission relates authored ordinary formals in order; clone provenance
//! carries that relation through capture and invocation. Runtime Value binders
//! add executable parameters, not authored public formals. Their exact actuals
//! come from this occurrence's retained application, never an instance's erased
//! runtime tuple. This join supplies carriers and identity, not proof or custody.

use std::borrow::Cow;

use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::static_applications::StaticMachineBindingValue;
use typed_trees::types::TypeReferenceHandle;

use super::{CallSite, call_site_argument_expressions, requirement_contract_signature};

pub struct SelectedCallArgument<'program> {
    pub executable_parameter: &'program StateParameter,
    pub logical_parameter: SymbolHandle,
    pub logical_type_reference: TypeReferenceHandle,
    pub public_position: Option<usize>,
    pub expression: ExpressionHandle,
}

/// Parameters for source admission, in executable argument order. Ordinary
/// public formals keep the applied public carriers; realized Value operands
/// retain their own declared carrier and cannot borrow another formal's facts.
pub(crate) fn executable_parameters<'program>(
    program: &'program dyn crate::CheckingInput,
    caller: SymbolHandle,
    site: &CallSite<'program>,
) -> Result<Cow<'program, [StateParameter]>, &'static str> {
    if site.static_requirement_dispatch().is_none() {
        return site
            .parameters(program, caller)
            .map(Cow::Borrowed)
            .ok_or("call has no retained parameter telescope");
    }
    let arguments = selected_call_arguments(program, caller, site)?;
    let public = site
        .parameters(program, caller)
        .ok_or("selected call has no applied public telescope")?;
    Ok(Cow::Owned(
        arguments
            .into_iter()
            .map(|argument| {
                if let Some(position) = argument.public_position {
                    public[position].clone()
                } else {
                    let mut parameter = argument.executable_parameter.clone();
                    parameter.symbol = argument.logical_parameter;
                    parameter.type_reference = argument.logical_type_reference;
                    parameter
                }
            })
            .collect(),
    ))
}

/// Compose one admitted selected application with its exact executable
/// parameters and actual expressions. Equal arity alone is not a mapping.
pub fn selected_call_arguments<'program>(
    program: &'program dyn crate::CheckingInput,
    caller: SymbolHandle,
    site: &CallSite<'program>,
) -> Result<Vec<SelectedCallArgument<'program>>, &'static str> {
    let dispatch = site
        .static_requirement_dispatch()
        .ok_or("argument correspondence requires a retained selected application")?;
    let (_, applied) = requirement_contract_signature(program, caller, site)
        .ok_or("selected call has no admitted public application")?;
    let logical = program.state_signature_parameters(applied);
    let state = program
        .state_by_symbol(site.target_symbol(program))
        .ok_or("selected call has no executable entry")?;
    let parameters = program.state_parameters(state);
    let actuals = call_site_argument_expressions(program, site);
    if parameters.len() != actuals.len() {
        return Err("selected call has different executable formal and actual counts");
    }
    let applications = program.selected_conformance_applications(
        caller,
        dispatch.selection_application,
        dispatch.requirement,
        dispatch.application_report_fingerprint,
        dispatch.application_commitment,
    );
    let [application] = applications.as_slice() else {
        return Err("selected call has no unique supplying application");
    };
    let conformance = program
        .conformance_by_symbol(application.declaration)
        .ok_or("selected call lost its supplying declaration")?;
    let capture_parameters = program.conformance_type_parameters(conformance);
    if capture_parameters.len() != application.arguments.len() {
        return Err("selected call lost its supplying argument telescope");
    }
    if !application.callable_application.is_valid() {
        return Err("selected call lost its retained supplying telescope");
    }
    let capture = program
        .expression_table
        .static_machine_application(application.callable_application);
    if capture.binding_declaration != application.declaration {
        return Err("selected call supplying telescope belongs to another declaration");
    }
    let bindings = program
        .expression_table
        .static_machine_application_bindings(application.callable_application);
    if bindings.len() != capture_parameters.len() {
        return Err("selected call lost its complete supplying bindings");
    }
    let mut substitutions = Vec::new();
    for (parameter, binding) in capture_parameters.iter().zip(bindings) {
        if parameter.symbol != binding.parameter {
            return Err("selected call changed its supplying binder order");
        }
        match (&parameter.kind, &binding.value) {
            (
                typed_trees::data::TypeParameterKind::Type,
                StaticMachineBindingValue::Type(reference),
            )
            | (
                typed_trees::data::TypeParameterKind::Const { .. }
                | typed_trees::data::TypeParameterKind::Value { .. },
                StaticMachineBindingValue::Const(reference),
            ) => {
                substitutions.push((parameter.symbol, *reference));
            }
            (
                typed_trees::data::TypeParameterKind::Machine { .. },
                StaticMachineBindingValue::Machine(_),
            ) => {}
            _ => return Err("selected call changed its supplying operand kind"),
        }
    }
    let lineage = program
        .realization_entry_lineage(state.symbol, application)
        .ok_or("selected call lost its capture or invocation lineage")?;
    if lineage.last().copied() != Some((dispatch.realization_machine, dispatch.realization_state)) {
        return Err("selected call belongs to another authored realization");
    }
    let source = program
        .state_by_symbol(dispatch.realization_state)
        .ok_or("selected call has no authored realization entry")?;
    let source_parameters = program.state_parameters(source);
    let (_, declared) = program
        .trait_machine_signature(dispatch.requirement)
        .filter(|(owner, _)| owner.symbol == dispatch.declaring_trait)
        .ok_or("selected call has no exact authored requirement")?;
    let declared_parameters = program.state_signature_parameters(declared);
    if source_parameters.len() != declared_parameters.len()
        || logical.len() != declared_parameters.len()
    {
        return Err("selected call lost its admitted authored parameter correspondence");
    }
    let authored_application = match site {
        CallSite::Expression { call, .. } => call.authored_machine_application,
        CallSite::Statement(call) => call.authored_machine_application,
        CallSite::TransitionNamed { .. } => Default::default(),
    };
    let runtime_bindings = if authored_application.is_valid() {
        if program
            .expression_table
            .static_machine_application(authored_application)
            .lexical_owner
            != caller
        {
            return Err("selected Value application belongs to another lexical owner");
        }
        program
            .expression_table
            .static_machine_application_bindings(authored_application)
            .iter()
            .filter(|binding| {
                matches!(
                    binding.value,
                    StaticMachineBindingValue::RuntimeValue { .. }
                )
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if parameters.len() != logical.len() + runtime_bindings.len() {
        return Err("selected call lost its complete runtime Value argument roster");
    }
    let mut mapped = Vec::with_capacity(parameters.len());
    let mut realized_binders = Vec::with_capacity(runtime_bindings.len());
    for (position, (parameter, expression)) in parameters.iter().zip(actuals).enumerate() {
        let origin = formal_origin(program, &lineage, parameter.symbol)?;
        let (logical_parameter, logical_type_reference, public_position) = if position
            < logical.len()
        {
            if source_parameters[position].symbol != origin {
                return Err("selected call changed its authored ordinary parameter order");
            }
            let public = &logical[position];
            if program.applied_signature_parameter(
                declared,
                applied,
                declared_parameters[position].symbol,
            ) != Some(public.symbol)
            {
                return Err("selected call lost its applied public formal identity");
            }
            (public.symbol, public.type_reference, Some(position))
        } else {
            let binding = runtime_bindings[position - logical.len()];
            if binding.parameter != origin || realized_binders.contains(&origin) {
                return Err("selected call changed or duplicated its realized Value binder order");
            }
            realized_binders.push(origin);
            let StaticMachineBindingValue::RuntimeValue {
                type_reference,
                subject,
                expression: bound_expression,
            } = binding.value
            else {
                return Err("selected Value argument lost its runtime source binding");
            };
            let same_source = if bound_expression.is_valid() {
                bound_expression == *expression
            } else {
                subject.is_valid()
                    && crate::canonical_place_from_expression(program, *expression).is_some_and(
                        |place| {
                            place.root.as_symbol() == Some(subject) && place.segments.is_empty()
                        },
                    )
            };
            if !same_source {
                return Err("selected Value argument belongs to another source expression");
            }
            let source_machine = program
                .machine_by_symbol(dispatch.realization_machine)
                .ok_or("selected Value argument lost its authored machine")?;
            let source_binders = program.machine_type_parameters(source_machine);
            let declared_binders = program.state_signature_type_parameters(declared);
            if source_binders.len() != declared_binders.len() {
                return Err("selected Value argument lost its admitted member telescope");
            }
            // The runtime application names the invocation template's binder.
            // Capturing a map freshens that independent telescope too; compose
            // those exact clone origins before using the authored ordinal.
            let authored_binder = authored_value_binder(program, &lineage, origin)?;
            let binder_position = source_binders
                .iter()
                .position(|binder| binder.symbol == authored_binder)
                .ok_or("selected Value argument is not an authored member binder")?;
            let logical_parameter = program
                .applied_signature_parameter(
                    declared,
                    applied,
                    declared_binders[binder_position].symbol,
                )
                .ok_or("selected Value argument lost its applied public binder")?;
            let public_binder = program
                .state_signature_type_parameters(applied)
                .iter()
                .find(|binder| binder.symbol == logical_parameter)
                .ok_or("selected Value argument has no applied public carrier")?;
            let typed_trees::data::TypeParameterKind::Value {
                type_reference: public_type,
            } = public_binder.kind
            else {
                return Err(
                    "selected runtime argument does not correspond to a public Value binder",
                );
            };
            if !crate::declared_carriers_match(program, type_reference, public_type) {
                return Err("selected runtime Value binding differs from its public carrier");
            }
            (logical_parameter, public_type, None)
        };
        if !crate::value_custody::places::declared_carriers_match_with_substitutions(
            program,
            parameter.type_reference,
            logical_type_reference,
            &substitutions,
        ) {
            return Err("selected executable carrier differs from its exact public application");
        }
        mapped.push(SelectedCallArgument {
            executable_parameter: parameter,
            logical_parameter,
            logical_type_reference,
            public_position,
            expression: *expression,
        });
    }
    Ok(mapped)
}

fn authored_value_binder(
    program: &dyn crate::CheckingInput,
    lineage: &[(SymbolHandle, SymbolHandle)],
    mut binder: SymbolHandle,
) -> Result<SymbolHandle, &'static str> {
    let owner = program.symbols.get(binder).parent;
    let first = lineage
        .iter()
        .position(|(machine, _)| *machine == owner)
        .ok_or("selected Value binder belongs to another invocation lineage")?;
    for frames in lineage[first..].windows(2) {
        if !program
            .captured_machine_applications
            .iter()
            .any(|(_, capture)| capture.instance == frames[0].0 && capture.template == frames[1].0)
        {
            return Err("selected Value binder has no exact capture frame");
        }
        let original = program.symbols.get(binder).generated_from;
        if !original.is_valid()
            || program.symbols.get(original).parent != frames[1].0
            || !program
                .data_type_parameter_by_symbol(original)
                .is_some_and(|parameter| {
                    matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Value { .. }
                    )
                })
        {
            return Err("selected Value binder lost its original captured declaration");
        }
        binder = original;
    }
    Ok(binder)
}

fn formal_origin(
    program: &dyn crate::CheckingInput,
    lineage: &[(SymbolHandle, SymbolHandle)],
    mut parameter: SymbolHandle,
) -> Result<SymbolHandle, &'static str> {
    for frames in lineage.windows(2) {
        let state = program
            .state_by_symbol(frames[0].1)
            .ok_or("selected formal lineage lost its current entry")?;
        let position = program
            .state_parameters(state)
            .iter()
            .position(|formal| formal.symbol == parameter)
            .ok_or("selected formal is absent from its current entry")?;
        let original = program.symbols.get(parameter).generated_from;
        let template = program
            .state_by_symbol(frames[1].1)
            .ok_or("selected formal lineage lost its template entry")?;
        if let Some(original_position) = program
            .state_parameters(template)
            .iter()
            .position(|formal| formal.symbol == original)
        {
            if position != original_position {
                return Err("selected formal clone changed its template's parameter order");
            }
            parameter = original;
            continue;
        }
        let realized = program.machine_specializations.iter().any(|application| {
            application.instance == frames[0].0
                && application.template == frames[1].0
                && program
                    .data_type_parameters
                    .span_or_empty(application.template_parameters)
                    .iter()
                    .any(|binder| {
                        binder.symbol == original
                            && matches!(
                                binder.kind,
                                typed_trees::data::TypeParameterKind::Value { .. }
                            )
                    })
        });
        if realized {
            return Ok(original);
        }
        return Err("selected formal clone does not belong to its exact template frame");
    }
    Ok(parameter)
}
