/*
Source call declarations, static applications and ordered argument correspondence.
Executable calls, writes and loans are judged by the lowered-form checker.
*/
use crate::declarations::symbols::TopLevelSymbols;
use crate::frozen_program::expression_table;
use crate::value_custody::expression_types::expression_type_name_handle;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::TableCall;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub(crate) mod closed_dispatch {
    use crate::frozen_program::expression_table;
    use symbols::SymbolHandle;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::typed_trees::ClosedConformanceConstArgument;
    use typed_trees::typed_trees::StaticRequirementDispatch;

    pub fn closed_requirement_dispatch_is_valid(
        program: &dyn crate::CheckingInput,
        owner: SymbolHandle,
        target: SymbolHandle,
        dispatch: &StaticRequirementDispatch,
    ) -> bool {
        validate_closed_requirement_dispatch(program, owner, target, dispatch).is_ok()
    }

    pub(crate) fn validate_closed_requirement_dispatch(
        program: &dyn crate::CheckingInput,
        owner: SymbolHandle,
        target: SymbolHandle,
        dispatch: &StaticRequirementDispatch,
    ) -> Result<(), &'static str> {
        if dispatch.application_commitment.is_zero() || dispatch.application_report_fingerprint == 0
        {
            return Err("selected map has no application commitment");
        }
        let applications = program.selected_conformance_applications(
            owner,
            dispatch.selection_application,
            dispatch.evidence_parameter,
            dispatch.requirement,
            dispatch.application_report_fingerprint,
            dispatch.application_commitment,
        );
        let [application] = applications.as_slice() else {
            return Err("selected call has no unique supplying map application");
        };
        let mut executable = match program.machine_holding_state(target) {
            Some(machine) => machine.symbol,
            None => return Err("selected executable entry has no owning machine"),
        };
        let supplying_application = if dispatch.selection_application.is_valid() {
            dispatch.selection_application
        } else {
            let supplying = program
                .machine_specializations
                .iter()
                .filter(|row| row.instance == owner)
                .collect::<Vec<_>>();
            let [supplying] = supplying.as_slice() else {
                return Err("selected call has no unique supplying specialization");
            };
            supplying.binding_application
        };
        let supplying_owner = expression_table(program)
            .static_machine_application(supplying_application)
            .lexical_owner;
        if application.callable_application.is_valid() {
            conformance_binding_is_exact(
                program,
                supplying_application,
                application.callable_application,
                application,
            )?;
        }
        let mut visited = Vec::new();
        loop {
            if visited.contains(&executable) {
                return Err("selected executable has cyclic template provenance");
            }
            visited.push(executable);
            for (_, capture) in program
                .captured_machine_applications
                .iter()
                .filter(|(_, capture)| capture.instance == executable)
            {
                capture_binding_is_exact(program, capture)?;
                if expression_table(program)
                    .static_machine_application(capture.supplying_application)
                    .lexical_owner
                    != supplying_owner
                {
                    return Err("captured executable belongs to a different supplying owner");
                }
            }
            match program.executable_application_template(executable) {
                Ok(Some(template)) => executable = template,
                Ok(None) => break,
                Err(reason) => return Err(reason),
            }
        }
        if program.realization_entry_template(target, application)
            != Some((dispatch.realization_machine, dispatch.realization_state))
        {
            return Err(
                "selected executable does not match the exact authored implementation entry",
            );
        }
        if dispatch.contract_signature.is_valid() {
            let Some(closed) = program.closed_requirement_application(dispatch.contract_signature)
            else {
                return Err("selected public contract has no closed application");
            };
            let Some(scope) = application.trait_scope.scope(program) else {
                return Err("selected map has no public trait scope");
            };
            if closed.declaring_trait != dispatch.declaring_trait
                || closed.template != dispatch.requirement
                || !program
                    .requirement_application_matches_scope(dispatch.contract_signature, &scope)
            {
                return Err("selected public contract belongs to a different trait application");
            }
            let public_scope = program.selected_requirement_contract_scope(owner, dispatch)?;
            if !program.requirement_application_matches_contract_scope(
                dispatch.contract_signature,
                &public_scope,
            ) {
                return Err("selected public contract differs from its supplying evidence bound");
            }
        }
        let requirement = dispatch.requirement;
        let Some(application_trait) =
            program.trait_definition_by_symbol(application.trait_definition)
        else {
            return Err("selected map has no declaring trait");
        };
        let mut pending = vec![application_trait];
        let mut visited = Vec::new();
        let mut declares = false;
        while let Some(definition) = pending.pop() {
            if visited.contains(&definition.symbol) {
                continue;
            }
            visited.push(definition.symbol);
            if definition.symbol == dispatch.declaring_trait {
                declares = true;
                break;
            }
            for requirement in program.trait_requirements(definition) {
                if let Some(parent) = program.trait_definition_by_symbol(requirement.symbol) {
                    pending.push(parent);
                }
            }
        }
        if !declares {
            return Err("selected trait does not declare the called requirement");
        }
        let Some(declaration) = program.trait_definition_by_symbol(dispatch.declaring_trait) else {
            return Err("called requirement has no declaring trait");
        };
        if program
            .trait_machine_signatures(declaration)
            .iter()
            .filter(|signature| signature.symbol == requirement)
            .count()
            != 1
        {
            return Err("called requirement does not occur exactly once in its declaring trait");
        }
        let Some(selected) = program.conformance_by_symbol(application.declaration) else {
            return Err("selected map declaration is missing");
        };
        let selected_rows = selected
            .rows
            .as_slice()
            .iter()
            .filter(|row| {
                row.declaring_trait == dispatch.declaring_trait && row.requirement == requirement
            })
            .collect::<Vec<_>>();
        let application_rows = application
            .rows
            .iter()
            .filter(|row| {
                row.declaring_trait == dispatch.declaring_trait && row.requirement == requirement
            })
            .collect::<Vec<_>>();
        let ([selected_row], [application_row]) =
            (selected_rows.as_slice(), application_rows.as_slice())
        else {
            return Err("selected requirement does not have exactly one implementation row");
        };
        if selected_row.realization_machine == dispatch.realization_machine
            && selected_row.realization_state == dispatch.realization_state
            && application_row.realization_machine == dispatch.realization_machine
            && application_row.realization_state == dispatch.realization_state
        {
            Ok(())
        } else {
            Err("selected rows name a different implementation")
        }
    }

    pub(crate) fn capture_binding_is_exact(
        program: &dyn crate::CheckingInput,
        capture: &typed_trees::typed_trees::CapturedMachineApplication,
    ) -> Result<(), &'static str> {
        conformance_binding_is_exact(
            program,
            capture.supplying_application,
            capture.binding_application,
            &capture.conformance_application,
        )?;
        let application = &capture.conformance_application;
        let declaration = program
            .conformance_by_symbol(application.declaration)
            .ok_or("capture lost its original declaration")?;
        let count = program
            .conformance_type_parameters(declaration)
            .iter()
            .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }))
            .count();
        let selected = expression_table(program)
            .static_machine_application_selected_machines(capture.binding_application);
        if selected.len() != count {
            return Err("capture executable callback roster differs from map telescope");
        }
        if count != 0
            && (!application.callable_application.is_valid()
                || selected
                    != expression_table(program).static_machine_application_selected_machines(
                        application.callable_application,
                    ))
        {
            return Err(
                "capture executable callbacks differ from their exact admitted map occurrence",
            );
        }
        Ok(())
    }

    pub(crate) fn conformance_binding_is_exact(
        program: &dyn crate::CheckingInput,
        supplying_application: typed_trees::static_applications::StaticMachineApplicationHandle,
        binding_application: typed_trees::static_applications::StaticMachineApplicationHandle,
        application: &typed_trees::typed_trees::ClosedConformanceApplication,
    ) -> Result<(), &'static str> {
        use typed_trees::static_applications::StaticMachineBindingValue;
        if !binding_application.is_valid() || !supplying_application.is_valid() {
            return Err("capture has missing supplying or binding application");
        }
        let source = expression_table(program).static_machine_application(supplying_application);
        let binding = expression_table(program).static_machine_application(binding_application);
        if binding.lexical_owner != source.lexical_owner
            || binding.registration_operation != application.declaration
            || binding.binding_declaration != application.declaration
        {
            return Err(
                "capture binding owner or map declaration differs from supplying application",
            );
        }
        let selected = if let Some(selection) = &source.conformance_selection {
            expression_table(program)
                .static_machine_application_arguments(supplying_application)
                .get(selection.operand_ordinal as usize)
                .and_then(|argument| {
                    crate::conformance_applications::close_conformance_application(
                        program, argument,
                    )
                    .ok()
                })
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            let source_bindings = expression_table(program)
                .static_machine_application_bindings(supplying_application);
            let evidence_positions = program
                .machine_specializations
                .iter()
                .filter(|specialization| {
                    specialization.binding_application == supplying_application
                })
                .flat_map(|specialization| {
                    specialization.conformance_applications.iter().enumerate()
                })
                .filter_map(|(position, selected)| {
                    (application.callable_application.is_valid()
                        && selected.callable_application == application.callable_application)
                        .then_some(position)
                });
            let declaration_positions = program
                .trait_implementation_applications
                .iter()
                .map(|(_, selected)| {
                    (
                        selected.supplying_application,
                        selected.parameter,
                        &selected.application,
                    )
                })
                .chain(
                    program
                        .data_implementation_applications
                        .iter()
                        .map(|(_, selected)| {
                            (
                                selected.supplying_application,
                                selected.parameter,
                                &selected.application,
                            )
                        }),
                )
                .filter(|(supplying, _, selected)| {
                    *supplying == supplying_application
                        && application.callable_application.is_valid()
                        && selected.callable_application == application.callable_application
                })
                .filter_map(|(_, parameter, _)| {
                    source_bindings
                        .iter()
                        .filter(|binding| {
                            matches!(binding.value, StaticMachineBindingValue::Conformance(_))
                        })
                        .position(|binding| binding.parameter == parameter)
                });
            let mut positions = evidence_positions.chain(declaration_positions);
            let position = positions
                .next()
                .ok_or("capture supplying application lost its exact evidence occurrence")?;
            if positions.next().is_some() {
                return Err("capture supplying application repeats its evidence occurrence");
            }
            source_bindings
                .iter()
                .filter(|binding| {
                    matches!(binding.value, StaticMachineBindingValue::Conformance(_))
                })
                .nth(position)
                .into_iter()
                .filter_map(|binding| match &binding.value {
                    StaticMachineBindingValue::Conformance(argument)
                        if argument.symbol == application.declaration =>
                    {
                        crate::conformance_applications::close_conformance_application(
                            program, argument,
                        )
                        .ok()
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let mut expected = application.clone();
        expected.callable_application = Default::default();
        expected.trait_scope = Default::default();
        let selected = selected
            .into_iter()
            .filter(|selection| selection == &expected)
            .collect::<Vec<_>>();
        if selected.len() != 1 {
            return Err("capture supplying application has no unique exact selected conformance");
        }
        let Some(declaration) = program.conformance_by_symbol(application.declaration) else {
            return Err("capture original conformance declaration is absent");
        };
        let parameters = program.conformance_type_parameters(declaration);
        let bindings =
            expression_table(program).static_machine_application_bindings(binding_application);
        if parameters.len() != application.arguments.len() || bindings.len() != parameters.len() {
            return Err("capture tuple or binding roster differs from map telescope");
        }
        let reclosed = &selected[0];
        let mut const_ordinal = 0;
        for (parameter, argument) in parameters.iter().zip(&application.arguments) {
            let selected = bindings
                .iter()
                .filter(|binding| binding.parameter == parameter.symbol)
                .collect::<Vec<_>>();
            let [selected] = selected.as_slice() else {
                return Err("capture binder has no unique selected operand");
            };
            let evaluated = if parameter.kind.is_const_or_value() {
                let value = reclosed.const_arguments.get(const_ordinal);
                const_ordinal += 1;
                match value {
                    Some(ClosedConformanceConstArgument::Evaluated { value, .. }) => Some(value),
                    _ => None,
                }
            } else {
                None
            };
            let matches = if let Some(expected) = evaluated {
                let StaticMachineBindingValue::Const(actual) = &selected.value else {
                    return Err("capture const binder changed its declared operand category");
                };
                typed_trees::typed_trees::static_const_value_identity(expected)
                    .zip(program.static_const_type_reference_identity(*actual))
                    .is_some_and(|(expected, actual)| expected == actual)
            } else {
                program.static_binding_matches_argument(
                    &parameter.kind,
                    &selected.value,
                    argument,
                    source.lexical_owner,
                    binding.lexical_owner,
                )
            };
            if !matches {
                return Err("capture operand does not match its exact original selected argument");
            }
        }
        let lifetimes =
            expression_table(program).static_machine_application_lifetimes(binding_application);
        let matches = binding.lifetime_binding_declaration == declaration.symbol
            && lifetimes.len() == declaration.lifetime_parameters.len()
            && lifetimes.len() == application.lifetime_arguments.len()
            && lifetimes
                .iter()
                .zip(
                    declaration
                        .lifetime_parameters
                        .iter()
                        .zip(&application.lifetime_arguments),
                )
                .all(|(binding, (parameter, argument))| {
                    binding.parameter == *parameter && binding.argument.as_str() == argument
                });
        if matches {
            Ok(())
        } else {
            Err("capture lifetime tuple differs from original map telescope")
        }
    }
}
pub use closed_dispatch::closed_requirement_dispatch_is_valid;
pub(crate) use closed_dispatch::{capture_binding_is_exact, conformance_binding_is_exact};

pub(crate) mod dynamic_requirement {
    use super::{
        Diagnostic, ExpressionHandle, ExpressionNode, Machine, State, TypeReferenceHandle,
    };
    use typed_trees::signature::StateSignature;
    use typed_trees::trait_definition::AppliedTraitScope;

    pub(crate) struct DynamicSelectorCall<'program> {
        pub(crate) selector_type: TypeReferenceHandle,
        pub(crate) signature: &'program StateSignature,
        pub(crate) closed_signature: Option<&'program StateSignature>,
        pub(crate) application: AppliedTraitScope<'program>,
    }

    pub(crate) fn dynamic_selector_call<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        receiver: symbols::SymbolHandle,
        target: symbols::SymbolHandle,
        arguments: &[ExpressionHandle],
    ) -> Result<Option<DynamicSelectorCall<'program>>, String> {
        let Some(&selector) = arguments.first() else {
            return Ok(None);
        };
        let target = program
            .dynamic_slot_by_symbol(target)
            .map_or(target, |slot| slot.call_requirement);
        let closed = program.closed_requirement_application(target);
        let target = closed
            .as_ref()
            .map_or(target, |application| application.template);
        if !receiver.is_valid() || program.symbols.get(receiver).kind != symbols::SymbolKind::Trait
        {
            return Ok(None);
        }
        if program
            .trait_machine_signature(target)
            .is_some_and(|(declaring, _)| declaring.is_boundary)
        {
            return Ok(None);
        }
        let Some(selector_type) = selector_type(program, machine, state, selector) else {
            return Ok(None);
        };
        if crate::declarations::traits::dynamic_selector_application(program, selector_type)
            .is_none()
        {
            return Ok(None);
        }
        let (source_selector, enclosing_applications) = if matches!(
            program.expression_table.expression(selector),
            ExpressionNode::Cast(_)
        ) {
            (selector_type, Vec::new())
        } else if let Some(state) = state {
            let place = crate::value_custody::places::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                usize::MAX,
                selector,
            )
            .ok_or_else(|| "dynamic selector lost its source occurrence".to_owned())?;
            crate::value_custody::places::canonical_place_source_type_application(
                program,
                state.symbol,
                usize::MAX,
                &place,
            )
            .ok_or_else(|| "dynamic selector lost its supplying type application".to_owned())?
        } else {
            (selector_type, Vec::new())
        };
        let Some((selector_trait, selector_arguments)) =
            crate::declarations::traits::dynamic_selector_application(program, source_selector)
        else {
            return Ok(None);
        };
        let trait_definition = |symbol| program.trait_definition_by_symbol(symbol);
        let (Some(selector_definition), Some(declaring)) =
            (trait_definition(selector_trait), trait_definition(receiver))
        else {
            return Err("dynamic call names an unresolved trait".to_owned());
        };
        let target = if program
            .trait_machine_signature_by_symbol(declaring, target)
            .is_some()
        {
            target
        } else {
            program
                .typed_program()
                .conformances
                .iter()
                .map(|(_, conformance)| conformance)
                .flat_map(|conformance| conformance.rows.iter())
                .find(|row| {
                    row.declaring_trait == declaring.symbol
                        && (row.realization_state == target || row.realization_machine == target)
                })
                .map(|row| row.requirement)
                .unwrap_or(target)
        };
        let Some(signature) = program.trait_machine_signature_by_symbol(declaring, target) else {
            return Err(format!(
                "`{}` declares no requirement named `{}`",
                declaring.name,
                program.symbols.name(target)
            ));
        };
        let application = AppliedTraitScope::new(
            program,
            selector_definition,
            typed_trees::trait_definition::TraitApplicationArguments {
                lexical_owner: machine.symbol,
                enclosing_data_applications: &enclosing_applications,
                arguments: selector_arguments,
                lifetimes: &[],
            },
        )
        .and_then(|scope| scope.declaring_scope(program, declaring.symbol));
        let Some(application) = application else {
            return Err(format!(
                "a `dyn {}` selector cannot call `{}::{}`: `{}` does not inherit `{}`",
                selector_definition.name,
                declaring.name,
                signature.name,
                selector_definition.name,
                declaring.name
            ));
        };
        if let Some(closed) = closed.as_ref()
            && !program.requirement_application_matches_scope(closed.signature.symbol, &application)
        {
            return Err(
                "dynamic call's closed requirement belongs to a different trait application"
                    .to_owned(),
            );
        }
        if let Some(reason) = crate::declarations::traits::dynamic_requirement_ineligibility(
            program, declaring, signature,
        ) {
            return Err(format!(
                "requirement `{}::{}` is absent from `dyn {}`: {reason}",
                declaring.name, signature.name, selector_definition.name
            ));
        }
        Ok(Some(DynamicSelectorCall {
            selector_type,
            signature,
            closed_signature: closed.as_ref().map(|application| application.signature),
            application,
        }))
    }

    fn validate_family_tuple(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        call: &DynamicSelectorCall<'_>,
        machine_arguments: &[typed_trees::expression::StaticMachineArgument],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let declaring = &call.application.declaration.name;
        let requirement = &call.signature.name;
        match program.finite_signature_family(call.signature) {
            typed_trees::finite_family::FamilyProbe::Finite { arity, tuples } => {
                let roster = || {
                    tuples
                        .iter()
                        .map(|tuple| format!("({})", tuple.display.join(", ")))
                        .collect::<Vec<_>>()
                        .join(" or ")
                };
                let tuple_identities: Option<Vec<String>> = machine_arguments
                    .iter()
                    .map(|argument| program.static_const_argument_identity(argument))
                    .collect();
                let selects_exact_tuple = tuple_identities.as_deref().is_some_and(|identities| {
                    identities.len() == arity
                        && tuples
                            .iter()
                            .any(|tuple| tuple.identities.as_ref() == identities)
                });
                if selects_exact_tuple {
                    return;
                }
                let parameters = program.state_signature_type_parameters(call.signature);
                let conditional = |(index, argument): (
                    usize,
                    &typed_trees::expression::StaticMachineArgument,
                )| {
                    if program
                        .data_type_parameter_by_symbol(argument.symbol)
                        .is_some_and(|parameter| {
                            matches!(
                                parameter.kind,
                                typed_trees::data::TypeParameterKind::Const { .. }
                            )
                        })
                        || program
                            .typed_program()
                            .finite_value_binder_in_scope(machine.symbol, argument.symbol)
                    {
                        return true;
                    }
                    let Some(parameter) = parameters.get(index) else {
                        return false;
                    };
                    let (typed_trees::data::TypeParameterKind::Const { type_reference }
                    | typed_trees::data::TypeParameterKind::Value { type_reference }) =
                        parameter.kind
                    else {
                        return false;
                    };
                    if !argument.type_reference.is_valid() {
                        return false;
                    }
                    let typed_trees::types::TypeReferenceNode::ConstExpression(expression) =
                        program
                            .type_reference_table
                            .type_reference(argument.type_reference)
                    else {
                        return false;
                    };
                    crate::value_custody::const_expressions::has_carrier(
                        program,
                        *expression,
                        &typed_trees::data::TypeParameterKind::Const { type_reference },
                        machine.symbol,
                    )
                };
                if machine_arguments.len() == arity
                    && machine_arguments
                        .iter()
                        .any(|argument| program.static_const_argument_identity(argument).is_none())
                    && machine_arguments
                        .iter()
                        .enumerate()
                        .all(|(index, argument)| {
                            program.static_const_argument_identity(argument).is_some()
                                || conditional((index, argument))
                        })
                {
                    return;
                }
                let runtime_capable = machine_arguments.len() == arity
                    && machine_arguments.iter().any(|argument| {
                        crate::machine_calls::static_arguments::is_runtime_value_subject(
                            program, argument,
                        )
                    })
                    && machine_arguments.iter().all(|argument| {
                        crate::machine_calls::static_arguments::is_runtime_value_subject(
                            program, argument,
                        ) || program.static_const_argument_identity(argument).is_some()
                    });
                if !runtime_capable {
                    diagnostics.push(Diagnostic::error(format!(
                    "dynamic family call `{declaring}::{requirement}` requires exactly one closed roster tuple ({}); a runtime value cannot select a family row without proved membership -- ordinary runtime subjects of that arity discharge it at the call's own proof",
                    roster()
                )));
                }
            }
            typed_trees::finite_family::FamilyProbe::NotFinite(reason) => {
                if !machine_arguments.is_empty() {
                    diagnostics.push(Diagnostic::error(format!(
                    "dynamic requirement call `{declaring}::{requirement}` supplies static machine arguments, but the requirement declares no finite family: {reason}"
                )));
                }
            }
        }
    }

    pub(crate) fn expression_trait_receiver(
        program: &dyn crate::CheckingInput,
        receiver: ExpressionHandle,
    ) -> symbols::SymbolHandle {
        match program.expression_table.expression(receiver) {
            ExpressionNode::Name(name)
                if name.symbol.is_valid()
                    && program.symbols.get(name.symbol).kind == symbols::SymbolKind::Trait =>
            {
                name.symbol
            }
            _ => symbols::SymbolHandle::invalid(),
        }
    }

    fn selector_type(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Cast(cast) => Some(cast.target_type),
            _ => crate::value_custody::places::declared_place_type_raw(
                program, machine, state, expression,
            ),
        }
    }
}

