/*
    Build-time evaluation of closed integer facts, const applications, domain
    facts, const-generic calls, fixed-array lengths, qualifications, constant
    recipes, contract expressions, data-layout queries, and range endpoints.

    We coordinate round admission over prepared build-machine programs and
    dispatch each constant-evaluation responsibility to its typed or Terminal
    evaluator.
*/

use crate::machine_execution::selected_operators::prepare_selected_execution;
use crate::{BuildTimeAdmissionPlan, PreparedBuildMachineProgram};

pub(crate) mod const_applications {
    use crate::build_time_evaluation::{EvaluationDomain, expression_table, type_reference_table};
    use crate::const_evaluation::const_generic_expressions::exact_probe_destination;
    use crate::const_evaluation::const_generic_expressions::value::evaluate_closed_const_argument;
    use crate::const_evaluation::range_endpoints::selected_endpoint_machine;
    use crate::machine_execution::admission::{
        expression_children, require_closed_expression_custody,
    };
    use crate::unique::extend_unique;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use language_semantics::const_value::DecodedCanonicalConstValue;
    use std::collections::{HashMap, HashSet};
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::expression::ExpressionNode;
    use typed_trees::{
        TypedTrees,
        data::{DataMember, TypeParameterKind},
        expression::ExpressionHandle,
        name::Identifier,
        types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode},
    };

    pub(crate) struct PendingConstApplication {
        pub(crate) site: TypeReferenceHandle,
        pub(crate) root: ExpressionHandle,
        pub(crate) destination: Option<TypeReferenceHandle>,
    }

    pub(crate) struct PendingApplicationCall {
        pub(crate) site: TypeReferenceHandle,
        pub(crate) root: ExpressionHandle,
        pub(crate) expression: ExpressionHandle,
        pub(crate) machine: SymbolHandle,
        pub(crate) static_application: bool,
    }

    pub(crate) struct ConstApplicationPlan {
        pub(crate) applications: Vec<PendingConstApplication>,
        pub(crate) calls: Vec<PendingApplicationCall>,
    }

    pub(crate) fn pending_const_applications_need_operator_selection(
        typed: &TypedTrees,
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        domain: &EvaluationDomain,
    ) -> Result<bool, Vec<Diagnostic>> {
        let plan = const_application_plan(typed, domain);
        if plan.applications.is_empty() {
            return Ok(false);
        }
        let prepared = crate::PreparedBuildMachineProgram::prepare_in(typed, domain)?;
        let execution = prepared.typed();
        let facts = validation::derive_pre_flow_operator_selections(execution);
        let admission = domain.admission(execution, selection_authority.clone())?;
        let boundary_use = |selected_operator_symbol: SymbolHandle| {
            execution
                .operators()
                .iter()
                .any(|operator| operator.symbol == selected_operator_symbol && operator.is_boundary)
        };
        let boundary_use_expression = |expression| {
            facts
                .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
                .any(|fact| {
                    fact.expression == expression && boundary_use(fact.selected_operator_symbol)
                })
                || facts.named_uses().any(|fact| {
                    fact.expression == expression && boundary_use(fact.selected_operator_symbol)
                })
        };
        Ok(plan.applications.iter().any(|application| {
            let subtree = expression_subtree(execution, application.root);
            subtree
                .iter()
                .any(|expression| boundary_use_expression(*expression))
                || plan
                    .calls
                    .iter()
                    .filter(|call| call.site == application.site)
                    .any(|call| {
                        call.machine.is_valid()
                            && admission.closure_needs_execution_preparation(
                                execution,
                                resolve_application_callee(execution, call)
                                    .map_or(call.machine, |callee| callee.instance),
                                &facts,
                            )
                    })
        }))
    }

    pub(crate) fn defer_pending_const_applications(
        typed: &mut TypedTrees,
        domain: &EvaluationDomain,
    ) -> Result<(), Vec<Diagnostic>> {
        let plan = const_application_plan(typed, domain);
        for application in &plan.applications {
            typed.pending_const_range_endpoints.insert(application.root);
        }
        for call in &plan.calls {
            typed.pending_const_range_endpoints.insert(call.expression);
        }
        Ok(())
    }

    pub(crate) fn evaluate_selected_const_applications(
        typed: &mut TypedTrees,
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        selected: crate::SelectedBuildTimeOperators<'_>,
        domain: &mut EvaluationDomain,
    ) -> Result<(), Vec<Diagnostic>> {
        let crate::SelectedBuildTimeOperators {
            operators,
            provider_bodies,
        } = selected;
        let mut published: Vec<(
            TypeReferenceHandle,
            TypeReferenceNode,
            Vec<ExpressionHandle>,
        )> = Vec::new();
        let mut warnings = Vec::new();
        let restore = |typed: &mut TypedTrees,
                       published: Vec<(
            TypeReferenceHandle,
            TypeReferenceNode,
            Vec<ExpressionHandle>,
        )>| {
            for (site, original, marked) in published.into_iter().rev() {
                typed.type_reference_table.substitute_node(site, original);
                typed.pending_const_range_endpoints.extend(marked);
            }
        };
        loop {
            let plan = const_application_plan(typed, domain);
            if plan.applications.is_empty() {
                break;
            }
            let round = match evaluate_application_round(
                typed,
                &plan,
                selection_authority.clone(),
                operators,
                provider_bodies,
                &mut warnings,
                domain,
            ) {
                Ok(round) => round,
                Err(errors) => {
                    restore(typed, published);
                    return Err(errors);
                }
            };
            let progress = !round.folds.is_empty();
            for fold in round.folds {
                let original = typed.type_reference_table.type_reference(fold.site).clone();
                typed
                    .type_reference_table
                    .substitute_node(fold.site, fold.node);
                let mut marked: Vec<_> = plan
                    .calls
                    .iter()
                    .filter(|call| call.site == fold.site)
                    .filter_map(|call| {
                        typed
                            .pending_const_range_endpoints
                            .remove(&call.expression)
                            .then_some(call.expression)
                    })
                    .collect();
                if typed.pending_const_range_endpoints.remove(&fold.root) {
                    marked.push(fold.root);
                }
                published.push((fold.site, original, marked));
            }
            if round.diagnostics.is_empty() {
                break;
            }
            if !progress {
                restore(typed, published);
                return Err(round.diagnostics);
            }
        }
        realize_folded_applications(typed, &published, domain);
        crate::report_evaluation_diagnostics(&warnings)?;
        Ok(())
    }

    fn realize_folded_applications(
        typed: &mut TypedTrees,
        published: &[(
            TypeReferenceHandle,
            TypeReferenceNode,
            Vec<ExpressionHandle>,
        )],
        domain: &mut EvaluationDomain,
    ) {
        if published.is_empty() {
            return;
        }
        let mut closed: HashSet<TypeReferenceHandle> =
            published.iter().map(|(site, _, _)| *site).collect();
        let mut declined: HashSet<TypeReferenceHandle> = HashSet::new();
        loop {
            let origins: HashSet<TypeReferenceHandle> = typed
                .data_definitions()
                .iter()
                .filter_map(|definition| definition.generic_instance)
                .collect();
            let mut progress = false;
            for site in domain.references(typed).type_references {
                let TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } = typed.type_reference_table.type_reference(site)
                else {
                    continue;
                };
                let base_symbol = *base_symbol;
                if closed.contains(&site) || declined.contains(&site) || origins.contains(&site) {
                    continue;
                }
                let argument_handles = type_reference_table(typed)
                    .type_reference_handles(*arguments)
                    .to_vec();
                if !argument_handles
                    .iter()
                    .any(|argument| argument_contains(typed, *argument, &closed))
                {
                    continue;
                }
                if !argument_handles
                    .iter()
                    .all(|argument| closed_argument(typed, *argument))
                {
                    continue;
                }
                let Some((symbol, name)) = realize_data_instance(typed, site, base_symbol) else {
                    declined.insert(site);
                    continue;
                };
                domain.include(symbol);
                typed.type_reference_table.substitute_node(
                    site,
                    TypeReferenceNode::Named {
                        symbol,
                        name: Identifier::generated(name),
                    },
                );
                closed.insert(site);
                progress = true;
            }
            if !progress {
                return;
            }
        }
    }

    fn argument_contains(
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
        closed: &HashSet<TypeReferenceHandle>,
    ) -> bool {
        if closed.contains(&reference) {
            return true;
        }
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Generic { arguments, .. } => type_reference_table(typed)
                .type_reference_handles(*arguments)
                .iter()
                .any(|argument| argument_contains(typed, *argument, closed)),
            TypeReferenceNode::Constrained { base_type, .. } => {
                argument_contains(typed, *base_type, closed)
            }
            TypeReferenceNode::Reference { referee, .. } => {
                argument_contains(typed, *referee, closed)
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                argument_contains(typed, *element_type, closed)
            }
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Unit => false,
        }
    }

    fn closed_argument(typed: &TypedTrees, reference: TypeReferenceHandle) -> bool {
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                !symbol.is_valid()
                    || matches!(
                        typed.symbols.get(*symbol).kind,
                        SymbolKind::BuiltinType | SymbolKind::Data
                    )
            }
            TypeReferenceNode::Generic { arguments, .. } => type_reference_table(typed)
                .type_reference_handles(*arguments)
                .iter()
                .all(|argument| closed_argument(typed, *argument)),
            TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Constrained { .. }
            | TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Unit => false,
        }
    }

    fn direct_const_call(
        typed: &TypedTrees,
        root: ExpressionHandle,
    ) -> Option<(SymbolHandle, Identifier)> {
        let call = typed.expression_table.call_node(root)?;
        if !expression_table(typed)
            .expression_handles(call.arguments)
            .is_empty()
            || !call.machine_arguments.is_empty()
        {
            return None;
        }
        let (machine, static_application) = selected_endpoint_machine(typed, root)?;
        if static_application || !machine.symbol.is_valid() {
            return None;
        }
        Some((machine.symbol, machine.name.clone()))
    }

    pub(crate) fn realize_symbolic_applications(
        typed: &mut TypedTrees,
        domain: &mut EvaluationDomain,
    ) -> Result<(), Vec<Diagnostic>> {
        loop {
            let mut progress = false;
            let origins: Vec<TypeReferenceHandle> = typed
                .data_definitions()
                .iter()
                .filter_map(|definition| definition.generic_instance)
                .collect();
            for site in domain.references(typed).type_references {
                if origins.contains(&site) {
                    continue;
                }
                let TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } = typed.type_reference_table.type_reference(site)
                else {
                    continue;
                };
                let base_symbol = *base_symbol;
                let argument_handles = type_reference_table(typed)
                    .type_reference_handles(*arguments)
                    .to_vec();
                let symbolic = |argument: &TypeReferenceHandle| {
                    matches!(
                        typed.type_reference_table.type_reference(*argument),
                        TypeReferenceNode::ConstExpression(root)
                            if direct_const_call(typed, *root).is_some()
                    )
                };
                if !argument_handles.iter().any(symbolic)
                    || !argument_handles
                        .iter()
                        .all(|argument| closed_argument(typed, *argument) || symbolic(argument))
                {
                    continue;
                }
                let Some((symbol, name)) = realize_data_instance(typed, site, base_symbol) else {
                    continue;
                };
                domain.include(symbol);
                typed.type_reference_table.substitute_node(
                    site,
                    TypeReferenceNode::Named {
                        symbol,
                        name: Identifier::generated(name),
                    },
                );
                progress = true;
                break;
            }
            if !progress {
                return Ok(());
            }
        }
    }

    fn realize_data_instance(
        typed: &mut TypedTrees,
        site: TypeReferenceHandle,
        base_symbol: SymbolHandle,
    ) -> Option<(SymbolHandle, String)> {
        let TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
            ..
        } = typed.type_reference_table.type_reference(site).clone()
        else {
            return None;
        };
        let argument_handles = type_reference_table(typed)
            .type_reference_handles(arguments)
            .to_vec();
        let template = typed.data_definition_by_symbol(base_symbol).cloned()?;
        if template.generic_instance.is_some()
            || template.quotient.is_some()
            || !template.requires_facts.is_empty()
            || template.zero_gated
            || !template.lifetime_parameters.is_empty()
            || typed
                .machines()
                .iter()
                .any(|machine| machine.attached_data_symbol == base_symbol)
        {
            return None;
        }
        let parameters = typed.data_type_parameters(&template).to_vec();
        if parameters.len() != argument_handles.len() {
            return None;
        }
        let mut bindings = HashMap::new();
        for (parameter, argument) in parameters.iter().zip(&argument_handles) {
            match parameter.kind {
                TypeParameterKind::Type
                | TypeParameterKind::Const { .. }
                | TypeParameterKind::Value { .. } => {
                    bindings.insert(parameter.symbol, *argument);
                }
                _ => return None,
            }
        }
        let synthetic_name = format!(
            "{}<{}>",
            template.name.as_str(),
            argument_handles
                .iter()
                .map(|argument| argument_slug(typed, *argument))
                .collect::<Option<Vec<_>>>()?
                .join(", ")
        );
        if let Some(existing) = typed.data_definitions().iter().find(|definition| {
            definition.generic_instance.is_some_and(|origin| {
                instance_origin_matches(typed, origin, base_symbol, &argument_handles)
            })
        }) {
            return Some((existing.symbol, synthetic_name));
        }
        let template_members = typed.data_members(&template).to_vec();
        let mut member_types = Vec::with_capacity(template_members.len());
        for member in &template_members {
            let field = member.as_field()?;
            let type_reference = substitute_member_type(typed, field.type_reference, &bindings)?;
            member_types.push((field.clone(), type_reference));
        }
        let instance_symbol = typed.symbols.insert_generated_root_from(
            base_symbol,
            SymbolKind::Data,
            &synthetic_name,
        );
        let member_span = typed.symbols.insert_generated_children(
            instance_symbol,
            member_types
                .iter()
                .map(|(field, _)| (SymbolKind::Field, field.name.as_str())),
        );
        let origin = typed
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            });
        let mut instance = template.clone();
        instance.symbol = instance_symbol;
        instance.name = Identifier::generated(synthetic_name.clone());
        instance.type_parameters = HandleSpan::empty();
        instance.generic_instance = Some(origin);
        instance.members = HandleSpan::empty();
        for ((field, type_reference), member) in member_types.iter().zip(member_span.handles()) {
            let mut field = field.clone();
            field.symbol = member;
            field.type_reference = *type_reference;
            typed.push_data_member(&mut instance, DataMember::Field(field));
        }
        typed.push_data_definition(instance);
        Some((instance_symbol, synthetic_name))
    }

    fn substitute_member_type(
        typed: &mut TypedTrees,
        reference: TypeReferenceHandle,
        bindings: &HashMap<SymbolHandle, TypeReferenceHandle>,
    ) -> Option<TypeReferenceHandle> {
        match typed.type_reference_table.type_reference(reference).clone() {
            TypeReferenceNode::Named { symbol, .. } => {
                bindings.get(&symbol).copied().or(Some(reference))
            }
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                let referee = substitute_member_type(typed, referee, bindings)?;
                Some(
                    typed
                        .type_reference_table
                        .insert(TypeReferenceNode::Reference {
                            referee,
                            access,
                            lifetime,
                        }),
                )
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let element_type = substitute_member_type(typed, element_type, bindings)?;
                let length = match length {
                    FixedArrayLength::Literal(_) => length,
                    FixedArrayLength::ConstParameter { symbol, .. } => {
                        let argument = bindings.get(&symbol).copied()?;
                        match typed.type_reference_table.type_reference(argument) {
                            TypeReferenceNode::Named { name, .. } => {
                                FixedArrayLength::Literal(name.as_str().parse().ok()?)
                            }
                            TypeReferenceNode::ConstExpression(root) => {
                                let (machine, name) = direct_const_call(typed, *root)?;
                                FixedArrayLength::ConstCall {
                                    machine,
                                    name,
                                    source_span: typed.expression_table.source_span(*root),
                                }
                            }
                            _ => return None,
                        }
                    }
                    FixedArrayLength::Expression(_) | FixedArrayLength::ConstCall { .. } => {
                        return None;
                    }
                };
                Some(
                    typed
                        .type_reference_table
                        .insert(TypeReferenceNode::FixedArray {
                            element_type,
                            length,
                        }),
                )
            }
            TypeReferenceNode::Slice { element_type } => {
                let element_type = substitute_member_type(typed, element_type, bindings)?;
                Some(
                    typed
                        .type_reference_table
                        .insert(TypeReferenceNode::Slice { element_type }),
                )
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                let mut substituted = Vec::new();
                for argument in type_reference_table(typed)
                    .type_reference_handles(arguments)
                    .to_vec()
                {
                    substituted.push(substitute_member_type(typed, argument, bindings)?);
                }
                let arguments = typed
                    .type_reference_table
                    .insert_type_reference_handles(substituted);
                Some(
                    typed
                        .type_reference_table
                        .insert(TypeReferenceNode::Generic {
                            base_symbol,
                            base_name,
                            lifetime_arguments,
                            arguments,
                        }),
                )
            }
            TypeReferenceNode::Constrained { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. } => None,
            TypeReferenceNode::Unit => Some(reference),
        }
    }

    fn argument_slug(typed: &TypedTrees, reference: TypeReferenceHandle) -> Option<String> {
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { name, .. } => Some(name.as_str().to_owned()),
            TypeReferenceNode::ConstExpression(root) => {
                direct_const_call(typed, *root).map(|(_, name)| format!("{}()", name.as_str()))
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => Some(format!(
                "{}<{}>",
                base_name.as_str(),
                type_reference_table(typed)
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| argument_slug(typed, *argument))
                    .collect::<Option<Vec<_>>>()?
                    .join(", ")
            )),
            _ => None,
        }
    }

    fn instance_origin_matches(
        typed: &TypedTrees,
        origin: TypeReferenceHandle,
        base_symbol: SymbolHandle,
        arguments: &[TypeReferenceHandle],
    ) -> bool {
        let TypeReferenceNode::Generic {
            base_symbol: origin_base,
            arguments: origin_arguments,
            ..
        } = typed.type_reference_table.type_reference(origin)
        else {
            return false;
        };
        let origin_handles = type_reference_table(typed).type_reference_handles(*origin_arguments);
        *origin_base == base_symbol
            && origin_handles.len() == arguments.len()
            && origin_handles
                .iter()
                .zip(arguments)
                .all(|(origin, argument)| closed_argument_eq(typed, *origin, *argument))
    }

    fn closed_argument_eq(
        typed: &TypedTrees,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
    ) -> bool {
        match (
            typed.type_reference_table.type_reference(left),
            typed.type_reference_table.type_reference(right),
        ) {
            (
                TypeReferenceNode::ConstExpression(left),
                TypeReferenceNode::ConstExpression(right),
            ) => {
                match (
                    direct_const_call(typed, *left),
                    direct_const_call(typed, *right),
                ) {
                    (Some((left, _)), Some((right, _))) => left == right,
                    _ => false,
                }
            }
            (
                TypeReferenceNode::Named {
                    symbol: left_symbol,
                    name: left_name,
                },
                TypeReferenceNode::Named {
                    symbol: right_symbol,
                    name: right_name,
                },
            ) => {
                if left_symbol.is_valid() && right_symbol.is_valid() {
                    left_symbol == right_symbol
                } else {
                    left_name == right_name
                }
            }
            (
                TypeReferenceNode::Generic {
                    base_symbol: left_base,
                    arguments: left_arguments,
                    ..
                },
                TypeReferenceNode::Generic {
                    base_symbol: right_base,
                    arguments: right_arguments,
                    ..
                },
            ) => {
                let left_handles =
                    type_reference_table(typed).type_reference_handles(*left_arguments);
                let right_handles =
                    type_reference_table(typed).type_reference_handles(*right_arguments);
                left_base == right_base
                    && left_handles.len() == right_handles.len()
                    && left_handles
                        .iter()
                        .zip(right_handles)
                        .all(|(left, right)| closed_argument_eq(typed, *left, *right))
            }
            (
                TypeReferenceNode::FixedArray {
                    element_type: left_element,
                    length: left_length,
                },
                TypeReferenceNode::FixedArray {
                    element_type: right_element,
                    length: right_length,
                },
            ) => {
                left_length == right_length
                    && closed_argument_eq(typed, *left_element, *right_element)
            }
            (
                TypeReferenceNode::Slice {
                    element_type: left_element,
                },
                TypeReferenceNode::Slice {
                    element_type: right_element,
                },
            ) => closed_argument_eq(typed, *left_element, *right_element),
            (
                TypeReferenceNode::Reference {
                    referee: left_referee,
                    access: left_access,
                    lifetime: left_lifetime,
                },
                TypeReferenceNode::Reference {
                    referee: right_referee,
                    access: right_access,
                    lifetime: right_lifetime,
                },
            ) => {
                left_access == right_access
                    && left_lifetime == right_lifetime
                    && closed_argument_eq(typed, *left_referee, *right_referee)
            }
            (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
            _ => false,
        }
    }

    struct ApplicationRound {
        folds: Vec<ApplicationFold>,
        diagnostics: Vec<Diagnostic>,
    }

    struct ApplicationFold {
        site: TypeReferenceHandle,
        root: ExpressionHandle,
        node: TypeReferenceNode,
    }

    fn evaluate_application_round(
        typed: &mut TypedTrees,
        plan: &ConstApplicationPlan,
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        operators: &[crate::SelectedBuildTimeBinaryOperator],
        provider_bodies: &[crate::SelectedBuildTimeProviderBody],
        warnings: &mut Vec<Diagnostic>,
        domain: &EvaluationDomain,
    ) -> Result<ApplicationRound, Vec<Diagnostic>> {
        let (prepared, admission) = crate::const_evaluation::prepared_admission(
            typed,
            provider_bodies,
            operators,
            selection_authority.clone(),
            domain,
        )?;
        let execution = prepared.typed();

        let mut round = ApplicationRound {
            folds: Vec::new(),
            diagnostics: Vec::new(),
        };
        for application in &plan.applications {
            match evaluate_const_application(
                typed,
                execution,
                &admission,
                application,
                selection_authority.as_deref(),
                warnings,
            ) {
                Ok(fold) => round.folds.push(fold),
                Err(reason) => {
                    round.diagnostics.push(
                        Diagnostic::error(format!(
                            "const-generic application: const evaluation failed: {reason}"
                        ))
                        .with_source_span(typed.expression_table.source_span(application.root)),
                    );
                }
            }
        }
        Ok(round)
    }

    fn evaluate_const_application(
        typed: &TypedTrees,
        execution: &TypedTrees,
        admission: &crate::BuildTimeAdmissionPlan,
        application: &PendingConstApplication,
        selection_authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<ApplicationFold, String> {
        require_closed_expression_custody(execution, application.root, selection_authority)?;
        let destination = application
            .destination
            .ok_or("const-generic application has no declared const destination".to_owned())?;
        let carrier = exact_probe_destination(execution, destination).ok_or(
        "const-generic application needs an unconstrained exact builtin integer or Boolean carrier"
            .to_owned(),
    )?;
        let invocation = crate::const_evaluation::range_endpoints::arguments::Invocation {
            working: typed,
            execution,
            admission,
            authority: selection_authority,
        };
        let (value, additions) =
            evaluate_closed_const_argument(execution, application.root, carrier, &invocation)?;
        extend_unique(warnings, additions);
        let spelling = match value.decode_encoding() {
            Some(DecodedCanonicalConstValue::Integer { value, .. }) => value.to_string(),
            Some(DecodedCanonicalConstValue::Boolean(_)) => value.atom(),
            _ => {
                return Err("const application returned an unsupported canonical value".to_owned());
            }
        };
        Ok(ApplicationFold {
            site: application.site,
            root: application.root,
            node: TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated(spelling),
            },
        })
    }

    fn resolve_application_callee(
        execution: &TypedTrees,
        call: &PendingApplicationCall,
    ) -> Result<crate::const_evaluation::range_endpoints::EndpointCallee, String> {
        crate::const_evaluation::range_endpoints::resolve_endpoint_callee(
            execution,
            &crate::const_evaluation::range_endpoints::PendingEndpoint {
                expression: call.expression,
                root: call.root,
                machine: call.machine,
                static_application: call.static_application,
                contract_expression: false,
            },
        )
    }

    fn expression_subtree(typed: &TypedTrees, root: ExpressionHandle) -> Vec<ExpressionHandle> {
        let mut subtree = Vec::new();
        let mut pending = vec![root];
        while let Some(expression) = pending.pop() {
            if !typed.expression_table.expression_is_valid(expression) {
                continue;
            }
            if subtree.contains(&expression) {
                continue;
            }
            subtree.push(expression);
            pending.extend(expression_children(typed, expression));
        }
        subtree
    }

    pub(crate) fn const_application_plan(
        typed: &TypedTrees,
        domain: &EvaluationDomain,
    ) -> ConstApplicationPlan {
        let mut plan = ConstApplicationPlan {
            applications: Vec::new(),
            calls: Vec::new(),
        };
        for site in domain.references(typed).type_references {
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = typed.type_reference_table.type_reference(site)
            else {
                continue;
            };
            let parameters = generic_type_parameters(typed, *base_symbol);
            for (index, argument) in type_reference_table(typed)
                .type_reference_handles(*arguments)
                .iter()
                .enumerate()
            {
                visit_argument(
                    typed,
                    *argument,
                    parameters.and_then(|parameters| parameters.get(index)),
                    &mut plan,
                );
            }
        }
        plan
    }

    fn generic_type_parameters(
        typed: &TypedTrees,
        base_symbol: SymbolHandle,
    ) -> Option<&[typed_trees::data::TypeParameter]> {
        if let Some(data) = typed.data_definition_by_symbol(base_symbol) {
            return Some(typed.data_type_parameters(data));
        }
        if let Some(machine) = typed.machine(base_symbol) {
            return Some(typed.machine_type_parameters(machine));
        }
        if let Some(declaration) = typed.conformance_by_symbol(base_symbol) {
            return Some(typed.conformance_type_parameters(declaration));
        }
        if let Some(trait_definition) = typed.trait_definition_by_symbol(base_symbol) {
            return Some(typed.trait_type_parameters(trait_definition));
        }
        None
    }

    fn visit_argument(
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
        parameter: Option<&typed_trees::data::TypeParameter>,
        plan: &mut ConstApplicationPlan,
    ) {
        let destination = parameter.and_then(|parameter| match parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => Some(type_reference),
            _ => None,
        });
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::ConstExpression(root) => {
                if parameter.is_some_and(|parameter| {
                    matches!(parameter.kind, TypeParameterKind::Value { .. })
                }) && expression_subtree(typed, *root).iter().any(|expression| {
                    match typed.expression_table.expression(*expression) {
                        ExpressionNode::Borrow(_) => true,
                        ExpressionNode::Name(path) if path.symbol.is_valid() => {
                            matches!(
                                typed.symbols.get(path.symbol).kind,
                                SymbolKind::Local | SymbolKind::Parameter | SymbolKind::Field
                            ) || typed
                                .data_type_parameter_by_symbol(path.symbol)
                                .is_some_and(|parameter| {
                                    matches!(parameter.kind, TypeParameterKind::Value { .. })
                                })
                        }
                        _ => false,
                    }
                }) {
                    return;
                }
                if selected_endpoint_machine(typed, *root).is_none() {
                    return;
                }
                let calls = collect_calls(typed, *root);
                if calls.is_empty() {
                    return;
                }
                plan.applications.push(PendingConstApplication {
                    site: reference,
                    root: *root,
                    destination,
                });
                plan.calls.extend(calls.into_iter().map(|call| {
                    let (machine, static_application) = selected_endpoint_machine(typed, call)
                        .map(|(machine, static_application)| (machine.symbol, static_application))
                        .unwrap_or((SymbolHandle::invalid(), false));
                    PendingApplicationCall {
                        site: reference,
                        root: *root,
                        expression: call,
                        machine,
                        static_application,
                    }
                }));
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                visit_argument(typed, *base_type, parameter, plan);
            }
            TypeReferenceNode::Reference { referee, .. } => {
                visit_argument(typed, *referee, parameter, plan);
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                visit_argument(typed, *element_type, parameter, plan);
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let parameters = generic_type_parameters(typed, *base_symbol);
                for (index, nested) in type_reference_table(typed)
                    .type_reference_handles(*arguments)
                    .iter()
                    .enumerate()
                {
                    visit_argument(
                        typed,
                        *nested,
                        parameters.and_then(|parameters| parameters.get(index)),
                        plan,
                    );
                }
            }
            _ => {}
        }
    }

    fn collect_calls(typed: &TypedTrees, root: ExpressionHandle) -> Vec<ExpressionHandle> {
        let mut calls = Vec::new();
        let mut pending = vec![root];
        let mut visited = Vec::new();
        while let Some(expression) = pending.pop() {
            if !typed.expression_table.expression_is_valid(expression) {
                continue;
            }
            if visited.contains(&expression) {
                continue;
            }
            visited.push(expression);
            if matches!(
                typed.expression_table.expression(expression),
                ExpressionNode::Call(_)
            ) {
                calls.push(expression);
            }
            pending.extend(expression_children(typed, expression));
        }
        calls
    }
}
pub(crate) mod const_domain_facts {
    use crate::build_time_evaluation::EvaluationDomain;
    use crate::machine_execution::selected_operators::prepare_selected_execution;
    use arena::Handle;
    use diagnostics::Diagnostic;
    use typed_trees::TypedTrees;
    use typed_trees::domain::{ProofFact, ProofMembershipFact};
    use typed_trees::expression::ExpressionNode;

