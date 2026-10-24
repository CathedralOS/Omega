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
            if !runtime_source_matches(program, subject, bound_expression, *expression) {
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

/// The retained header of one executable application, related to the contract
/// namespace its captured actuals use. This construction scratch authenticates
/// identity only: runtime parameters are never constant substitutions.
pub(crate) struct CallBindingCorrespondence {
    pub declaration: SymbolHandle,
    pub public_owner: SymbolHandle,
    /// Zero means that exact static binder was erased by realization; it
    /// supplies no receiving proof name. Runtime binders always have a name.
    pub parameters: Vec<(SymbolHandle, SymbolHandle)>,
    pub runtime_parameters: Vec<SymbolHandle>,
}

pub(crate) fn call_binding_correspondence(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    site: &CallSite<'_>,
) -> Result<CallBindingCorrespondence, &'static str> {
    let handle = match site {
        CallSite::Expression { call, .. } => call.authored_machine_application,
        CallSite::Statement(call) => call.authored_machine_application,
        CallSite::TransitionNamed { .. } => {
            return Err("an internal edge has no application header");
        }
    };
    if !handle.is_valid() {
        return Err("the call has no retained application header");
    }
    let application = program.expression_table.static_machine_application(handle);
    if application.lexical_owner != caller {
        return Err("the application header belongs to another caller");
    }
    let Some(declaration) = program.machine_by_symbol(application.binding_declaration) else {
        return requirement_binding_correspondence(program, caller, site, handle);
    };
    let header = program.machine_type_parameters(declaration);
    let retained = program
        .expression_table
        .static_machine_application_bindings(handle);
    let mut parameters = Vec::with_capacity(header.len());
    let mut runtime_parameters = Vec::new();
    let public_owner;
    if let Some(dispatch) = site.static_requirement_dispatch() {
        // This existing owner authenticates every executable actual, public
        // carrier, clone origin and retained runtime source before header
        // identities can be transported into that same public namespace.
        let arguments = selected_call_arguments(program, caller, site)?;
        let (_, public) = requirement_contract_signature(program, caller, site)
            .ok_or("the selected header lost its public application")?;
        let (_, declared) = program
            .trait_machine_signature(dispatch.requirement)
            .filter(|(owner, _)| owner.symbol == dispatch.declaring_trait)
            .ok_or("the selected header lost its authored requirement")?;
        let source = program
            .machine_by_symbol(dispatch.realization_machine)
            .ok_or("the selected header lost its authored realization")?;
        let source_header = program.machine_type_parameters(source);
        let declared_header = program.state_signature_type_parameters(declared);
        if source_header.len() != declared_header.len() || header.len() != source_header.len() {
            return Err("the selected header changed its declared telescope");
        }
        public_owner = public.symbol;
        let lineage = application_lineage(program, declaration.symbol, source.symbol)?;
        for (ordinal, parameter) in header.iter().enumerate() {
            let original = application_parameter_origin(program, &lineage, parameter.symbol)?;
            let position = source_header
                .iter()
                .position(|source| source.symbol == original)
                .ok_or("the selected header parameter has no authored origin")?;
            if position != ordinal {
                return Err("the selected header changed its authored parameter order");
            }
            let logical = program
                .applied_signature_parameter(declared, public, declared_header[position].symbol)
                .ok_or("the selected header parameter has no public binder")?;
            let public_parameter = program
                .state_signature_type_parameters(public)
                .iter()
                .find(|parameter| parameter.symbol == logical)
                .ok_or("the selected header parameter has no public declaration")?;
            if std::mem::discriminant(&parameter.kind)
                != std::mem::discriminant(&public_parameter.kind)
                || parameters.iter().any(|(_, previous)| *previous == logical)
            {
                return Err("the selected header changed or duplicated a public binder");
            }
            if retained.iter().any(|binding| {
                binding.parameter == parameter.symbol
                    && matches!(
                        binding.value,
                        StaticMachineBindingValue::RuntimeValue { .. }
                    )
            }) {
                if !arguments.iter().any(|argument| {
                    argument.public_position.is_none() && argument.logical_parameter == logical
                }) {
                    return Err("the runtime header binder has no authenticated executable actual");
                }
                runtime_parameters.push(parameter.symbol);
            }
            parameters.push((parameter.symbol, logical));
        }
    } else {
        let (machine, state) = crate::find_state(program, site.target_symbol(program))
            .ok_or("the application has no exact executable entry")?;
        public_owner = state.symbol;
        // A concrete realization preserves the source header through the
        // existing executable-template chain. Only its runtime Value binders
        // become ordinary trailing parameters; a static tuple adds no executable argument.
        let lineage = application_lineage(program, machine.symbol, declaration.symbol)?;
        let actuals = call_site_argument_expressions(program, site);
        let executable = program.state_parameters(state);
        let source_state = application_parameter_origin(program, &lineage, state.symbol)
            .ok()
            .and_then(|symbol| program.state_by_symbol(symbol))
            .ok_or("the executable entry lost its exact source state")?;
        if program
            .machine_holding_state(source_state.symbol)
            .is_none_or(|owner| owner.symbol != declaration.symbol)
        {
            return Err("the executable entry belongs to another source machine");
        }
        let ordinary = program.state_parameters(source_state);
        let runtime_header = header
            .iter()
            .filter(|parameter| {
                retained.iter().any(|binding| {
                    binding.parameter == parameter.symbol
                        && matches!(
                            binding.value,
                            StaticMachineBindingValue::RuntimeValue { .. }
                        )
                })
            })
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        if executable.len() != actuals.len()
            || executable.len() != ordinary.len() + runtime_header.len()
            || !executable
                .iter()
                .take(ordinary.len())
                .zip(ordinary)
                .all(|(actual, declared)| {
                    application_parameter_origin(program, &lineage, actual.symbol).ok()
                        == Some(declared.symbol)
                })
        {
            return Err("the application lost its ordered executable parameter correspondence");
        }
        let mut next_header_position = 0;
        for logical in program.machine_type_parameters(machine) {
            let original = application_parameter_origin(program, &lineage, logical.symbol)?;
            let position = header
                .iter()
                .position(|parameter| parameter.symbol == original)
                .ok_or("the realized header has no source parameter")?;
            if position < next_header_position
                || std::mem::discriminant(&logical.kind)
                    != std::mem::discriminant(&header[position].kind)
            {
                return Err("the realized header changed its source order or parameter kind");
            }
            next_header_position = position + 1;
        }
        for parameter in header {
            let mut public_parameters =
                program
                    .machine_type_parameters(machine)
                    .iter()
                    .filter(|logical| {
                        application_parameter_origin(program, &lineage, logical.symbol).ok()
                            == Some(parameter.symbol)
                    });
            let mut logical = public_parameters
                .next()
                .map_or_else(SymbolHandle::invalid, |parameter| parameter.symbol);
            if public_parameters.next().is_some() {
                return Err("the application duplicated a retained static header binder");
            }
            let mut rows = retained
                .iter()
                .filter(|binding| binding.parameter == parameter.symbol);
            let binding = rows
                .next()
                .ok_or("the application header has a missing binding")?;
            if rows.next().is_some() {
                return Err("the application header has duplicate bindings");
            }
            if let StaticMachineBindingValue::RuntimeValue {
                type_reference,
                subject,
                expression,
            } = binding.value
            {
                let position = runtime_header
                    .iter()
                    .position(|symbol| *symbol == parameter.symbol)
                    .ok_or("the runtime header binder has no retained ordinal")?;
                let index = ordinary.len() + position;
                let formal = &executable[index];
                let actual = actuals[index];
                if application_parameter_origin(program, &lineage, formal.symbol).ok()
                    != Some(parameter.symbol)
                    || formal.is_const
                    || formal.is_mutable
                    || !matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Value { .. }
                    )
                    || !crate::declared_carriers_match(
                        program,
                        type_reference,
                        formal.type_reference,
                    )
                    || !runtime_source_matches(program, subject, expression, actual)
                {
                    return Err(
                        "the runtime header binder has no exact captured executable argument",
                    );
                }
                logical = formal.symbol;
                runtime_parameters.push(parameter.symbol);
            }
            parameters.push((parameter.symbol, logical));
        }
    }
    Ok(CallBindingCorrespondence {
        declaration: declaration.symbol,
        public_owner,
        parameters,
        runtime_parameters,
    })
}