pub(crate) mod generic_requirement {
    use super::{Diagnostic, ExpressionHandle, ExpressionNode, Machine, State, StateParameter};
    use symbols::SymbolKind;
    use typed_trees::signature::StateSignature;
    use typed_trees::trait_definition::TraitDefinition;

    pub fn selected_requirement_signature(
        program: &dyn crate::CheckingInput,
        target: symbols::SymbolHandle,
    ) -> Option<(&TraitDefinition, &StateSignature)> {
        if let Some(selected) = program.trait_machine_signature(target) {
            return Some(selected);
        }
        if !target.is_valid() {
            return None;
        }
        let binder = program.symbols.get(target).parent;
        if !binder.is_valid()
            || program.symbols.get(binder).kind != SymbolKind::ConformanceParameter
        {
            return None;
        }
        let requirement = program
            .evidence_bound_requirement(program.symbols.get(binder).parent, binder, target)
            .ok()??;
        let application = &requirement.application;
        let signature = program
            .applied_callable_signature(application, requirement.signature.symbol)
            .or_else(|| {
                program
                    .closed_requirement_applications()
                    .find(|closed| {
                        closed.template == requirement.signature.symbol
                            && closed.local_arguments.is_empty()
                            && program.requirement_application_matches_scope(
                                closed.signature.symbol,
                                application,
                            )
                    })
                    .map(|closed| closed.signature)
            })
            .or_else(|| {
                (application.substitutions.is_empty()
                    && application.declaration.lifetime_parameters.is_empty())
                .then_some(requirement.signature)
            })?;
        Some((application.declaration, signature))
    }

    pub(crate) fn requirement_namespace_member(
        program: &dyn crate::CheckingInput,
        namespace: symbols::SymbolHandle,
        target: symbols::SymbolHandle,
    ) -> bool {
        namespace.is_valid()
            && target.is_valid()
            && program.symbols.get(namespace).kind == SymbolKind::ConformanceParameter
            && program.symbols.get(target).parent == namespace
            && selected_requirement_signature(program, target).is_some()
    }

    pub fn requirement_contract_signature<'program>(
        program: &'program dyn crate::CheckingInput,
        caller: symbols::SymbolHandle,
        site: &super::CallSite<'_>,
    ) -> Option<(&'program TraitDefinition, &'program StateSignature)> {
        resolve_requirement_contract(program, caller, site).ok()
    }

    pub fn resolve_requirement_contract<'program>(
        program: &'program dyn crate::CheckingInput,
        caller: symbols::SymbolHandle,
        site: &super::CallSite<'_>,
    ) -> Result<(&'program TraitDefinition, &'program StateSignature), &'static str> {
        let target = site.target_symbol(program);
        let public_target = if let Some(dispatch) = site.static_requirement_dispatch() {
            super::closed_dispatch::validate_closed_requirement_dispatch(
                program, caller, target, dispatch,
            )?;
            dispatch.contract_symbol()
        } else {
            target
        };
        let (declaration, selected) = selected_requirement_signature(program, public_target)
            .ok_or("selected public declaration has no requirement signature")?;
        let exact = match site {
            super::CallSite::Expression { call, .. } => {
                program.requirement_call_signature(caller, call)
            }
            super::CallSite::Statement(call) => {
                program.requirement_statement_call_signature(caller, call)
            }
            super::CallSite::TransitionNamed { .. } => None,
        };
        if let Some(signature) = exact {
            let same_source = signature.symbol == selected.symbol
                || program
                    .closed_requirement_application(selected.symbol)
                    .is_some_and(|closed| closed.template == signature.symbol);
            return same_source
                .then_some((declaration, signature))
                .ok_or("applied public signature belongs to a different source declaration");
        }
        let requires_exact_application = match site {
            super::CallSite::Expression { call, .. } => {
                call.requirement_application.is_valid()
                    || call.static_requirement_dispatch.is_some()
            }
            super::CallSite::Statement(call) => call.static_requirement_dispatch.is_some(),
            super::CallSite::TransitionNamed { .. } => false,
        };
        if requires_exact_application {
            return Err("selected call has no exact applied public signature");
        }
        Ok((declaration, selected))
    }

    pub fn named_conformance_target_requirement<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &'program Machine,
        target: symbols::SymbolHandle,
    ) -> Result<Option<crate::declarations::traits::GenericBoundRequirement<'program>>, String>
    {
        program.conformance_target_requirement(machine, target)
    }

    pub(crate) fn named_conformance_receiver(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        receiver: symbols::SymbolHandle,
    ) -> bool {
        receiver.is_valid()
            && program.symbols.get(receiver).kind == SymbolKind::ConformanceParameter
            && machine
                .conformance_bounds
                .iter()
                .any(|bound| bound.binder == receiver)
    }

    pub(crate) fn expression_conformance_receiver(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        receiver: ExpressionHandle,
    ) -> symbols::SymbolHandle {
        let Some(path) = program.expression_table.name_path_node(receiver) else {
            return symbols::SymbolHandle::invalid();
        };
        if named_conformance_receiver(program, machine, path.symbol) {
            path.symbol
        } else {
            symbols::SymbolHandle::invalid()
        }
    }

    pub(crate) fn named_conformance_requirement<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &'program Machine,
        receiver: symbols::SymbolHandle,
        target: symbols::SymbolHandle,
    ) -> Result<Option<crate::declarations::traits::GenericBoundRequirement<'program>>, String>
    {
        program.conformance_bound_requirement(machine, receiver, target)
    }

    pub(crate) fn unresolved_implementation_call(
        program: &dyn crate::CheckingInput,
        implementation: symbols::SymbolHandle,
        target: &str,
    ) -> String {
        let name = program.symbols.name(implementation);
        let rows = program
            .conformance_by_symbol(implementation)
            .map(|conformance| conformance.rows.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|row| row.requirement_name.as_str() == target)
            .map(|row| format!("`{}::{}`", row.declaring_trait_name, row.requirement_name))
            .collect::<Vec<_>>();
        if rows.is_empty() {
            format!("implementation `{name}` has no row for requirement `{target}`")
        } else {
            format!(
                "implementation `{name}` has {} rows named `{target}` ({}); a call through the implementation must select exactly one",
                rows.len(),
                rows.join(", ")
            )
        }
    }

    fn call_value_arguments_match(
        program: &dyn crate::CheckingInput,
        actual: typed_trees::types::TypeReferenceHandle,
        required: typed_trees::types::TypeReferenceHandle,
        requirement: &crate::declarations::traits::GenericBoundRequirement<'_>,
        selected: &[typed_trees::expression::StaticMachineArgument],
    ) -> bool {
        use typed_trees::data::TypeParameterKind;
        use typed_trees::type_identity::{IndexIdentityArgument, TypeIdentityRequest};
        let parameters = program.state_signature_type_parameters(requirement.signature);
        if parameters.len() != selected.len() {
            return false;
        }
        let arguments = parameters
            .iter()
            .zip(selected)
            .filter_map(|(parameter, actual)| {
                matches!(
                    parameter.kind,
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
                )
                .then_some((
                    parameter.symbol,
                    IndexIdentityArgument::StaticArgument(actual),
                ))
            })
            .collect::<Vec<_>>();
        if arguments.is_empty() {
            return false;
        }
        let result = program.scoped_type_identity_with_index_arguments(
            TypeIdentityRequest {
                substitutions: &requirement.application.substitutions,
                ..TypeIdentityRequest::ordinary(required)
            },
            &requirement.application.substitution_scope_starts,
            &arguments,
        );
        result
            .zip(program.scoped_type_identity_with_index_arguments(
                TypeIdentityRequest::ordinary(actual),
                &[],
                &[],
            ))
            .is_some_and(|(expected, actual)| actual == expected)
    }
}
pub(crate) use generic_requirement::requirement_namespace_member;
pub use generic_requirement::{
    named_conformance_target_requirement, requirement_contract_signature,
    resolve_requirement_contract, selected_requirement_signature,
};
pub(crate) mod inline_assembly {
    use crate::value_custody::expression_types::expression_type_name_handle;
    use diagnostics::Diagnostic;
    use language_core::inline_assembly::AsmInstructionContract;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::types::PrimitiveType;

    pub(crate) fn register_read_mnemonic(
        function: symbols::BuiltinFunction,
    ) -> Option<&'static str> {
        let name = function.name();
        language_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(name)
            .map(|register| register.read_mnemonic())
            .or_else(|| {
                language_core::inline_assembly::AsmSystemRegister::from_read_intrinsic_name(name)
                    .map(|register| register.read_mnemonic())
            })
    }

    pub(crate) fn user_asm_contract(mnemonic: &str) -> AsmInstructionContract {
        let Some(language_core::inline_assembly::AsmCatalogEntry::Contract(contract)) =
            language_core::inline_assembly::asm_catalog_entry(mnemonic)
        else {
            panic!("accepted asm intrinsic `{mnemonic}` is absent from the shared catalog");
        };
        assert_eq!(
            contract.availability,
            language_core::inline_assembly::AsmInstructionAvailability::UserChecked,
            "source asm intrinsic `{mnemonic}` must be user-checked"
        );
        contract
    }

    fn asm_operand_primitive_type(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        operand: ExpressionHandle,
    ) -> Option<PrimitiveType> {
        match program.expression_table.expression(operand) {
            ExpressionNode::Borrow(inner) => {
                asm_operand_primitive_type(program, machine, state, inner.target)
            }
            ExpressionNode::Cast(cast) => program.primitive_type_reference(cast.target_type),
            _ => {
                crate::value_custody::places::declared_place_type(program, machine, state, operand)
                    .and_then(|type_reference| program.primitive_type_reference(type_reference))
            }
        }
    }
}

pub(crate) mod type_arguments {
    use super::*;
    use symbols::SymbolHandle;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::expression::StaticMachineArgument;

    #[allow(clippy::too_many_arguments)]
    pub fn call_type_arguments(
        program: &dyn crate::CheckingInput,
        caller: &Machine,
        state: Option<&State>,
        callee: &Machine,
        entry: &State,
        selected: &[StaticMachineArgument],
        arguments: &[ExpressionHandle],
    ) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
        if program.machine_type_parameters(callee).is_empty() {
            return Some(Vec::new());
        }
        callable_type_arguments(
            program,
            caller,
            state,
            program.machine_type_parameters(callee),
            program.state_parameters(entry),
            selected,
            arguments,
        )
    }

    pub(crate) fn requirement_call_type_arguments(
        program: &dyn crate::CheckingInput,
        caller: &Machine,
        state: Option<&State>,
        signature: &typed_trees::signature::StateSignature,
        selected: &[StaticMachineArgument],
        arguments: &[ExpressionHandle],
    ) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
        let parameters = program.state_signature_type_parameters(signature);
        if parameters.is_empty() {
            return Some(Vec::new());
        }
        callable_type_arguments(
            program,
            caller,
            state,
            parameters,
            program.state_signature_parameters(signature),
            selected,
            arguments,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn callable_type_arguments(
        program: &dyn crate::CheckingInput,
        caller: &Machine,
        state: Option<&State>,
        parameters: &[typed_trees::data::TypeParameter],
        value_parameters: &[typed_trees::signature::StateParameter],
        selected: &[StaticMachineArgument],
        arguments: &[ExpressionHandle],
    ) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
        scoped_callable_type_arguments(
            program,
            caller.symbol,
            program.machine_type_parameters(caller),
            parameters,
            value_parameters,
            selected,
            arguments,
            &|argument| {
                crate::value_custody::places::declared_place_type_raw(
                    program, caller, state, argument,
                )
                .or_else(|| {
                    state.and_then(|state| {
                        crate::expression_result_type_reference(program, caller, state, argument)
                    })
                })
            },
        )
    }

    pub(crate) fn declaration_call_type_arguments(
        program: &dyn crate::CheckingInput,
        owner: SymbolHandle,
        callee: &Machine,
        entry: &State,
        selected: &[StaticMachineArgument],
        arguments: &[ExpressionHandle],
    ) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
        let parameters = program.machine_type_parameters(callee);
        if parameters.is_empty() {
            return Some(Vec::new());
        }
        scoped_callable_type_arguments(
            program,
            owner,
            &[],
            parameters,
            program.state_parameters(entry),
            selected,
            arguments,
            &|argument| {
                crate::parameter_expression_result_type_reference(program, owner, &[], argument)
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn scoped_callable_type_arguments(
        program: &dyn crate::CheckingInput,
        owner: SymbolHandle,
        caller_parameters: &[typed_trees::data::TypeParameter],
        parameters: &[typed_trees::data::TypeParameter],
        value_parameters: &[typed_trees::signature::StateParameter],
        selected: &[StaticMachineArgument],
        arguments: &[ExpressionHandle],
        actual_type: &dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
    ) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
        let type_parameters = parameters
            .iter()
            .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
            .collect::<Vec<_>>();
        let const_parameters = parameters
            .iter()
            .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Const { .. }))
            .collect::<Vec<_>>();
        let mut bindings = Vec::new();
        let mut type_ordinal = 0;
        let mut const_selections = Vec::new();
        for selected in selected {
            let const_expression = selected.type_reference.is_valid()
            && const_parameters
                .get(const_selections.len())
                .is_some_and(|parameter| {
                    matches!(program.type_reference_table.type_reference(selected.type_reference),
                    typed_trees::types::TypeReferenceNode::ConstExpression(expression)
                        if crate::value_custody::const_expressions::has_carrier(
                            program, *expression, &parameter.kind, owner,
                        ))
                });
            // This query projects Type/Const substitutions from a complete
            // static tuple. Implementation source references remain evidence.
            let actual = if const_expression || selected.names_implementation(&program.symbols) {
                None
            } else if selected.type_reference.is_valid() {
                Some(selected.type_reference)
            } else if selected.symbol.is_valid() {
                let is_type =
                    match program.symbols.get(selected.symbol).kind {
                        symbols::SymbolKind::BuiltinType | symbols::SymbolKind::Data => true,
                        symbols::SymbolKind::TypeParameter => {
                            let parameter = program.data_type_parameters.iter().find_map(
                                |(_, parameter)| {
                                    (parameter.symbol == selected.symbol).then_some(parameter)
                                },
                            )?;
                            matches!(parameter.kind, TypeParameterKind::Type)
                        }
                        _ => false,
                    };
                if is_type {
                    Some(
                        program
                            .type_reference_table
                            .find_named_type_reference(selected.symbol)?,
                    )
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(actual) = actual {
                if !program.type_reference_table.contains_type_reference(actual) {
                    return None;
                }
                let parameter = type_parameters.get(type_ordinal)?;
                if !parameter.symbol.is_valid() {
                    return None;
                }
                bindings.push((parameter.symbol, actual));
                type_ordinal += 1;
            } else if const_expression
                || program.static_const_argument_identity(selected).is_some()
                || (selected.symbol.is_valid()
                    && program.data_type_parameters.iter().any(|(_, parameter)| {
                        parameter.symbol == selected.symbol
                            && matches!(parameter.kind, TypeParameterKind::Const { .. })
                    }))
            {
                let parameter = const_parameters.get(const_selections.len())?;
                if !parameter.symbol.is_valid() {
                    return None;
                }
                if selected.symbol.is_valid()
                    && program.static_const_argument_identity(selected).is_none()
                    && let Some(actual) = program
                        .type_reference_table
                        .find_named_type_reference(selected.symbol)
                {
                    if !crate::value_custody::type_references::const_argument_matches_parameter(
                        program,
                        parameter,
                        actual,
                        caller_parameters,
                    ) {
                        return None;
                    }
                    bindings.push((parameter.symbol, actual));
                }
                const_selections.push((*parameter, selected));
            }
        }
        if parameters.iter().any(|parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Type | TypeParameterKind::Const { .. }
            ) && !bindings
                .iter()
                .any(|(symbol, _)| *symbol == parameter.symbol)
        }) {
            for (parameter, argument) in value_parameters.iter().zip(arguments) {
                let actual = actual_type(*argument);
                if let Some(actual) = actual {
                    crate::machine_calls::machine_parameters::infer_call_type_arguments(
                        program,
                        parameters,
                        caller_parameters,
                        parameter.type_reference,
                        actual,
                        &mut bindings,
                    );
                }
            }
        }
        for (parameter, selected) in const_selections {
            let Some(actual) =
                crate::queries::substitutions::actual_for(&bindings, parameter.symbol)
            else {
                continue;
            };
            if !const_selection_matches(program, selected, actual) {
                return None;
            }
        }
        Some(bindings)
    }

    fn const_selection_matches(
        program: &dyn crate::CheckingInput,
        selected: &StaticMachineArgument,
        actual: TypeReferenceHandle,
    ) -> bool {
        if let Some(identity) = program.static_const_argument_identity(selected) {
            return program.normalized_type_identity_text(actual) == identity;
        }
        matches!(program.type_reference_table.type_reference(actual),
        TypeReferenceNode::Named { symbol, .. }
            if symbol.is_valid() && *symbol == selected.symbol)
    }
}
pub use type_arguments::call_type_arguments;
pub(crate) use type_arguments::{callable_type_arguments, declaration_call_type_arguments};