    mod fact_expression {
        use typed_trees::TypedTrees;
        use typed_trees::data::{DataDefinition, DataField, DataMember};
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };
        use typed_trees::operator::{
            comparison_operator_spelling, has_builtin_spelled_expression_meaning,
        };
        use typed_trees::types::PrimitiveType;

        #[derive(Clone, Copy)]
        pub(crate) enum ConstProofValue {
            Integer(i64),
            Boolean(bool),
        }

        #[derive(Clone, Copy)]
        enum FactBindings<'a> {
            DomainSubject(&'a typed_trees::domain::DomainDefinition, i64),
            StaticData(&'a DataDefinition),
            ZeroStorage(&'a DataDefinition),
        }

        pub(crate) fn data_fact_is_static_true(
            typed: &TypedTrees,
            data: &DataDefinition,
            expression: ExpressionHandle,
        ) -> bool {
            matches!(
                evaluate_fact_expression(typed, expression, FactBindings::StaticData(data)),
                Ok(Some(ConstProofValue::Boolean(true)))
            )
        }

        pub(crate) fn data_fact_holds_at_zero(
            typed: &TypedTrees,
            data: &DataDefinition,
            expression: ExpressionHandle,
        ) -> bool {
            matches!(
                evaluate_fact_expression(typed, expression, FactBindings::ZeroStorage(data)),
                Ok(Some(ConstProofValue::Boolean(true)))
            )
        }

        pub(crate) fn evaluate_domain_fact_expression(
            typed: &TypedTrees,
            expression: ExpressionHandle,
            domain: &typed_trees::domain::DomainDefinition,
            subject_value: i64,
        ) -> Result<Option<ConstProofValue>, String> {
            evaluate_fact_expression(
                typed,
                expression,
                FactBindings::DomainSubject(domain, subject_value),
            )
        }

        fn evaluate_fact_expression(
            typed: &TypedTrees,
            expression: ExpressionHandle,
            bindings: FactBindings<'_>,
        ) -> Result<Option<ConstProofValue>, String> {
            match typed.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value
            .value_i64()
            .map(ConstProofValue::Integer)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "proof operand `{value}` does not fit the build-time evaluator's signed integer boundary"
                )
            }),
        ExpressionNode::Boolean(value) => Ok(Some(ConstProofValue::Boolean(*value))),
        ExpressionNode::Name(_) => {
            if matches!(bindings, FactBindings::StaticData(_)) {
                return Ok(None);
            }
            if let FactBindings::ZeroStorage(data) = bindings {
                return Ok(zero_storage_field(typed, data, expression).and_then(|field| {
                    match typed.primitive_type_reference(field.type_reference)? {
                        PrimitiveType::Bool => Some(ConstProofValue::Boolean(false)),
                        PrimitiveType::I8 | PrimitiveType::I16 | PrimitiveType::I32
                        | PrimitiveType::I64 | PrimitiveType::U8 | PrimitiveType::U16
                        | PrimitiveType::U32 | PrimitiveType::U64 => Some(ConstProofValue::Integer(0)),
                        _ => None,
                    }
                }));
            }
            let FactBindings::DomainSubject(domain, subject_value) = bindings else {
                return Ok(None);
            };
            Ok(typed_trees::domain::domain_subject_reference(typed, domain, expression)
                .then_some(ConstProofValue::Integer(subject_value)))
        }
        ExpressionNode::Binary(binary) => {
            if let FactBindings::ZeroStorage(data) | FactBindings::StaticData(data) = bindings
                && !zero_storage_binary_is_builtin(typed, data, expression, binary)
            {
                return Ok(None);
            }
            let Some(left) = evaluate_fact_expression(typed, binary.left, bindings)? else {
                return Ok(None);
            };
            let Some(right) = evaluate_fact_expression(typed, binary.right, bindings)?
            else {
                return Ok(None);
            };
            evaluate_domain_fact_binary(binary.operator, left, right).map(Some)
        }
        ExpressionNode::Unary(unary) => {
            if matches!(bindings, FactBindings::ZeroStorage(_) | FactBindings::StaticData(_))
                && unary.operator != UnaryOperator::LogicalNot
            {
                return Ok(None);
            }
            let Some(operand) = evaluate_fact_expression(typed, unary.operand, bindings)?
            else {
                return Ok(None);
            };
            match (unary.operator, operand) {
                (UnaryOperator::BitwiseNot, ConstProofValue::Integer(value)) => {
                    Ok(Some(ConstProofValue::Integer(!value)))
                }
                (UnaryOperator::BitwiseNot, ConstProofValue::Boolean(_)) => {
                    Err("bitwise complement requires an integer proof operand".to_string())
                }
                (UnaryOperator::LogicalNot, ConstProofValue::Boolean(value)) => {
                    Ok(Some(ConstProofValue::Boolean(!value)))
                }
                (UnaryOperator::LogicalNot, ConstProofValue::Integer(_)) => {
                    Err("logical negation requires a boolean proof operand".to_string())
                }
            }
        }
        _ => Ok(None),
    }
        }

        fn zero_storage_field<'program>(
            typed: &'program TypedTrees,
            data: &DataDefinition,
            expression: ExpressionHandle,
        ) -> Option<&'program DataField> {
            let path = typed.expression_table.name_path_node(expression)?;
            if path.members.count() != 1
                || !path.symbol.is_valid()
                || path.head_symbol != path.symbol
            {
                return None;
            }
            typed
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    DataMember::Field(field)
                        if field.symbol == path.symbol && !field.relevance.is_erased() =>
                    {
                        Some(field)
                    }
                    _ => None,
                })
        }

        fn zero_storage_binary_is_builtin(
            typed: &TypedTrees,
            data: &DataDefinition,
            expression: ExpressionHandle,
            binary: &typed_trees::expression::TableBinaryExpression,
        ) -> bool {
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                return true;
            }
            let Some(spelling) = comparison_operator_spelling(binary.operator) else {
                return false;
            };
            let operands = [binary.left, binary.right];
            if operands.iter().all(|operand| {
                matches!(typed.expression_table.expression(*operand),
        ExpressionNode::Integer(literal) if literal.landing().is_none())
            }) {
                return true;
            }
            let operand_types = operands.map(|operand| {
                zero_storage_field(typed, data, operand)
                    .map(|field| field.type_reference)
                    .or_else(|| validation::landed_integer_literal_type_reference(typed, operand))
            });
            has_builtin_spelled_expression_meaning(
                typed,
                data.symbol,
                expression,
                spelling,
                &operand_types,
            )
        }

        fn evaluate_domain_fact_binary(
            operator: BinaryOperator,
            left: ConstProofValue,
            right: ConstProofValue,
        ) -> Result<ConstProofValue, String> {
            use BinaryOperator::*;
            match (left, right) {
                (ConstProofValue::Integer(left), ConstProofValue::Integer(right)) => match operator
                {
                    Add => left
                        .checked_add(right)
                        .map(ConstProofValue::Integer)
                        .ok_or_else(|| "proof addition overflows `i64`".to_string()),
                    Subtract => left
                        .checked_sub(right)
                        .map(ConstProofValue::Integer)
                        .ok_or_else(|| "proof subtraction overflows `i64`".to_string()),
                    Multiply => left
                        .checked_mul(right)
                        .map(ConstProofValue::Integer)
                        .ok_or_else(|| "proof multiplication overflows `i64`".to_string()),
                    Divide => left
                        .checked_div(right)
                        .map(ConstProofValue::Integer)
                        .ok_or_else(|| "proof division is invalid".to_string()),
                    Modulo => left
                        .checked_rem(right)
                        .map(ConstProofValue::Integer)
                        .ok_or_else(|| "proof remainder is invalid".to_string()),
                    ShiftLeft => u32::try_from(right)
                        .ok()
                        .and_then(|amount| left.checked_shl(amount))
                        .map(ConstProofValue::Integer)
                        .ok_or_else(|| "proof left shift exceeds the `i64` width".to_string()),
                    ShiftRight => u32::try_from(right)
                        .ok()
                        .and_then(|amount| left.checked_shr(amount))
                        .map(ConstProofValue::Integer)
                        .ok_or_else(|| "proof right shift exceeds the `i64` width".to_string()),
                    BitwiseAnd => Ok(ConstProofValue::Integer(left & right)),
                    BitwiseOr => Ok(ConstProofValue::Integer(left | right)),
                    BitwiseXor => Ok(ConstProofValue::Integer(left ^ right)),
                    Equal => Ok(ConstProofValue::Boolean(left == right)),
                    NotEqual => Ok(ConstProofValue::Boolean(left != right)),
                    Greater => Ok(ConstProofValue::Boolean(left > right)),
                    GreaterOrEqual => Ok(ConstProofValue::Boolean(left >= right)),
                    Less => Ok(ConstProofValue::Boolean(left < right)),
                    LessOrEqual => Ok(ConstProofValue::Boolean(left <= right)),
                    And | Or => Err("logical proof operators require boolean operands".to_string()),
                    CaseMembership => Err(
                        "case membership requires a nominal subject and case classifier"
                            .to_string(),
                    ),
                },
                (ConstProofValue::Boolean(left), ConstProofValue::Boolean(right)) => match operator
                {
                    And => Ok(ConstProofValue::Boolean(left && right)),
                    Or => Ok(ConstProofValue::Boolean(left || right)),
                    Equal => Ok(ConstProofValue::Boolean(left == right)),
                    NotEqual => Ok(ConstProofValue::Boolean(left != right)),
                    _ => Err("arithmetic proof operators require integer operands".to_string()),
                },
                _ => Err("const proof operands have incompatible types".to_string()),
            }
        }
    }
    mod membership {
        use build_time_values::BuildTimeValue;
        use typed_trees::TypedTrees;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::{ExpressionHandle, TableCallExpression};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::PrimitiveType;

        use super::PendingMembership;
        use super::fact_expression::{ConstProofValue, evaluate_domain_fact_expression};
        use crate::BuildTimeAdmissionPlan;
        use crate::build_time_evaluation::{expression_table, type_reference_table};

        pub(crate) fn evaluate_membership(
            typed: &TypedTrees,
            admission: &BuildTimeAdmissionPlan,
            pending: &PendingMembership,
        ) -> Result<Option<bool>, String> {
            let Some(domain) = typed.domain_definition(pending.membership.domain_symbol) else {
                return Ok(None);
            };
            let Some(literal) = expression_table(typed).integer_node(pending.membership.value)
            else {
                return Ok(None);
            };
            let value = literal.value_i64().ok_or_else(|| {
        format!(
            "the concrete value `{literal}` does not fit the build-time evaluator's signed integer boundary"
        )
    })?;

            evaluate_domain_facts(typed, admission, domain, value, &mut vec![domain.symbol])
        }

        pub(crate) fn evaluate_closed_membership(
            typed: &TypedTrees,
            admission: &BuildTimeAdmissionPlan,
            domain_symbol: symbols::SymbolHandle,
            value: &numerics::bignum::BigInt,
        ) -> Result<Option<bool>, String> {
            let Some(domain) = typed.domain_definition(domain_symbol) else {
                return Ok(None);
            };
            let value = value.to_i64().ok_or_else(|| {
        format!(
            "the concrete value `{value}` does not fit the build-time evaluator's signed integer boundary"
        )
    })?;
            evaluate_domain_facts(typed, admission, domain, value, &mut vec![domain.symbol])
        }

        fn resolve_fact_machine<'program>(
            typed: &'program TypedTrees,
            domain: &typed_trees::domain::DomainDefinition,
            expression: ExpressionHandle,
        ) -> Option<(&'program Machine, &'program State)> {
            let call = typed.expression_table.call_node(expression)?;
            if !is_direct_subject_call(typed, domain, call) {
                return None;
            }
            typed
                .machines()
                .iter()
                .find_map(|machine| {
                    typed
                        .state_in_machine(machine, call.target_symbol)
                        .map(|state| (machine, state))
                })
                .or_else(|| {
                    typed
                        .machines()
                        .iter()
                        .find(|machine| {
                            machine.attached_data.is_none()
                                && machine.name.as_str() == call.target.as_str()
                        })
                        .and_then(|machine| {
                            typed
                                .machine_states(machine)
                                .first()
                                .map(|state| (machine, state))
                        })
                })
        }

        pub(crate) fn domain_facts_need_operator_selection(
            typed: &TypedTrees,
            admission: &BuildTimeAdmissionPlan,
            facts: &checked_trees::CheckedOperatorFacts,
            domain_symbol: symbols::SymbolHandle,
            visiting: &mut Vec<symbols::SymbolHandle>,
        ) -> bool {
            if visiting.contains(&domain_symbol) {
                return false;
            }
            let Some(domain) = typed.domain_definition(domain_symbol) else {
                return false;
            };
            visiting.push(domain_symbol);
            let needed =
                typed
                    .proof_facts
                    .span_or_empty(domain.facts)
                    .iter()
                    .any(|fact| match fact {
                        ProofFact::Expression(expression) => {
                            resolve_fact_machine(typed, domain, *expression).is_some_and(
                                |(machine, _)| {
                                    admission.closure_needs_execution_preparation(
                                        typed,
                                        machine.symbol,
                                        facts,
                                    )
                                },
                            )
                        }
                        ProofFact::Membership(membership) => domain_facts_need_operator_selection(
                            typed,
                            admission,
                            facts,
                            membership.domain_symbol,
                            visiting,
                        ),
                        ProofFact::Proposition(_) => false,
                    });
            visiting.pop();
            needed
        }

        fn evaluate_machine_fact(
            typed: &TypedTrees,
            admission: &BuildTimeAdmissionPlan,
            domain: &typed_trees::domain::DomainDefinition,
            expression: ExpressionHandle,
            subject_value: i64,
        ) -> Result<Option<bool>, String> {
            let Some((machine, state)) = resolve_fact_machine(typed, domain, expression) else {
                return Ok(None);
            };
            let machine_name = machine.name.as_str();

            let parameters = typed.state_parameters(state);
            if parameters.len() != 1
                || parameters[0].is_mutable
                || matches!(
                    type_reference_table(typed).type_reference(parameters[0].type_reference),
                    typed_trees::types::TypeReferenceNode::Reference { .. }
                )
            {
                return Err(format!(
                    "machine `{machine_name}` must take exactly one by-value, non-mutable integer parameter"
                ));
            }
            if !typed
                .primitive_type_reference(parameters[0].type_reference)
                .is_some_and(|primitive| primitive.accepts_integer_literal())
            {
                return Err(format!(
                    "machine `{machine_name}` must take exactly one by-value integer parameter"
                ));
            }
            if typed.primitive_type_reference(state.return_type) != Some(PrimitiveType::Bool) {
                return Err(format!(
                    "machine `{machine_name}` must return `bool` when used as a domain fact"
                ));
            }

            let fact_holds = match admission.evaluate_const_evaluable_machine_for_invocation(
                typed,
                machine_name,
                vec![BuildTimeValue::Int(subject_value)],
                crate::BuildTimeInvocationCustody::Source(
                    typed.expression_table.source_span(expression),
                ),
            )? {
                BuildTimeValue::Bool(holds) => holds,
                other => {
                    return Err(format!(
                        "machine `{machine_name}` returned `{other:?}` instead of `bool`"
                    ));
                }
            };
            Ok(Some(fact_holds))
        }

        fn evaluate_domain_facts(
            typed: &TypedTrees,
            admission: &BuildTimeAdmissionPlan,
            domain: &typed_trees::domain::DomainDefinition,
            subject_value: i64,
            visiting: &mut Vec<symbols::SymbolHandle>,
        ) -> Result<Option<bool>, String> {
            for fact in typed.proof_facts.span_or_empty(domain.facts) {
                match fact {
                    ProofFact::Expression(expression) => {
                        let value = match evaluate_machine_fact(
                            typed,
                            admission,
                            domain,
                            *expression,
                            subject_value,
                        )? {
                            Some(value) => Some(ConstProofValue::Boolean(value)),
                            None => evaluate_domain_fact_expression(
                                typed,
                                *expression,
                                domain,
                                subject_value,
                            )?,
                        };
                        match value {
                            Some(ConstProofValue::Boolean(true)) => {}
                            Some(ConstProofValue::Boolean(false)) => return Ok(Some(false)),
                            Some(ConstProofValue::Integer(_)) => {
                                return Err(format!(
                                    "domain `{}` has a non-boolean proof fact",
                                    domain.name
                                ));
                            }
                            None => return Ok(None),
                        }
                    }
                    ProofFact::Membership(membership) => {
                        let nested_value = match evaluate_domain_fact_expression(
                            typed,
                            membership.value,
                            domain,
                            subject_value,
                        )? {
                            Some(ConstProofValue::Integer(value)) => value,
                            Some(ConstProofValue::Boolean(_)) => {
                                return Err(format!(
                                    "domain `{}` has a boolean membership operand",
                                    domain.name
                                ));
                            }
                            None => return Ok(None),
                        };
                        match evaluate_nested_domain_membership(
                            typed,
                            admission,
                            membership.domain_symbol,
                            nested_value,
                            visiting,
                        )? {
                            Some(true) => {}
                            Some(false) => return Ok(Some(false)),
                            None => return Ok(None),
                        }
                    }
                    ProofFact::Proposition(_) => return Ok(None),
                }
            }
            Ok(Some(true))
        }

        fn evaluate_nested_domain_membership(
            typed: &TypedTrees,
            admission: &BuildTimeAdmissionPlan,
            domain_symbol: symbols::SymbolHandle,
            subject_value: i64,
            visiting: &mut Vec<symbols::SymbolHandle>,
        ) -> Result<Option<bool>, String> {
            if visiting.contains(&domain_symbol) {
                return Ok(None);
            }
            let Some(domain) = typed.domain_definition(domain_symbol) else {
                return Ok(None);
            };

            visiting.push(domain_symbol);
            let result = evaluate_domain_facts(typed, admission, domain, subject_value, visiting);
            visiting.pop();
            result
        }

        fn is_direct_subject_call(
            typed: &TypedTrees,
            domain: &typed_trees::domain::DomainDefinition,
            call: &TableCallExpression,
        ) -> bool {
            if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
                return false;
            }
            let [argument] = typed.expression_table.expression_handles(call.arguments) else {
                return false;
            };
            typed_trees::domain::domain_subject_reference(typed, domain, *argument)
        }
    }

    pub(crate) use membership::evaluate_closed_membership;
    use membership::evaluate_membership;

    struct PendingMembership {
        fact: Handle<ProofFact>,
        data_symbol: symbols::SymbolHandle,
        instance_name: String,
        membership: ProofMembershipFact,
    }

    fn pending_memberships(
        typed: &TypedTrees,
        domain: &EvaluationDomain,
    ) -> Vec<PendingMembership> {
        let mut pending = Vec::new();
        for data in typed.data_definitions() {
            if !domain.contains(data.symbol) {
                continue;
            }
            if data.generic_instance.is_none() {
                continue;
            }
            for fact in data.requires_facts.handles() {
                let Some(membership) = typed.proof_facts.get(fact).as_membership() else {
                    continue;
                };
                if matches!(
                    typed.expression_table.expression(membership.value),
                    ExpressionNode::Integer(_)
                ) {
                    pending.push(PendingMembership {
                        fact,
                        data_symbol: data.symbol,
                        instance_name: data.name.as_str().to_owned(),
                        membership: *membership,
                    });
                }
            }
        }
        pending
    }

    pub(crate) fn pending_memberships_need_operator_selection(
        typed: &TypedTrees,
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        domain: &EvaluationDomain,
    ) -> Result<bool, Vec<Diagnostic>> {
        let pending = pending_memberships(typed, domain);
        if pending.is_empty() {
            return Ok(false);
        }
        let execution = domain.execution_program(typed);
        let facts = validation::derive_pre_flow_operator_selections(&execution);
        let admission = domain.admission(&execution, selection_authority)?;
        Ok(pending.iter().any(|pending| {
            membership::domain_facts_need_operator_selection(
                &execution,
                &admission,
                &facts,
                pending.membership.domain_symbol,
                &mut Vec::new(),
            )
        }))
    }

    pub fn evaluate_const_domain_facts(
        typed: &mut TypedTrees,
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    ) -> Result<(), Vec<Diagnostic>> {
        evaluate_selected_domain_facts(
            typed,
            selection_authority,
            crate::SelectedBuildTimeOperators::default(),
            &EvaluationDomain::all(),
        )
    }

    pub(crate) fn evaluate_selected_domain_facts(
        typed: &mut TypedTrees,
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        selected: crate::SelectedBuildTimeOperators<'_>,
        domain: &EvaluationDomain,
    ) -> Result<(), Vec<Diagnostic>> {
        let pending = pending_memberships(typed, domain);
        if pending.is_empty() {
            return Ok(());
        }

        let selected_program =
            prepare_selected_execution(typed, selected.provider_bodies, selected.operators)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let execution_program =
            domain.execution_program(selected_program.as_ref().unwrap_or(typed));
        let execution = &execution_program;
        let admission = domain
            .admission(execution, selection_authority)?
            .with_selected_operators(execution, selected.operators)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let mut replacements = Vec::new();
        let mut affected_data = Vec::new();
        let mut diagnostics = Vec::new();

        for pending in &pending {
            match evaluate_membership(execution, &admission, pending) {
                Ok(Some(true)) => {
                    replacements.push(pending.fact);
                    affected_data.push(pending.data_symbol);
                }
                Ok(Some(false)) => diagnostics.push(Diagnostic::error(format!(
                    "const fact for generic instance `{}` is false",
                    pending.instance_name
                ))),
                Ok(None) => {}
                Err(reason) => diagnostics.push(Diagnostic::error(format!(
                    "const domain fact evaluation for generic instance `{}` failed: {reason}",
                    pending.instance_name
                ))),
            }
        }

        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        let proven = typed.expression_table.insert(ExpressionNode::Boolean(true));
        for fact in replacements {
            *typed.proof_facts.get_mut(fact) = ProofFact::Expression(proven);
        }
        let residual_facts = typed
            .data_definitions()
            .iter()
            .filter(|data| affected_data.contains(&data.symbol))
            .map(|data| {
                let residual = typed
                    .conjunctive_proof_facts(data.requires_facts)
                    .filter(|fact| {
                        fact.as_expression().is_none_or(|expression| {
                            !fact_expression::data_fact_is_static_true(typed, data, *expression)
                        })
                    })
                    .collect::<Vec<_>>();
                (data.symbol, residual)
            })
            .collect::<Vec<_>>();
        for (symbol, residual) in residual_facts {
            let facts = typed.proof_facts.insert_many(residual);
            typed.data_definitions.for_each_mut(|_, data| {
                if data.symbol == symbol {
                    data.requires_facts = facts;
                }
            });
        }
        let ungated: Vec<_> = typed
            .data_definitions()
            .iter()
            .filter(|data| affected_data.contains(&data.symbol))
            .filter(|data| {
                typed
                    .proof_facts
                    .span_or_empty(data.requires_facts)
                    .iter()
                    .all(|fact| match fact {
                        ProofFact::Expression(expression) => {
                            fact_expression::data_fact_holds_at_zero(typed, data, *expression)
                        }
                        ProofFact::Membership(_) => false,
                        ProofFact::Proposition(_) => false,
                    })
                    && typed
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Variant(variant) => Some(variant),
                            _ => None,
                        })
                        .is_none_or(|variant| variant.requires_facts.is_empty())
            })
            .map(|data| data.symbol)
            .collect();
        typed
            .data_definitions
            .for_each_mut(|_, data| data.zero_gated &= !ungated.contains(&data.symbol));
        Ok(())
    }
}
pub(crate) mod const_generic_calls {
    use crate::build_time_evaluation::{expression_table, type_references};
    use crate::const_evaluation::const_initializers::CheckedInitializers;
    use crate::{BuildTimeAdmissionPlan, PreparedBuildMachineProgram};
    use diagnostics::Diagnostic;
    use language_semantics::const_value::DecodedCanonicalConstValue;
    use numerics::literals::{IntegerLiteral, IntegerRadix};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
    use syntax_trees::identifier::Identifier;
    use syntax_trees::types::TypeReferenceNode;
    use syntax_trees_to_symbol_resolved_trees::pre_resolution::{
        ClosedDeclarationConstantUses, closed_array_extent_expressions,
        resolve_deferred_initializer_probe,
    };