/// An open requirement has no executable instance or trailing Value operands.
/// The exact retained invocation view still transports its authored header.
/// Rejoin through the same applied-signature owner that prepared its public
/// contract; a same-sized telescope or a provider body cannot supply this map.
fn requirement_binding_correspondence(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    site: &CallSite<'_>,
    handle: typed_trees::static_applications::StaticMachineApplicationHandle,
) -> Result<CallBindingCorrespondence, &'static str> {
    if site.static_requirement_dispatch().is_some() {
        return Err("a realized requirement header has no authored machine declaration");
    }
    let application = program.expression_table.static_machine_application(handle);
    let (declaring_trait, declared) = program
        .trait_machine_signature(application.binding_declaration)
        .ok_or("the open application has no exact authored requirement header")?;
    let (selected_trait, public) = requirement_contract_signature(program, caller, site)
        .ok_or("the open application lost its exact public invocation view")?;
    let header = program.state_signature_type_parameters(declared);
    let logical = program.state_signature_type_parameters(public);
    let retained = program
        .expression_table
        .static_machine_application_bindings(handle);
    if declaring_trait.symbol != selected_trait.symbol
        || declared.symbol != public.symbol
        || header.len() != logical.len()
        || retained.len() != header.len()
        || retained.iter().any(|binding| {
            header
                .iter()
                .filter(|parameter| parameter.symbol == binding.parameter)
                .count()
                != 1
        })
    {
        return Err("the open application changed its declared public telescope");
    }
    let mut parameters = Vec::with_capacity(header.len());
    let mut runtime_parameters = Vec::new();
    for parameter in header {
        let selected = program
            .applied_signature_parameter(declared, public, parameter.symbol)
            .ok_or("the open application lost its applied header binder")?;
        let mut public_parameters = logical
            .iter()
            .filter(|parameter| parameter.symbol == selected);
        let (Some(public_parameter), None) = (public_parameters.next(), public_parameters.next())
        else {
            return Err("the open application duplicated its public header binder");
        };
        if std::mem::discriminant(&parameter.kind) != std::mem::discriminant(&public_parameter.kind)
            || parameters.iter().any(|(_, previous)| *previous == selected)
        {
            return Err("the open application changed its public binder kind or identity");
        }
        let mut bindings = retained
            .iter()
            .filter(|binding| binding.parameter == parameter.symbol);
        let (Some(binding), None) = (bindings.next(), bindings.next()) else {
            return Err("the open application has a missing or duplicate header binding");
        };
        if matches!(
            binding.value,
            StaticMachineBindingValue::RuntimeValue { .. }
        ) {
            if !matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Value { .. }
            ) {
                return Err("the open runtime operand does not belong to a Value binder");
            }
            immutable_runtime_parameter(program, caller, &binding.value)?;
            runtime_parameters.push(parameter.symbol);
        }
        parameters.push((parameter.symbol, selected));
    }
    Ok(CallBindingCorrespondence {
        declaration: declared.symbol,
        public_owner: public.symbol,
        parameters,
        runtime_parameters,
    })
}