pub(crate) mod signature_queries {
    use std::sync::OnceLock;
    use symbols::SymbolHandle;
    use typed_trees::statement::TableCall;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) struct CallDestinationQueries {
        rows: Vec<(SymbolHandle, OnceLock<Option<Vec<TypeReferenceHandle>>>)>,
    }

    impl CallDestinationQueries {
        fn of(program: &dyn crate::CheckingInput) -> Self {
            let symbols = program
                .machines()
                .iter()
                .flat_map(|machine| program.machine_states(machine))
                .map(|state| state.symbol);
            Self {
                rows: crate::frozen_program::once_lock_rows(symbols),
            }
        }
    }

    pub(crate) fn call_argument_destinations(
        program: &dyn crate::CheckingInput,
        target: SymbolHandle,
        argument_count: usize,
    ) -> Option<Vec<TypeReferenceHandle>> {
        let compute = || call_argument_signature(program, target);
        let signature = match program.checking_memos() {
            None => compute(),
            Some(memos) => {
                let queries = memos
                    .call_destinations
                    .get_or_init(|| CallDestinationQueries::of(program));
                match queries.rows.binary_search_by_key(
                    &(target.arena_index(), target.generation()),
                    |(symbol, _)| (symbol.arena_index(), symbol.generation()),
                ) {
                    Ok(position) => queries.rows[position].1.get_or_init(compute).clone(),
                    Err(_) => compute(),
                }
            }
        }?;
        (signature.len() == argument_count).then_some(signature)
    }

    fn call_argument_signature(
        program: &dyn crate::CheckingInput,
        target: SymbolHandle,
    ) -> Option<Vec<TypeReferenceHandle>> {
        let (machine, state) = super::machine_state_by_symbol(program, target)?;
        if !machine.symbol.is_valid()
            || !program.machine_type_parameters(machine).is_empty()
            || !machine.lifetime_parameters.is_empty()
            || !machine.conformance_bounds.is_empty()
            || program
                .machines()
                .iter()
                .filter(|candidate| candidate.symbol == machine.symbol)
                .count()
                != 1
        {
            return None;
        }
        Some(
            program
                .state_parameters(state)
                .iter()
                .map(|parameter| {
                    if parameter.is_const || !parameter.symbol.is_valid() {
                        TypeReferenceHandle::invalid()
                    } else {
                        parameter.type_reference
                    }
                })
                .collect(),
        )
    }

    pub(crate) fn unique_state_by_symbol(
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
    ) -> Option<&typed_trees::state::State> {
        let matches = program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .filter(|state| state.symbol == symbol)
            .collect::<Vec<_>>();
        let [state] = matches.as_slice() else {
            return None;
        };
        Some(state)
    }

    pub(crate) fn call_receiver_member_names(
        program: &dyn crate::CheckingInput,
        call: &TableCall,
    ) -> Vec<String> {
        program
            .statement_table
            .name_path_members(call.receiver)
            .iter()
            .map(|member| member.as_str().to_owned())
            .collect()
    }
}
pub(crate) mod unit_returns {

    use super::{ExpressionHandle, Machine, State, TypeReferenceHandle, TypeReferenceNode};
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use language_semantics::declaration_selection::BuildOperation;
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    pub fn unit_statement_call_is_admitted(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> bool {
        if !expression_table(program).expression_is_valid(expression) {
            return false;
        }
        if expression_table(program)
            .call_node(expression)
            .and_then(|call| call.native_code_layout.as_ref())
            .is_some_and(|query| query.discards_result)
        {
            return true;
        }
        let Some((position, terminal_arm)) = statement_occurrence(program, state, expression)
        else {
            return false;
        };
        if terminal_arm {
            // The selected result completes this caller. Match subjects, patterns
            // and call arguments still have ordinary value-formation obligations.
            return unit_type(program, state.return_type)
                && completion_returns_unit(program, machine, expression);
        }
        if position + 1
            == program
                .statement_table
                .statements(state.statement_nodes)
                .len()
        {
            return call_completes_unit_state(program, machine, state, expression);
        }
        call_returns_unit(program, machine, expression)
    }

    pub fn unit_return_call_is_admitted(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> bool {
        statement_occurrence(program, state, expression).is_some_and(|(position, terminal_arm)| {
            !terminal_arm
                && position + 1
                    == program
                        .statement_table
                        .statements(state.statement_nodes)
                        .len()
        }) && call_completes_unit_state(program, machine, state, expression)
    }

    /// One exact statement root owns completion; reused operands do not inherit it.
    fn statement_occurrence(
        program: &dyn CheckingInput,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<(usize, bool)> {
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut occurrences = statements
            .iter()
            .enumerate()
            .filter_map(|(position, statement)| match statement {
                StatementNode::Expression(root) if *root == expression => Some((position, false)),
                StatementNode::Transition(transition)
                    if transition.targets().any(|target| {
                        matches!(
                            program.statement_table.transition_target(target),
                            TransitionTargetNode::Value(root) if *root == expression
                        )
                    }) =>
                {
                    Some((position, true))
                }
                _ => None,
            });
        let occurrence = occurrences.next()?;
        occurrences.next().is_none().then_some(occurrence)
    }

    /// Result compatibility for a Unit completion. Only selected result arms
    /// inherit completion; their subjects and operands receive no such position.
    pub(crate) fn call_completes_unit_state(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> bool {
        unit_type(program, state.return_type)
            && expression_table(program).expression_is_valid(expression)
            && completion_returns_unit(program, machine, expression)
    }

    /// The result leaves of one completion, without walking evaluated operands.
    /// Full coordinates avoid revisiting shared match nodes without allocating
    /// scratch for unrelated expressions in the program's arena.
    fn completion_calls_all(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        mut accepts: impl FnMut(ExpressionHandle) -> bool,
    ) -> bool {
        let table = expression_table(program);
        if table.call_node(expression).is_some() {
            return accepts(expression);
        }
        let mut pending = vec![(expression, false)];
        let mut active = std::collections::BTreeMap::new();
        let mut saw_call = false;
        while let Some((expression, completed)) = pending.pop() {
            if !table.expression_is_valid(expression) {
                return false;
            }
            let coordinate = (expression.arena_index(), expression.generation());
            if completed {
                active.insert(coordinate, false);
                continue;
            }
            match active.get(&coordinate) {
                Some(true) => return false,
                Some(false) => continue,
                None => {}
            }
            active.insert(coordinate, true);
            match table.expression(expression) {
                typed_trees::expression::ExpressionNode::Match(dispatch) => {
                    let arms = table.match_arms(dispatch.arms);
                    if arms.is_empty() {
                        return false;
                    }
                    pending.push((expression, true));
                    pending.extend(arms.iter().rev().map(|arm| (arm.value, false)));
                }
                typed_trees::expression::ExpressionNode::Call(_) => {
                    if !accepts(expression) {
                        return false;
                    }
                    saw_call = true;
                    active.insert(coordinate, false);
                }
                _ => return false,
            }
        }
        saw_call
    }

    fn completion_returns_unit(
        program: &dyn CheckingInput,
        machine: &Machine,
        expression: ExpressionHandle,
    ) -> bool {
        completion_calls_all(program, expression, |leaf| {
            call_returns_unit(program, machine, leaf)
        })
    }

    pub(crate) enum DeclaredCallResult<'program> {
        IntrinsicUnit,
        Signature(TypeReferenceHandle),
        Source {
            machine: &'program Machine,
            state: &'program State,
        },
    }

    fn declared_call_result<'program>(
        program: &'program dyn CheckingInput,
        machine: &Machine,
        expression: ExpressionHandle,
    ) -> Option<DeclaredCallResult<'program>> {
        let call = expression_table(program).call_node(expression)?;
        if admitted_build_declaration_call(program, expression) {
            return Some(DeclaredCallResult::IntrinsicUnit);
        }
        if !call.target_symbol.is_valid()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
        {
            return None;
        }
        if let Some(signature) = program.requirement_call_signature(machine.symbol, call) {
            return Some(DeclaredCallResult::Signature(signature.return_type));
        }
        if call.static_requirement_dispatch.is_some() {
            return None;
        }
        if let Some((machine, state)) = super::find_machine_head(program, call.target_symbol)
            .or_else(|| super::find_state(program, call.target_symbol))
        {
            return Some(DeclaredCallResult::Source { machine, state });
        }
        if let Some((owner, signature)) = program.machine_parameter_signature(call.target_symbol) {
            return (owner.symbol == machine.symbol)
                .then_some(DeclaredCallResult::Signature(signature.return_type));
        }
        super::call_target_return_type(program, call.target_symbol)
            .map(DeclaredCallResult::Signature)
    }

    pub fn call_returns_unit(
        program: &dyn CheckingInput,
        machine: &Machine,
        expression: ExpressionHandle,
    ) -> bool {
        match declared_call_result(program, machine, expression) {
            Some(DeclaredCallResult::IntrinsicUnit) => true,
            Some(DeclaredCallResult::Signature(reference)) => unit_type(program, reference),
            Some(DeclaredCallResult::Source { machine, state }) => {
                unit_type(program, state.return_type)
                    && (state.return_type.is_valid()
                        || !has_inferred_value_return(program, machine))
            }
            None => false,
        }
    }

    pub(crate) fn admitted_build_declaration_call(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
        let Some(call) = expression_table(program).call_node(expression) else {
            return false;
        };
        let (intrinsic, static_count, value_count) =
            match BuildOperation::from_call_target(call.target.as_str()) {
                Some(BuildOperation::AssumedBindingContract) => (
                    AuthoredDeclarationSelectionIntrinsic::BuildAssumedBindingContract,
                    2..=2,
                    1..=1,
                ),
                Some(BuildOperation::ProviderSelection) => (
                    AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection,
                    2..=2,
                    1..=2,
                ),
                Some(BuildOperation::ServiceExclusion) => (
                    AuthoredDeclarationSelectionIntrinsic::BuildServiceExclusion,
                    1..=1,
                    1..=1,
                ),
                Some(BuildOperation::ServiceAudit) => (
                    AuthoredDeclarationSelectionIntrinsic::BuildServiceAudit,
                    1..=1,
                    1..=1,
                ),
                Some(BuildOperation::TestGroupRegistration) => (
                    AuthoredDeclarationSelectionIntrinsic::BuildTestGroupRegistration,
                    1..=1,
                    1..=1,
                ),
                Some(BuildOperation::TestGroupService) => (
                    AuthoredDeclarationSelectionIntrinsic::BuildTestGroupService,
                    2..=2,
                    1..=1,
                ),
                Some(BuildOperation::BoundaryAcceptance) => (
                    AuthoredDeclarationSelectionIntrinsic::BuildBoundaryAcceptance,
                    1..=1,
                    1..=1,
                ),
                Some(BuildOperation::RepresentationSelection) => (
                    AuthoredDeclarationSelectionIntrinsic::BuildRepresentationSelection,
                    2..=2,
                    1..=1,
                ),
                Some(BuildOperation::WireCompatibilityRequest) => (
                    AuthoredDeclarationSelectionIntrinsic::BuildWireCompatibilityRequest,
                    5..=usize::MAX,
                    1..=1,
                ),
                _ => return false,
            };
        if call.target_symbol.is_valid()
            || !call.receiver.is_valid()
            || !static_count.contains(&call.machine_arguments.len())
            || !value_count.contains(
                &expression_table(program)
                    .expression_handles(call.arguments)
                    .len(),
            )
            || !call.evidence_arguments.is_empty()
            || !call.selects_only_nominal_route()
        {
            return false;
        }
        crate::compiler_operations::authored_call_intrinsic(
            program,
            typed_trees::references::StaticArgumentCall::Expression(expression, call),
        ) == Some(intrinsic)
    }

    pub fn machine_has_unit_result(program: &dyn CheckingInput, machine: &Machine) -> bool {
        let states = program.machine_states(machine);
        !states.is_empty()
            && states
                .iter()
                .all(|state| unit_type(program, state.return_type))
            && !has_inferred_value_return(program, machine)
    }

    fn has_inferred_value_return(program: &dyn CheckingInput, machine: &Machine) -> bool {
        use typed_trees::statement::TransitionTargetNode;
        let mut pending = vec![machine];
        let mut visited = Vec::new();
        while let Some(machine) = pending.pop() {
            if visited.contains(&machine.symbol) {
                continue;
            }
            visited.push(machine.symbol);
            for state in program.machine_states(machine) {
                if !unit_type(program, state.return_type) {
                    return true;
                }
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    for target in transition.targets() {
                        let TransitionTargetNode::Value(expression) =
                            program.statement_table.transition_target(target)
                        else {
                            continue;
                        };
                        if !completion_calls_all(program, *expression, |expression| {
                            match declared_call_result(program, machine, expression) {
                                Some(DeclaredCallResult::IntrinsicUnit) => true,
                                Some(DeclaredCallResult::Signature(reference))
                                    if unit_type(program, reference) =>
                                {
                                    true
                                }
                                Some(DeclaredCallResult::Source { machine, state })
                                    if unit_type(program, state.return_type) =>
                                {
                                    if !state.return_type.is_valid() {
                                        pending.push(machine);
                                    }
                                    true
                                }
                                _ => false,
                            }
                        }) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub(crate) fn unit_type(
        program: &dyn CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> bool {
        loop {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Unit => return true,
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                _ => return false,
            }
        }
    }
}

pub use unit_returns::machine_has_unit_result;
pub(crate) use unit_returns::unit_type;
pub use unit_returns::{
    call_returns_unit, unit_return_call_is_admitted, unit_statement_call_is_admitted,
};

use inline_assembly::{register_read_mnemonic, user_asm_contract};
pub(crate) use typed_trees::source_names::declared_receiver_type_reference;
use typed_trees::source_names::receiver_member_chain;

pub fn call_result_type(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let call = expression_table(program).call_node(expression)?;
    let applied = expression_table(program).constructed_value_type_reference(expression);
    applied
        .is_valid()
        .then_some(applied)
        .or_else(|| resolved_call_result_type(program, call))
}

pub(crate) fn resolved_call_result_type(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<TypeReferenceHandle> {
    if call.layout_query.is_some() {
        return crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
            program,
            symbols::BuiltinTypeAtom::U64,
        );
    }
    if let Some((_, state)) = machine_state_by_symbol(program, call.target_symbol) {
        return state.return_type.is_valid().then_some(state.return_type);
    }
    if let Some(slot) = program.dynamic_slot_by_symbol(call.target_symbol) {
        return slot.return_type.is_valid().then_some(slot.return_type);
    }
    requirement_call_result_type(program, call.target_symbol)
        .or_else(|| topology::asm_intrinsic_result_type(program, call.target_symbol))
}

pub(crate) struct CalleeContract<'program> {
    pub(crate) callable: symbols::SymbolHandle,
    pub(crate) return_type: TypeReferenceHandle,
    pub(crate) parameters: &'program [typed_trees::signature::StateParameter],
    pub(crate) contracts: &'program [typed_trees::signature::SignatureContract],
    pub(crate) leading_operands: usize,
}

pub(crate) fn callee_entry_contract(
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
) -> Option<CalleeContract<'_>> {
    if let Some((machine, state)) = machine_state_by_symbol(program, target) {
        if program.machine_states(machine).first()?.symbol != state.symbol {
            return None;
        }
        return Some(CalleeContract {
            callable: machine.symbol,
            return_type: state.return_type,
            parameters: program.state_parameters(state),
            contracts: program.machine_contracts(machine),
            leading_operands: 0,
        });
    }
    if let Some(return_type) = requirement_call_result_type(program, target) {
        let requirement = program
            .closed_requirement_application(target)
            .map(|application| application.signature)
            .or_else(|| {
                selected_requirement_signature(program, target).map(|(_, signature)| signature)
            })
            .or_else(|| {
                program
                    .retained_static_machine_contract(target)
                    .map(|contract| contract.signature())
            })?;
        return Some(CalleeContract {
            callable: requirement.symbol,
            return_type,
            parameters: program.state_signature_parameters(requirement),
            contracts: program.state_signature_contracts(requirement),
            leading_operands: 0,
        });
    }
    let slot = program.dynamic_slot_by_symbol(target)?;
    let requirement = program.dynamic_slot_requirement(slot)?;
    Some(CalleeContract {
        callable: requirement.symbol,
        return_type: requirement.return_type,
        parameters: program.state_signature_parameters(requirement),
        contracts: program.state_signature_contracts(requirement),
        leading_operands: 1,
    })
}

pub(crate) fn call_contract<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<CalleeContract<'program>> {
    let target = call
        .static_requirement_dispatch
        .as_ref()
        .map_or(call.target_symbol, |dispatch| dispatch.contract_symbol());
    callee_entry_contract(program, target)
}

fn requirement_call_result_type(
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !target.is_valid() {
        return None;
    }
    if let Some(application) = program.closed_requirement_application(target) {
        let signature = application.signature;
        return (signature.type_parameters.is_empty() && signature.return_type.is_valid())
            .then_some(signature.return_type);
    }
    if let Some(contract) = program.retained_static_machine_contract(target) {
        let signature = contract.signature();
        return (signature.type_parameters.is_empty() && signature.return_type.is_valid())
            .then_some(signature.return_type);
    }
    let (definition, signature) = selected_requirement_signature(program, target)?;
    if !definition.type_parameters.is_empty() && signature.symbol == target {
        return None;
    }
    if !signature.type_parameters.is_empty() || !signature.return_type.is_valid() {
        return None;
    }
    Some(signature.return_type)
}

pub(crate) fn selected_float_operation_retains_argument_policy(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> bool {
    let Some(operator) = typed_trees::operator::declaration_by_symbol(program, symbol) else {
        return false;
    };
    if !operator.is_boundary
        || !matches!(
            program.primitive_type_reference(operator.return_type),
            Some(PrimitiveType::F32 | PrimitiveType::F64)
        )
        || !(crate::float_result_semantic_contract(program, operator).is_some()
            || crate::numeric_float_conversion_contract(program, operator).is_some())
    {
        return false;
    }
    crate::proof_contracts::float_projection_bindings::semantic_operations::
        is_sealed_toolchain_float_operator(program, operator)
}

pub(crate) mod topology {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use language_semantics::MachineSupplyMode;
    use symbols::SymbolHandle;
    use typed_trees::expression::TableCallExpression;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;

    pub fn find_state_in_machine(
        program: &dyn CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Option<&State> {
        let machine = program.machine_by_symbol(machine_symbol)?;
        program.state_in_machine(machine, state_symbol)
    }

    pub fn find_state_on_machine<'program>(
        program: &'program dyn CheckingInput,
        machine: &Machine,
        state_symbol: SymbolHandle,
    ) -> Option<&'program State> {
        find_state_in_machine(program, machine.symbol, state_symbol)
    }

    pub fn find_state_for_exit<'program>(
        program: &'program dyn CheckingInput,
        exit: &checked_trees::FlowExitFact,
    ) -> Option<&'program State> {
        find_state_in_machine(program, exit.machine_symbol, exit.state_symbol)
    }