    mod dependencies {
        use crate::unique::push_unique;
        use diagnostics::Diagnostic;
        use syntax_trees::{SyntaxTrees, expression::ExpressionHandle, types::TypeReferenceHandle};

        pub(crate) struct ReadyLayer {
            pub(crate) arguments: Vec<usize>,
        }

        pub(crate) fn ready_layer(
            preparation: &crate::syntax_probes::PreparedEvaluationInputs,
            applications: &[(
                TypeReferenceHandle,
                ExpressionHandle,
                TypeReferenceHandle,
                bool,
            )],
            owners: &[symbols::SymbolHandle],
            syntax: &SyntaxTrees,
        ) -> Result<ReadyLayer, Vec<Diagnostic>> {
            let mut dependencies = Vec::with_capacity(applications.len());
            for ((_, expression, ..), owner) in applications.iter().zip(owners) {
                let reference = syntax.expressions.source_span(*expression);
                let machine = preparation.typed.machine(*owner).ok_or_else(|| {
                    vec![
                        Diagnostic::error("application probe lost its exact declaration owner")
                            .with_source_span(reference),
                    ]
                })?;
                let domain = preparation.domain_for_roots(&[machine.symbol])?;
                let mut required = Vec::new();
                for &(owner, argument) in &preparation.pending_type_inputs {
                    if !domain.contains(owner) {
                        continue;
                    }
                    let dependency = applications
                        .iter()
                        .position(|(candidate, ..)| *candidate == argument)
                        .ok_or_else(|| {
                            vec![
                                Diagnostic::error("pending type input lost its original argument")
                                    .with_source_span(reference),
                            ]
                        })?;
                    push_unique(&mut required, dependency);
                }
                dependencies.push(required);
            }
            let arguments = dependencies
                .iter()
                .enumerate()
                .filter_map(|(ordinal, required)| required.is_empty().then_some(ordinal))
                .collect::<Vec<_>>();
            let mut ordered = vec![false; applications.len()];
            loop {
                let mut changed = false;
                for (ordinal, required) in dependencies.iter().enumerate() {
                    if !ordered[ordinal] && required.iter().all(|dependency| ordered[*dependency]) {
                        ordered[ordinal] = true;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            if let Some(ordinal) = ordered.iter().position(|ordered| !ordered) {
                return Err(vec![
                    Diagnostic::error("const-generic application dependency cycle")
                        .with_source_span(syntax.expressions.source_span(applications[ordinal].1)),
                ]);
            }
            Ok(ReadyLayer { arguments })
        }
    }
    mod extent_entries {
        use crate::machine_execution::admission::require_closed_expression_custody;
        use diagnostics::Diagnostic;
        use source::SourceSpan;
        use symbols::SymbolHandle;
        use syntax_trees::{
            SyntaxTrees,
            expression::ExpressionHandle,
            identifier::Identifier,
            types::{ConstArgumentOrigin, TypeReferenceHandle},
        };
        use syntax_trees_to_symbol_resolved_trees::pre_resolution::retain_deferred_array_extent;
        use typed_trees::TypedTrees;
        use validation::expression_result_type_reference;

        type Application = (
            TypeReferenceHandle,
            ExpressionHandle,
            TypeReferenceHandle,
            bool,
        );
        type LexicalSelection = (
            TypeReferenceHandle,
            Vec<ConstArgumentOrigin>,
            Vec<(SourceSpan, SourceSpan)>,
        );

        pub(crate) fn retain(
            syntax: &mut SyntaxTrees,
            execution: &TypedTrees,
            applications: &[Application],
            owners: &[SymbolHandle],
            extents: &[TypeReferenceHandle],
            lexical_selections: &[LexicalSelection],
            authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
        ) -> Result<bool, Vec<Diagnostic>> {
            let mut retained = false;
            for ((argument, expression, _, public), owner) in applications.iter().zip(owners) {
                if !extents.contains(argument) {
                    continue;
                }
                let reference = syntax.expressions.source_span(*expression);
                let failure = |reason: String| {
                    vec![
                        Diagnostic::error(format!("array extent entry retention failed: {reason}"))
                            .with_source_span(reference),
                    ]
                };
                let machine = execution
                    .machine(*owner)
                    .ok_or_else(|| failure("extent probe lost its exact declaration".into()))?;
                let [state] = execution.machine_states(machine) else {
                    return Err(failure("extent probe lost its state".into()));
                };
                let [typed_trees::statement::StatementNode::Transition(transition)] =
                    execution.statement_table.statements(state.statement_nodes)
                else {
                    return Err(failure("extent probe lost its return".into()));
                };
                let typed_trees::statement::TransitionTargetNode::Value(root) = execution
                    .statement_table
                    .transition_target(transition.target)
                else {
                    return Err(failure("extent probe return is not a value".into()));
                };
                let (origins, _, calls) =
                    super::super::const_generic_expressions::pending_leaf_call_custody(
                        execution,
                        machine,
                        state,
                        &[*root],
                        *public,
                        syntax,
                    )
                    .map_err(&failure)?;
                let (_, expected_origins, expected_calls) = lexical_selections
                    .iter()
                    .find(|(original, ..)| original == argument)
                    .ok_or_else(|| failure("extent job lost lexical custody".into()))?;
                if origins.len() != expected_origins.len()
                    || origins
                        .iter()
                        .any(|origin| !expected_origins.contains(origin))
                    || calls.len() != expected_calls.len()
                    || calls.iter().any(|call| !expected_calls.contains(call))
                {
                    return Err(failure(
                        "extent entry changed original declaration selections".into(),
                    ));
                }
                require_closed_expression_custody(execution, *root, authority).map_err(&failure)?;
                let (natural, policy) =
                    expression_result_type_reference(execution, machine, state, *root)
                        .and_then(|result| {
                            super::super::const_generic_expressions::extent_integer_destination(
                                execution, result,
                            )
                        })
                        .ok_or_else(|| {
                            failure(
                    "array extent entry has no supported integer result carrier and policy".into(),
                )
                        })?;
                let base = syntax
                    .type_references
                    .insert_named(Identifier::new(natural.name(), reference));
                let destination = if policy == numerics::arithmetic::ArithmeticDomain::Exact {
                    base
                } else {
                    let constraint = syntax.type_references.append_constraint(
                        syntax_trees::types::TypeConstraintNode::ArithmeticDomain(policy),
                    );
                    syntax
                        .type_references
                        .insert_constrained(base, arena::HandleSpan::from_parts(constraint, 1))
                };
                let entry = super::super::const_generic_expressions::append_extent_probe(
                    syntax,
                    *argument,
                    *expression,
                    destination,
                    *public,
                );
                retain_deferred_array_extent(syntax, *argument, entry);
                retained = true;
            }
            Ok(retained)
        }
    }

    pub fn evaluate_const_generic_calls(
        syntax: SyntaxTrees,
        context: crate::BuildTimeSources<'_>,
    ) -> Result<SyntaxTrees, Vec<Diagnostic>> {
        evaluate_requested_calls(syntax, context, None)
    }

    pub(crate) fn evaluate_declaration_calls(
        syntax: SyntaxTrees,
        context: crate::BuildTimeSources<'_>,
        declarations: &[syntax_trees::item::ItemHandle],
    ) -> Result<SyntaxTrees, Vec<Diagnostic>> {
        let mut syntax = syntax;
        let uses =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::closed_declaration_constant_uses(
            &mut syntax,
            context.sources.clone(),
            context.source_scoped_top_level_bindings.to_vec(),
            context.retained_base(),
            context.declaration_syntax,
            declarations,
        )?;
        evaluate_requested_calls(syntax, context, Some(&uses))
    }

    fn evaluate_requested_calls(
        mut syntax: SyntaxTrees,
        context: crate::BuildTimeSources<'_>,
        requested: Option<&ClosedDeclarationConstantUses>,
    ) -> Result<SyntaxTrees, Vec<Diagnostic>> {
        loop {
            let (evaluated, progress) =
                evaluate_application_group(syntax, context.clone(), requested)?;
            syntax = evaluated;
            if !progress {
                return Ok(syntax);
            }
        }
    }

    fn evaluate_application_group(
        mut syntax: SyntaxTrees,
        context: crate::BuildTimeSources<'_>,
        requested: Option<&ClosedDeclarationConstantUses>,
    ) -> Result<(SyntaxTrees, bool), Vec<Diagnostic>> {
        let mut progress = false;
        let sources = context.sources.clone();
        let source_scoped_top_level_bindings = context.source_scoped_top_level_bindings;
        let selection_authority = context.selection_authority.clone();
        let mut endpoint_leaves = Vec::new();
        let has_call_const_expression = type_references(&syntax)
            .const_expression_nodes()
            .into_iter()
            .any(|(_, expression)| contains_call(&syntax, expression));
        if endpoint_leaves.is_empty() && !has_call_const_expression {
            return Ok((syntax, false));
        }
        let (const_arguments, extent_uses) = if let Some(requested) = requested {
            let live_extents = closed_array_extent_expressions(&mut syntax);
            let extents = requested
                .array_extents
                .iter()
                .filter(|(argument, ..)| live_extents.iter().any(|(live, ..)| live == argument))
                .copied()
                .collect::<Vec<_>>();
            let extent_uses = extents.iter().map(|(argument, ..)| *argument).collect();
            let mut arguments = requested.arguments.clone();
            arguments.extend(extents);
            (arguments, extent_uses)
        } else {
            let mut const_arguments =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::closed_data_const_argument_expressions(
            &mut syntax, sources.clone(), source_scoped_top_level_bindings.to_vec(), context.retained_base(), context.declaration_syntax,
        )?;
            const_arguments.extend(
            syntax_trees_to_symbol_resolved_trees::pre_resolution::closed_machine_const_arguments(
                &mut syntax,
                sources.clone(),
                source_scoped_top_level_bindings.to_vec(),
                context.retained_base(),
                context.declaration_syntax,
            )?,
        );
            let extent_arguments = closed_array_extent_expressions(&mut syntax);
            let extent_uses = extent_arguments
                .iter()
                .map(|(use_handle, ..)| *use_handle)
                .collect::<Vec<_>>();
            const_arguments.extend(extent_arguments);
            (const_arguments, extent_uses)
        };
        let mut applications = Vec::new();
        for (type_reference, ..) in &const_arguments {
            if let TypeReferenceNode::ConstExpression(expression) =
                syntax.type_references.type_reference(*type_reference)
                && contains_call(&syntax, *expression)
                && !applications
                    .iter()
                    .any(|(existing, _)| existing == type_reference)
            {
                applications.push((*type_reference, *expression));
            }
        }
        if endpoint_leaves.is_empty() && applications.is_empty() {
            return Ok((syntax, false));
        }

        let mut application_destinations = Vec::new();
        if !applications.is_empty() {
            for (type_reference, expression) in &applications {
                let Some((_, destination, public)) = const_arguments
                    .iter()
                    .find(|(argument, _, _)| argument == type_reference)
                else {
                    return Err(vec![Diagnostic::error(
                    "a const-generic application call must occupy a declared const argument destination",
                )
                .with_source_span(syntax.expressions.source_span(*expression))]);
                };
                application_destinations.push((
                    *type_reference,
                    *expression,
                    *destination,
                    *public,
                ));
            }
        }

        let mut probe = syntax.clone();
        crate::syntax_probes::retain_pending_const_inputs(
            &mut probe,
            &application_destinations
                .iter()
                .map(|(argument, ..)| *argument)
                .collect::<Vec<_>>(),
        );
        let mut lexical_selections = Vec::new();
        if !applications.is_empty() {
            let resolved = syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_const_argument_selection(
            context.resolution_request(probe.clone()),
        )?;
            for (argument, expression) in &applications {
                let failure = |reason| {
                    vec![
                        Diagnostic::error(reason)
                            .with_source_span(syntax.expressions.source_span(*expression)),
                    ]
                };
                let origins = super::const_generic_expressions::lexical_selection::retain(
                    &syntax,
                    &resolved,
                    *expression,
                )
                .map_err(&failure)?;
                let calls = super::const_generic_expressions::lexical_selection::retain_calls(
                    &syntax,
                    &resolved,
                    *expression,
                )
                .map_err(failure)?;
                lexical_selections.push((*argument, origins, calls));
            }
        }

        let declaration_frontier = probe.root_item_handles().len();
        let owners = application_destinations
            .iter()
            .enumerate()
            .map(|(ordinal, (_, expression, destination, _))| {
                super::const_generic_expressions::append_probe(
                    &mut probe,
                    ordinal,
                    *expression,
                    *destination,
                )
            })
            .collect::<Vec<_>>();
        let declarations = probe.root_item_handles()[declaration_frontier..].to_vec();
        let probe = crate::syntax_probes::normalize_initializer_probe_data(
            probe,
            sources.clone(),
            source_scoped_top_level_bindings,
            context.retained_base(),
            [
                context.declaration_syntax,
                context.retained_declaration_syntax,
            ],
            &declarations,
        )?;
        let endpoint_references = endpoint_leaves
            .iter()
            .map(|(_, source)| *source)
            .collect::<Vec<_>>();
        let frontier = resolve_deferred_initializer_probe(context.resolution_request(probe))?;
        let owner_symbols = owners
            .iter()
            .map(|owner| owner.resolve(frontier.trees()))
            .collect::<Result<Vec<_>, _>>()?;
        let preparation = crate::syntax_probes::prepare_owned_probe_evaluation(
            frontier,
            &owner_symbols,
            &endpoint_references,
            context.predecessor.clone(),
        )?;
        if requested.is_none()
            && extent_entries::retain(
                &mut syntax,
                &preparation.typed,
                &application_destinations,
                &owner_symbols,
                &extent_uses,
                &lexical_selections,
                selection_authority.as_deref(),
            )?
        {
            return Ok((syntax, true));
        }
        let layer = dependencies::ready_layer(
            &preparation,
            &application_destinations,
            &owner_symbols,
            &syntax,
        )?;

        if !layer.arguments.is_empty() {
            let probe_symbols = layer
                .arguments
                .iter()
                .map(|ordinal| owner_symbols[*ordinal])
                .collect::<Vec<_>>();
            let ready_domain = preparation.domain_for_roots(&probe_symbols)?;
            let prepared =
                PreparedBuildMachineProgram::prepare_in(&preparation.typed, &ready_domain)?;
            let execution = prepared.typed();
            let admission = BuildTimeAdmissionPlan::infer(execution, selection_authority.clone())?;
            let operators = validation::derive_pre_flow_operator_selections(execution);
            let independent = probe_symbols
                .iter()
                .copied()
                .filter(|symbol| {
                    !admission.closure_needs_execution_preparation(execution, *symbol, &operators)
                })
                .collect::<Vec<_>>();
            let deferred_extents = extent_uses
                .iter()
                .copied()
                .filter(|argument| {
                    requested.is_none()
                        || !application_destinations.iter().enumerate().any(
                            |(ordinal, (candidate, ..))| {
                                candidate == argument
                                    && independent.contains(&owner_symbols[ordinal])
                            },
                        )
                })
                .collect::<Vec<_>>();
            progress |= extent_entries::retain(
                &mut syntax,
                &preparation.typed,
                &application_destinations,
                &owner_symbols,
                &deferred_extents,
                &lexical_selections,
                selection_authority.as_deref(),
            )?;
            let independent_domain = preparation.domain_for_roots(&independent)?;
            let independent_program =
                PreparedBuildMachineProgram::prepare_in(&preparation.typed, &independent_domain)?;
            let mut checked_program = None;
            for ordinal in &layer.arguments {
                let (argument, expression, _, public) = &application_destinations[*ordinal];
                if deferred_extents.contains(argument) {
                    continue;
                }
                let reference = syntax.expressions.source_span(*expression);
                let failure = |reason: String| {
                    vec![
                        Diagnostic::error(format!(
                            "const-generic application evaluation failed: {reason}"
                        ))
                        .with_source_span(reference),
                    ]
                };
                let machine = execution.machine(owner_symbols[*ordinal]).ok_or_else(|| {
                    failure("application probe lost its exact private declaration".into())
                })?;
                if admission.closure_needs_execution_preparation(
                    execution,
                    machine.symbol,
                    &operators,
                ) {
                    continue;
                }
                let checked = match checked_program {
                    Some(ref checked) => checked,
                    None => checked_program.insert(CheckedInitializers::prepare(
                        &independent_program,
                        selection_authority.clone(),
                        &independent,
                        &independent_domain,
                        &[],
                    )?),
                };
                let invocation = checked.calls_for_symbol(machine.symbol).map_err(&failure)?;
                let evaluated = invocation
                    .evaluate_application_probe(
                        reference,
                        *public,
                        &mut syntax,
                        extent_uses.contains(argument),
                    )
                    .map_err(&failure)?;
                let (_, origins, calls) = lexical_selections
                    .iter()
                    .find(|(original, ..)| original == argument)
                    .ok_or_else(|| {
                        failure("application lost its original lexical selections".into())
                    })?;
                if calls.iter().any(|call| !evaluated.calls.contains(call))
                    || origins
                        .iter()
                        .any(|origin| !evaluated.origins.contains(origin))
                {
                    return Err(failure(
                        "application probe changed the original constant or call selection"
                            .to_owned(),
                    ));
                }
                let folded = match evaluated.value.decode_encoding() {
                    Some(DecodedCanonicalConstValue::Integer { value, .. }) => value.to_string(),
                    Some(_) => evaluated.value.atom(),
                    _ => {
                        return Err(failure(
                            "application probe returned an invalid canonical value".to_owned(),
                        ));
                    }
                };
                crate::report_evaluation_diagnostics(&evaluated.warnings)?;
                syntax.type_references.retain_const_argument_normalization(
                    *argument,
                    reference,
                    evaluated.value.encoding.clone(),
                    evaluated.origins,
                    evaluated.operators,
                );
                if evaluated.materialized_expression.is_valid() {
                    syntax
                        .type_references
                        .retain_const_argument_expression(*argument, *expression);
                    syntax
                        .type_references
                        .retain_const_argument_materialized_expression(
                            *argument,
                            evaluated.materialized_expression,
                        );
                }
                syntax.type_references.replace_type_reference(
                    *argument,
                    TypeReferenceNode::Named(Identifier::generated(folded)),
                );
                progress = true;
            }
        } else if extent_entries::retain(
            &mut syntax,
            &preparation.typed,
            &application_destinations,
            &owner_symbols,
            &extent_uses,
            &lexical_selections,
            selection_authority.as_deref(),
        )? {
            return Ok((syntax, true));
        }

        let mut retained_endpoint_leaves = Vec::with_capacity(endpoint_leaves.len());
        for leaf in endpoint_leaves {
            let domain = preparation.domain_for_inputs(&[], &[leaf.1])?;
            if !preparation
                .pending_type_inputs
                .iter()
                .any(|(owner, _)| domain.contains(*owner))
            {
                retained_endpoint_leaves.push(leaf);
            }
        }
        endpoint_leaves = retained_endpoint_leaves;
        let endpoint_sources = endpoint_leaves
            .iter()
            .map(|(_, source)| *source)
            .collect::<Vec<_>>();
        let evaluation_domain = preparation.domain_for_inputs(&[], &endpoint_sources)?;
        let mut typed = preparation.typed;
        if !endpoint_leaves.is_empty() {
            crate::const_evaluation::range_endpoints::evaluate_selected_range_endpoints(
                &mut typed,
                selection_authority.clone(),
                crate::SelectedBuildTimeOperators {
                    operators: &[],
                    provider_bodies: &[],
                },
                &evaluation_domain,
            )?;
            for (expression, source_span) in endpoint_leaves {
                let value = expression_table(&typed)
                    .iter_expressions()
                    .filter(|(handle, _)| {
                        typed.expression_table.source_span(*handle) == source_span
                    })
                    .find_map(|(_, node)| match node {
                        typed_trees::expression::ExpressionNode::Integer(literal) => {
                            literal.value_bignum()
                        }
                        _ => None,
                    });
                let Some(value) = value else {
                    continue;
                };
                let literal = IntegerLiteral::from_parts(
                    value.is_negative(),
                    IntegerRadix::Decimal,
                    value.abs().to_string().as_str(),
                )
                .expect("an evaluated range endpoint is a valid integer literal");
                syntax
                    .expressions
                    .replace_expression(expression, ExpressionNode::Integer(literal));
                progress = true;
            }
        }
        Ok((syntax, progress))
    }

    pub(crate) fn contains_call(syntax: &SyntaxTrees, expression: ExpressionHandle) -> bool {
        match syntax.expressions.expression(expression) {
            ExpressionNode::Binary(binary) => {
                contains_call(syntax, binary.left) || contains_call(syntax, binary.right)
            }
            ExpressionNode::Unary(unary) => contains_call(syntax, unary.operand),
            ExpressionNode::Cast(cast) => contains_call(syntax, cast.value),
            ExpressionNode::Match(dispatch) => contains_call(syntax, dispatch.subject)
                || syntax
                    .expressions
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        matches!(arm.pattern, syntax_trees::expression::MatchPattern::Value(pattern)
                        if contains_call(syntax, pattern))
                            || contains_call(syntax, arm.value)
                    }),
            ExpressionNode::Call(_) => true,
            _ => false,
        }
    }
}
pub(crate) use crate::const_generic_expressions;
pub(crate) use crate::const_initializers;
pub(crate) mod const_lengths {
    use crate::build_time_evaluation::{EvaluationDomain, type_reference_table};
    use crate::machine_execution::selected_operators::prepare_selected_execution;
    use diagnostics::Diagnostic;
    use numerics::bignum::BigInt;
    use typed_trees::TypedTrees;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn pending_array_lengths(
        typed: &TypedTrees,
        domain: &EvaluationDomain,
    ) -> Vec<(
        TypeReferenceHandle,
        symbols::SymbolHandle,
        String,
        source::SourceSpan,
    )> {
        domain
            .references(typed)
            .type_references
            .into_iter()
            .filter_map(
                |handle| match typed.type_reference_table.type_reference(handle) {
                    TypeReferenceNode::FixedArray {
                        length:
                            FixedArrayLength::ConstCall {
                                machine,
                                name,
                                source_span,
                            },
                        ..
                    } => Some((handle, *machine, name.as_str().to_owned(), *source_span)),
                    _ => None,
                },
            )
            .collect()
    }