/// A lexical Value binder is an immutable runtime subject, never a constant.
/// It needs no fresh read after this call's effects. Local/effectful actuals
/// instead require their captured executable operand; this query cannot
/// reconstruct that observation from a late source expression.
pub(crate) fn immutable_runtime_parameter(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    value: &StaticMachineBindingValue,
) -> Result<SymbolHandle, &'static str> {
    let StaticMachineBindingValue::RuntimeValue {
        type_reference,
        subject,
        expression,
    } = value
    else {
        return Err("the runtime header binding has another operand kind");
    };
    if !subject.is_valid() || expression.is_valid() {
        return Err("the runtime header actual needs a captured executable observation");
    }
    let machine = program
        .machine_by_symbol(caller)
        .ok_or("the runtime header actual has no lexical machine owner")?;
    let mut parameters = program
        .machine_type_parameters(machine)
        .iter()
        .filter(|parameter| parameter.symbol == *subject);
    let (Some(parameter), None) = (parameters.next(), parameters.next()) else {
        return Err("the runtime header actual has no unique lexical Value binder");
    };
    let typed_trees::data::TypeParameterKind::Value {
        type_reference: declared,
    } = parameter.kind
    else {
        return Err("the runtime header actual is not an immutable Value binder");
    };
    if !crate::declared_carriers_match(program, declared, *type_reference) {
        return Err("the runtime header actual differs from its lexical carrier");
    }
    Ok(*subject)
}

fn runtime_source_matches(
    program: &dyn crate::CheckingInput,
    subject: SymbolHandle,
    bound: ExpressionHandle,
    actual: ExpressionHandle,
) -> bool {
    if bound.is_valid() {
        bound == actual
    } else {
        subject.is_valid()
            && crate::canonical_place_from_expression(program, actual).is_some_and(|place| {
                place.root.as_symbol() == Some(subject) && place.segments.is_empty()
            })
    }
}

/// Retained executable-template identity, never a body or spelling search.
fn application_lineage(
    program: &dyn crate::CheckingInput,
    mut machine: SymbolHandle,
    declaration: SymbolHandle,
) -> Result<Vec<SymbolHandle>, &'static str> {
    let mut lineage = Vec::new();
    while machine != declaration {
        if lineage.contains(&machine) {
            return Err("the executable application template chain is cyclic");
        }
        lineage.push(machine);
        machine = program
            .executable_application_template(machine)
            .ok()
            .flatten()
            .ok_or("the executable application lost its authored template")?;
    }
    lineage.push(declaration);
    Ok(lineage)
}

fn application_parameter_origin(
    program: &dyn crate::CheckingInput,
    lineage: &[SymbolHandle],
    mut parameter: SymbolHandle,
) -> Result<SymbolHandle, &'static str> {
    if !parameter.is_valid() {
        return Err("the application parameter has no identity");
    }
    for frame in lineage.windows(2) {
        let original = program.symbols.get(parameter).generated_from;
        if !original.is_valid() {
            return Err("the application parameter lost its clone origin");
        }
        let parent = program.symbols.get(original).parent;
        if parent != frame[1]
            && program
                .machine_holding_state(parent)
                .is_none_or(|owner| owner.symbol != frame[1])
        {
            return Err("the application parameter belongs to another template scope");
        }
        parameter = original;
    }
    Ok(parameter)
}