    pub fn find_state(
        program: &dyn CheckingInput,
        state_symbol: SymbolHandle,
    ) -> Option<(&Machine, &State)> {
        typed_trees::calls::declaration_queries::find_state(program.typed_program(), state_symbol)
    }

    pub fn find_machine_head(
        program: &dyn CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<(&Machine, &State)> {
        typed_trees::calls::declaration_queries::find_machine_head(program.typed_program(), symbol)
    }

    pub fn call_target_state(program: &dyn CheckingInput, symbol: SymbolHandle) -> Option<&State> {
        typed_trees::calls::declaration_queries::call_target_state(program.typed_program(), symbol)
    }

    pub fn call_target_entry_symbol(
        program: &dyn CheckingInput,
        symbol: SymbolHandle,
    ) -> SymbolHandle {
        typed_trees::calls::declaration_queries::call_target_entry_symbol(
            program.typed_program(),
            symbol,
        )
    }

    pub fn call_target_has_checked_body_contract(
        program: &dyn CheckingInput,
        symbol: SymbolHandle,
    ) -> bool {
        let Some((machine, _)) =
            find_machine_head(program, symbol).or_else(|| find_state(program, symbol))
        else {
            return false;
        };
        if machine.target_family_interface {
            return target_family_members(program, machine).is_some_and(|members| {
                members.iter().all(|member| {
                    program
                        .machine(member.machine)
                        .is_some_and(|body| body.body_is_present)
                })
            });
        }
        machine.body_is_present
    }

    pub fn terminal_callable_target(
        program: &dyn CheckingInput,
        machine: &Machine,
        target: SymbolHandle,
    ) -> Option<(SymbolHandle, typed_trees::types::TypeReferenceHandle)> {
        typed_trees::calls::declaration_queries::terminal_callable_target(
            program.typed_program(),
            machine,
            target,
        )
    }

    pub fn find_machine_by_entry_state(
        program: &dyn CheckingInput,
        state_symbol: SymbolHandle,
    ) -> Option<(&Machine, &State)> {
        program.machine_by_entry_state_symbol(state_symbol)
    }

    pub fn find_machine(program: &dyn CheckingInput, symbol: SymbolHandle) -> Option<&Machine> {
        if !symbol.is_valid() {
            return None;
        }
        program.machine_by_symbol(symbol).or_else(|| {
            let parent = program.symbols.get(symbol).parent;
            parent
                .is_valid()
                .then(|| program.machine_by_symbol(parent))
                .flatten()
        })
    }

    pub fn call_target_parameters(
        program: &dyn CheckingInput,
        target_state_symbol: SymbolHandle,
    ) -> Option<&[typed_trees::signature::StateParameter]> {
        if let Some(slot) = program.dynamic_slot_by_symbol(target_state_symbol) {
            return Some(&slot.parameters);
        }
        if let Some(state) = call_target_state(program, target_state_symbol) {
            return Some(program.state_parameters(state));
        }

        if let Some((_, signature)) = program.machine_parameter_signature(target_state_symbol) {
            return Some(program.state_signature_parameters(signature));
        }

        if let Some(operator) = program
            .operators()
            .iter()
            .find(|operator| operator.symbol == target_state_symbol)
        {
            return Some(program.operator_parameters(operator));
        }

        super::selected_requirement_signature(program, target_state_symbol)
            .map(|(_, signature)| program.state_signature_parameters(signature))
    }

    pub fn call_target_type_parameters(
        program: &dyn CheckingInput,
        target_state_symbol: SymbolHandle,
    ) -> impl Iterator<Item = &typed_trees::data::TypeParameter> {
        let empty: &[typed_trees::data::TypeParameter] = &[];
        let (local, enclosing) = if target_state_symbol.is_valid()
            && let Some(machine) = program.machine_by_own_or_state_symbol(target_state_symbol)
        {
            (program.machine_type_parameters(machine), empty)
        } else if let Some((machine, _)) = program.machine_parameter_signature(target_state_symbol)
        {
            (program.machine_type_parameters(machine), empty)
        } else if let Some(operator) = program
            .operators()
            .iter()
            .find(|operator| operator.symbol == target_state_symbol)
        {
            (program.operator_type_parameters(operator), empty)
        } else if let Some((definition, signature)) =
            super::selected_requirement_signature(program, target_state_symbol)
        {
            (
                program.state_signature_type_parameters(signature),
                program.trait_type_parameters(definition),
            )
        } else {
            (empty, empty)
        };
        local.iter().chain(enclosing)
    }

    pub fn machine_state_count(program: &dyn CheckingInput) -> usize {
        program
            .machines()
            .iter()
            .map(|machine| program.machine_states(machine).len())
            .sum()
    }

    pub fn machine_state_by_symbols(
        program: &dyn CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Option<(&Machine, &State)> {
        let machine = program.machine_by_symbol(machine_symbol)?;
        let state = find_state_in_machine(program, machine.symbol, state_symbol)?;
        Some((machine, state))
    }

    pub fn call_target_return_type(
        program: &dyn CheckingInput,
        target_state_symbol: SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        typed_trees::calls::declaration_queries::call_target_return_type(
            program.typed_program(),
            target_state_symbol,
        )
    }

    pub(crate) fn asm_intrinsic_result_type(
        program: &dyn CheckingInput,
        target: SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        typed_trees::calls::declaration_queries::asm_intrinsic_result_type(
            program.typed_program(),
            target,
        )
    }

    pub fn call_target_symbol(
        program: &dyn CheckingInput,
        call: &TableCallExpression,
    ) -> SymbolHandle {
        if let Some(request) = &call.quotient_operation {
            return request.representative_operation.symbol;
        }
        typed_trees::operator::resolve_named_expression_call(program, call)
            .map_or(call.target_symbol, |operator| operator.symbol)
    }

    pub fn target_family_members<'program>(
        program: &'program dyn CheckingInput,
        interface: &Machine,
    ) -> Option<&'program [typed_trees::machine::TargetMachineFamilyMember]> {
        if !interface.target_family_interface {
            return None;
        }
        let mut families = program
            .target_machine_families
            .iter()
            .filter(|family| family.canonical == interface.symbol);
        let family = families.next()?;
        if families.next().is_some() || family.members.is_empty() {
            return None;
        }
        for (index, member) in family.members.iter().enumerate() {
            let machine = program.machine(member.machine)?;
            if machine.target_family_interface
                || machine.authored_target.as_ref() != Some(&member.target)
                || program.machine_states(machine).is_empty()
                || family.members[..index]
                    .iter()
                    .any(|prior| prior.machine == member.machine || prior.target == member.target)
            {
                return None;
            }
        }
        Some(&family.members)
    }