    pub fn evaluate_const_array_lengths(
        typed: &mut TypedTrees,
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    ) -> Result<(), Vec<Diagnostic>> {
        evaluate_const_array_lengths_in(typed, selection_authority, &EvaluationDomain::all())
    }

    pub(crate) fn evaluate_const_array_lengths_in(
        typed: &mut TypedTrees,
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        domain: &EvaluationDomain,
    ) -> Result<(), Vec<Diagnostic>> {
        LengthJobs::new(
            selection_authority,
            domain,
            LengthProviderSelection::Independent,
        )
        .evaluate(typed)
        .map(|_| ())
    }

    pub(crate) fn evaluate_independent_lengths(
        typed: &mut TypedTrees,
        authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        domain: &EvaluationDomain,
    ) -> Result<bool, Vec<Diagnostic>> {
        LengthJobs::new(authority, domain, LengthProviderSelection::Pending).evaluate(typed)
    }

    pub(crate) fn evaluate_selected_array_lengths(
        typed: &mut TypedTrees,
        authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        selected: crate::SelectedBuildTimeOperators<'_>,
        domain: &EvaluationDomain,
    ) -> Result<(), Vec<Diagnostic>> {
        let crate::SelectedBuildTimeOperators {
            operators,
            provider_bodies,
        } = selected;
        let selected_program = prepare_selected_execution(typed, provider_bodies, operators)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let mut execution = selected_program.unwrap_or_else(|| typed.clone());
        LengthJobs::new(
            authority,
            domain,
            LengthProviderSelection::Selected(operators),
        )
        .evaluate(&mut execution)?;
        publish_evaluated_array_lengths(typed, &execution, domain);
        Ok(())
    }

    pub(crate) fn publish_evaluated_array_lengths(
        typed: &mut TypedTrees,
        execution: &TypedTrees,
        domain: &EvaluationDomain,
    ) {
        for (type_reference, _, _, _) in pending_array_lengths(typed, domain) {
            if let TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(value),
                ..
            } = type_reference_table(execution).type_reference(type_reference)
            {
                typed
                    .type_reference_table
                    .set_fixed_array_length(type_reference, *value);
            }
        }
    }

    enum LengthProviderSelection<'selection> {
        Pending,
        Independent,
        Selected(&'selection [crate::SelectedBuildTimeBinaryOperator]),
    }

    struct LengthJobs<'selection> {
        authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        domain: &'selection EvaluationDomain,
        providers: LengthProviderSelection<'selection>,
        active: Vec<(TypeReferenceHandle, String)>,
    }

    impl<'selection> LengthJobs<'selection> {
        fn new(
            authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
            domain: &'selection EvaluationDomain,
            providers: LengthProviderSelection<'selection>,
        ) -> Self {
            Self {
                authority,
                domain,
                providers,
                active: Vec::new(),
            }
        }

        fn evaluate(&mut self, typed: &mut TypedTrees) -> Result<bool, Vec<Diagnostic>> {
            let pending = pending_array_lengths(typed, self.domain);
            if pending.is_empty() {
                return Ok(false);
            }
            let prepared = crate::PreparedBuildMachineProgram::prepare_in(typed, self.domain)?;
            let mut execution = prepared.typed().clone();
            let mut deferred = false;
            let mut diagnostics = Vec::new();
            for (handle, _, _, _) in pending {
                match self.evaluate_length(&mut execution, handle) {
                    Ok(Some(value)) => typed
                        .type_reference_table
                        .set_fixed_array_length(handle, value),
                    Ok(None) => deferred = true,
                    Err(errors) => diagnostics.extend(errors),
                }
            }
            if diagnostics.is_empty() {
                Ok(deferred)
            } else {
                Err(diagnostics)
            }
        }

        fn evaluate_length(
            &mut self,
            typed: &mut TypedTrees,
            handle: TypeReferenceHandle,
        ) -> Result<Option<usize>, Vec<Diagnostic>> {
            let (machine, name, source) = match typed.type_reference_table.type_reference(handle) {
                TypeReferenceNode::FixedArray {
                    length: FixedArrayLength::Literal(value),
                    ..
                } => {
                    return Ok(Some(*value));
                }
                TypeReferenceNode::FixedArray {
                    length:
                        FixedArrayLength::ConstCall {
                            machine,
                            name,
                            source_span,
                        },
                    ..
                } => (*machine, name.as_str().to_owned(), *source_span),
                _ => {
                    return Err(vec![Diagnostic::error(
                        "length job lost its exact array type",
                    )]);
                }
            };
            if let Some(position) = self.active.iter().position(|(active, _)| *active == handle) {
                let chain = self.active[position..]
                    .iter()
                    .map(|(reference, name)| format!("{name} at {reference:?}"))
                    .chain(std::iter::once(format!("{name} at {handle:?}")))
                    .collect::<Vec<_>>()
                    .join(" -> ");
                return Err(vec![
                    Diagnostic::error(format!("fixed-array length dependency cycle: {chain}"))
                        .with_source_span(source),
                ]);
            }
            self.active.push((handle, name.clone()));
            let result = self.prepare_and_invoke(typed, machine, source);
            self.active.pop();
            let value = result.map_err(|mut errors| {
                for error in &mut errors {
                    error.message = format!(
                        "fixed-array length `{}`: const evaluation of `{name}` failed: {}",
                        typed.type_reference_table.display_name(handle),
                        error.message,
                    );
                    if error.source_span.is_none() {
                        error.source_span = Some(source);
                    }
                }
                errors
            })?;
            if let Some(value) = value {
                typed
                    .type_reference_table
                    .set_fixed_array_length(handle, value);
            }
            Ok(value)
        }

        fn prepare_and_invoke(
            &mut self,
            typed: &mut TypedTrees,
            machine: symbols::SymbolHandle,
            source: source::SourceSpan,
        ) -> Result<Option<usize>, Vec<Diagnostic>> {
            let domain = EvaluationDomain::all()
                .with_accounting(self.domain.accounting.clone())
                .demanded_roots(typed, &[machine])?;
            let mut execution = domain.execution_program(typed);
            let admission = domain.admission(&execution, self.authority.clone())?;
            if matches!(self.providers, LengthProviderSelection::Pending) {
                let facts = validation::derive_pre_flow_operator_selections(&execution);
                if admission.closure_needs_execution_preparation(&execution, machine, &facts) {
                    return Ok(None);
                }
            }
            let mut deferred = false;
            let mut dependencies = pending_array_lengths(&execution, &EvaluationDomain::all());
            dependencies.sort_by_key(|(handle, _, _, _)| {
                self.active.iter().any(|(active, _)| active == handle)
            });
            for (dependency, _, _, _) in dependencies {
                if self.evaluate_length(typed, dependency)?.is_none() {
                    deferred = true;
                }
            }
            if deferred {
                return Ok(None);
            }

            typed_trees_to_typed_trees::prepare_execution_applications(typed)?;
            let domain = EvaluationDomain::all()
                .with_accounting(self.domain.accounting.clone())
                .demanded_roots(typed, &[machine])?;
            execution = domain.execution_program(typed);

            let references = typed_trees::references::collect_program_references(&execution);
            let operators = match &self.providers {
                LengthProviderSelection::Selected(operators) => operators
                    .iter()
                    .filter(|row| references.expressions.contains(&row.expression))
                    .cloned()
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            };
            let all_endpoints =
                EvaluationDomain::all().with_accounting(self.domain.accounting.clone());
            if matches!(self.providers, LengthProviderSelection::Pending)
            && crate::const_evaluation::range_endpoints::pending_endpoint_calls_need_operator_selection(
                &execution, self.authority.clone(), &all_endpoints,
            )?
        {
            return Ok(None);
        }
            crate::const_evaluation::range_endpoints::evaluate_selected_range_endpoints(
                &mut execution,
                self.authority.clone(),
                crate::SelectedBuildTimeOperators {
                    operators: &operators,
                    provider_bodies: &[],
                },
                &all_endpoints,
            )?;
            let preparation_request = typed_trees_to_typed_trees::PreparationRequest::settled();
            let checking = typed_trees_to_checked_trees::CheckingRequest::new();
            let (execution, preparation) =
                typed_trees_to_typed_trees::prepare_typed_trees(execution, &preparation_request)?;
            let checked =
                typed_trees_to_checked_trees::lower_typed_trees(execution, preparation, &checking)?;
            let admitted = prepare_selected_execution(&checked.typed, &[], &operators)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let execution = admitted.as_ref().unwrap_or(&checked.typed);
            let root = execution.machine_by_symbol(machine).ok_or_else(|| {
                vec![Diagnostic::error(
                    "array length lost its exact selected machine",
                )]
            })?;
            require_zero_argument_entry(execution, root)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let admission = self
                .domain
                .admission(execution, self.authority.clone())?
                .with_selected_operators(execution, &operators)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            admission
                .require_const_evaluable_invocation(
                    execution,
                    root,
                    &[],
                    crate::BuildTimeInvocationCustody::Source(source),
                    false,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let prepared = crate::PreparedBuildMachineProgram::from_checked(checked);
            let entry = prepared
                .entry(machine)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let value = prepared
                .evaluate_checked_constant(&entry, &operators, &self.domain.accounting)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let execution = prepared.typed();
            let root = execution.machine_by_symbol(machine).ok_or_else(|| {
                vec![Diagnostic::error(
                    "checked length job lost its exact machine",
                )]
            })?;
            decode_length_result(execution, root, value)
                .map(Some)
                .map_err(|reason| vec![Diagnostic::error(reason)])
        }
    }

    fn require_zero_argument_entry(typed: &TypedTrees, machine: &Machine) -> Result<(), String> {
        let parameter_count = entry_state_parameter_count(typed, machine);
        if parameter_count > 0 {
            let machine_name = machine.name.as_str();
            return Err(format!(
                "machine `{machine_name}` takes {parameter_count} parameter(s); a const-evaluated \
             array length must call a zero-argument machine (const arguments are not supported yet)"
            ));
        }
        Ok(())
    }

    fn decode_length_result(
        typed: &TypedTrees,
        machine: &Machine,
        value: terminal_interpreter::TerminalExecutionResult,
    ) -> Result<usize, String> {
        let value = match value {
            terminal_interpreter::TerminalExecutionResult::Scalar(
                terminal_interpreter::TerminalScalarValue::Integer { value, .. },
            ) => match value {
                semantic_vocabulary::IntegerValue::Signed(value) => BigInt::from_i128(value),
                semantic_vocabulary::IntegerValue::Unsigned(value) => BigInt::from_u128(value),
            },
            _ => {
                let primitive = entry_state(typed, machine)
                    .and_then(|state| typed.primitive_type_reference(state.return_type));
                return Err(match primitive {
                    Some(primitive) => format!(
                        "machine `{}` returns `{}`, not an integer type",
                        machine.name.as_str(),
                        primitive.name(),
                    ),
                    None => format!(
                        "machine `{}` does not declare an integer return type",
                        machine.name.as_str(),
                    ),
                });
            }
        };
        if value.is_negative() {
            return Err(format!(
                "the call returned {value}, but an array length must be a non-negative integer"
            ));
        }
        value
            .to_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("the call returned {value}, which does not fit an array length"))
    }

    pub(crate) fn decode_integer_result(
        typed: &TypedTrees,
        machine: &Machine,
        value: crate::BuildTimeValue,
    ) -> Result<BigInt, String> {
        let machine_name = machine.name.as_str();
        let primitive = entry_state(typed, machine)
            .and_then(|state| typed.primitive_type_reference(state.return_type));
        if let (Some(primitive), crate::BuildTimeValue::Int(value)) = (primitive, &value)
            && primitive.accepts_integer_literal()
        {
            return Ok(if primitive.is_signed_integer() {
                BigInt::from_i64(*value)
            } else {
                BigInt::from_u64(*value as u64)
            });
        }
        match primitive {
            Some(primitive) => Err(format!(
                "machine `{machine_name}` returns `{}`, not an integer type",
                primitive.name()
            )),
            None => Err(format!(
                "machine `{machine_name}` does not declare an integer return type"
            )),
        }
    }

    fn entry_state_parameter_count(typed: &TypedTrees, machine: &Machine) -> usize {
        entry_state(typed, machine)
            .map(|state| typed.state_parameters(state).len())
            .unwrap_or(0)
    }

    fn entry_state<'a>(typed: &'a TypedTrees, machine: &Machine) -> Option<&'a State> {
        let leaf = machine
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or_default();
        let states = typed.machine_states(machine);
        states
            .iter()
            .find(|state| state.name.as_str() == leaf)
            .or_else(|| states.first())
    }
}
pub(crate) mod const_qualifications {
    use crate::build_time_evaluation::type_references;
    use crate::syntax_probes::prepare_owned_probe_evaluation;
    use diagnostics::Diagnostic;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::item::{ConstQualification, Item};
    use syntax_trees::types::TypeReferenceNode;
    use syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_deferred_initializer_probe;

    pub(crate) fn prove_ready(
        mut syntax: SyntaxTrees,
        context: &crate::BuildTimeSources<'_>,
    ) -> Result<SyntaxTrees, Vec<Diagnostic>> {
        let candidates = syntax
            .root_item_handles()
            .iter()
            .filter_map(|item| {
                let Item::Const(definition) = syntax.root_item(*item) else {
                    return None;
                };
                definition
                    .qualification
                    .is_none()
                    .then_some((*item, definition.clone()))
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Ok(syntax);
        }

        let crate::syntax_probes::InitializerSelectionInputs {
            authored,
            mut probe,
        } = crate::syntax_probes::initializer_selection_inputs(syntax, context)?;
        syntax = authored;
        let Some(selection) =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::prepare_const_qualification_selection(
            context.resolution_request(probe.clone()),
            &syntax,
            candidates.iter().map(|(_, definition)| definition),
        )?
    else {
        return Ok(syntax);
    };
        let mut pending = Vec::new();
        for (item, mut definition) in candidates {
            if (definition.pending_qualification.is_some()
                || selection.initializer_requires_destination_proof(&syntax, &definition))
                && selection.pending_leaves(&syntax, &definition)?.is_empty()
                && destination_inputs_ready(&selection, &syntax, &definition)?
            {
                let expected = syntax_trees::item::ConstDestinationObligation {
                    declaration: definition.name.source_span(),
                    destination: definition.type_reference,
                    initializer: definition.value,
                };
                if definition
                    .pending_qualification
                    .as_ref()
                    .is_some_and(|actual| *actual != expected)
                {
                    return Err(vec![
                        Diagnostic::error(
                            "constant destination obligation drifted from its declaration",
                        )
                        .with_source_span(definition.name.source_span()),
                    ]);
                }
                definition.pending_qualification = Some(expected);
                syntax
                    .items
                    .replace_item(item, Item::Const(definition.clone()));
                let Item::Const(mut private) = probe.root_item(item).clone() else {
                    return Err(vec![Diagnostic::error(
                        "constant destination probe lost its declaration",
                    )]);
                };
                private.pending_qualification = definition.pending_qualification.clone();
                probe.items.replace_item(item, Item::Const(private));
                pending.push((item, definition));
            }
        }
        if pending.is_empty() {
            return Ok(syntax);
        }
        let declaration_frontier = probe.root_item_handles().len();
        let owners = pending
            .iter()
            .enumerate()
            .map(|(ordinal, (_, definition))| {
                super::const_generic_expressions::append_named_probe(
                    &mut probe,
                    syntax_trees::identifier::Identifier::new(
                        format!(
                            "@const-qualification-{}-{ordinal}",
                            definition.name.as_str(&syntax)
                        ),
                        syntax.expressions.source_span(definition.value),
                    ),
                    definition.value,
                    definition.type_reference,
                )
            })
            .collect::<Vec<_>>();
        let mut probe_declarations = probe.root_item_handles()[declaration_frontier..].to_vec();
        for (_, definition) in &pending {
            let dependencies = selection.initializer_dependencies(&syntax, definition)?;
            crate::syntax_probes::extend_normalization_owners(
                &probe,
                &mut probe_declarations,
                &dependencies.normalization_owners,
            )?;
        }
        let probe = crate::syntax_probes::normalize_initializer_probe_data(
            probe,
            context.sources.clone(),
            context.source_scoped_top_level_bindings,
            context.retained_base(),
            [
                context.declaration_syntax,
                context.retained_declaration_syntax,
            ],
            &probe_declarations,
        )?;
        let frontier = resolve_deferred_initializer_probe(context.resolution_request(probe))?;
        let owners = owners
            .iter()
            .map(|owner| owner.resolve(frontier.trees()))
            .collect::<Result<Vec<_>, _>>()?;
        let prepared =
            prepare_owned_probe_evaluation(frontier, &owners, &[], context.predecessor.clone())?;
        let program = prepared.domain.execution_program(&prepared.typed);
        let request = typed_trees_to_typed_trees::PreparationRequest::settled();
        let (program, preparation) =
            typed_trees_to_typed_trees::prepare_typed_trees(program, &request)?;
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            program,
            preparation,
            &typed_trees_to_checked_trees::CheckingRequest::new(),
        )?;

        for (owner, (_, definition)) in owners.iter().zip(&pending) {
            require_complete_destination_judgment(&checked, *owner, definition.name.source_span())?;
        }

        for (item, mut definition) in pending {
            definition.pending_qualification = None;
            definition.qualification = Some(ConstQualification {
                destination: definition.type_reference,
                initializer: definition.value,
            });
            syntax.items.replace_item(item, Item::Const(definition));
        }
        Ok(syntax)
    }