    pub struct CompletingTransfer<'program> {
        pub state: &'program State,
        pub target: SymbolHandle,
        pub arguments: &'program [typed_trees::expression::ExpressionHandle],
        pub expression: typed_trees::expression::ExpressionHandle,
    }

    pub struct CompletingInvocation<'program> {
        pub target: SymbolHandle,
        pub arguments: &'program [typed_trees::expression::ExpressionHandle],
        pub expression: typed_trees::expression::ExpressionHandle,
    }

    fn completing_call_invocation(
        program: &dyn CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<CompletingInvocation<'_>> {
        let call = expression_table(program).call_node(expression)?;
        if call.receiver.is_valid() && !program.call_has_no_runtime_receiver_for_target(call) {
            return None;
        }
        let arguments = expression_table(program).expression_handles(call.arguments);
        (arguments.len() == call_target_parameters(program, call.target_symbol)?.len()).then_some(
            CompletingInvocation {
                target: call.target_symbol,
                arguments,
                expression,
            },
        )
    }

    pub fn completing_transition_invocation(
        program: &dyn CheckingInput,
        target: typed_trees::statement::TransitionTargetHandle,
    ) -> Option<CompletingInvocation<'_>> {
        match program.statement_table.transition_target(target) {
            typed_trees::statement::TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                let arguments = program.statement_table.expression_handles(*arguments);
                (arguments.len() == call_target_parameters(program, path.symbol)?.len()).then_some(
                    CompletingInvocation {
                        target: path.symbol,
                        arguments,
                        expression: Default::default(),
                    },
                )
            }
            typed_trees::statement::TransitionTargetNode::Value(expression) => {
                completing_call_invocation(program, *expression)
            }
            typed_trees::statement::TransitionTargetNode::Terminal => None,
        }
    }

    pub fn completing_transition_invocations(
        program: &dyn CheckingInput,
        target: typed_trees::statement::TransitionTargetHandle,
    ) -> Vec<CompletingInvocation<'_>> {
        use typed_trees::expression::ExpressionNode;
        use typed_trees::statement::TransitionTargetNode;
        let root = match program.statement_table.transition_target(target) {
            TransitionTargetNode::Named { .. } => {
                return completing_transition_invocation(program, target)
                    .into_iter()
                    .collect();
            }
            TransitionTargetNode::Value(expression) => *expression,
            TransitionTargetNode::Terminal => return Vec::new(),
        };
        let mut pending = vec![root];
        let mut visited = Vec::new();
        let mut invocations = Vec::new();
        while let Some(expression) = pending.pop() {
            if visited.contains(&expression) {
                continue;
            }
            visited.push(expression);
            match expression_table(program).expression(expression) {
                ExpressionNode::Match(dispatch) => {
                    pending.extend(
                        expression_table(program)
                            .match_arms(dispatch.arms)
                            .iter()
                            .rev()
                            .map(|arm| arm.value),
                    );
                }
                ExpressionNode::Call(_) => {
                    invocations.extend(completing_call_invocation(program, expression));
                }
                _ => {}
            }
        }
        invocations
    }

    fn concrete_transfer<'program>(
        program: &'program dyn CheckingInput,
        invocation: CompletingInvocation<'program>,
    ) -> Option<CompletingTransfer<'program>> {
        if let Some(slot) = program.dynamic_slot_by_symbol(invocation.target) {
            let state = unique_dispatched_entry(program, slot)?;
            let arguments = invocation.arguments.get(1..)?;
            if arguments.len() != program.state_parameters(state).len() {
                return None;
            }
            return Some(CompletingTransfer {
                state,
                target: state.symbol,
                arguments,
                expression: invocation.expression,
            });
        }
        Some(CompletingTransfer {
            state: call_target_state(program, invocation.target)?,
            target: invocation.target,
            arguments: invocation.arguments,
            expression: invocation.expression,
        })
    }

    pub fn unique_dispatched_entry<'program>(
        program: &'program dyn CheckingInput,
        slot: &typed_trees::dynamic_traits::DynamicSlotSignature,
    ) -> Option<&'program State> {
        let selections = crate::collect_dynamic_implementation_selections(program).ok()?;
        let mut entries = selections
            .iter()
            .filter(|selection| {
                program.normalized_type_identity(selection.selector_type) == slot.interface_identity
            })
            .filter_map(|selection| {
                let row = program
                    .conformance_by_symbol(selection.implementation)?
                    .rows
                    .iter()
                    .find(|row| row.requirement == slot.requirement)?;
                let machine = program.machine_by_symbol(row.realization_machine)?;
                program.machine_states(machine).first()
            });
        let entry = entries.next()?;
        entries.next().is_none().then_some(entry)
    }

    pub fn completing_call_transfer(
        program: &dyn CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<CompletingTransfer<'_>> {
        concrete_transfer(program, completing_call_invocation(program, expression)?)
    }

    pub fn completing_transition_transfer(
        program: &dyn CheckingInput,
        target: typed_trees::statement::TransitionTargetHandle,
    ) -> Option<CompletingTransfer<'_>> {
        concrete_transfer(program, completing_transition_invocation(program, target)?)
    }
}
pub use topology::target_family_members;
pub use topology::{
    CompletingTransfer, call_target_entry_symbol, call_target_has_checked_body_contract,
    call_target_parameters, call_target_return_type, call_target_state, call_target_symbol,
    call_target_type_parameters, completing_call_transfer, completing_transition_invocation,
    completing_transition_invocations, completing_transition_transfer, find_machine,
    find_machine_by_entry_state, find_machine_head, find_state, find_state_for_exit,
    find_state_in_machine, find_state_on_machine, machine_state_by_symbols, machine_state_count,
    terminal_callable_target, unique_dispatched_entry,
};
pub(crate) mod receiver_topology {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use crate::machine_symbol_from_type_reference_handle;
    use ::symbols::SymbolHandle;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode, NamePath};
    use checked_trees::name::Identifier;
    use checked_trees::statement::TableCall;

    pub fn statement_call_can_dispatch_to_machine(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        call: &TableCall,
    ) -> bool {
        resolve_state_call_target(
            program,
            machine,
            state,
            call.receiver_symbol,
            call.target_symbol,
            statement_call_receiver_members(program, call),
            &call.target,
        )
        .is_valid()
            || receiver_can_dispatch_to_machine(
                program,
                machine,
                state,
                call.receiver_symbol,
                statement_call_receiver_members(program, call),
            )
    }

    pub fn statement_call_receiver_members<'a>(
        program: &'a dyn CheckingInput,
        call: &TableCall,
    ) -> Option<&'a [Identifier]> {
        (!call.receiver.is_empty())
            .then(|| program.statement_table.name_path_members(call.receiver))
    }

    pub fn statement_call_receiver_path(
        program: &dyn CheckingInput,
        state: &typed_trees::state::State,
        before: usize,
        call: &TableCall,
    ) -> Option<NamePath> {
        let members = statement_call_receiver_members(program, call)?;

        if let Some((root, segments)) =
            crate::projected_statement_receiver_place(program, state, before, call)
        {
            let mut path = NamePath::resolved_from_iter(members[..1].iter().cloned(), root, root);
            for (member, segment) in members[1..].iter().zip(segments) {
                let facts::PlaceSegment::Field { symbol } = segment else {
                    return None;
                };
                path.push_resolved(member.clone(), symbol);
            }
            return Some(path);
        }

        Some(NamePath::resolved_from_iter(
            members.iter().cloned(),
            call.receiver_root_symbol,
            call.receiver_symbol,
        ))
    }

    pub fn call_receiver_parts(
        program: &dyn CheckingInput,
        receiver: ExpressionHandle,
    ) -> (SymbolHandle, Option<checked_trees::expression::NamePath>) {
        if !receiver.is_valid() {
            return (SymbolHandle::invalid(), None);
        }

        match expression_table(program).expression(receiver) {
            ExpressionNode::Borrow(inner) => call_receiver_parts(program, inner.target),
            ExpressionNode::Name(path) => (
                resolve_name_path_member_symbol(
                    program,
                    path,
                    expression_table(program)
                        .name_path_members(path.members)
                        .len()
                        .saturating_sub(1),
                ),
                Some(NamePath::resolved_from_iter(
                    expression_table(program)
                        .name_path_members(path.members)
                        .iter()
                        .cloned(),
                    path.head_symbol,
                    resolve_name_path_member_symbol(
                        program,
                        path,
                        expression_table(program)
                            .name_path_members(path.members)
                            .len()
                            .saturating_sub(1),
                    ),
                )),
            ),
            ExpressionNode::Member(member) => {
                let (_, path) = call_receiver_parts(program, member.receiver);
                let mut path = path.unwrap_or_default();
                let member_symbol =
                    crate::effective_member_symbol(program, member.receiver, member);
                path.push_resolved(member.member.clone(), member_symbol);
                (member_symbol, Some(path))
            }
            _ => (SymbolHandle::invalid(), None),
        }
    }

    pub fn resolve_name_path_member_symbol(
        program: &dyn CheckingInput,
        path: &typed_trees::expression::TableNamePath,
        target_index: usize,
    ) -> SymbolHandle {
        let members = expression_table(program).name_path_members(path.members);
        let authored = expression_table(program).name_path_member_symbols(path.member_symbols);
        if let Some(declaration) = authored
            .get(target_index)
            .copied()
            .filter(|declaration| declaration.is_valid())
        {
            return declaration;
        }
        let mut selected = SymbolHandle::invalid();

        for (index, member) in members.iter().enumerate() {
            let mut direct = authored
                .get(index)
                .copied()
                .unwrap_or_else(SymbolHandle::invalid);
            if !direct.is_valid() && index == 0 {
                direct = path.head_symbol;
            }
            if !direct.is_valid() && index + 1 == members.len() {
                direct = path.symbol;
            }
            selected = if direct.is_valid() {
                direct
            } else {
                crate::symbol_type_symbol(program, selected)
                    .and_then(|type_symbol| {
                        crate::resolve_member_symbol_from_type_symbol(
                            program,
                            type_symbol,
                            member.as_str(),
                        )
                    })
                    .unwrap_or_else(SymbolHandle::invalid)
            };
            if index == target_index {
                return selected;
            }
            if !selected.is_valid() {
                break;
            }
        }
        SymbolHandle::invalid()
    }

    pub fn resolve_state_call_target(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        receiver_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
        receiver: Option<&[Identifier]>,
        target_state: &Identifier,
    ) -> SymbolHandle {
        if let Some(receiver) = receiver {
            let members = receiver
                .iter()
                .map(|member| member.as_str().to_owned())
                .collect::<Vec<_>>();
            if let Some(target) = accessor_target(
                program,
                machine,
                state,
                &members,
                target_symbol,
                target_state.as_str(),
            ) {
                return target;
            }
        }
        if program.symbols.get(receiver_symbol).kind == ::symbols::SymbolKind::ConformanceParameter
            && program
                .symbols
                .child_handles(receiver_symbol)
                .is_some_and(|mut children| children.any(|child| child == target_symbol))
            && let Some((_, signature)) =
                super::selected_requirement_signature(program, target_symbol)
        {
            return signature.symbol;
        }
        if program.dynamic_slot_by_symbol(target_symbol).is_some() {
            return target_symbol;
        }
        if target_symbol.is_valid()
            && program
                .operators()
                .iter()
                .any(|operator| operator.symbol == target_symbol)
        {
            return target_symbol;
        }
        if receiver.is_none() || receiver_symbol == machine.symbol {
            let local = resolve_state_symbol_in_machine(program, machine, target_symbol);
            if local.is_valid() {
                return local;
            }
            if machine_parameter_signature_symbol(program, machine, target_symbol).is_valid() {
                return target_symbol;
            }
            return state_symbol_in_any_machine(program, target_symbol);
        }

        if !receiver_symbol.is_valid() {
            return SymbolHandle::invalid();
        }

        if let Some(target_machine) = program.machine_by_symbol(receiver_symbol) {
            return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
        }

        if program.data_definition_by_symbol(receiver_symbol).is_some() {
            return attached_machine_state_symbol(
                program,
                receiver_symbol,
                target_symbol,
                target_state,
            );
        }

        let type_symbol = program
            .state_parameter_by_symbol(state, receiver_symbol)
            .map(|parameter| {
                machine_symbol_from_type_reference_handle(program, parameter.type_reference)
            })
            .or_else(|| receiver_field_type_machine_symbol(program, machine, receiver_symbol))
            .or_else(|| crate::symbol_type_symbol(program, receiver_symbol))
            .unwrap_or_else(SymbolHandle::invalid);
        if type_symbol.is_valid()
            && let Some(target_machine) = program.machine_by_symbol(type_symbol)
        {
            return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
        }

        let attached =
            attached_machine_state_symbol(program, type_symbol, target_symbol, target_state);
        if attached.is_valid() {
            return attached;
        }

        if program.data_definition_by_symbol(type_symbol).is_some()
            && !trait_machine_signature_symbol(program, target_symbol).is_valid()
        {
            return SymbolHandle::invalid();
        }

        let target_state = state_symbol_in_any_machine(program, target_symbol);
        if target_state.is_valid() {
            return target_state;
        }

        if trait_machine_signature_symbol(program, target_symbol).is_valid() {
            return target_symbol;
        }

        SymbolHandle::invalid()
    }

    pub fn machine_parameter_signature_symbol(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        target_symbol: SymbolHandle,
    ) -> SymbolHandle {
        if target_symbol.is_valid()
            && program
                .machine_parameter_signature_in(machine, target_symbol)
                .is_some()
        {
            return target_symbol;
        }

        SymbolHandle::invalid()
    }

    fn state_symbol_in_any_machine(
        program: &dyn CheckingInput,
        target_symbol: SymbolHandle,
    ) -> SymbolHandle {
        super::call_target_state(program, target_symbol)
            .map(|state| state.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }

    pub fn trait_machine_signature_symbol(
        program: &dyn CheckingInput,
        target_symbol: SymbolHandle,
    ) -> SymbolHandle {
        if program.trait_machine_signature(target_symbol).is_some() {
            return target_symbol;
        }

        SymbolHandle::invalid()
    }

    fn attached_machine_state_symbol(
        program: &dyn CheckingInput,
        data_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
        target_state: &Identifier,
    ) -> SymbolHandle {
        if !data_symbol.is_valid() {
            return SymbolHandle::invalid();
        }
        if !program.data_definition_by_symbol(data_symbol).is_some() {
            return SymbolHandle::invalid();
        }
        for candidate in program
            .machines()
            .iter()
            .filter(|candidate| candidate.attached_data_symbol == data_symbol)
        {
            for state in program.machine_states(candidate) {
                if (target_symbol.is_valid() && state.symbol == target_symbol)
                    || (!target_symbol.is_valid() && state.name == *target_state)
                {
                    return state.symbol;
                }
            }
        }
        SymbolHandle::invalid()
    }

    fn data_type_has_attached_machine(
        program: &dyn CheckingInput,
        data_symbol: SymbolHandle,
    ) -> bool {
        data_symbol.is_valid()
            && program
                .data_definition_by_symbol(data_symbol)
                .is_some_and(|data| {
                    program
                        .machines()
                        .iter()
                        .any(|machine| machine.attached_data.as_ref() == Some(&data.name))
                })
    }

    pub fn receiver_can_dispatch_to_machine(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        receiver_symbol: SymbolHandle,
        receiver: Option<&[Identifier]>,
    ) -> bool {
        if receiver.is_none() || receiver_symbol == machine.symbol {
            return true;
        }

        if !receiver_symbol.is_valid() {
            return false;
        }

        let type_symbol = program
            .state_parameter_by_symbol(state, receiver_symbol)
            .map(|parameter| {
                machine_symbol_from_type_reference_handle(program, parameter.type_reference)
            })
            .or_else(|| receiver_field_type_machine_symbol(program, machine, receiver_symbol))
            .unwrap_or_else(SymbolHandle::invalid);

        program.machine_by_symbol(receiver_symbol).is_some()
            || (type_symbol.is_valid() && program.machine_by_symbol(type_symbol).is_some())
            || data_type_has_attached_machine(program, type_symbol)
            || data_type_has_attached_machine(program, receiver_symbol)
    }

    fn receiver_field_type_machine_symbol(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        receiver_symbol: SymbolHandle,
    ) -> Option<SymbolHandle> {
        program
            .machine_owned_data_by_symbol(machine, receiver_symbol)
            .map(|owned| machine_symbol_from_type_reference_handle(program, owned.type_reference))
            .or_else(|| {
                attached_data_field_type_reference(program, machine, receiver_symbol).map(
                    |type_reference| {
                        machine_symbol_from_type_reference_handle(program, type_reference)
                    },
                )
            })
            .filter(|symbol| symbol.is_valid())
    }

    fn attached_data_field_type_reference(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        receiver_symbol: SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let data = program.attached_data_definition(machine)?;

        program
            .data_field(data, receiver_symbol)
            .map(|field| field.type_reference)
    }

    fn resolve_state_symbol_in_machine(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state_symbol: SymbolHandle,
    ) -> SymbolHandle {
        if !state_symbol.is_valid() {
            return SymbolHandle::invalid();
        }

        if state_symbol == machine.symbol {
            return program
                .machine_states(machine)
                .first()
                .map_or_else(SymbolHandle::invalid, |state| state.symbol);
        }

        program
            .state_in_machine(machine, state_symbol)
            .map(|state| state.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }

    pub(crate) fn accessor_target(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        members: &[String],
        target_symbol: SymbolHandle,
        operation: &str,
    ) -> Option<SymbolHandle> {
        let reference = crate::value_custody::places::declared_member_path_type(
            program,
            machine,
            Some(state),
            members,
        )?;
        let (_, field) = program.access_view_field_plan_for_type_reference(reference)?;
        field
            .accessor_targets
            .iter()
            .find(|accessor| {
                accessor.operation == operation
                    && (!target_symbol.is_valid() || target_symbol == accessor.state_symbol)
            })
            .map(|accessor| accessor.state_symbol)
    }
}
pub(crate) use receiver_topology::accessor_target;
pub use receiver_topology::{
    call_receiver_parts, machine_parameter_signature_symbol, receiver_can_dispatch_to_machine,
    resolve_name_path_member_symbol, resolve_state_call_target,
    statement_call_can_dispatch_to_machine, statement_call_receiver_members,
    statement_call_receiver_path, trait_machine_signature_symbol,
};
pub(crate) mod argument_correspondence {
    use crate::frozen_program::expression_table;
    use std::borrow::Cow;
    use std::cell::OnceCell;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::expression::ExpressionNode;

    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::StateParameter;
    use typed_trees::static_applications::StaticMachineBindingValue;
    use typed_trees::types::TypeReferenceHandle;

    use super::{CallSite, call_site_argument_expressions, requirement_contract_signature};

    pub(crate) mod carriers {
        use crate::frozen_program::expression_table;
        use symbols::SymbolHandle;
        use typed_trees::static_applications::StaticMachineApplicationHandle;
        use typed_trees::type_identity::{
            AppliedTypeIdentityScope, IndexIdentityArgument, applied_declared_carrier_identity,
        };
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) struct Frame<'program> {
            declaration: SymbolHandle,
            application: StaticMachineApplicationHandle,
            arguments: Vec<(SymbolHandle, IndexIdentityArgument<'program>)>,
        }

        pub(crate) struct CarrierCorrespondence<'program> {
            frames: Vec<Frame<'program>>,
            pub(crate) supplier: SymbolHandle,
        }

        impl<'program> CarrierCorrespondence<'program> {
            pub(crate) fn new(
                program: &'program dyn crate::CheckingInput,
                caller: SymbolHandle,
                selection: StaticMachineApplicationHandle,
                application: &'program typed_trees::typed_trees::ClosedConformanceApplication,
                lineage: &[(SymbolHandle, SymbolHandle)],
                authored_application: StaticMachineApplicationHandle,
            ) -> Result<Self, &'static str> {
                let supplying = if selection.is_valid() {
                    selection
                } else {
                    let receipts = program
                        .machine_specializations
                        .iter()
                        .filter(|receipt| receipt.instance == caller)
                        .collect::<Vec<_>>();
                    let [receipt] = receipts.as_slice() else {
                        return Err("selected carrier has no unique supplying invocation");
                    };
                    receipt.binding_application
                };
                if !supplying.is_valid() {
                    return Err("selected carrier lost its supplying application");
                }
                let supplier = expression_table(program)
                    .static_machine_application(supplying)
                    .lexical_owner;
                if !supplier.is_valid() {
                    return Err("selected carrier lost its supplying declaration");
                }
                let mut frames = vec![Frame::from_conformance(program, application)?];
                for pair in lineage.windows(2).rev() {
                    let (instance, template) = (pair[0].0, pair[1].0);
                    let invocations = program
                        .machine_specializations
                        .iter()
                        .filter(|row| row.instance == instance && row.template == template)
                        .collect::<Vec<_>>();
                    let captures = program
                        .captured_machine_applications
                        .iter()
                        .filter(|(_, row)| row.instance == instance && row.template == template)
                        .map(|(_, row)| row)
                        .collect::<Vec<_>>();
                    let application = match (invocations.as_slice(), captures.as_slice()) {
                        ([invocation], []) => {
                            let handle = invocation.binding_application;
                            if !handle.is_valid() {
                                return Err(
                                    "selected carrier invocation lost its binding application",
                                );
                            }
                            let binding =
                                expression_table(program).static_machine_application(handle);
                            if binding.registration_operation != template
                                || binding.binding_declaration != template
                            {
                                return Err("selected carrier invocation names another template");
                            }
                            let map = program
                                .conformance_by_symbol(application.declaration)
                                .ok_or("selected carrier lost its map declaration")?;
                            for binding_value in expression_table(program)
                                .static_machine_application_bindings(handle)
                            {
                                if let Some((parameter, argument)) = program
                                    .conformance_type_parameters(map)
                                    .iter()
                                    .zip(&application.arguments)
                                    .find(|(parameter, _)| {
                                        parameter.symbol == binding_value.parameter
                                    })
                                    && !program.static_binding_matches_argument(
                                        &parameter.kind,
                                        &binding_value.value,
                                        argument,
                                        supplier,
                                        binding.lexical_owner,
                                    )
                                {
                                    return Err(
                                        "selected invocation changes its map's static operand",
                                    );
                                }
                            }
                            handle
                        }
                        ([], [capture]) => {
                            super::super::capture_binding_is_exact(program, capture)?;
                            continue;
                        }
                        _ => return Err("selected carrier frame has no unique application"),
                    };
                    frames.push(Frame::from_application(program, application)?);
                }
                if lineage.is_empty() {
                    return Err("selected carrier has no executable lineage");
                }
                if frames.len() == 1 && authored_application.is_valid() {
                    let template = lineage[0].0;
                    let binding =
                        expression_table(program).static_machine_application(authored_application);
                    if binding.binding_declaration == template {
                        let map = program
                            .conformance_by_symbol(application.declaration)
                            .ok_or("selected carrier lost its map declaration")?;
                        for binding_value in expression_table(program)
                            .static_machine_application_bindings(authored_application)
                        {
                            if let Some((parameter, argument)) = program
                                .conformance_type_parameters(map)
                                .iter()
                                .zip(&application.arguments)
                                .find(|(parameter, _)| parameter.symbol == binding_value.parameter)
                                && !program.static_binding_matches_argument(
                                    &parameter.kind,
                                    &binding_value.value,
                                    argument,
                                    supplier,
                                    binding.lexical_owner,
                                )
                            {
                                return Err("selected invocation changes its map's static operand");
                            }
                        }
                        frames.push(Frame::from_application(program, authored_application)?);
                    }
                }
                Ok(Self { frames, supplier })
            }

            pub(crate) fn result_lifetimes(
                &self,
                program: &dyn crate::CheckingInput,
            ) -> Result<Vec<Vec<(String, String)>>, &'static str> {
                self.frames
                    .iter()
                    .map(|frame| {
                        program
                            .static_application_lifetimes_for_declaration(
                                frame.application,
                                frame.declaration,
                            )
                            .map(|bindings| {
                                bindings
                                    .iter()
                                    .map(|binding| {
                                        (
                                            binding.parameter.as_str().to_owned(),
                                            binding.argument.as_str().to_owned(),
                                        )
                                    })
                                    .collect()
                            })
                    })
                    .collect()
            }

            pub(crate) fn result_scopes<'scope>(
                &'scope self,
                lifetimes: &'scope [Vec<(String, String)>],
            ) -> Vec<AppliedTypeIdentityScope<'scope>> {
                assert_eq!(self.frames.len(), lifetimes.len());
                self.frames
                    .iter()
                    .zip(lifetimes)
                    .map(|(frame, lifetimes)| AppliedTypeIdentityScope {
                        declaration: frame.declaration,
                        arguments: &frame.arguments,
                        lifetimes,
                    })
                    .collect()
            }

            pub(crate) fn identities(
                &self,
                program: &dyn crate::CheckingInput,
                executable: TypeReferenceHandle,
                public: TypeReferenceHandle,
            ) -> (
                Option<typed_trees::type_identity::NormalizedTypeIdentity>,
                Option<typed_trees::type_identity::NormalizedTypeIdentity>,
            ) {
                let scopes = self
                    .frames
                    .iter()
                    .map(|frame| AppliedTypeIdentityScope {
                        declaration: frame.declaration,
                        arguments: &frame.arguments,
                        lifetimes: &[],
                    })
                    .collect::<Vec<_>>();
                let left = applied_declared_carrier_identity(program, executable, &scopes);
                let right = applied_declared_carrier_identity(program, public, &[]);
                (left, right)
            }
        }

        impl<'program> Frame<'program> {
            fn from_conformance(
                program: &'program dyn crate::CheckingInput,
                application: &'program typed_trees::typed_trees::ClosedConformanceApplication,
            ) -> Result<Self, &'static str> {
                let declaration = program
                    .conformance_by_symbol(application.declaration)
                    .ok_or("selected carrier lost its map declaration")?;
                let parameters = program.conformance_type_parameters(declaration);
                if parameters.len() != application.arguments.len()
                    || declaration.lifetime_parameters.len() != application.lifetime_arguments.len()
                {
                    return Err("selected carrier lost its complete map telescope");
                }
                if !application.callable_application.is_valid() {
                    return Err("selected carrier lost its retained map application");
                }
                let bindings = expression_table(program)
                    .static_machine_application_bindings(application.callable_application);
                if bindings.len() != parameters.len()
                    || !parameters
                        .iter()
                        .zip(bindings)
                        .all(|(parameter, binding)| parameter.symbol == binding.parameter)
                {
                    return Err("selected carrier changed its supplying binder order");
                }
                let arguments = parameters
                    .iter()
                    .zip(&application.arguments)
                    .map(|(parameter, argument)| {
                        (
                            parameter.symbol,
                            IndexIdentityArgument::StaticArgument(argument),
                        )
                    })
                    .collect();
                Ok(Self {
                    declaration: declaration.symbol,
                    application: application.callable_application,
                    arguments,
                })
            }

            fn from_application(
                program: &'program dyn crate::CheckingInput,
                handle: StaticMachineApplicationHandle,
            ) -> Result<Self, &'static str> {
                let application = expression_table(program).static_machine_application(handle);
                if !application.lexical_owner.is_valid() {
                    return Err("selected carrier frame lost its lexical supplier");
                }
                let arguments = crate::machine_calls::static_bindings::index_identity_arguments(
                    expression_table(program).static_machine_application_bindings(handle),
                );
                Ok(Self {
                    declaration: application.binding_declaration,
                    application: handle,
                    arguments,
                })
            }
        }
    }
    pub(crate) mod result_domains {
        use super::{
            CallSite, CarrierCorrespondence, SelectedCallArgumentError, selected_call_arguments,
        };
        use symbols::SymbolHandle;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::type_identity::{
            IndexIdentityArgument, applied_index_identity, applied_type_identity,
            applied_type_identity_in_scope,
        };

        pub fn selected_call_result_domain_matches(
            program: &dyn crate::CheckingInput,
            caller: SymbolHandle,
            site: &CallSite<'_>,
            domain_symbol: SymbolHandle,
            implementation: &[typed_trees::types::TypeReferenceHandle],
            receiving: &[typed_trees::types::TypeReferenceHandle],
        ) -> Result<bool, SelectedCallArgumentError> {
            selected_call_arguments(program, caller, site)?;
            let dispatch = site
                .static_requirement_dispatch()
                .ok_or("result domain comparison has no selected application")?;
            let applications = program.selected_conformance_applications(
                caller,
                dispatch.selection_application,
                dispatch.evidence_parameter,
                dispatch.requirement,
                dispatch.application_report_fingerprint,
                dispatch.application_commitment,
            );
            let [application] = applications.as_slice() else {
                return Err("result domain comparison has no unique supplying application".into());
            };
            let target = site.target_symbol(program);
            let lineage = program
                .realization_entry_lineage(target, application)
                .ok_or("result domain comparison lost its realization lineage")?;
            let authored_application = match site {
                CallSite::Expression { call, .. } => call.authored_machine_application,
                CallSite::Statement(call) => call.authored_machine_application,
                CallSite::TransitionNamed { .. } => Default::default(),
            };
            let correspondence = CarrierCorrespondence::new(
                program,
                caller,
                dispatch.selection_application,
                application,
                &lineage,
                authored_application,
            )?;
            let lifetimes = correspondence.result_lifetimes(program)?;
            let scopes = correspondence.result_scopes(&lifetimes);
            let definition = program
                .domain_definition(domain_symbol)
                .ok_or("result domain comparison lost its domain declaration")?;
            let parameters = typed_trees::domain::index_parameters(program, definition);
            if parameters.len() != implementation.len() || parameters.len() != receiving.len() {
                return Err("result domain comparison changed its complete index roster".into());
            }
            if parameters.is_empty() {
                return Ok(false);
            }
            for ((parameter, own), received) in parameters.iter().zip(implementation).zip(receiving)
            {
                let (left, right) = match parameter.kind {
                    TypeParameterKind::Type => (
                        applied_type_identity_in_scope(
                            program,
                            *own,
                            target,
                            correspondence.supplier,
                            &scopes,
                        ),
                        applied_type_identity(program, *received, caller, &[]),
                    ),
                    TypeParameterKind::Const { .. } => (
                        applied_index_identity(
                            program,
                            IndexIdentityArgument::TypeReference(*own),
                            correspondence.supplier,
                            &scopes,
                        ),
                        applied_index_identity(
                            program,
                            IndexIdentityArgument::TypeReference(*received),
                            caller,
                            &[],
                        ),
                    ),
                    _ => return Err("result domain has an ineligible index binder".into()),
                };
                let left = left.ok_or("implementation result index has no applied identity")?;
                let right = right.ok_or("receiving result index has no caller identity")?;
                if left != right {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
    pub use result_domains::selected_call_result_domain_matches;
    pub(crate) mod requirement_realizations {
        use super::{SelectedCallArgument, formal_origin};
        use crate::frozen_program::expression_table;
        use symbols::SymbolHandle;
        use typed_trees::expression::TableCallExpression;
        use typed_trees::static_applications::StaticMachineBindingValue;

        pub(crate) fn requirement_realization_arguments<'program>(
            program: &'program dyn crate::CheckingInput,
            caller: SymbolHandle,
            call: &TableCallExpression,
            association: &crate::ProviderApplicationAssociation,
        ) -> Result<
            (
                &'program typed_trees::machine::Machine,
                &'program typed_trees::state::State,
                Vec<SelectedCallArgument<'program>>,
            ),
            &'static str,
        > {
            let row = &association.identity;
            let application = program
                .closed_requirement_application(call.target_symbol)
                .filter(|application| {
                    application.signature.symbol == row.closed_requirement
                        && application.template == row.template
                        && application.trait_application_identity == row.receiving_application
                })
                .ok_or("provider association belongs to another complete public application")?;
            if call.receiver.is_valid() && !program.call_has_no_runtime_receiver_for_target(call) {
                return Err("public realization has an unaccounted runtime receiver");
            }
            let (_, declared) = program
                .trait_machine_signature(application.template)
                .filter(|(owner, _)| owner.symbol == application.declaring_trait)
                .ok_or("public realization lost its original requirement declaration")?;
            let applied = application.signature;
            let logical = program.state_signature_parameters(applied);
            let declared_parameters = program.state_signature_parameters(declared);
            let declared_binders = program.requirement_invocation_contract_parameters(declared);
            let actuals = expression_table(program).expression_handles(call.arguments);
            if logical.len() != declared_parameters.len() || actuals.len() != logical.len() {
                return Err("public realization lost its complete ordinary argument roster");
            }
            let canonical_source =
                expression_table(program).static_machine_application(application.application);
            let canonical_arguments = expression_table(program)
                .static_machine_application_arguments(application.application);
            let bindings = if call.authored_machine_application.is_valid() {
                let source = expression_table(program)
                    .static_machine_application(call.authored_machine_application);
                if source.lexical_owner != caller {
                    return Err("public application belongs to another lexical caller");
                }
                let current = expression_table(program)
                    .static_machine_application_bindings(call.authored_machine_application);
                let lifetimes = program.static_application_lifetimes_for_declaration(
                    call.authored_machine_application,
                    declared.symbol,
                )?;
                let canonical_lifetimes = program.static_application_lifetimes_for_declaration(
                    application.application,
                    declared.symbol,
                )?;
                if declared_binders.len() != current.len()
                    || current.len() != canonical_arguments.len()
                    || !declared_binders
                        .iter()
                        .zip(current)
                        .zip(canonical_arguments)
                        .all(|((parameter, binding), argument)| {
                            parameter.symbol == binding.parameter
                                && program.static_binding_matches_argument(
                                    &parameter.kind,
                                    &binding.value,
                                    argument,
                                    canonical_source.lexical_owner,
                                    source.lexical_owner,
                                )
                        })
                    || lifetimes != canonical_lifetimes
                    || (!lifetimes.is_empty()
                        && source.lexical_owner != canonical_source.lexical_owner)
                {
                    return Err("public application changed its exact retained invocation tuple");
                }
                current
            } else {
                if !declared_binders.is_empty()
                    || !canonical_arguments.is_empty()
                    || !program
                        .static_application_lifetimes_for_declaration(
                            application.application,
                            declared.symbol,
                        )?
                        .is_empty()
                {
                    return Err("public application has no complete source header");
                }
                &[]
            };
            let provider = program
                .machine_by_symbol(row.provider)
                .filter(|machine| {
                    machine.body_is_present
                        && machine.supply_mode.is_checked_body()
                        && program.machine_type_parameters(machine).is_empty()
                })
                .ok_or("public realization has no closed checked body")?;
            let (realization_machine, realization_state) =
                if let Some(selecting) = program.conformance_by_symbol(row.selecting_declaration) {
                    let mut entries = selecting.rows.iter().filter(|member| {
                        member.declaring_trait == application.declaring_trait
                            && member.requirement == application.template
                            && program.requirement_realization_matches(
                                call.target_symbol,
                                member.realization_machine,
                                provider.symbol,
                            )
                    });
                    let member = entries
                        .next()
                        .ok_or("public realization lost its selecting map row")?;
                    if entries.next().is_some() {
                        return Err("public realization has ambiguous selecting map rows");
                    }
                    (member.realization_machine, member.realization_state)
                } else {
                    if row.selecting_declaration != provider.symbol {
                        return Err(
                            "direct provider association belongs to another selecting declaration",
                        );
                    }
                    let mut receipts = program
                        .machine_specializations
                        .iter()
                        .filter(|receipt| receipt.instance == provider.symbol);
                    let template = receipts
                        .next()
                        .map_or(provider.symbol, |receipt| receipt.template);
                    if receipts.next().is_some() {
                        return Err("direct provider has ambiguous invocation origins");
                    }
                    let source = program
                        .machine_by_symbol(template)
                        .ok_or("direct provider lost its source declaration")?;
                    (
                        source.symbol,
                        program
                            .machine_states(source)
                            .first()
                            .ok_or("direct provider lost its source entry")?
                            .symbol,
                    )
                };
            let mut entries = program.machine_states(provider).iter().filter_map(|entry| {
                program
                    .requirement_realization_entry_lineage(
                        entry.symbol,
                        call.target_symbol,
                        row.selecting_declaration,
                        realization_machine,
                    )
                    .filter(|lineage| {
                        lineage.last().copied() == Some((realization_machine, realization_state))
                    })
                    .map(|lineage| (entry, lineage))
            });
            let (entry, lineage) = entries
                .next()
                .ok_or("public realization lost its exact authored entry lineage")?;
            if entries.next().is_some() {
                return Err("public realization has ambiguous authored entry copies");
            }
            let source = program
                .state_by_symbol(realization_state)
                .ok_or("public realization lost its authored state")?;
            let source_parameters = program.state_parameters(source);
            let parameters = program.state_parameters(entry);
            if bindings.iter().any(|binding| {
                matches!(
                    binding.value,
                    StaticMachineBindingValue::RuntimeValue { .. }
                )
            }) || source_parameters.len() != logical.len()
                || parameters.len() != logical.len()
            {
                return Err("closed public realization has unaccounted executable Value operands");
            }
            let mut mapped = Vec::with_capacity(parameters.len());
            for (position, parameter) in parameters.iter().enumerate() {
                let origin = formal_origin(program, &lineage, parameter.symbol)?;
                if source_parameters[position].symbol != origin
                    || program.applied_signature_parameter(
                        declared,
                        applied,
                        declared_parameters[position].symbol,
                    ) != Some(logical[position].symbol)
                {
                    return Err(
                        "public realization changed its authored ordinary formal correspondence",
                    );
                }
                let logical_parameter = logical[position].symbol;
                let logical_type_reference = logical[position].type_reference;
                let expression = actuals[position];
                if !crate::declared_carriers_match(
                    program,
                    parameter.type_reference,
                    logical_type_reference,
                ) {
                    return Err(
                        "public realization carrier differs from its exact applied contract",
                    );
                }
                mapped.push(SelectedCallArgument {
                    executable_parameter: parameter,
                    logical_parameter,
                    logical_type_reference,
                    public_position: Some(position),
                    expression,
                });
            }
            Ok((provider, entry, mapped))
        }
    }
    use carriers::CarrierCorrespondence;
    pub(crate) use requirement_realizations::requirement_realization_arguments;

    #[derive(Debug)]
    pub struct SelectedCallArgumentError {
        reason: &'static str,
        detail: Option<String>,
    }

    impl From<&'static str> for SelectedCallArgumentError {
        fn from(reason: &'static str) -> Self {
            Self {
                reason,
                detail: None,
            }
        }
    }

    impl std::fmt::Display for SelectedCallArgumentError {
        fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            output.write_str(self.reason)?;
            if let Some(detail) = &self.detail {
                write!(output, " [{detail}]")?;
            }
            Ok(())
        }
    }

    pub struct SelectedCallArgument<'program> {
        pub executable_parameter: &'program StateParameter,
        pub logical_parameter: SymbolHandle,
        pub logical_type_reference: TypeReferenceHandle,
        pub public_position: Option<usize>,
        pub expression: ExpressionHandle,
    }

    pub(crate) fn executable_parameters<'program>(
        program: &'program dyn crate::CheckingInput,
        caller: SymbolHandle,
        site: &CallSite<'program>,
    ) -> Result<Cow<'program, [StateParameter]>, SelectedCallArgumentError> {
        if site.static_requirement_dispatch().is_none() {
            return site
                .parameters(program, caller)
                .map(Cow::Borrowed)
                .ok_or_else(|| {
                    SelectedCallArgumentError::from("call has no retained parameter telescope")
                });
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

    pub fn selected_call_arguments<'program>(
        program: &'program dyn crate::CheckingInput,
        caller: SymbolHandle,
        site: &CallSite<'program>,
    ) -> Result<Vec<SelectedCallArgument<'program>>, SelectedCallArgumentError> {
        let dispatch = site
            .static_requirement_dispatch()
            .ok_or("argument correspondence requires a retained selected application")?;
        let (_, applied) =
            super::generic_requirement::resolve_requirement_contract(program, caller, site)?;
        let (_, declared) = program
            .trait_machine_signature(dispatch.requirement)
            .filter(|(owner, _)| owner.symbol == dispatch.declaring_trait)
            .ok_or("selected call has no exact authored requirement")?;
        let logical = program
            .applied_signature_authored_parameters(declared, applied)
            .ok_or("selected call lost its exact authored public formal projection")?;
        let state = program
            .state_by_symbol(site.target_symbol(program))
            .ok_or("selected call has no executable entry")?;
        let parameters = program.state_parameters(state);
        let actuals = call_site_argument_expressions(program, site);
        if parameters.len() != actuals.len() {
            return Err("selected call has different executable formal and actual counts".into());
        }
        let applications = program.selected_conformance_applications(
            caller,
            dispatch.selection_application,
            dispatch.evidence_parameter,
            dispatch.requirement,
            dispatch.application_report_fingerprint,
            dispatch.application_commitment,
        );
        let [application] = applications.as_slice() else {
            return Err("selected call has no unique supplying application".into());
        };
        let lineage = program
            .realization_entry_lineage(state.symbol, application)
            .ok_or("selected call lost its capture or invocation lineage")?;
        if lineage.last().copied()
            != Some((dispatch.realization_machine, dispatch.realization_state))
        {
            return Err("selected call belongs to another authored realization".into());
        }
        let authored_application = match site {
            CallSite::Expression { call, .. } => call.authored_machine_application,
            CallSite::Statement(call) => call.authored_machine_application,
            CallSite::TransitionNamed { .. } => Default::default(),
        };
        let carriers = CarrierCorrespondence::new(
            program,
            caller,
            dispatch.selection_application,
            application,
            &lineage,
            authored_application,
        )?;
        let source = program
            .state_by_symbol(dispatch.realization_state)
            .ok_or("selected call has no authored realization entry")?;
        let source_parameters = program.state_parameters(source);
        let declared_parameters = program.state_signature_parameters(declared);
        if source_parameters.len() != declared_parameters.len()
            || logical.len() != declared_parameters.len()
        {
            return Err("selected call lost its admitted authored parameter correspondence".into());
        }
        let runtime_bindings = if authored_application.is_valid() {
            if expression_table(program)
                .static_machine_application(authored_application)
                .lexical_owner
                != caller
            {
                return Err("selected Value application belongs to another lexical owner".into());
            }
            expression_table(program)
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
            return Err("selected call lost its complete runtime Value argument roster".into());
        }
        let mut mapped = Vec::with_capacity(parameters.len());
        let mut realized_binders = Vec::with_capacity(runtime_bindings.len());
        for (position, (parameter, expression)) in parameters.iter().zip(actuals).enumerate() {
            let origin = formal_origin(program, &lineage, parameter.symbol)?;
            let (logical_parameter, logical_type_reference, public_position) = if position
                < logical.len()
            {
                if source_parameters[position].symbol != origin {
                    return Err(
                        "selected call changed its authored ordinary parameter order".into(),
                    );
                }
                let public = &logical[position];
                if program.applied_signature_parameter(
                    declared,
                    applied,
                    declared_parameters[position].symbol,
                ) != Some(public.symbol)
                {
                    return Err("selected call lost its applied public formal identity".into());
                }
                (public.symbol, public.type_reference, Some(position))
            } else {
                let binding = runtime_bindings[position - logical.len()];
                if realized_binders.contains(&origin) {
                    return Err(
                        "selected call changed or duplicated its realized Value binder order"
                            .into(),
                    );
                }
                realized_binders.push(origin);
                let StaticMachineBindingValue::RuntimeValue {
                    type_reference,
                    subject,
                    expression: bound_expression,
                } = binding.value
                else {
                    return Err("selected Value argument lost its runtime source binding".into());
                };
                if !runtime_source_matches(program, subject, bound_expression, *expression) {
                    return Err(
                        "selected Value argument belongs to another source expression".into(),
                    );
                }
                let source_machine = program
                    .machine_by_symbol(dispatch.realization_machine)
                    .ok_or("selected Value argument lost its authored machine")?;
                let source_binders = program.requirement_callable_parameters(
                    declared,
                    &program.machine_callable_parameters(source_machine),
                );
                let declared_binders = program.requirement_invocation_contract_parameters(declared);
                if source_binders.len() != declared_binders.len() {
                    return Err("selected Value argument lost its admitted member telescope".into());
                }
                let authored_binder = authored_value_binder(program, &lineage, origin)?;
                let binder_position = source_binders
                    .iter()
                    .position(|binder| binder.symbol == authored_binder)
                    .ok_or("selected Value argument is not an authored member binder")?;
                let binding_declaration = expression_table(program)
                    .static_machine_application(authored_application)
                    .binding_declaration;
                let retained_parameter = if program.machine_by_symbol(binding_declaration).is_some()
                {
                    origin
                } else {
                    let (source_trait, source_signature) = program
                        .trait_machine_signature(binding_declaration)
                        .ok_or("selected Value header has no exact source declaration")?;
                    if source_trait.symbol != dispatch.declaring_trait
                        || (source_signature.symbol != declared.symbol
                            && program
                                .closed_requirement_application(source_signature.symbol)
                                .is_none_or(|closed| {
                                    closed.template != declared.symbol
                                        || closed.declaring_trait != dispatch.declaring_trait
                                }))
                    {
                        return Err("selected Value header belongs to another requirement".into());
                    }
                    program
                        .applied_signature_parameter(
                            declared,
                            source_signature,
                            declared_binders[binder_position].symbol,
                        )
                        .ok_or("selected Value header lost its exact public source binder")?
                };
                if binding.parameter != retained_parameter {
                    return Err("selected Value operand belongs to another source binder".into());
                }
                let logical_parameter = program
                    .applied_signature_parameter(
                        declared,
                        applied,
                        declared_binders[binder_position].symbol,
                    )
                    .ok_or("selected Value argument lost its applied public binder")?;
                let public_binder = program
                    .requirement_invocation_contract_parameters(applied)
                    .iter()
                    .find(|binder| binder.symbol == logical_parameter)
                    .ok_or("selected Value argument has no applied public carrier")?;
                let TypeParameterKind::Value {
                    type_reference: public_type,
                } = public_binder.kind
                else {
                    return Err(
                        "selected runtime argument does not correspond to a public Value binder"
                            .into(),
                    );
                };
                if !crate::declared_carriers_match(program, type_reference, public_type) {
                    return Err(
                        "selected runtime Value binding differs from its public carrier".into(),
                    );
                }
                (logical_parameter, public_type, None)
            };
            let (executable_identity, public_identity) =
                carriers.identities(program, parameter.type_reference, logical_type_reference);
            if !executable_identity
                .as_ref()
                .zip(public_identity.as_ref())
                .is_some_and(|(left, right)| left == right)
            {
                return Err(SelectedCallArgumentError {
                    reason: "selected executable carrier differs from its exact public application",
                    detail: Some(format!(
                        "executable: {executable_identity:?}; public: {public_identity:?}"
                    )),
                });
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
                .any(|(_, capture)| {
                    capture.instance == frames[0].0 && capture.template == frames[1].0
                })
            {
                return Err("selected Value binder has no exact capture frame");
            }
            let original = program.symbols.get(binder).generated_from;
            if !original.is_valid()
                || program.symbols.get(original).parent != frames[1].0
                || !program
                    .data_type_parameter_by_symbol(original)
                    .is_some_and(|parameter| {
                        matches!(parameter.kind, TypeParameterKind::Value { .. })
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
                                && matches!(binder.kind, TypeParameterKind::Value { .. })
                        })
            });
            if realized {
                return Ok(original);
            }
            return Err("selected formal clone does not belong to its exact template frame");
        }
        Ok(parameter)
    }

    pub(crate) struct CallBindingCorrespondence {
        pub declaration: SymbolHandle,
        pub public_owner: SymbolHandle,
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
        let application = expression_table(program).static_machine_application(handle);
        if application.lexical_owner != caller {
            return Err("the application header belongs to another caller");
        }
        let Some(declaration) = program.machine_by_symbol(application.binding_declaration) else {
            return requirement_binding_correspondence(program, caller, site, handle);
        };
        let header = program.machine_type_parameters(declaration);
        let retained = expression_table(program).static_machine_application_bindings(handle);
        let mut parameters = Vec::with_capacity(header.len());
        let mut runtime_parameters = Vec::new();
        let public_owner;
        if let Some(dispatch) = site.static_requirement_dispatch() {
            let arguments =
                selected_call_arguments(program, caller, site).map_err(|error| error.reason)?;
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
            let declared_header = program.requirement_invocation_contract_parameters(declared);
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
                    .requirement_invocation_contract_parameters(public)
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
                        return Err(
                            "the runtime header binder has no authenticated executable actual",
                        );
                    }
                    runtime_parameters.push(parameter.symbol);
                }
                parameters.push((parameter.symbol, logical));
            }
        } else {
            let (machine, state) = crate::find_state(program, site.target_symbol(program))
                .ok_or("the application has no exact executable entry")?;
            public_owner = state.symbol;
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
            let realized_runtime_header = header
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
                .filter(|parameter| {
                    !program
                        .machine_type_parameters(machine)
                        .iter()
                        .any(|logical| {
                            application_parameter_origin(program, &lineage, logical.symbol).ok()
                                == Some(parameter.symbol)
                        })
                })
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>();
            if executable.len() != actuals.len()
                || executable.len() != ordinary.len() + realized_runtime_header.len()
                || !executable.iter().take(ordinary.len()).zip(ordinary).all(
                    |(actual, declared)| {
                        application_parameter_origin(program, &lineage, actual.symbol).ok()
                            == Some(declared.symbol)
                    },
                )
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
                    if logical.is_valid() {
                        if !matches!(parameter.kind, TypeParameterKind::Value { .. }) {
                            return Err(
                                "the open runtime operand does not belong to a Value binder",
                            );
                        }
                        open_runtime_source(program, caller, site, &binding.value)?;
                        runtime_parameters.push(parameter.symbol);
                        parameters.push((parameter.symbol, logical));
                        continue;
                    }
                    let position = realized_runtime_header
                        .iter()
                        .position(|symbol| *symbol == parameter.symbol)
                        .ok_or("the runtime header binder has no retained ordinal")?;
                    let index = ordinary.len() + position;
                    let formal = &executable[index];
                    let actual = actuals[index];
                    if application_parameter_origin(program, &lineage, formal.symbol)?
                        != parameter.symbol
                    {
                        return Err("the runtime argument's formal has another source binder");
                    }
                    if formal.is_const || formal.is_mutable {
                        return Err("the runtime argument's formal is not an immutable value");
                    }
                    if !matches!(parameter.kind, TypeParameterKind::Value { .. }) {
                        return Err("the runtime argument does not belong to a Value binder");
                    }
                    if !crate::declared_carriers_match(
                        program,
                        type_reference,
                        formal.type_reference,
                    ) {
                        return Err(
                            "the runtime argument's carrier differs from its source binding",
                        );
                    }
                    if !runtime_source_matches(program, subject, expression, actual) {
                        return Err(
                            "the runtime argument differs from its retained source occurrence",
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

    pub fn authored_call_parameters<'program>(
        program: &'program dyn crate::CheckingInput,
        caller: SymbolHandle,
        site: &CallSite<'_>,
    ) -> Result<(SymbolHandle, &'program [StateParameter]), &'static str> {
        if site.static_requirement_dispatch().is_some() {
            return Err("a selected requirement has a separate public parameter signature");
        }
        let correspondence = call_binding_correspondence(program, caller, site)?;
        let (machine, state) = crate::find_state(program, site.target_symbol(program))
            .ok_or("the application has no exact executable entry")?;
        let lineage = application_lineage(program, machine.symbol, correspondence.declaration)?;
        let source_symbol = application_parameter_origin(program, &lineage, state.symbol)?;
        let source_state = program
            .state_by_symbol(source_symbol)
            .ok_or("the executable entry lost its exact source state")?;
        Ok((
            correspondence.declaration,
            program.state_parameters(source_state),
        ))
    }

    fn requirement_binding_correspondence(
        program: &dyn crate::CheckingInput,
        caller: SymbolHandle,
        site: &CallSite<'_>,
        handle: typed_trees::static_applications::StaticMachineApplicationHandle,
    ) -> Result<CallBindingCorrespondence, &'static str> {
        let selected_arguments = site
            .static_requirement_dispatch()
            .map(|_| selected_call_arguments(program, caller, site).map_err(|error| error.reason))
            .transpose()?;
        let application = expression_table(program).static_machine_application(handle);
        let (declared, public, declaration) = if let Some(signature) =
            program.machine_by_symbol(caller).and_then(|machine| {
                program.machine_parameter_signature_in(machine, application.binding_declaration)
            }) {
            if application.registration_operation != application.binding_declaration
                || site.target_symbol(program) != application.binding_declaration
            {
                return Err("the callable parameter application lost its exact lexical target");
            }
            (signature, signature, application.binding_declaration)
        } else {
            let (declaring_trait, declared) = program
                .trait_machine_signature(application.binding_declaration)
                .ok_or("the open application has no exact authored requirement header")?;
            let (selected_trait, public) =
                if let Some(slot) = program.dynamic_slot_by_symbol(site.target_symbol(program)) {
                    if slot.declaring_trait != declaring_trait.symbol
                        || slot.requirement != declared.symbol
                        || application.registration_operation != declared.symbol
                    {
                        return Err("the dynamic header belongs to another public requirement");
                    }
                    (declaring_trait, declared)
                } else {
                    requirement_contract_signature(program, caller, site)
                        .ok_or("the open application lost its exact public invocation view")?
                };
            if declaring_trait.symbol != selected_trait.symbol {
                return Err("the open application changed its declaring trait");
            }
            if declared.symbol != public.symbol {
                let closed = program
                    .closed_requirement_application(public.symbol)
                    .ok_or("the applied requirement lost its exact closed declaration")?;
                let supplied =
                    expression_table(program).static_machine_application(closed.application);
                let arguments = expression_table(program)
                    .static_machine_application_arguments(closed.application);
                let header = program.requirement_invocation_contract_parameters(declared);
                let retained =
                    expression_table(program).static_machine_application_bindings(handle);
                let lifetimes =
                    expression_table(program).static_machine_application_lifetimes(handle);
                if closed.template != declared.symbol
                || closed.declaring_trait != declaring_trait.symbol
                || application.registration_operation != declared.symbol
                || arguments.len() != header.len()
                || retained.len() != header.len()
                || expression_table(program)
                    .static_machine_application_lifetimes(closed.application)
                    != lifetimes
                || (!lifetimes.is_empty()
                    && (supplied.lexical_owner != application.lexical_owner
                        || supplied.lifetime_binding_declaration
                            != application.lifetime_binding_declaration))
                || header.iter().zip(arguments).enumerate().any(|(ordinal, (parameter, argument))| {
                    let mut bindings = retained
                        .iter()
                        .filter(|binding| binding.parameter == parameter.symbol);
                    let Some(binding) = bindings.next() else {
                        return true;
                    };
                    bindings.next().is_some()
                        || !(match closed.local_arguments.get(ordinal) {
                            Some(typed_trees::typed_trees::RequirementLocalArgument::RuntimeValue { carrier }) => {
                                matches!((&parameter.kind, &binding.value),
                                    (TypeParameterKind::Value { .. }, StaticMachineBindingValue::RuntimeValue { type_reference, .. })
                                        if crate::declared_carriers_match(program, *carrier, *type_reference))
                            }
                            _ => program.static_binding_matches_argument(
                            &parameter.kind,
                            &binding.value,
                            argument,
                            supplied.lexical_owner,
                            application.lexical_owner,
                        ) })
                })
            {
                return Err("the applied requirement changed its exact closed invocation tuple");
            }
            }
            (declared, public, declared.symbol)
        };
        let header = program.requirement_invocation_contract_parameters(declared);
        let logical = program.requirement_invocation_contract_parameters(public);
        let retained = expression_table(program).static_machine_application_bindings(handle);
        if header.len() != logical.len()
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
        for (ordinal, parameter) in header.iter().enumerate() {
            let selected = program
                .applied_signature_parameter(declared, public, parameter.symbol)
                .ok_or("the open application lost its applied header binder")?;
            let mut public_parameters = logical
                .iter()
                .filter(|parameter| parameter.symbol == selected);
            let (Some(public_parameter), None) =
                (public_parameters.next(), public_parameters.next())
            else {
                return Err("the open application duplicated its public header binder");
            };
            if std::mem::discriminant(&parameter.kind)
                != std::mem::discriminant(&public_parameter.kind)
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
                if !matches!(parameter.kind, TypeParameterKind::Value { .. }) {
                    return Err("the open runtime operand does not belong to a Value binder");
                }
                if let Some(arguments) = &selected_arguments {
                    let mut supplied = arguments.iter().filter(|argument| {
                        argument.public_position.is_none() && argument.logical_parameter == selected
                    });
                    let (Some(argument), None) = (supplied.next(), supplied.next()) else {
                        return Err(
                            "the selected public Value binder has no unique executable operand",
                        );
                    };
                    let StaticMachineBindingValue::RuntimeValue {
                        type_reference,
                        subject,
                        expression,
                    } = binding.value
                    else {
                        unreachable!()
                    };
                    if !crate::declared_carriers_match(
                        program,
                        type_reference,
                        argument.logical_type_reference,
                    ) || !runtime_source_matches(
                        program,
                        subject,
                        expression,
                        argument.expression,
                    ) {
                        return Err(
                            "the selected public Value operand differs from its retained source binding",
                        );
                    }
                    runtime_parameters.push(parameter.symbol);
                    parameters.push((parameter.symbol, selected));
                    continue;
                }
                let executable = program.state_signature_parameters(public);
                let mut formals = executable
                    .iter()
                    .enumerate()
                    .filter(|(_, formal)| formal.symbol == selected);
                if let Some((position, formal)) = formals.next() {
                    if formals.next().is_some() || formal.is_mutable || formal.is_const {
                        return Err(
                            "the runtime requirement projection has no unique immutable formal",
                        );
                    }
                    let StaticMachineBindingValue::RuntimeValue {
                        type_reference,
                        subject,
                        expression,
                    } = binding.value
                    else {
                        unreachable!()
                    };
                    let actual = call_site_argument_expressions(program, site)
                        .get(position)
                        .ok_or("the runtime requirement projection lost its executable operand")?;
                    if !crate::declared_carriers_match(
                        program,
                        formal.type_reference,
                        type_reference,
                    ) || !runtime_source_matches(program, subject, expression, *actual)
                    {
                        return Err(
                            "the runtime requirement operand differs from its retained source binding",
                        );
                    }
                } else if program
                    .dynamic_slot_by_symbol(site.target_symbol(program))
                    .is_some()
                {
                    dynamic_family_subject(program, caller, site, handle, ordinal, &binding.value)?;
                } else {
                    immutable_runtime_parameter(program, caller, &binding.value)?;
                }
                runtime_parameters.push(parameter.symbol);
            }
            parameters.push((parameter.symbol, selected));
        }
        Ok(CallBindingCorrespondence {
            declaration,
            public_owner: public.symbol,
            parameters,
            runtime_parameters,
        })
    }

    fn dynamic_family_subject(
        program: &dyn crate::CheckingInput,
        caller: SymbolHandle,
        site: &CallSite<'_>,
        handle: typed_trees::static_applications::StaticMachineApplicationHandle,
        ordinal: usize,
        value: &StaticMachineBindingValue,
    ) -> Result<(), &'static str> {
        let arguments = match site {
            CallSite::Expression { call, .. } => call.machine_arguments.as_ref(),
            CallSite::Statement(call) => call.machine_arguments.as_ref(),
            CallSite::TransitionNamed { .. } => {
                return Err("a family header has no source invocation");
            }
        };
        if expression_table(program).static_machine_application_arguments(handle) != arguments {
            return Err("the dynamic header changed its source arguments");
        }
        let StaticMachineBindingValue::RuntimeValue {
            subject,
            expression,
            ..
        } = value
        else {
            return Err("the family operand has another binding kind");
        };
        let argument = arguments
            .get(ordinal)
            .ok_or("the family operand lost its source ordinal")?;
        if !subject.is_valid()
            || expression.is_valid()
            || argument.symbol != *subject
            || !crate::machine_calls::static_arguments::is_runtime_value_subject(program, argument)
        {
            return Err("the family operand lost its exact runtime subject");
        }
        let parent = program.symbols.get(*subject).parent;
        if parent != caller
            && program
                .machine_holding_state(parent)
                .is_none_or(|machine| machine.symbol != caller)
        {
            return Err("the family operand belongs to another caller");
        }
        Ok(())
    }

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
        let TypeParameterKind::Value {
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

    pub(crate) struct ApplicationTemplateScope {
        machine: SymbolHandle,
        header_parameters: OnceCell<Vec<(u32, u32)>>,
    }

    impl ApplicationTemplateScope {
        fn contains_header_parameter(
            &self,
            program: &dyn crate::CheckingInput,
            parameter: SymbolHandle,
        ) -> bool {
            let parameters = self.header_parameters.get_or_init(|| {
                let mut parameters = program
                    .machine_by_symbol(self.machine)
                    .map(|machine| {
                        program
                            .machine_type_parameters(machine)
                            .iter()
                            .map(|parameter| {
                                (
                                    parameter.symbol.arena_index(),
                                    parameter.symbol.generation(),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                parameters.sort_unstable();
                parameters
            });
            parameters
                .binary_search(&(parameter.arena_index(), parameter.generation()))
                .is_ok()
        }
    }

    fn application_lineage(
        program: &dyn crate::CheckingInput,
        mut machine: SymbolHandle,
        declaration: SymbolHandle,
    ) -> Result<Vec<ApplicationTemplateScope>, &'static str> {
        let mut lineage: Vec<ApplicationTemplateScope> = Vec::new();
        while machine != declaration {
            if lineage.iter().any(|frame| frame.machine == machine) {
                return Err("the executable application template chain is cyclic");
            }
            lineage.push(ApplicationTemplateScope {
                machine,
                header_parameters: OnceCell::new(),
            });
            machine = program
                .executable_application_template(machine)
                .ok()
                .flatten()
                .ok_or("the executable application lost its authored template")?;
        }
        lineage.push(ApplicationTemplateScope {
            machine: declaration,
            header_parameters: OnceCell::new(),
        });
        Ok(lineage)
    }

    fn application_parameter_origin(
        program: &dyn crate::CheckingInput,
        lineage: &[ApplicationTemplateScope],
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
            if parent != frame[1].machine
                && program
                    .machine_holding_state(parent)
                    .is_none_or(|owner| owner.symbol != frame[1].machine)
                && !frame[1].contains_header_parameter(program, original)
            {
                return Err("the application parameter belongs to another template scope");
            }
            parameter = original;
        }
        Ok(parameter)
    }

    fn open_runtime_source(
        program: &dyn crate::CheckingInput,
        caller: SymbolHandle,
        site: &CallSite<'_>,
        value: &StaticMachineBindingValue,
    ) -> Result<(), &'static str> {
        let StaticMachineBindingValue::RuntimeValue {
            subject,
            expression,
            ..
        } = value
        else {
            return Err("the open runtime header binding has another operand kind");
        };
        if subject.is_valid() {
            immutable_runtime_parameter(program, caller, value)?;
            return Ok(());
        }
        if !expression.is_valid() {
            return Err("the open runtime header actual has no retained source");
        }
        let mut root = *expression;
        while let ExpressionNode::Member(member) = expression_table(program).expression(root) {
            if !member.member_symbol.is_valid() || member.case_variant.is_some() {
                return Err("the open runtime header projection has no exact field");
            }
            root = member.receiver;
        }
        let ExpressionNode::SavedSubject(saved) = expression_table(program).expression(root) else {
            return Err("the open runtime header actual needs a completed argument");
        };
        captured_argument_position(program, saved, *site)
            .ok_or("the open runtime header actual belongs to another capture occurrence")?;
        Ok(())
    }

    pub(crate) fn captured_argument_position(
        program: &dyn crate::CheckingInput,
        saved: &typed_trees::expression::TableSavedSubject,
        site: CallSite<'_>,
    ) -> Option<usize> {
        use typed_trees::expression::ValueCaptureOwner;
        program.saved_value_source(saved)?;
        match (
            expression_table(program).value_capture(saved.capture).owner,
            site,
        ) {
            (
                ValueCaptureOwner::ExpressionArgument { call, position },
                CallSite::Expression { expression, .. },
            ) if call == expression => Some(position as usize),
            (
                ValueCaptureOwner::StatementArgument { call, position },
                CallSite::Statement(statement),
            ) if program
                .statement_table
                .statement(call)
                .as_call()
                .is_some_and(|stored| std::ptr::eq(stored, statement)) =>
            {
                Some(position as usize)
            }
            _ => None,
        }
    }
}
pub(crate) use argument_correspondence::{
    call_binding_correspondence, captured_argument_position, executable_parameters,
};
pub(crate) mod occurrences {

    use super::topology::find_state_in_machine;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId;
    use symbols::SymbolHandle;
    use typed_trees::StaticRequirementDispatch;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::name::Identifier;
    use typed_trees::statement::StatementNode;
    pub(crate) mod traversal {
        pub(crate) mod context {
            //! Call ordinals and their owning target lanes come from one traversal.

            enum CollectedCalls<'program, 'query> {
                Sites(&'query mut Vec<super::super::CallSite<'program>>),
                Occurrences(&'query mut Vec<super::super::StatementCallOccurrence<'program>>),
            }

            // Callers select one statement before entering traversal. Ordinals belong to
            // that walk; source statement coordinates remain on the authored call facts.
            pub(crate) struct CallSiteTraversal<'program, 'ordinal> {
                pub(crate) program: &'program dyn crate::CheckingInput,
                pub(crate) machine: &'program typed_trees::machine::Machine,
                pub(crate) state: &'program typed_trees::state::State,
                pub(crate) target_call_ordinal: usize,
                pub(crate) current_ordinal: usize,
                /// When set, every reached call site is pushed in ordinal order instead
                /// of stopping at `target_call_ordinal` — one statement walk binds all of
                /// a statement's call sites at once.
                /// Selection ancestry does not change call ordinals or lookup membership.
                conditional: bool,
                pub(crate) matched_conditionally: bool,
                collected: Option<CollectedCalls<'program, 'ordinal>>,
                transition_target: typed_trees::statement::TransitionTargetHandle,
                /// Normalization preserves a DAG's authored expression handles. A second
                /// edge to one call is not a second authored invocation or ordinal.
                expressions: std::collections::BTreeSet<(u32, u32)>,
            }

            impl<'program> CallSiteTraversal<'program, '_> {
                pub(crate) fn new<'ordinal>(
                    program: &'program dyn crate::CheckingInput,
                    machine: &'program typed_trees::machine::Machine,
                    state: &'program typed_trees::state::State,
                    target_call_ordinal: usize,
                ) -> CallSiteTraversal<'program, 'ordinal> {
                    CallSiteTraversal {
                        program,
                        machine,
                        state,
                        target_call_ordinal,
                        current_ordinal: 0,
                        collected: None,
                        transition_target: Default::default(),
                        conditional: false,
                        matched_conditionally: false,
                        expressions: Default::default(),
                    }
                }

                pub(crate) fn collecting<'ordinal>(
                    program: &'program dyn crate::CheckingInput,
                    machine: &'program typed_trees::machine::Machine,
                    state: &'program typed_trees::state::State,
                    collected: &'ordinal mut Vec<super::super::CallSite<'program>>,
                ) -> CallSiteTraversal<'program, 'ordinal> {
                    CallSiteTraversal {
                        program,
                        machine,
                        state,
                        target_call_ordinal: usize::MAX,
                        current_ordinal: 0,
                        collected: Some(CollectedCalls::Sites(collected)),
                        transition_target: Default::default(),
                        conditional: false,
                        matched_conditionally: false,
                        expressions: Default::default(),
                    }
                }

                pub(in crate::calls::occurrences) fn collecting_occurrences<'ordinal>(
                    program: &'program dyn crate::CheckingInput,
                    machine: &'program typed_trees::machine::Machine,
                    state: &'program typed_trees::state::State,
                    collected: &'ordinal mut Vec<super::super::StatementCallOccurrence<'program>>,
                ) -> CallSiteTraversal<'program, 'ordinal> {
                    let mut traversal = Self::new(program, machine, state, usize::MAX);
                    traversal.collected = Some(CollectedCalls::Occurrences(collected));
                    traversal
                }

                /// Site admission shared by every traversal arm: collect mode records the
                /// site and keeps walking; match mode returns it once the ordinal lines up.
                pub(crate) fn visit(
                    &mut self,
                    site: super::super::CallSite<'program>,
                ) -> Option<super::super::CallSite<'program>> {
                    if let super::super::CallSite::Expression { expression, .. } = site
                        && !self
                            .expressions
                            .insert((expression.arena_index(), expression.generation()))
                    {
                        return None;
                    }
                    if let Some(collected) = self.collected.as_mut() {
                        match collected {
                            CollectedCalls::Sites(collected) => collected.push(site),
                            CollectedCalls::Occurrences(collected) => {
                                collected.push(super::super::StatementCallOccurrence {
                                    site,
                                    transition_target: self.transition_target,
                                })
                            }
                        }
                        self.advance_call_ordinal();
                        return None;
                    }
                    if self.is_target_call_site() {
                        self.matched_conditionally = self.conditional;
                        return Some(site);
                    }
                    self.advance_call_ordinal();
                    None
                }

                pub(crate) fn selected(
                    &mut self,
                    visit: impl FnOnce(&mut Self) -> Option<super::super::CallSite<'program>>,
                ) -> Option<super::super::CallSite<'program>> {
                    let enclosing = self.conditional;
                    self.conditional = true;
                    let result = visit(self);
                    self.conditional = enclosing;
                    result
                }

                pub(crate) fn selected_target(
                    &mut self,
                    target: typed_trees::statement::TransitionTargetHandle,
                    visit: impl FnOnce(&mut Self) -> Option<super::super::CallSite<'program>>,
                ) -> Option<super::super::CallSite<'program>> {
                    let enclosing = self.transition_target;
                    self.transition_target = target;
                    let result = self.selected(visit);
                    self.transition_target = enclosing;
                    result
                }

                pub(super) fn is_target_call_site(&self) -> bool {
                    self.current_ordinal == self.target_call_ordinal
                }

                pub(crate) fn advance_call_ordinal(&mut self) {
                    self.current_ordinal = self.current_ordinal.saturating_add(1);
                }
            }
        }
        pub(crate) mod expression {
            use crate::machine_calls::calls::occurrences::{CallSite, CallSiteTraversal};
            use typed_trees::expression::{ExpressionHandle, TableCallExpression};
            impl<'program> typed_trees::source_call_occurrences::ExpressionCalls<'program>
                for CallSiteTraversal<'program, '_>
            {
                type Site = CallSite<'program>;
                fn source(&self) -> &'program typed_trees::TypedTrees {
                    self.program.typed_program()
                }
                fn named_call_is_selected(
                    &self,
                    expression: ExpressionHandle,
                    call: &'program TableCallExpression,
                ) -> bool {
                    let (receiver_symbol, receiver_path) =
                        crate::call_receiver_parts(self.program, call.receiver);
                    let selected = crate::call_target_symbol(self.program, call);
                    crate::resolve_state_call_target(
                        self.program,
                        self.machine,
                        self.state,
                        receiver_symbol,
                        selected,
                        receiver_path.as_deref(),
                        &call.target,
                    )
                    .is_valid()
                        || crate::receiver_can_dispatch_to_machine(
                            self.program,
                            self.machine,
                            self.state,
                            receiver_symbol,
                            receiver_path.as_deref(),
                        )
                        || typed_trees::byte_predicates::byte_predicate_call(
                            self.program.typed_program(),
                            expression,
                        )
                        .is_some()
                }
                fn visit_call(
                    &mut self,
                    expression: ExpressionHandle,
                    call: &'program TableCallExpression,
                ) -> Option<Self::Site> {
                    self.visit(CallSite::Expression { expression, call })
                }
                fn selected(
                    &mut self,
                    visit: impl FnOnce(&mut Self) -> Option<Self::Site>,
                ) -> Option<Self::Site> {
                    CallSiteTraversal::selected(self, visit)
                }
            }
            pub(crate) fn find_call_site_in_expression<'program>(
                traversal: &mut CallSiteTraversal<'program, '_>,
                expression: ExpressionHandle,
            ) -> Option<CallSite<'program>> {
                typed_trees::source_call_occurrences::find_in_expression(traversal, expression)
            }
        }
        pub(crate) mod statement {
            use super::expression::find_call_site_in_expression;
            use crate::machine_calls::calls::occurrences::{CallSite, CallSiteTraversal};
            use typed_trees::statement::{
                StatementNode, TableCall, TransitionGuardNode, TransitionTargetHandle,
                TransitionTargetNode,
            };
            impl<'program> typed_trees::source_call_occurrences::StatementCalls<'program>
                for CallSiteTraversal<'program, '_>
            {
                fn named_statement_call_is_selected(&self, call: &'program TableCall) -> bool {
                    crate::statement_call_can_dispatch_to_machine(
                        self.program,
                        self.machine,
                        self.state,
                        call,
                    )
                }
                fn visit_statement_call(
                    &mut self,
                    call: &'program TableCall,
                ) -> Option<Self::Site> {
                    self.visit(CallSite::Statement(call))
                }
                fn visit_transition_call(
                    &mut self,
                    target: &'program TransitionTargetNode,
                ) -> Option<Self::Site> {
                    let TransitionTargetNode::Named {
                        arguments,
                        evidence_arguments,
                        path,
                        static_machine_parameter,
                        source_span,
                        authored_call_selection,
                        ..
                    } = target
                    else {
                        return None;
                    };
                    self.visit(CallSite::TransitionNamed {
                        path,
                        static_machine_parameter: *static_machine_parameter,
                        arguments: *arguments,
                        evidence_arguments,
                        source_span: *source_span,
                        authored_call_selection: *authored_call_selection,
                    })
                }
                fn selected_target(
                    &mut self,
                    target: TransitionTargetHandle,
                    visit: impl FnOnce(&mut Self) -> Option<Self::Site>,
                ) -> Option<Self::Site> {
                    CallSiteTraversal::selected_target(self, target, visit)
                }
            }
            pub(crate) fn find_call_site_in_statement<'program>(
                traversal: &mut CallSiteTraversal<'program, '_>,
                statement: &'program StatementNode,
            ) -> Option<CallSite<'program>> {
                typed_trees::source_call_occurrences::find_in_statement(traversal, statement)
            }
            pub(crate) fn transition_call_target(
                traversal: &mut CallSiteTraversal<'_, '_>,
                transition: &typed_trees::statement::TableTransition,
            ) -> Option<TransitionTargetHandle> {
                if let TransitionGuardNode::When(expression) = transition.guard
                    && find_call_site_in_expression(traversal, expression).is_some()
                {
                    return Some(Default::default());
                }
                transition.targets().find(|target| {
                    typed_trees::source_call_occurrences::find_in_transition(traversal, *target)
                        .is_some()
                })
            }
        }

        pub(crate) use context::CallSiteTraversal;
        pub(crate) use statement::find_call_site_in_statement;
        pub(crate) use statement::transition_call_target;
    }
    use traversal::{CallSiteTraversal, find_call_site_in_statement};

    #[derive(Clone, Copy)]
    pub enum CallSite<'program> {
        Statement(&'program typed_trees::statement::TableCall),
        Expression {
            expression: ExpressionHandle,
            call: &'program typed_trees::expression::TableCallExpression,
        },
        TransitionNamed {
            path: &'program typed_trees::statement::TableNamePath,
            static_machine_parameter: SymbolHandle,
            arguments: arena::HandleSpan<ExpressionHandle>,
            evidence_arguments: &'program [Identifier],
            source_span: source::SourceSpan,
            authored_call_selection: Option<AuthoredDeclarationSelectionOccurrenceId>,
        },
    }

    /// One source call and the transition operand lane which owns its ordinal.
    /// Guard and ordinary statement calls retain the empty target.
    pub(crate) struct StatementCallOccurrence<'program> {
        pub(crate) site: CallSite<'program>,
        pub(crate) transition_target: typed_trees::statement::TransitionTargetHandle,
    }

    impl<'program> CallSite<'program> {
        pub fn authored_application(
            &self,
        ) -> typed_trees::static_applications::StaticMachineApplicationHandle {
            match self {
                Self::Expression { call, .. } => call.authored_machine_application,
                Self::Statement(call) => call.authored_machine_application,
                Self::TransitionNamed { .. } => Default::default(),
            }
        }

        pub fn captured_argument_position(
            &self,
            program: &dyn crate::CheckingInput,
            saved: &typed_trees::expression::TableSavedSubject,
        ) -> Option<usize> {
            super::captured_argument_position(program, saved, *self)
        }
        pub fn argument_evaluation_order(
            &self,
            program: &dyn crate::CheckingInput,
        ) -> Result<typed_trees::calls::CallArgumentEvaluationOrder, &'static str> {
            let application = self.authored_application();
            program.call_argument_evaluation_order(
                application,
                self.target_symbol(program),
                super::call_site_argument_expressions(program, self).len(),
            )
        }
        pub fn completes_transition(
            &self,
            program: &dyn crate::CheckingInput,
            target: typed_trees::statement::TransitionTargetHandle,
        ) -> bool {
            use typed_trees::statement::TransitionTargetNode;
            match (self, program.statement_table.transition_target(target)) {
                (
                    Self::TransitionNamed { path, .. },
                    TransitionTargetNode::Named {
                        path: destination, ..
                    },
                ) => std::ptr::eq(*path, destination),
                (Self::Expression { expression, .. }, TransitionTargetNode::Value(_)) => {
                    super::topology::completing_transition_invocations(program, target)
                        .iter()
                        .any(|invocation| invocation.expression == *expression)
                }
                _ => false,
            }
        }

        pub fn static_machine_parameter(&self) -> SymbolHandle {
            match self {
                Self::Statement(call) => call.static_machine_parameter,
                Self::Expression { call, .. } => call.static_machine_parameter,
                Self::TransitionNamed {
                    static_machine_parameter,
                    ..
                } => *static_machine_parameter,
            }
        }

        pub fn parameters(
            &self,
            program: &'program dyn crate::CheckingInput,
            lexical_owner: SymbolHandle,
        ) -> Option<&'program [typed_trees::signature::StateParameter]> {
            let signature = match self {
                Self::Statement(call) => {
                    program.requirement_statement_call_signature(lexical_owner, call)
                }
                Self::Expression { call, .. } => {
                    program.requirement_call_signature(lexical_owner, call)
                }
                Self::TransitionNamed { .. } => None,
            };
            if let Some(signature) = signature {
                return Some(program.state_signature_parameters(signature));
            }
            if self.static_requirement_dispatch().is_some() {
                return None;
            }
            super::topology::call_target_parameters(program, self.target_symbol(program))
        }

        pub fn target_symbol(&self, program: &dyn crate::CheckingInput) -> SymbolHandle {
            match self {
                Self::Statement(call) => {
                    let candidates =
                        typed_trees::operator::named_statement_call_candidates(program, call);
                    match candidates.as_slice() {
                        [operator] if operator.is_boundary => operator.symbol,
                        _ => call.target_symbol,
                    }
                }
                Self::Expression { call, .. } => super::topology::call_target_symbol(program, call),
                Self::TransitionNamed { path, .. } => path.symbol,
            }
        }

        pub fn is_internal_state_transfer(
            &self,
            program: &dyn crate::CheckingInput,
            caller_machine: SymbolHandle,
        ) -> bool {
            matches!(self, Self::TransitionNamed { path, .. }
            if path.symbol.is_valid()
                && program.symbols.get(path.symbol).kind == symbols::SymbolKind::State
                && program.symbols.get(path.symbol).parent == caller_machine)
        }

        pub fn static_requirement_dispatch(&self) -> Option<&'program StaticRequirementDispatch> {
            match self {
                Self::Statement(call) => call.static_requirement_dispatch.as_ref(),
                Self::Expression { call, .. } => call.static_requirement_dispatch.as_ref(),
                Self::TransitionNamed { .. } => None,
            }
        }
    }

    pub fn find_call_site<'program>(
        program: &'program dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<CallSite<'program>> {
        let state = find_state_in_machine(program, machine_symbol, state_symbol)?;
        let machine = program.machine_by_symbol(machine_symbol)?;
        find_call_site_in_state(program, machine, state, statement_index, call_ordinal)
    }

    pub fn call_is_unconditional(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<()> {
        let state = find_state_in_machine(program, machine_symbol, state_symbol)?;
        let machine = program.machine_by_symbol(machine_symbol)?;
        let statements = program.statement_table.statements(state.statement_nodes);
        if statements
            .get(..statement_index)?
            .iter()
            .any(|statement| matches!(statement, StatementNode::Transition(_)))
        {
            return None;
        }
        let statement = statements.get(statement_index)?;
        let mut traversal = CallSiteTraversal::new(program, machine, state, call_ordinal);
        find_call_site_in_statement(&mut traversal, statement)?;
        (!traversal.matched_conditionally).then_some(())
    }

    pub fn find_call_site_in_state<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<CallSite<'program>> {
        let statement = program
            .statement_table
            .statement_at(state, statement_index)?;
        let mut traversal = CallSiteTraversal::new(program, machine, state, call_ordinal);
        find_call_site_in_statement(&mut traversal, statement)
    }

    pub fn collect_call_sites_in_statement<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
        statement_index: usize,
    ) -> Vec<CallSite<'program>> {
        let mut collected = Vec::new();
        let Some(statement) = program.statement_table.statement_at(state, statement_index) else {
            return collected;
        };
        let mut traversal = CallSiteTraversal::collecting(program, machine, state, &mut collected);
        let _ = find_call_site_in_statement(&mut traversal, statement);
        collected
    }

    /// Retain call/target association during the same ordinal-producing walk.
    /// Queries that inspect a complete statement must not rediscover its lane by
    /// walking the statement again for each call.
    pub(crate) fn collect_statement_call_occurrences<'program>(
        program: &'program dyn crate::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
        statement_index: usize,
    ) -> Vec<StatementCallOccurrence<'program>> {
        let mut collected = Vec::new();
        if let Some(statement) = program.statement_table.statement_at(state, statement_index) {
            let mut traversal =
                CallSiteTraversal::collecting_occurrences(program, machine, state, &mut collected);
            let _ = find_call_site_in_statement(&mut traversal, statement);
        }
        collected
    }

    /// Reuse call-ordinal traversal to distinguish guard evaluation from the two
    /// mutually exclusive target operands of a transition.
    pub fn transition_call_target(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<typed_trees::statement::TransitionTargetHandle> {
        let transition = program
            .statement_table
            .statement_at(state, statement_index)?
            .as_transition()?;
        let mut traversal = CallSiteTraversal::new(program, machine, state, call_ordinal);
        traversal::transition_call_target(&mut traversal, transition)
    }

    pub fn call_site_argument_expressions<'program>(
        program: &'program dyn crate::CheckingInput,
        call_site: &CallSite<'program>,
    ) -> &'program [ExpressionHandle] {
        match call_site {
            CallSite::Statement(call) => program.statement_table.expression_handles(call.arguments),
            CallSite::Expression { call, .. } => {
                program.expression_table.expression_handles(call.arguments)
            }
            CallSite::TransitionNamed { arguments, .. } => {
                program.statement_table.expression_handles(*arguments)
            }
        }
    }

    pub fn call_site_evidence_arguments<'program>(
        call_site: &CallSite<'program>,
    ) -> &'program [Identifier] {
        match call_site {
            CallSite::Statement(call) => &call.evidence_arguments,
            CallSite::Expression { call, .. } => &call.evidence_arguments,
            CallSite::TransitionNamed {
                evidence_arguments, ..
            } => evidence_arguments,
        }
    }

    pub fn invocation_output_parameter(
        program: &dyn crate::CheckingInput,
        identity: facts::InvocationOutputIdentity,
    ) -> Option<&typed_trees::signature::StateParameter> {
        if !identity.is_valid() {
            return None;
        }
        let (machine, state) = crate::find_state(program, identity.state_symbol)?;
        let site = find_call_site_in_state(
            program,
            machine,
            state,
            identity.statement_index,
            identity.call_ordinal,
        )?;
        if let Some((callee, selected)) = crate::find_state(program, site.target_symbol(program))
            .or_else(|| crate::find_machine_head(program, site.target_symbol(program)))
            && callee.body_is_present
            && callee.supply_mode.is_checked_body()
        {
            let parameters = program.state_parameters(selected);
            let position = typed_trees::signature::state_parameter_position(
                parameters,
                identity.parameter_symbol,
            )?;
            return parameters.get(position);
        }
        let parameters = site.parameters(program, machine.symbol)?;
        let position = typed_trees::signature::state_parameter_position(
            parameters,
            identity.parameter_symbol,
        )?;
        parameters.get(position)
    }
}
pub use argument_correspondence::{
    SelectedCallArgument, SelectedCallArgumentError, authored_call_parameters,
    selected_call_arguments, selected_call_result_domain_matches,
};
pub(crate) use occurrences::collect_statement_call_occurrences;
pub use occurrences::{
    CallSite, call_is_unconditional, call_site_argument_expressions, call_site_evidence_arguments,
    collect_call_sites_in_statement, find_call_site, find_call_site_in_state,
    invocation_output_parameter, transition_call_target,
};

pub(crate) fn free_machine_entry_state<'program>(
    program: &'program dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'program>,
    target: &str,
) -> Option<(&'program Machine, &'program State)> {
    let machine = symbols.machine(target)?;
    if machine.attached_data.is_some() {
        return None;
    }

    let states = program.machine_states(machine);
    states
        .iter()
        .find(|state| state.name.as_str() == target)
        .or_else(|| states.iter().find(|state| state.name.as_str() == "entry"))
        .or_else(|| states.first())
        .map(|state| (machine, state))
}

pub(crate) fn machine_state_by_symbol(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<(&Machine, &State)> {
    if !symbol.is_valid() {
        return None;
    }
    let symbol = crate::call_target_entry_symbol(program, symbol);
    let machine_symbol = program.symbols.get(symbol).parent;
    let machine = program.machine_by_symbol(machine_symbol)?;
    program
        .state_in_machine(machine, symbol)
        .map(|state| (machine, state))
}