    fn destination_inputs_ready(
        selection: &syntax_trees_to_symbol_resolved_trees::pre_resolution::ConstInitializerSelection,
        syntax: &SyntaxTrees,
        definition: &syntax_trees::item::ConstDefinition,
    ) -> Result<bool, Vec<Diagnostic>> {
        for (_, source) in selection
            .initializer_dependencies(syntax, definition)?
            .constants
        {
            let dependency = selection
                .trees()
                .const_declaration_at_source_span(source)
                .ok_or_else(|| {
                    vec![
                        Diagnostic::error(
                            "constant destination dependency lost its exact declaration",
                        )
                        .with_source_span(source),
                    ]
                })?;
            if dependency.evaluated_value.is_none() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn require_complete_destination_judgment(
        checked: &checked_trees::CheckedTrees,
        owner: symbols::SymbolHandle,
        declaration: source::SourceSpan,
    ) -> Result<(), Vec<Diagnostic>> {
        if checked.typed.machine_by_symbol(owner).is_none() {
            return Err(vec![
                Diagnostic::error("constant qualification lost its exact checked probe root")
                    .with_source_span(declaration),
            ]);
        }
        crate::machine_execution::admission::require_complete_checked_conditions(checked, owner)
            .map_err(|reason| {
                vec![
                    Diagnostic::error(format!(
                        "constant destination qualification remains conditional: {reason}",
                    ))
                    .with_source_span(declaration),
                ]
            })
    }

    pub(crate) fn defer_unproved(syntax: &mut SyntaxTrees) {
        for item in syntax.root_item_handles().to_vec() {
            let Item::Const(mut definition) = syntax.root_item(item).clone() else {
                continue;
            };
            if definition
                .qualification
                .as_ref()
                .is_some_and(|qualification| {
                    qualification.destination == definition.type_reference
                        && qualification.initializer == definition.value
                })
            {
                continue;
            }
            definition.qualification = None;
            while let TypeReferenceNode::Constrained { base_type, .. } =
                type_references(syntax).type_reference(definition.type_reference)
            {
                definition.type_reference = *base_type;
            }
            syntax.items.replace_item(item, Item::Const(definition));
        }
    }

    #[cfg(test)]
    mod tests {
        use super::require_complete_destination_judgment;
        use checked_trees::{CheckFacts, CheckedTrees, IntegerRangeObligation, IntegerRangeRecipe};
        use numerics::bignum::BigInt;
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::machine::Machine;

        #[test]
        fn destination_receipt_requires_its_exact_complete_checked_probe() {
            let probe = SymbolHandle::from_parts(1, 1);
            let unrelated = SymbolHandle::from_parts(2, 1);
            let missing = SymbolHandle::from_parts(3, 1);
            let mut program = TypedTrees::default();
            for symbol in [probe, unrelated] {
                program.push_machine(Machine {
                    symbol,
                    ..Default::default()
                });
            }
            let mut checked = CheckedTrees::with_roots(program, CheckFacts::default());
            let declaration = source::SourceSpan::default();
            assert!(require_complete_destination_judgment(&checked, probe, declaration).is_ok());
            assert!(require_complete_destination_judgment(&checked, missing, declaration).is_err());

            checked
                .facts
                .integer_range_obligations
                .push(IntegerRangeObligation::retain(
                    ExpressionHandle::invalid(),
                    unrelated,
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                    IntegerRangeRecipe::bounds(BigInt::from_i64(0), BigInt::from_i64(1)),
                    BigInt::from_i64(0),
                    BigInt::from_i64(1),
                    "unrelated retained target condition".to_owned(),
                ));
            assert!(require_complete_destination_judgment(&checked, probe, declaration).is_ok());
            checked
                .facts
                .integer_range_obligations
                .push(IntegerRangeObligation::retain(
                    ExpressionHandle::invalid(),
                    probe,
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                    IntegerRangeRecipe::bounds(BigInt::from_i64(0), BigInt::from_i64(1)),
                    BigInt::from_i64(0),
                    BigInt::from_i64(1),
                    "destination retained target condition".to_owned(),
                ));
            assert!(require_complete_destination_judgment(&checked, probe, declaration).is_err());
        }
    }
}
pub(crate) mod constant_recipes {
    use crate::build_time_evaluation::EvaluationDomain;
    use crate::const_evaluation::{const_applications, const_lengths, range_endpoints};
    use diagnostics::Diagnostic;
    use typed_trees::TypedTrees;
    use typed_trees::constant_recipes::{
        ConstantExpressionContext, RetainedConstantPosition, RetainedConstantRecipe,
        retain_constant_recipes,
    };
    use typed_trees::name::Identifier;

    pub(crate) fn retain_deferred_constant_recipes(
        typed: &mut TypedTrees,
        domain: &EvaluationDomain,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut recipes = Vec::new();
        for (type_reference, machine, name, source_span) in
            const_lengths::pending_array_lengths(typed, domain)
        {
            recipes.push(RetainedConstantRecipe {
                position: RetainedConstantPosition::ArrayLength(type_reference),
                contexts: Default::default(),
                invocations: Default::default(),
                machine,
                name: Identifier::generated(name),
                source_span,
            });
        }
        let integers = range_endpoints::integer_root_plan(typed, domain)?;
        for root in &integers.roots {
            let context = root.context;
            let position = RetainedConstantPosition::ContractExpression(root.expression);
            if typed.constant_recipes.iter().any(|(_, recipe)| {
                recipe.position == position
                    && typed
                        .constant_expression_contexts
                        .span_or_empty(recipe.contexts)
                        .iter()
                        .any(|retained| retained.same_scope(&context))
            }) {
                continue;
            }
            let calls: Vec<_> = integers
                .calls
                .iter()
                .filter(|call| call.contract_expression && call.root == root.expression)
                .collect();
            let machine = calls.first().map(|call| call.machine).unwrap_or_default();
            recipes.push(RetainedConstantRecipe {
                position,
                contexts: typed.constant_expression_contexts.insert_many([context]),
                invocations: typed
                    .expression_table
                    .insert_expression_handles(calls.iter().map(|call| call.expression)),
                machine,
                name: Identifier::generated(if machine.is_valid() {
                    typed.symbols.name(machine)
                } else {
                    "constant expression"
                }),
                source_span: typed.expression_table.source_span(root.expression),
            });
        }
        let plan = const_applications::const_application_plan(typed, domain);
        let owners = typed_trees::references::collect_program_operand_owners(typed);
        for application in &plan.applications {
            let call = plan.calls.iter().find(|call| call.site == application.site);
            let machine = call.map(|call| call.machine).unwrap_or_default();
            let destination = application.destination.ok_or_else(|| {
                vec![
                    Diagnostic::error(
                        "const-generic application has no declared const destination",
                    )
                    .with_source_span(typed.expression_table.source_span(application.root)),
                ]
            })?;
            let contexts = owners
                .type_owners(application.site)
                .map(|owner| ConstantExpressionContext::new(owner, destination));
            recipes.push(RetainedConstantRecipe {
                position: RetainedConstantPosition::ConstApplication {
                    site: application.site,
                    root: application.root,
                },
                contexts: typed.constant_expression_contexts.insert_many(contexts),
                machine,
                invocations: typed.expression_table.insert_expression_handles(
                    plan.calls
                        .iter()
                        .filter(|call| call.site == application.site)
                        .map(|call| call.expression),
                ),
                name: Identifier::generated(if machine.is_valid() {
                    typed.symbols.name(machine)
                } else {
                    "const application"
                }),
                source_span: typed.expression_table.source_span(application.root),
            });
        }
        retain_constant_recipes(typed, recipes);
        Ok(())
    }
}
pub(crate) mod contract_expressions {
    use crate::build_time_evaluation::{EvaluationDomain, expression_table};
    use crate::machine_execution::admission::expression_children;
    use typed_trees::TypedTrees;
    use typed_trees::constant_recipes::ConstantExpressionContext;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    pub(crate) struct ContractRoot {
        pub expression: ExpressionHandle,
        pub context: ConstantExpressionContext,
        pub invocations: Vec<super::range_endpoints::PendingEndpoint>,
    }

    pub(crate) fn roots(
        program: &TypedTrees,
        domain: &EvaluationDomain,
    ) -> Result<Vec<ContractRoot>, Vec<diagnostics::Diagnostic>> {
        let mut pending = Vec::new();
        for (owner, _, fact) in typed_trees::references::program_scoped_proof_facts(program) {
            if !domain.contains(owner) {
                continue;
            }
            match fact {
                ProofFact::Expression(expression) => pending.push((owner, *expression)),
                ProofFact::Membership(membership) => pending.push((owner, membership.value)),
                ProofFact::Proposition(application) => pending.extend(
                    expression_table(program)
                        .expression_handles(application.arguments)
                        .iter()
                        .map(|expression| (owner, *expression)),
                ),
            }
        }
        let mut visited = std::collections::HashSet::new();
        let mut candidates = Vec::new();
        while let Some((owner, expression)) = pending.pop() {
            if !program.expression_table.expression_is_valid(expression)
                || !visited.insert((owner, expression))
            {
                continue;
            }
            pending.extend(
                expression_children(program, expression)
                    .into_iter()
                    .map(|child| (owner, child)),
            );
            if !closed(program, expression, &mut Vec::new()) {
                continue;
            }
            let has_recipe = program
                .constant_recipe_for_contract_expression(expression)
                .is_valid();
            let children = subtree(program, expression);
            let has_observation = children.iter().any(|child| {
                matches!(program.expression_table.expression(*child),
                ExpressionNode::Call(call) if call.layout_query.is_some())
            });
            let invocations = children
                .into_iter()
                .filter_map(|call_expression| {
                    let (machine, static_application) =
                        super::range_endpoints::selected_endpoint_machine(
                            program,
                            call_expression,
                        )?;
                    Some(super::range_endpoints::PendingEndpoint {
                        expression: call_expression,
                        root: expression,
                        machine: machine.symbol,
                        static_application,
                        contract_expression: true,
                    })
                })
                .collect::<Vec<_>>();
            if !has_recipe && !has_observation && invocations.is_empty() {
                continue;
            }
            let Some(result_type) = validation::parameter_expression_result_type_reference(
                program,
                owner,
                &[],
                expression,
            ) else {
                continue;
            };
            if !program
                .primitive_type_reference(result_type)
                .is_some_and(|primitive| primitive.is_integer())
            {
                continue;
            }
            candidates.push((
                expression,
                ConstantExpressionContext::new(owner, result_type),
                has_recipe || has_observation,
                invocations,
            ));
        }
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let mut admission_and_selections = None;
        let mut roots = Vec::new();
        for (expression, context, has_recipe, invocations) in candidates {
            let keep = if has_recipe {
                true
            } else {
                let (admission, selections) = admission_and_selections
                    .get_or_insert_with(|| {
                        crate::BuildTimeAdmissionPlan::infer(program, None).map(|admission| {
                            (
                                admission,
                                validation::derive_pre_flow_operator_selections(program),
                            )
                        })
                    })
                    .as_ref()
                    .map_err(Clone::clone)?;
                invocations.iter().any(|call| {
                    admission.closure_needs_execution_preparation(program, call.machine, selections)
                })
            };
            if keep {
                roots.push(ContractRoot {
                    expression,
                    context,
                    invocations,
                });
            }
        }
        let nested = roots
            .iter()
            .flat_map(|root| {
                subtree(program, root.expression)
                    .into_iter()
                    .filter(move |expression| *expression != root.expression)
                    .map(move |expression| (root.context.owner, expression))
            })
            .collect::<std::collections::HashSet<_>>();
        roots.retain(|root| !nested.contains(&(root.context.owner, root.expression)));
        roots.sort_by_key(|root| {
            (
                root.expression.arena_index(),
                root.expression.generation(),
                root.context.owner.arena_index(),
                root.context.owner.generation(),
            )
        });
        Ok(roots)
    }

    fn subtree(program: &TypedTrees, expression: ExpressionHandle) -> Vec<ExpressionHandle> {
        let mut pending = vec![expression];
        let mut result = Vec::new();
        while let Some(expression) = pending.pop() {
            if !program.expression_table.expression_is_valid(expression)
                || result.contains(&expression)
            {
                continue;
            }
            result.push(expression);
            pending.extend(expression_children(program, expression));
        }
        result
    }

    fn closed(
        program: &TypedTrees,
        expression: ExpressionHandle,
        active: &mut Vec<ExpressionHandle>,
    ) -> bool {
        if !program.expression_table.expression_is_valid(expression) || active.contains(&expression)
        {
            return false;
        }
        active.push(expression);
        let result = match program.expression_table.expression(expression) {
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
            ExpressionNode::Name(name) => program
                .const_declaration_by_symbol(name.symbol)
                .is_some_and(|declaration| {
                    closed(program, declaration.materialized_initializer, active)
                }),
            ExpressionNode::Call(_)
            | ExpressionNode::Binary(_)
            | ExpressionNode::Unary(_)
            | ExpressionNode::Cast(_) => expression_children(program, expression)
                .into_iter()
                .all(|child| closed(program, child, active)),
            _ => false,
        };
        active.pop();
        result
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn typed(source: &str) -> TypedTrees {
            let tokens = source_files_to_tokens::Lexer::new(source)
                .tokenize()
                .expect("lex");
            let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
            let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
            )
            .expect("resolve");
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type")
        }

        #[test]
        fn constant_entries_do_not_revive_excluded_contract_owners() {
            let mut program = typed(
                "data Cell { code: u64; tag: u64; }
             machine extent() -> u64 { size_of<Cell>() }
             machine unrelated() requires size_of<Cell>() + align_of<Cell>() == 24 {}",
            );
            let extent = program
                .realized_machine_named("extent")
                .expect("extent")
                .symbol;
            super::super::constant_recipes::retain_deferred_constant_recipes(
                &mut program,
                &EvaluationDomain::all(),
            )
            .expect("retain contract jobs");
            assert!(!program.constant_recipes.is_empty());
            let projected = EvaluationDomain::all()
                .demanded_roots(&program, &[extent])
                .expect("runtime callable preflight")
                .execution_program(&program);
            assert!(projected.realized_machine_named("unrelated").is_none());
            assert!(
                !projected.constant_recipes.is_empty(),
                "projection preserves backing arenas"
            );
            let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
                projected,
                &typed_trees_to_typed_trees::PreparationRequest::preliminary(),
            )
            .expect("prepare the exact execution projection");
            for (_, recipe) in prepared.constant_recipes.iter() {
                for context in prepared
                    .constant_expression_contexts
                    .span_or_empty(recipe.contexts)
                {
                    assert!(
                        !context.evaluation_entry.is_valid(),
                        "excluded scope must not gain an executable entry"
                    );
                }
            }
        }

        #[test]
        fn constant_entries_keep_callee_recipe_scope_separate_from_caller() {
            let mut program = typed(
                "data Cell { code: u64; tag: u64; }
             machine require_size(value: u64)
             requires value >= size_of<Cell>() + align_of<Cell>() {}
             machine program() { require_size(24); }",
            );
            let callee = program
                .realized_machine_named("require_size")
                .expect("callee")
                .symbol;
            let caller = program
                .realized_machine_named("program")
                .expect("caller")
                .symbol;
            super::super::constant_recipes::retain_deferred_constant_recipes(
                &mut program,
                &EvaluationDomain::all(),
            )
            .expect("retain requirement root");
            let (prepared, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
                program,
                &typed_trees_to_typed_trees::PreparationRequest::preliminary(),
            )
            .expect("prepare entries");
            let checked = typed_trees_to_checked_trees::lower_typed_trees(
                prepared,
                preparation,
                &typed_trees_to_checked_trees::CheckingRequest::new(),
            )
            .expect("retain conditional call requirement");
            let obligations = checked
                .facts
                .constant_recipe_obligations
                .iter()
                .filter(|obligation| obligation.owner() == caller)
                .collect::<Vec<_>>();
            assert!(
                !obligations.is_empty(),
                "caller must retain the deferred requirement"
            );
            for obligation in obligations {
                assert_eq!(obligation.recipe_owner(), callee);
            }
        }

        #[test]
        fn constant_entries_leave_open_requirement_signatures_descriptive() {
            let mut program =
                typed("trait Geometry { machine check<T>() requires size_of<T>() >= 8; }");
            super::super::constant_recipes::retain_deferred_constant_recipes(
                &mut program,
                &EvaluationDomain::all(),
            )
            .expect("retain generic requirement root");
            assert!(!program.constant_recipes.is_empty());
            let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
                program,
                &typed_trees_to_typed_trees::PreparationRequest::preliminary(),
            )
            .expect("prepare descriptions without executing open signatures");
            for (_, recipe) in prepared.constant_recipes.iter() {
                for context in prepared
                    .constant_expression_contexts
                    .span_or_empty(recipe.contexts)
                {
                    assert!(!context.evaluation_entry.is_valid());
                }
            }
        }

        #[test]
        fn constant_entries_are_checked_per_specialized_scope() {
            let mut program = typed(
                "data Cell { code: u64; tag: u64; }
             machine require_geometry<T>()
             requires size_of<Cell>() + align_of<Cell>() == 24 {}
             machine program() {
                 require_geometry<u8>();
                 require_geometry<u64>();
             }",
            );
            let template = program
                .realized_machine_named("require_geometry")
                .expect("template")
                .symbol;
            super::super::constant_recipes::retain_deferred_constant_recipes(
                &mut program,
                &EvaluationDomain::all(),
            )
            .expect("retain complete roots");
            let (prepared, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
                program,
                &typed_trees_to_typed_trees::PreparationRequest::preliminary(),
            )
            .expect("prepare complete entries");
            let contexts = prepared
                .constant_recipes
                .iter()
                .flat_map(|(_, recipe)| {
                    prepared
                        .constant_expression_contexts
                        .span_or_empty(recipe.contexts)
                })
                .copied()
                .collect::<Vec<_>>();
            assert!(
                contexts
                    .iter()
                    .filter(|context| context.owner == template)
                    .all(|context| context.evaluation_entry.is_valid()),
                "the closed template contract owns an entry of its own"
            );
            let entries = contexts
                .iter()
                .filter(|context| context.evaluation_entry.is_valid())
                .collect::<Vec<_>>();
            assert_eq!(
                entries.len(),
                3,
                "the closed template and both concrete applications own complete entries"
            );
            for (position, left) in entries.iter().enumerate() {
                for right in &entries[position + 1..] {
                    assert_ne!(left.evaluation_entry, right.evaluation_entry);
                    assert_ne!(left.evaluation_expression, right.evaluation_expression);
                }
            }
            let checked = typed_trees_to_checked_trees::lower_typed_trees(
                prepared,
                preparation,
                &typed_trees_to_checked_trees::CheckingRequest::new(),
            )
            .expect("ordinary checking must accept the conditional entries");
            for context in entries {
                let entry = checked
                    .typed
                    .constant_evaluation_entry(context.evaluation_entry)
                    .expect("checked entry retains exact source ownership");
                assert_eq!(entry.owner, context.owner);
                assert_eq!(entry.evaluation_expression, context.evaluation_expression);
                assert_ne!(entry.source_expression, entry.evaluation_expression);
            }
            assert!(
                !checked.facts.integer_range_obligations.is_empty(),
                "checking must retain addition formation, not assume it succeeded"
            );
        }

        #[test]
        fn constant_expression_contexts_keep_shared_contract_occurrences() {
            let mut program = typed(
                "data Cell { code: u64; }
             machine extent() -> u64 { size_of<Cell>() }
             machine first() requires extent() >= 8 {}
             machine second() {}",
            );
            let first = program
                .realized_machine_named("first")
                .expect("first")
                .clone();
            let second = program
                .realized_machine_named("second")
                .expect("second")
                .symbol;
            program.tables.machines.for_each_mut(|_, machine| {
                if machine.symbol == second {
                    machine.contracts = first.contracts;
                }
            });
            let roots = roots(&program, &EvaluationDomain::all());
            assert_eq!(
                roots.len(),
                2,
                "one shared expression still has two lexical occurrences"
            );
            assert_eq!(roots[0].expression, roots[1].expression);
            assert_ne!(roots[0].context.owner, roots[1].context.owner);
            for root in roots {
                assert!(root.context.owner == first.symbol || root.context.owner == second);
                assert_eq!(
                    program.primitive_type_reference(root.context.result_type),
                    Some(typed_trees::types::PrimitiveType::U64),
                );
            }
            let domain = EvaluationDomain::all();
            let plan = super::super::range_endpoints::integer_root_plan(&program, &domain)
                .expect("shared root plan");
            assert_eq!(
                plan.calls.len(),
                1,
                "shared storage has one invocation occurrence"
            );
            super::super::constant_recipes::retain_deferred_constant_recipes(&mut program, &domain)
                .expect("retain complete roots");
            assert_eq!(program.constant_recipes.len(), 1);
            let (_, recipe) = program.constant_recipes.iter().next().expect("recipe");
            assert_eq!(
                expression_table(&program)
                    .expression_handles(recipe.invocations)
                    .len(),
                1
            );
            assert_eq!(
                program
                    .constant_expression_contexts
                    .span_or_empty(recipe.contexts)
                    .len(),
                2
            );
            let context_count = program.constant_expression_contexts.len();
            super::super::constant_recipes::retain_deferred_constant_recipes(&mut program, &domain)
                .expect("retain roots again");
            assert_eq!(program.constant_expression_contexts.len(), context_count);
        }

        #[test]
        fn constant_expression_contexts_keep_a_root_nested_only_in_another_owner() {
            let mut program = typed(
                "data Cell { code: u64; }
             machine first() requires size_of<Cell>() + 0 >= 8 {}
             machine second() requires size_of<Cell>() >= 8 {}",
            );
            let first = program
                .realized_machine_named("first")
                .expect("first")
                .symbol;
            let second = program
                .realized_machine_named("second")
                .expect("second")
                .symbol;
            let domain = EvaluationDomain::all();
            let initial = roots(&program, &domain);
            let outer = initial
                .iter()
                .find(|root| root.context.owner == first)
                .expect("arithmetic root")
                .expression;
            let ExpressionNode::Binary(arithmetic) = program.expression_table.expression(outer)
            else {
                panic!("the complete root must include the addition");
            };
            let shared = arithmetic.left;
            let predicate = typed_trees::references::program_scoped_proof_facts(&program)
                .find_map(|(owner, _, fact)| match fact {
                    ProofFact::Expression(expression) if owner == second => Some(*expression),
                    _ => None,
                })
                .expect("second predicate");
            let ExpressionNode::Binary(comparison) =
                program.expression_table.expression_mut(predicate)
            else {
                panic!("comparison predicate");
            };
            comparison.left = shared;
            let roots = roots(&program, &domain);
            assert_eq!(roots.len(), 2);
            assert!(
                roots
                    .iter()
                    .any(|root| root.context.owner == first && root.expression == outer)
            );
            assert!(
                roots
                    .iter()
                    .any(|root| root.context.owner == second && root.expression == shared)
            );
        }
    }
}
pub(crate) mod data_layout_queries {
    use crate::build_time_evaluation::{expression_table, type_reference_table};
    use diagnostics::Diagnostic;
    use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};
    use typed_trees::TypedTrees;
    use typed_trees::expression::{DataLayoutQueryRequest, ExpressionNode};
    use typed_trees::references::DeclarationSelection;
    use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn has_pending_target_observations(
        typed: &TypedTrees,
        selection: &DeclarationSelection,
    ) -> bool {
        typed_trees::references::collect_selected_program_references(typed, selection)
            .expressions
            .into_iter()
            .any(
                |expression| match typed.expression_table.expression(expression) {
                    ExpressionNode::Call(call) => call.layout_query.is_some(),
                    _ => false,
                },
            )
            || has_pending_recast_geometry(typed, selection)
            || typed.const_declarations().iter().any(|declaration| {
                selection.contains(declaration.symbol)
                    && declaration.pending_target_observation
                    && declaration.evaluated_value.is_none()
            })
    }

    fn has_pending_recast_geometry(typed: &TypedTrees, selection: &DeclarationSelection) -> bool {
        let casts = typed_trees::references::collect_selected_program_references(typed, selection)
            .expressions
            .into_iter()
            .filter_map(
                |expression| match typed.expression_table.expression(expression) {
                    ExpressionNode::Cast(cast) if cast.form.recast_access().is_some() => {
                        Some((expression, cast))
                    }
                    _ => None,
                },
            )
            .collect::<Vec<_>>();
        if casts.is_empty() {
            return false;
        }
        let owners = typed_trees::references::collect_program_operand_owners(typed);
        casts.into_iter().any(|(expression, cast)| {
            if validation::pending_layout_geometry(typed, [cast.target_type]) {
                return true;
            }
            owners
                .expression_owners(expression)
                .filter(|owner| selection.contains(*owner))
                .any(|owner| {
                    let Some(machine) = typed
                        .machine_by_symbol(owner)
                        .or_else(|| typed.machine_holding_state(owner))
                    else {
                        return false;
                    };
                    typed.machine_states(machine).iter().any(|state| {
                        validation::declared_place_type_raw(typed, machine, Some(state), cast.value)
                            .is_some_and(|source| {
                                validation::pending_layout_geometry(typed, [source])
                            })
                    })
                })
        })
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct LayoutApplicationTargetInput {
        pub application_identity: String,
        pub target: layout_plans::LayoutTargetFacts,
        pub lexical_owner: symbols::SymbolHandle,
    }

    pub trait LayoutQueryProvider {
        type Measurements: LayoutQueryMeasurements;

        fn prepare(&self, program: &TypedTrees) -> Self::Measurements;
        fn layout_target_inputs(&self) -> Vec<layout_plans::LayoutTargetFacts> {
            Vec::new()
        }
        fn layout_application_inputs(&self) -> Option<&[LayoutApplicationTargetInput]> {
            None
        }
    }

    pub trait LayoutQueryMeasurements {
        fn measure(
            &self,
            program: &TypedTrees,
            query: &DataLayoutQueryRequest,
        ) -> Result<Option<usize>, Diagnostic>;
    }

    pub(crate) fn fold_data_layout_queries(
        program: &mut TypedTrees,
        defer_uninstalled_plans: bool,
        selection: &DeclarationSelection,
        provider: &impl LayoutQueryProvider,
    ) -> Result<(), Vec<Diagnostic>> {
        let proof_only = checked_trees::typed_trees::proof_only::classify(program);
        let owned_expressions =
            checked_trees::typed_trees::references::collect_selected_program_references(
                program, selection,
            )
            .expressions;
        let queries: Vec<(
            checked_trees::typed_trees::expression::ExpressionHandle,
            checked_trees::typed_trees::expression::DataLayoutQueryRequest,
            source::SourceSpan,
        )> = expression_table(program)
            .iter_expressions()
            .filter(|(handle, _)| owned_expressions.contains(handle))
            .filter_map(|(handle, node)| match node {
                ExpressionNode::Call(call) => call.layout_query.as_ref().map(|query| {
                    (
                        handle,
                        query.clone(),
                        program.expression_table.source_span(handle),
                    )
                }),
                _ => None,
            })
            .collect();
        let measurements = provider.prepare(program);
        for (handle, query, reference) in queries {
            let operand = query.operand.type_reference;
            if defer_uninstalled_plans
                && program.layout_storage_arguments(operand).is_some()
                && program.plan_laid_layout_for_type(operand).is_none()
            {
                continue;
            }
            let spelling = query.kind.target_name();
            if let Some(held) = proof_only.proof_only_mention(program, operand) {
                return Err(vec![
                Diagnostic::error(format!(
                    "`{spelling}` cannot measure a proof-only carrier: `{held}` has no runtime layout"
                ))
                .with_source_span(reference),
            ]);
            }
            if !defer_uninstalled_plans {
                reject_unresolved_layout_operands(program, operand).map_err(|reason| {
                    vec![Diagnostic::error(reason).with_source_span(reference)]
                })?;
            }
            let extent = measurements
                .measure(program, &query)
                .map_err(|error| vec![error])?;
            let Some(extent) = extent else {
                if defer_uninstalled_plans {
                    continue;
                }
                return Err(vec![
                    Diagnostic::error(format!(
                        "`{spelling}` cannot measure a carrier with an unresolved array extent"
                    ))
                    .with_source_span(reference),
                ]);
            };
            let value = u64::try_from(extent).map_err(|_| {
                vec![
                    Diagnostic::error(format!(
                        "`{spelling}` layout extent exceeds the `u64` result carrier"
                    ))
                    .with_source_span(reference),
                ]
            })?;
            let literal =
                IntegerLiteral::from_parts(false, IntegerRadix::Decimal, &value.to_string())
                    .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(reference)])?
                    .with_landing(IntegerLanding {
                        landed_type: LandedIntegerType::U64,
                        domain: numerics::arithmetic::ArithmeticDomain::Exact,
                    });
            *program.expression_table.expression_mut(handle) = ExpressionNode::Integer(literal);
        }
        Ok(())
    }

    fn reject_unresolved_layout_operands(
        typed: &checked_trees::typed_trees::TypedTrees,
        root: TypeReferenceHandle,
    ) -> Result<(), String> {
        let mut pending = vec![root];
        let mut visited = Vec::new();
        while let Some(reference) = pending.pop() {
            if !reference.is_valid() || visited.contains(&reference) {
                continue;
            }
            visited.push(reference);
            if let Some(storage) = typed.layout_storage_arguments(reference) {
                pending.push(storage.semantic_type);
                continue;
            }
            match typed.type_reference_table.type_reference(reference) {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    match length {
                    FixedArrayLength::Literal(_) => {}
                    FixedArrayLength::Expression(_) => return Err("`size_of`/`align_of` cannot measure an array with an unresolved extent expression".into()),
                    FixedArrayLength::ConstParameter { name, .. } => {
                        return Err(format!(
                            "`size_of`/`align_of` cannot measure a fixed array: \
                             the length `{name}` is not resolved to a constant"
                        ));
                    }
                    FixedArrayLength::ConstCall { .. }
                        if typed.closed_fixed_array_length(reference).is_some() => {}
                    FixedArrayLength::ConstCall { name, .. } => {
                        return Err(format!(
                            "`size_of`/`align_of` cannot measure a fixed array whose \
                             length `{name}` never evaluated to a constant"
                        ));
                    }
                }
                    pending.push(*element_type);
                }
                TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
                TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
                TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
                TypeReferenceNode::Generic { arguments, .. } => {
                    pending.extend(
                        type_reference_table(typed)
                            .type_reference_handles(*arguments)
                            .iter()
                            .copied(),
                    );
                }
                TypeReferenceNode::ConstExpression(_) => {
                    return Err(
                        "`size_of`/`align_of` takes a data carrier, not a proof-static \
                     const expression"
                            .to_owned(),
                    );
                }
                TypeReferenceNode::Named { .. }
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Unit => {}
            }
        }
        Ok(())
    }
}
pub(crate) mod initializer_probe {
    use source::SourceSpan;
    use typed_trees::TypedTrees;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::state::State;
    use typed_trees::statement::{
        StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
    };

    pub(crate) struct ValueProbeBody {
        pub(crate) reference: SourceSpan,
        pub(crate) roots: Vec<ExpressionHandle>,
        pub(crate) value: ExpressionHandle,
    }

    pub(crate) fn value_probe_body(
        program: &TypedTrees,
        state: &State,
    ) -> Result<ValueProbeBody, String> {
        let Some((last, captures)) = program
            .statement_table
            .statements(state.statement_nodes)
            .split_last()
        else {
            return Err("initializer probe lost its value exit".into());
        };
        let StatementNode::Transition(transition) = last else {
            return Err("initializer probe lost its value exit".into());
        };
        if !matches!(transition.guard, TransitionGuardNode::Always)
            || !matches!(transition.exit, TransitionExit::Ordinary)
            || transition.continuation.is_valid()
            || !transition.proof_selectors.is_empty()
        {
            return Err("initializer probe changed its unconditional value exit".into());
        }
        let TransitionTargetNode::Value(value) =
            program.statement_table.transition_target(transition.target)
        else {
            return Err("initializer probe does not return its value".into());
        };
        if !value.is_valid() {
            return Err("initializer probe lost its value expression".into());
        }
        let mut roots = Vec::new();
        for capture in captures {
            let StatementNode::LocalData(local) = capture else {
                return Err("initializer probe has an operation outside its value captures".into());
            };
            if local.is_mutable || local.ordinary_value_initializer().is_none() {
                return Err("initializer probe has an incomplete or mutable value capture".into());
            }
            roots.push(local.initial_value);
        }
        roots.push(*value);
        Ok(ValueProbeBody {
            reference: transition.source_span,
            roots,
            value: *value,
        })
    }
}
pub(crate) mod range_endpoints {
    use crate::build_time_evaluation::EvaluationDomain;
    use crate::const_evaluation::const_generic_expressions::value::evaluate_integer_endpoint;
    use crate::machine_execution::admission::require_closed_expression_custody;
    use crate::unique::extend_unique;
    use crate::{BuildTimeAdmissionPlan, SelectedBuildTimeOperators};
    use diagnostics::Diagnostic;
    use numerics::literals::LandedIntegerType;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::state::state_by_symbol;
    use typed_trees::types::PrimitiveType;

    pub(crate) mod arguments {
        use super::integer_type::ScalarPosition;
        use crate::build_time_evaluation::expression_table;
        use crate::const_evaluation::const_generic_expressions::value::{self, ConstantCalls};
        use crate::const_evaluation::const_lengths::decode_integer_result;
        use crate::machine_execution::admission::{
            SnapshotArgument, require_call_expression_selection,
        };
        use crate::unique::extend_unique;
        use crate::{BuildTimeAdmissionPlan, BuildTimeSelectionAuthority, BuildTimeValue};
        use diagnostics::Diagnostic;
        use language_semantics::const_value::{
            CanonicalConstIdentity, CanonicalConstValue, DecodedCanonicalConstValue,
        };
        use typed_trees::{
            TypedTrees,
            expression::{ExpressionHandle, ExpressionNode},
            machine::Machine,
            state::State,
            types::{PrimitiveType, TypeReferenceHandle},
        };

        pub(crate) struct Invocation<'program> {
            pub working: &'program TypedTrees,
            pub execution: &'program TypedTrees,
            pub admission: &'program BuildTimeAdmissionPlan,
            pub authority: Option<&'program dyn BuildTimeSelectionAuthority>,
        }

        impl Invocation<'_> {
            fn callee(
                &self,
                expression: ExpressionHandle,
            ) -> Result<super::EndpointCallee, String> {
                let (machine, static_application) =
                    super::selected_endpoint_machine(self.working, expression)
                        .ok_or("range endpoint call needs an exact closed selected machine")?;
                let endpoint = super::PendingEndpoint {
                    expression,
                    root: expression,
                    machine: machine.symbol,
                    static_application,
                    contract_expression: false,
                };
                super::resolve_endpoint_callee(self.execution, &endpoint)
            }

            fn machine(&self, callee: super::EndpointCallee) -> Result<&Machine, String> {
                self.execution
                    .machine(callee.instance)
                    .ok_or_else(|| "range endpoint lost its selected machine".to_string())
            }

            fn entry(&self, machine: &Machine) -> Result<&State, String> {
                self.execution
                    .machine_states(machine)
                    .first()
                    .ok_or_else(|| "range endpoint lost entry".to_string())
            }

            fn position(
                &self,
                callee: super::EndpointCallee,
                reference: TypeReferenceHandle,
            ) -> Result<ScalarPosition, String> {
                let types = if callee.static_application {
                    self.execution
                } else {
                    self.working
                };
                ScalarPosition::prepare(types, reference)
            }

            fn result_position(
                &self,
                callee: super::EndpointCallee,
            ) -> Result<ScalarPosition, String> {
                let entry = self.entry(self.machine(callee)?)?;
                self.position(callee, entry.return_type)
            }

            fn arguments(
                &self,
                expression: ExpressionHandle,
                callee: super::EndpointCallee,
            ) -> Result<(Vec<BuildTimeValue>, Vec<ExpressionNode>, Vec<Diagnostic>), String>
            {
                let Some(call) = self.execution.expression_table.call_node(expression) else {
                    return Err("range endpoint lost its call".into());
                };
                let entry = self.entry(self.machine(callee)?)?;
                let arguments = expression_table(self.execution).expression_handles(call.arguments);
                let parameters = self.execution.state_parameters(entry);
                if arguments.len() != parameters.len() {
                    return Err(
                        "range endpoint call argument count does not match its selected entry"
                            .into(),
                    );
                }
                let mut values = Vec::new();
                let mut literals = Vec::new();
                let mut warnings = Vec::new();
                for (argument, parameter) in arguments.iter().zip(parameters) {
                    let position = self.position(callee, parameter.type_reference)?;
                    let (value, new_warnings) = value::evaluate_closed_scalar_with_policy(
                        self.execution,
                        *argument,
                        primitive(&position),
                        policy(&position),
                        self,
                    )?;
                    extend_unique(&mut warnings, new_warnings);
                    match (position, value.decode_encoding()) {
                        (
                            ScalarPosition::Boolean,
                            Some(DecodedCanonicalConstValue::Boolean(value)),
                        ) => {
                            literals.push(ExpressionNode::Boolean(value));
                            values.push(BuildTimeValue::Bool(value))
                        }
                        (
                            ScalarPosition::Integer(position),
                            Some(DecodedCanonicalConstValue::Integer { value, .. }),
                        ) => {
                            position.require_value(
                                self.execution,
                                self.admission,
                                &numerics::bignum::BigInt::from_i128(value),
                            )?;
                            literals.push(ExpressionNode::Integer(
                                numerics::literals::IntegerLiteral::from_parts(
                                    value < 0,
                                    numerics::literals::IntegerRadix::Decimal,
                                    &value.unsigned_abs().to_string(),
                                )
                                .map_err(|_| "range endpoint argument lost its literal spelling")?
                                .with_landing(
                                    numerics::literals::IntegerLanding {
                                        landed_type: super::integer_landing(position.primitive)?,
                                        domain: numerics::arithmetic::ArithmeticDomain::Exact,
                                    },
                                ),
                            ));
                            let bits = if position.primitive.is_signed_integer() {
                                i64::try_from(value).map_err(
                            |_| "range endpoint argument exceeds signed interpreter storage",
                        )?
                            } else {
                                u64::try_from(value).map_err(
                            |_| "range endpoint argument exceeds unsigned interpreter storage",
                        )? as i64
                            };
                            values.push(BuildTimeValue::Int(bits));
                        }
                        _ => {
                            return Err(
                                "range endpoint argument differs from its selected carrier".into(),
                            );
                        }
                    }
                }
                Ok((values, literals, warnings))
            }

            fn snapshot_requires_discharge(
                &self,
                expression: ExpressionHandle,
                machine: &typed_trees::machine::Machine,
                literals: &[ExpressionNode],
            ) -> Result<bool, String> {
                let Some(call) = self.execution.expression_table.call_node(expression) else {
                    return Err("range endpoint lost its call".into());
                };
                let argument_handles =
                    expression_table(self.execution).expression_handles(call.arguments);
                if argument_handles.len() != literals.len() {
                    return Err(
                        "range endpoint call argument count does not match its evaluated snapshots"
                            .into(),
                    );
                }
                let entry = self.entry(machine)?;
                let snapshots = argument_handles
                    .iter()
                    .zip(literals)
                    .map(|(argument, literal)| SnapshotArgument {
                        original: *argument,
                        literal: literal.clone(),
                    })
                    .collect();
                let retained: Vec<symbols::SymbolHandle> = self
                    .execution
                    .machines()
                    .iter()
                    .map(|machine| machine.symbol)
                    .collect();
                crate::machine_execution::admission::snapshot_arguments_discharge(
                    self.admission,
                    self.execution,
                    &retained,
                    machine.symbol,
                    "@const-endpoint",
                    &typed_trees_to_typed_trees::PreparationRequest::preliminary(),
                    &typed_trees_to_checked_trees::CheckingRequest::new(),
                    expression,
                    machine.name.as_str(),
                    entry.return_type,
                    snapshots,
                )?;
                Ok(true)
            }
        }

        impl ConstantCalls for Invocation<'_> {
            fn result_type(&self, expression: ExpressionHandle) -> TypeReferenceHandle {
                self.callee(expression)
                    .ok()
                    .and_then(|callee| self.execution.machine(callee.instance))
                    .and_then(|machine| self.execution.machine_states(machine).first())
                    .map_or(TypeReferenceHandle::invalid(), |entry| entry.return_type)
            }

            fn validate_call(
                &self,
                expression: ExpressionHandle,
            ) -> Result<(PrimitiveType, Vec<Diagnostic>), String> {
                require_call_expression_selection(self.execution, expression, self.authority)?;
                let callee = self.callee(expression)?;
                let machine = self.machine(callee)?;
                let entry = self.entry(machine)?;
                let custody = crate::BuildTimeInvocationCustody::Source(
                    self.execution.expression_table.source_span(expression),
                );
                let discharged = if !self
                    .admission
                    .closure_includes_authored_requires(self.execution, machine)
                {
                    false
                } else if self
                    .admission
                    .closure_requires_are_entry_parameter_domains(self.execution, machine)
                {
                    true
                } else {
                    let (_, literals, _) = self.arguments(expression, callee)?;
                    self.snapshot_requires_discharge(expression, machine, &literals)?
                };
                if discharged {
                    self.admission
                        .require_common_floor_for_concrete_premise_invocation(
                            self.execution,
                            machine,
                            custody,
                        )?;
                } else {
                    self.admission.require_common_floor_for_invocation(
                        self.execution,
                        machine,
                        custody,
                    )?;
                }
                let Some(call) = self.execution.expression_table.call_node(expression) else {
                    return Err("range endpoint lost its call".into());
                };
                let arguments = expression_table(self.execution).expression_handles(call.arguments);
                let parameters = self.execution.state_parameters(entry);
                if arguments.len() != parameters.len() {
                    return Err(
                        "range endpoint call argument count does not match its selected entry"
                            .into(),
                    );
                }
                let mut warnings = Vec::new();
                for (argument, parameter) in arguments.iter().zip(parameters) {
                    let position = self.position(callee, parameter.type_reference)?;
                    extend_unique(
                        &mut warnings,
                        value::validate_closed_scalar_with_policy(
                            self.execution,
                            *argument,
                            primitive(&position),
                            policy(&position),
                            self,
                        )?,
                    );
                }
                Ok((primitive(&self.result_position(callee)?), warnings))
            }

            fn evaluate_call(
                &self,
                expression: ExpressionHandle,
            ) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
                let callee = self.callee(expression)?;
                let (arguments, literals, warnings) = self.arguments(expression, callee)?;
                let machine = self.machine(callee)?;
                let custody = crate::BuildTimeInvocationCustody::Source(
                    self.execution.expression_table.source_span(expression),
                );
                let discharged = if !self
                    .admission
                    .closure_includes_authored_requires(self.execution, machine)
                {
                    false
                } else if self
                    .admission
                    .closure_requires_are_entry_parameter_domains(self.execution, machine)
                {
                    true
                } else {
                    self.snapshot_requires_discharge(expression, machine, &literals)?
                };
                let result = if discharged {
                    self.admission
                        .evaluate_const_evaluable_machine_symbol_for_concrete_premise_invocation(
                            self.execution,
                            callee.instance,
                            arguments,
                            custody,
                        )?
                } else {
                    self.admission
                        .evaluate_const_evaluable_machine_symbol_for_invocation(
                            self.execution,
                            callee.instance,
                            arguments,
                            custody,
                        )?
                };
                let value = match (self.result_position(callee)?, result) {
                    (ScalarPosition::Boolean, BuildTimeValue::Bool(value)) => {
                        CanonicalConstValue::boolean(value)
                    }
                    (ScalarPosition::Integer(position), result) => {
                        let result = decode_integer_result(self.execution, machine, result)?;
                        position.require_value(self.execution, self.admission, &result)?;
                        let value = if position.primitive.is_signed_integer() {
                            i128::from(
                                result
                                    .to_i64()
                                    .ok_or("range endpoint result exceeds signed storage")?,
                            )
                        } else {
                            i128::from(
                                result
                                    .to_u64()
                                    .ok_or("range endpoint result exceeds unsigned storage")?,
                            )
                        };
                        let identity =
                            CanonicalConstIdentity::integer(position.primitive.name(), value);
                        CanonicalConstValue::new(
                            identity.type_name,
                            identity.encoding,
                            value.to_string(),
                        )
                    }
                    _ => {
                        return Err(
                            "range endpoint result differs from its declared scalar carrier".into(),
                        );
                    }
                };
                Ok((value, warnings))
            }
        }

        fn primitive(position: &ScalarPosition) -> PrimitiveType {
            match position {
                ScalarPosition::Boolean => PrimitiveType::Bool,
                ScalarPosition::Integer(position) => position.primitive,
            }
        }

        fn policy(position: &ScalarPosition) -> numerics::arithmetic::ArithmeticDomain {
            match position {
                ScalarPosition::Boolean => numerics::arithmetic::ArithmeticDomain::Exact,
                ScalarPosition::Integer(position) => position.policy,
            }
        }
    }
    mod integer_type {
        use numerics::{arithmetic::ArithmeticDomain, bignum::BigInt};
        use typed_trees::{
            TypedTrees,
            types::{
                DomainConstraintSubject, PrimitiveType, TypeConstraintNode, TypeReferenceHandle,
                TypeReferenceNode,
            },
        };

        use crate::BuildTimeAdmissionPlan;
        use crate::build_time_evaluation::type_reference_table;
        use crate::const_evaluation::const_domain_facts::evaluate_closed_membership;
        use crate::const_evaluation::const_generic_expressions::exact_probe_destination;

        pub(crate) enum ScalarPosition {
            Integer(IntegerPosition),
            Boolean,
        }

        impl ScalarPosition {
            pub(crate) fn prepare(
                program: &TypedTrees,
                reference: TypeReferenceHandle,
            ) -> Result<Self, String> {
                if type_reference_table(program).contains_type_reference(reference)
                    && exact_probe_destination(program, reference) == Some(PrimitiveType::Bool)
                {
                    return Ok(Self::Boolean);
                }
                IntegerPosition::prepare(program, reference).map(Self::Integer)
            }
        }

        pub(crate) struct IntegerPosition {
            pub(crate) primitive: PrimitiveType,
            pub(crate) policy: ArithmeticDomain,
            domains: Vec<(symbols::SymbolHandle, String)>,
        }

        impl IntegerPosition {
            pub(crate) fn prepare(
                program: &TypedTrees,
                mut reference: TypeReferenceHandle,
            ) -> Result<Self, String> {
                let mut policy = None;
                let mut domains = Vec::new();
                let mut visited = Vec::new();
                loop {
                    if !type_reference_table(program).contains_type_reference(reference)
                        || visited.contains(&reference)
                    {
                        return Err("invalid or cyclic range endpoint integer type".to_owned());
                    }
                    visited.push(reference);
                    let TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } = program.type_reference_table.type_reference(reference)
                    else {
                        break;
                    };
                    let rows = program.type_reference_table.constraints(*constraints);
                    if rows.is_empty() || rows.len() != constraints.count() as usize {
                        return Err(
                            "range endpoint integer type has invalid constraints".to_owned()
                        );
                    }
                    for constraint in rows {
                        match constraint {
                            TypeConstraintNode::Domain(domain)
                                if domain.subject == DomainConstraintSubject::Declared
                                    && domain.symbol.is_valid()
                                    && domain.arguments.is_empty() =>
                            {
                                domains.push((domain.symbol, domain.name.as_str().to_owned()));
                            }
                            TypeConstraintNode::ArithmeticDomain(domain)
                                if matches!(
                                    domain,
                                    ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating
                                ) =>
                            {
                                if policy.replace(*domain).is_some() {
                                    return Err(
                                        "range endpoint position has multiple arithmetic policies"
                                            .into(),
                                    );
                                }
                            }
                            _ => {
                                return Err(
                            "range endpoint integer types admit only range refinements and declared integer domains"
                                .to_owned(),
                        );
                            }
                        }
                    }
                    reference = *base_type;
                }
                let primitive = exact_probe_destination(program, reference)
                    .filter(|primitive| primitive.accepts_integer_literal())
                    .ok_or("range endpoint position requires an exact builtin integer carrier")?;
                Ok(Self {
                    primitive,
                    policy: policy.unwrap_or(ArithmeticDomain::Exact),
                    domains,
                })
            }

            pub(crate) fn require_value(
                &self,
                program: &TypedTrees,
                admission: &BuildTimeAdmissionPlan,
                value: &BigInt,
            ) -> Result<(), String> {
                typed_trees::closed_numeric::land_integer(value, self.primitive)
                    .ok_or("range endpoint value does not fit its declared integer carrier")?;
                for (symbol, name) in &self.domains {
                    match evaluate_closed_membership(program, admission, *symbol, value)? {
                        Some(true) => {}
                        Some(false) => {
                            return Err(format!(
                                "range endpoint value `{value}` is outside domain `{name}`"
                            ));
                        }
                        None => {
                            return Err(format!(
                                "range endpoint value `{value}` cannot prove membership in domain `{name}` at build time"
                            ));
                        }
                    }
                }
                Ok(())
            }
        }
    }

    pub(crate) fn integer_landing(carrier: PrimitiveType) -> Result<LandedIntegerType, String> {
        match carrier {
            PrimitiveType::I8 => Ok(LandedIntegerType::I8),
            PrimitiveType::I16 => Ok(LandedIntegerType::I16),
            PrimitiveType::I32 => Ok(LandedIntegerType::I32),
            PrimitiveType::I64 => Ok(LandedIntegerType::I64),
            PrimitiveType::U8 => Ok(LandedIntegerType::U8),
            PrimitiveType::U16 => Ok(LandedIntegerType::U16),
            PrimitiveType::U32 => Ok(LandedIntegerType::U32),
            PrimitiveType::U64 => Ok(LandedIntegerType::U64),
            _ => Err("range endpoint lost its builtin integer carrier".into()),
        }
    }

    #[derive(PartialEq, Eq)]
    pub(crate) struct PendingEndpoint {
        pub(crate) expression: ExpressionHandle,
        pub(crate) root: ExpressionHandle,
        pub(crate) machine: symbols::SymbolHandle,
        pub(crate) static_application: bool,
        pub(crate) contract_expression: bool,
    }

    pub(crate) struct IntegerRoot {
        pub(crate) context: typed_trees::constant_recipes::ConstantExpressionContext,
        pub(crate) expression: ExpressionHandle,
    }

    pub(crate) struct IntegerRootPlan {
        pub(crate) roots: Vec<IntegerRoot>,
        pub(crate) calls: Vec<PendingEndpoint>,
    }

    impl IntegerRootPlan {
        fn evaluation_roots(&self) -> impl Iterator<Item = &IntegerRoot> {
            let mut visited = std::collections::HashSet::new();
            self.roots
                .iter()
                .filter(move |root| visited.insert(root.expression))
        }
    }

    #[cfg(test)]
    mod root_plan_tests {
        use super::*;
        use typed_trees::constant_recipes::ConstantExpressionContext;
        use typed_trees::types::TypeReferenceHandle;

        #[test]
        fn constant_expression_contexts_share_legacy_folds_without_losing_owners() {
            let expression = ExpressionHandle::from_parts(17, 1);
            let plan = IntegerRootPlan {
                roots: [23, 29]
                    .map(|owner| IntegerRoot {
                        expression,
                        context: ConstantExpressionContext {
                            owner: symbols::SymbolHandle::from_parts(owner, 1),
                            result_type: TypeReferenceHandle::from_parts(31, 1),
                            ..Default::default()
                        },
                    })
                    .into(),
                calls: Vec::new(),
            };
            assert_eq!(plan.evaluation_roots().count(), 1);
            assert_eq!(plan.roots.len(), 2);
        }
    }

    pub fn evaluate_const_range_endpoints(
        typed: &mut TypedTrees,
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    ) -> Result<(), Vec<Diagnostic>> {
        evaluate_selected_range_endpoints(
            typed,
            selection_authority,
            SelectedBuildTimeOperators {
                operators: &[],
                provider_bodies: &[],
            },
            &EvaluationDomain::all(),
        )
    }

    pub(crate) fn pending_endpoint_calls_need_operator_selection(
        typed: &TypedTrees,
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        domain: &EvaluationDomain,
    ) -> Result<bool, Vec<Diagnostic>> {
        let pending = pending_endpoints(typed, domain)?;
        if pending.is_empty() {
            return Ok(false);
        }
        let prepared = crate::PreparedBuildMachineProgram::prepare_in(typed, domain)?;
        let execution = prepared.typed();
        let facts = validation::derive_pre_flow_operator_selections(execution);
        let admission = domain.admission(execution, selection_authority)?;
        Ok(pending.iter().any(|endpoint| {
            let machine = resolve_endpoint_callee(execution, endpoint)
                .map_or(endpoint.machine, |callee| callee.instance);
            admission.closure_needs_execution_preparation(execution, machine, &facts)
        }))
    }

    #[derive(Clone, Copy)]
    pub(crate) struct EndpointCallee {
        pub(crate) instance: symbols::SymbolHandle,
        pub(crate) static_application: bool,
    }

    impl EndpointCallee {}

    pub(crate) fn resolve_endpoint_callee(
        execution: &TypedTrees,
        endpoint: &PendingEndpoint,
    ) -> Result<EndpointCallee, String> {
        if !endpoint.static_application {
            return Ok(EndpointCallee {
                instance: endpoint.machine,
                static_application: false,
            });
        }
        let Some(call) = execution.expression_table.call_node(endpoint.expression) else {
            return Err("range endpoint lost its authored static application".to_owned());
        };
        let template = execution
            .machine(endpoint.machine)
            .ok_or("range endpoint lost its generic template")?;
        if !call.machine_arguments.is_empty() {
            if call.machine_arguments.len() == template.type_parameters.len() {
                return Err(format!(
                    "static application of `{}` awaits a complete validated specialization",
                    template.name,
                ));
            }
            return Err(format!(
                "static application of `{}` supplies {} of {} static arguments; supply every static argument explicitly",
                template.name,
                call.machine_arguments.len(),
                template.type_parameters.len(),
            ));
        }
        let instance = execution
            .machines()
            .iter()
            .find(|machine| {
                state_by_symbol(execution.machine_states(machine), call.target_symbol).is_some()
            })
            .filter(|machine| machine.type_parameters.is_empty())
            .ok_or_else(|| {
                format!(
                    "static application of `{}` was not specialized to a concrete instance",
                    template.name
                )
            })?;
        Ok(EndpointCallee {
            instance: instance.symbol,
            static_application: true,
        })
    }

    pub(crate) fn defer_pending_endpoint_calls(
        typed: &mut TypedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        defer_pending_endpoint_calls_in(typed, &EvaluationDomain::all())
    }

    pub(crate) fn defer_pending_endpoint_calls_in(
        typed: &mut TypedTrees,
        domain: &EvaluationDomain,
    ) -> Result<(), Vec<Diagnostic>> {
        for endpoint in pending_endpoints(typed, domain)? {
            typed
                .pending_const_range_endpoints
                .insert(endpoint.expression);
            typed.pending_const_range_endpoints.insert(endpoint.root);
        }
        Ok(())
    }

    pub(crate) fn evaluate_selected_range_endpoints(
        typed: &mut TypedTrees,
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        selected: crate::SelectedBuildTimeOperators<'_>,
        domain: &EvaluationDomain,
    ) -> Result<(), Vec<Diagnostic>> {
        let crate::SelectedBuildTimeOperators {
            operators,
            provider_bodies,
        } = selected;
        let mut published: Vec<(ExpressionHandle, ExpressionNode, Vec<ExpressionHandle>)> =
            Vec::new();
        let mut warnings = Vec::new();
        let restore = |typed: &mut TypedTrees,
                       published: Vec<(
            ExpressionHandle,
            ExpressionNode,
            Vec<ExpressionHandle>,
        )>| {
            for (expression, original, marked) in published.into_iter().rev() {
                *typed.expression_table.expression_mut(expression) = original;
                typed.pending_const_range_endpoints.extend(marked);
            }
        };
        loop {
            let plan = match integer_root_plan(typed, domain) {
                Ok(plan) => plan,
                Err(errors) => {
                    restore(typed, published);
                    return Err(errors);
                }
            };
            if plan.roots.is_empty() {
                break;
            }
            let round = match evaluate_round(
                typed,
                &plan,
                selection_authority.clone(),
                operators,
                provider_bodies,
                &mut warnings,
                domain,
            ) {
                Ok(round) => round,
                Err(_) if plan.calls.is_empty() => break,
                Err(errors) => {
                    restore(typed, published);
                    return Err(errors);
                }
            };
            let progress = !round.folds.is_empty();
            for (expression, folded) in round.folds {
                let original =
                    std::mem::replace(typed.expression_table.expression_mut(expression), folded);
                let mut marked: Vec<_> = plan
                    .calls
                    .iter()
                    .filter(|call| call.root == expression)
                    .filter_map(|call| {
                        typed
                            .pending_const_range_endpoints
                            .remove(&call.expression)
                            .then_some(call.expression)
                    })
                    .collect();
                if typed.pending_const_range_endpoints.remove(&expression) {
                    marked.push(expression);
                }
                published.push((expression, original, marked));
            }
            if round.diagnostics.is_empty() {
                break;
            }
            if !progress {
                restore(typed, published);
                return Err(round.diagnostics);
            }
        }
        crate::report_evaluation_diagnostics(&warnings)?;
        Ok(())
    }

    struct Round {
        folds: Vec<(ExpressionHandle, ExpressionNode)>,
        diagnostics: Vec<Diagnostic>,
    }

    fn evaluate_round(
        typed: &mut TypedTrees,
        plan: &IntegerRootPlan,
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
        operators: &[crate::SelectedBuildTimeBinaryOperator],
        provider_bodies: &[crate::SelectedBuildTimeProviderBody],
        warnings: &mut Vec<Diagnostic>,
        domain: &EvaluationDomain,
    ) -> Result<Round, Vec<Diagnostic>> {
        let (prepared, admission) = crate::const_evaluation::prepared_admission(
            typed,
            provider_bodies,
            operators,
            selection_authority.clone(),
            domain,
        )?;
        let execution = prepared.typed();

        let mut round = Round {
            folds: Vec::new(),
            diagnostics: Vec::new(),
        };
        let mut originals = Vec::new();
        for endpoint in plan.evaluation_roots() {
            let result = evaluate_root(
                typed,
                execution,
                &admission,
                endpoint,
                selection_authority.as_deref(),
                warnings,
            );
            match result {
                Ok(folded) => {
                    let original = std::mem::replace(
                        typed.expression_table.expression_mut(endpoint.expression),
                        folded.clone(),
                    );
                    originals.push((endpoint.expression, original));
                    round.folds.push((endpoint.expression, folded));
                }
                Err(reason) => {
                    let required = plan
                        .calls
                        .iter()
                        .any(|call| call.root == endpoint.expression);
                    if !required {
                        continue;
                    }
                    round.diagnostics.push(
                        Diagnostic::error(format!(
                            "contract integer expression: const evaluation failed: {reason}"
                        ))
                        .with_source_span(typed.expression_table.source_span(endpoint.expression)),
                    );
                }
            }
        }

        for (expression, original) in originals.into_iter().rev() {
            *typed.expression_table.expression_mut(expression) = original;
        }
        Ok(round)
    }

    fn evaluate_root(
        typed: &TypedTrees,
        execution: &TypedTrees,
        admission: &BuildTimeAdmissionPlan,
        endpoint: &IntegerRoot,
        selection_authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<ExpressionNode, String> {
        require_closed_expression_custody(execution, endpoint.expression, selection_authority)?;
        let invocation = arguments::Invocation {
            working: typed,
            execution,
            admission,
            authority: selection_authority,
        };
        let (literal, additions) =
            evaluate_integer_endpoint(execution, endpoint.expression, &invocation)?;
        extend_unique(warnings, additions);
        Ok(ExpressionNode::Integer(literal))
    }

    pub(crate) fn pending_endpoints(
        typed: &TypedTrees,
        domain: &EvaluationDomain,
    ) -> Result<Vec<PendingEndpoint>, Vec<Diagnostic>> {
        Ok(integer_root_plan(typed, domain)?.calls)
    }

    pub(crate) fn integer_root_plan(
        typed: &TypedTrees,
        domain: &EvaluationDomain,
    ) -> Result<IntegerRootPlan, Vec<Diagnostic>> {
        let mut pending = Vec::new();
        let mut roots = Vec::new();
        for root in super::contract_expressions::roots(typed, domain)? {
            roots.push(IntegerRoot {
                context: root.context,
                expression: root.expression,
            });
            extend_unique(&mut pending, root.invocations);
        }
        Ok(IntegerRootPlan {
            roots,
            calls: pending,
        })
    }

    pub(crate) fn selected_endpoint_machine(
        typed: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Option<(&typed_trees::machine::Machine, bool)> {
        let call = typed.expression_table.call_node(expression)?;
        if !call.evidence_arguments.is_empty()
            || call.static_machine_parameter.is_valid()
            || !call.selects_only_nominal_route()
        {
            return None;
        }
        if !call.machine_arguments.iter().all(|argument| {
            argument.application.is_none()
                && argument.evidence_projection.is_none()
                && (argument.type_reference.is_valid()
                    || matches!(
                        typed.symbols.get(argument.symbol).kind,
                        symbols::SymbolKind::BuiltinType | symbols::SymbolKind::Data
                    )
                    || argument.const_literal.is_some()
                    || typed.const_declaration_by_symbol(argument.symbol).is_some())
        }) {
            return None;
        }
        let machine = typed.machines().iter().find(|machine| {
            typed
                .machine_states(machine)
                .first()
                .is_some_and(|entry| typed.call_has_no_runtime_receiver(call, machine, entry))
        })?;
        if machine.type_parameters.is_empty() {
            return call
                .machine_arguments
                .is_empty()
                .then_some((machine, false));
        }
        machine
            .conformance_bounds
            .is_empty()
            .then_some((machine, true))
    }
}
pub(crate) mod selected_constants {
    use crate::{
        LayoutQueryProvider, PreCheckEvaluation, SelectedBuildTimeOperators,
        SelectedBuildTimeTargetBody,
    };
    use diagnostics::Diagnostic;
    use typed_trees::TypedTrees;
    use typed_trees::references::DeclarationSelection;

    pub struct SelectedConstantEvaluationRequest<'request, Layouts> {
        pub program: &'request mut TypedTrees,
        pub pending: Vec<PreCheckEvaluation>,
        pub operators: SelectedBuildTimeOperators<'request>,
        pub target_bodies: &'request [SelectedBuildTimeTargetBody],
        pub selection: &'request DeclarationSelection,
        pub layouts: &'request Layouts,
    }

    impl<Layouts: LayoutQueryProvider> SelectedConstantEvaluationRequest<'_, Layouts> {
        pub fn evaluate(self) -> Result<(), Vec<Diagnostic>> {
            crate::prepare_selected_target_family_calls(self.program, self.target_bodies)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            super::data_layout_queries::fold_data_layout_queries(
                self.program,
                true,
                self.selection,
                self.layouts,
            )?;
            let mut evaluated = Vec::with_capacity(self.pending.len());
            let layout_targets = self.layouts.layout_target_inputs();
            for continuation in self.pending {
                let continuation = continuation
                    .with_layout_targets(&layout_targets)
                    .with_layout_applications(self.layouts.layout_application_inputs());
                evaluated
                    .push(continuation.evaluate_selected_operators(self.program, self.operators)?);
            }
            super::data_layout_queries::fold_data_layout_queries(
                self.program,
                false,
                self.selection,
                self.layouts,
            )?;
            for continuation in evaluated {
                continuation.finish(self.program)?;
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::LayoutQueryMeasurements;
        use crate::build_time_evaluation::expression_table;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use typed_trees::expression::{DataLayoutQueryRequest, ExpressionNode};

        fn query_program() -> TypedTrees {
            let tokens =
                source_files_to_tokens::Lexer::new("machine query() -> u64 { size_of<u64>() }")
                    .tokenize()
                    .expect("query tokens");
            let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("query syntax");
            let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
            )
            .expect("query resolution");
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("query typing")
        }

        struct DeferredMeasurements {
            preparations: AtomicUsize,
            resolves: bool,
        }

        struct Measurement(Option<usize>);

        impl LayoutQueryProvider for DeferredMeasurements {
            type Measurements = Measurement;

            fn prepare(&self, _: &TypedTrees) -> Measurement {
                let pass = self.preparations.fetch_add(1, Ordering::Relaxed);
                Measurement((self.resolves && pass > 0).then_some(8))
            }
        }

        impl LayoutQueryMeasurements for Measurement {
            fn measure(
                &self,
                _: &TypedTrees,
                _: &DataLayoutQueryRequest,
            ) -> Result<Option<usize>, Diagnostic> {
                Ok(self.0)
            }
        }

        #[test]
        fn deferred_query_uses_a_fresh_measurement_view() {
            let mut program = query_program();
            let query = expression_table(&program)
                .iter_expressions()
                .find_map(|(handle, node)| {
                    matches!(node, ExpressionNode::Call(call) if call.layout_query.is_some())
                        .then_some(handle)
                })
                .expect("retained layout query");
            let selection = DeclarationSelection::from_program(&program);
            let layouts = DeferredMeasurements {
                preparations: AtomicUsize::new(0),
                resolves: true,
            };
            SelectedConstantEvaluationRequest {
                program: &mut program,
                pending: Vec::new(),
                operators: SelectedBuildTimeOperators::default(),
                target_bodies: &[],
                selection: &selection,
                layouts: &layouts,
            }
            .evaluate()
            .expect("second pass closes the query");
            assert_eq!(layouts.preparations.load(Ordering::Relaxed), 2);
            let ExpressionNode::Integer(value) = program.expression_table.expression(query) else {
                panic!("closed query must become its measured literal");
            };
            assert_eq!(value.to_string(), "8");
        }

        #[test]
        fn unresolved_second_pass_is_not_a_zero_sized_layout() {
            let mut program = query_program();
            let selection = DeclarationSelection::from_program(&program);
            let layouts = DeferredMeasurements {
                preparations: AtomicUsize::new(0),
                resolves: false,
            };
            let errors = SelectedConstantEvaluationRequest {
                program: &mut program,
                pending: Vec::new(),
                operators: SelectedBuildTimeOperators::default(),
                target_bodies: &[],
                selection: &selection,
                layouts: &layouts,
            }
            .evaluate()
            .expect_err("unresolved geometry must refuse");
            assert_eq!(layouts.preparations.load(Ordering::Relaxed), 2);
            assert!(format!("{errors:?}").contains("unresolved array extent"));
            assert!(
                expression_table(&program)
                    .iter_expressions()
                    .any(|(_, node)| {
                        matches!(node, ExpressionNode::Call(call) if call.layout_query.is_some())
                    })
            );
        }
    }
}

use typed_trees::TypedTrees;

pub(crate) fn prepared_admission(
    typed: &TypedTrees,
    provider_bodies: &[crate::SelectedBuildTimeProviderBody],
    operators: &[crate::SelectedBuildTimeBinaryOperator],
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &crate::build_time_evaluation::EvaluationDomain,
) -> Result<(PreparedBuildMachineProgram, BuildTimeAdmissionPlan), Vec<diagnostics::Diagnostic>> {
    let selected_execution = prepare_selected_execution(typed, provider_bodies, operators)
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    let prepared = PreparedBuildMachineProgram::prepare_in(
        selected_execution.as_ref().unwrap_or(typed),
        domain,
    )?;
    let execution = prepared.typed();
    let admission = domain
        .admission(execution, selection_authority)?
        .with_selected_operators(execution, operators)
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    Ok((prepared, admission))
}
