//! Selected call preconditions are obligations of contract term formation.
//!
//! Walk operands before judging their enclosing call. Only independently
//! formed, preceding requires facts are assumptions: neither the fact being
//! formed, another ensures, nor a future body citation can license a partial
//! application. Reflexive equality and body unfolding do not waive this gate.

mod applied_bindings;
pub(super) use applied_bindings::AppliedProofBindings;

use super::refuted_requires::instantiated_selected_fact_judgment;
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use super::structural_terms::structural_term;
use crate::expression_traversal::children;
use crate::machine_calls::calls::machine_state_by_symbol;
use crate::value_custody::content_conservation::{
    ContentDeclarationFormation, content_conservation_inputs,
};
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};
use typed_trees::state::State;

#[derive(Clone, Copy)]
enum SpecificationCaller<'program> {
    Machine(&'program Machine, Option<&'program State>),
    Declaration(SymbolHandle, &'program [StateParameter]),
}

impl<'program> SpecificationCaller<'program> {
    fn symbol(self) -> SymbolHandle {
        match self {
            Self::Machine(machine, _) => machine.symbol,
            Self::Declaration(symbol, _) => symbol,
        }
    }

    fn state_scope(self) -> Option<(&'program Machine, &'program State)> {
        match self {
            Self::Machine(machine, Some(state)) => Some((machine, state)),
            _ => None,
        }
    }

    fn expression_owner(self) -> SymbolHandle {
        self.state_scope()
            .map_or(self.symbol(), |(_, state)| state.symbol)
    }

    fn parameters(
        self,
        program: &'program dyn crate::CheckingInput,
    ) -> Vec<&'program StateParameter> {
        match self {
            Self::Machine(machine, state) => program
                .machine_states(machine)
                .first()
                .into_iter()
                .chain(state)
                .flat_map(|state| program.state_parameters(state))
                .collect(),
            Self::Declaration(_, parameters) => parameters.iter().collect(),
        }
    }

    fn expression_type(
        self,
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let ordinary = match self {
            Self::Machine(machine, state) => {
                let state = state.or_else(|| program.machine_states(machine).first())?;
                crate::expression_result_type_reference(program, machine, state, expression)
            }
            Self::Declaration(symbol, parameters) => {
                crate::parameter_expression_result_type_reference(
                    program, symbol, parameters, expression,
                )
            }
        };
        ordinary.or_else(|| {
            super::proof_integer::proof_integer_expression(program, expression)
                .then(|| {
                    crate::proof_contracts::proof_embeddings::proof_int_type_reference(program)
                })
                .flatten()
        })
    }

    fn selection_requirements(
        self,
        program: &'program dyn crate::CheckingInput,
    ) -> Vec<&'program ProofFact> {
        let contracts = match self {
            Self::Machine(machine, state) => program
                .machine_contracts(machine)
                .iter()
                .chain(
                    state
                        .into_iter()
                        .flat_map(|state| program.state_contracts(state)),
                )
                .collect::<Vec<_>>(),
            Self::Declaration(symbol, _) => selected_contract_by_symbol(program, symbol)
                .map(|owner| owner_contracts(program, &owner).collect::<Vec<_>>())
                .unwrap_or_default(),
        };
        contracts
            .into_iter()
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .collect()
    }

    fn term(
        self,
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<StructuralTerm> {
        let parameters = self
            .parameters(program)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        structural_term(
            program,
            self.state_scope()
                .map(|(machine, state)| (machine, state, Default::default())),
            Some((self.symbol(), &parameters)),
            expression,
            None,
        )
    }
}

/// Type-bound constant recipes use their real declaration telescope. Result
/// typing alone cannot establish an application's argument formation. This
/// checks owned operands and selected contracts without executing open binders.
pub(crate) fn validate_declaration_expression_formation(
    program: &dyn crate::CheckingInput,
    owner_symbol: SymbolHandle,
    parameters: &[StateParameter],
    root: ExpressionHandle,
    prior_facts: &[ExpressionHandle],
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        children(program, node, |child| pending.push(child));
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        if selected_call_contract_for_parameters(program, call, owner_symbol, parameters).is_none()
        {
            diagnostics.push(
                Diagnostic::error(format!(
                    "{owner} constant expression call has no exact declared contract owner"
                ))
                .with_source_span(program.expression_table.source_span(expression)),
            );
        }
    }
    validate_specification_expression(
        program,
        SpecificationCaller::Declaration(owner_symbol, parameters),
        root,
        prior_facts,
        owner,
        diagnostics,
        None,
        &mut Vec::new(),
        formation_obligations,
    );
}

/// Validate a selected application's operands before its requirements or result
/// can contribute facts. Constant recipes and abstract predicates share this owner.
#[allow(clippy::too_many_arguments)]
fn validate_declaration_call_arguments(
    program: &dyn crate::CheckingInput,
    owner_symbol: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    call: &TableCallExpression,
    selected: SelectedCallContract<'_>,
    content_formation: Option<ContentDeclarationFormation<'_>>,
    application: Option<&AppliedProofBindings>,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(expected) = owner_parameter_scope(program, &selected) else {
        return;
    };
    let actual = program
        .expression_table
        .expression_handles(call.arguments)
        .get(owner_leading_operands(&selected)..)
        .unwrap_or(&[]);
    if actual.len() != expected.len() {
        diagnostics.push(
            Diagnostic::error(format!(
                "{owner} constant expression call has incompatible argument arity"
            ))
            .with_source_span(program.expression_table.source_span(expression)),
        );
        return;
    }
    for (argument, parameter) in actual.iter().zip(expected.iter()) {
        // This exact proof observation already has a checked place, carrier,
        // access and qualification. It is not an ordinary runtime reference.
        // Selected call preconditions remain with validate_expression below.
        if let Some(formation) = content_formation
            && let SelectedCallContract::Concrete(projection, _) = &selected
            && formation.forms_argument(projection.symbol, expression, *argument)
        {
            continue;
        }
        let required_type = application.map_or(parameter.type_reference, |application| {
            application.project_type(program, parameter.type_reference)
        });
        let natural = crate::parameter_expression_result_type_reference(
            program,
            owner_symbol,
            parameters,
            *argument,
        );
        let matches = natural.map_or_else(|| {
                // Anonymous and literal structural operands keep the shared
                // destination formation judgment. An unresolved computation
                // never inherits a type from its expected destination.
                (!matches!(program.expression_table.expression(*argument), ExpressionNode::Binary(_) | ExpressionNode::Unary(_) | ExpressionNode::Call(_))
                    || crate::has_anonymous_numeric_results(program, *argument))
                    && crate::value_custody::expression_types::argument_matches_type_reference_with_substitutions(program, *argument, required_type, &[])
            }, |actual| crate::value_custody::expression_types::completed_value_type_matches(program, actual, required_type));
        if !matches {
            let actual = natural.map_or_else(
                || "no retained result type".to_owned(),
                |reference| program.display_type_reference_with_constraints(reference),
            );
            diagnostics.push(Diagnostic::error(format!("{owner} constant expression call argument `{}` does not form parameter `{}` of `{}`: actual {actual}, required {}",
                    program.expression_table.display_name(*argument), parameter.name, owner_name(program, &selected), program.display_type_reference_with_constraints(required_type)))
                    .with_source_span(program.expression_table.source_span(*argument)));
        }
    }
}

/// Abstract declarations reuse the machine contract's formation judgment
/// under their exact telescope, before the current fact becomes a premise.
pub(crate) fn validate_declaration_call_requirements(
    program: &dyn crate::CheckingInput,
    owner_symbol: SymbolHandle,
    parameters: &[StateParameter],
    fact: &ProofFact,
    prior_facts: &[ExpressionHandle],
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    let mut roots = Vec::new();
    fact_roots(program, fact, &mut roots);
    let content = content_conservation_inputs(program);
    let mut visited = Vec::new();
    for root in roots {
        validate_specification_expression(
            program,
            SpecificationCaller::Declaration(owner_symbol, parameters),
            root,
            prior_facts,
            owner,
            diagnostics,
            content.declaration_formation(owner_symbol, root),
            &mut visited,
            formation_obligations,
        );
    }
}

pub(crate) fn validate_specification_call_requirements(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    fact: &ProofFact,
    prior_facts: &[ExpressionHandle],
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    let mut roots = Vec::new();
    fact_roots(program, fact, &mut roots);
    let mut visited = Vec::new();
    for root in roots {
        validate_specification_expression(
            program,
            SpecificationCaller::Machine(machine, state),
            root,
            prior_facts,
            owner,
            diagnostics,
            None,
            &mut visited,
            formation_obligations,
        );
    }
}

/// A proof-valued body forms its applications under its declaration entry,
/// before normalization erases operation syntax. No executable invocation or
/// runtime effect is manufactured for this proof material.
pub(crate) fn validate_proof_body_formation(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    let Some(state) = program.machine_states(machine).first() else {
        return;
    };
    let hypotheses = program
        .contract_expressions(
            program
                .machine_contracts(machine)
                .iter()
                .chain(program.state_contracts(state)),
            SignatureContractKind::Requires,
        )
        .collect::<Vec<_>>();
    let owner = format!("proof body `{}`", machine.name);
    super::proof_integer::validate_proof_fact_integer_formation(
        program,
        state.symbol,
        &ProofFact::Expression(expression),
        &hypotheses,
        &owner,
        &|candidate| {
            crate::proof_contracts::bound_expression_meaning::builtin_arithmetic(
                program,
                machine,
                Some(state),
                candidate,
            )
        },
        diagnostics,
    );
    validate_specification_expression(
        program,
        SpecificationCaller::Machine(machine, Some(state)),
        expression,
        &hypotheses,
        &owner,
        diagnostics,
        None,
        &mut Vec::new(),
        formation_obligations,
    );
}

fn declaration_anonymous_fit(
    program: &dyn crate::CheckingInput,
    caller: SpecificationCaller<'_>,
    expression: ExpressionHandle,
    prior_facts: &[ExpressionHandle],
    target: typed_trees::types::PrimitiveType,
) -> super::operand_fits::CarrierFit {
    use super::{ScopedArithmeticExpression, ScopedArithmeticHypothesis, ScopedArithmeticValue};
    let bindings = arithmetic_caller_bindings(program, caller);
    let hypotheses = prior_facts
        .iter()
        .map(|fact| ScopedArithmeticHypothesis {
            proposition: ScopedArithmeticExpression {
                owner: caller.expression_owner(),
                expression: *fact,
                bindings: bindings.clone(),
            },
            holds: true,
        })
        .collect::<Vec<_>>();
    let value = ScopedArithmeticValue::Term(ScopedArithmeticExpression {
        owner: caller.expression_owner(),
        expression,
        bindings,
    });
    super::scoped_arithmetic::scoped_arithmetic_value_fits_for_declaration(
        program,
        caller.symbol(),
        &hypotheses,
        &value,
        target,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_specification_expression(
    program: &dyn crate::CheckingInput,
    caller: SpecificationCaller<'_>,
    expression: ExpressionHandle,
    prior_facts: &[ExpressionHandle],
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    content_formation: Option<ContentDeclarationFormation<'_>>,
    visited: &mut Vec<ExpressionHandle>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    if !expression.is_valid() || visited.contains(&expression) {
        return;
    }
    visited.push(expression);
    let node = program.expression_table.expression(expression);
    let diagnostics_before = diagnostics.len();
    children(program, node, |child| {
        validate_specification_expression(
            program,
            caller,
            child,
            prior_facts,
            owner,
            diagnostics,
            content_formation,
            visited,
            formation_obligations,
        );
    });
    if diagnostics.len() != diagnostics_before {
        return;
    }
    if let ExpressionNode::Cast(cast) = node
        && !cast.form.is_recast()
        && program.primitive_type_reference(cast.target_type)
            == Some(typed_trees::types::PrimitiveType::Addr)
        && crate::value_custody::literals::has_anonymous_numeric_results(program, cast.value)
    {
        let before = diagnostics.len();
        crate::proof_contracts::arithmetic_domains::validate_anonymous_integer_primitive_range(
            program,
            typed_trees::types::PrimitiveType::Addr,
            cast.value,
            owner,
            diagnostics,
        );
        if diagnostics.len() == before
            && !declaration_anonymous_fit(
                program,
                caller,
                cast.value,
                prior_facts,
                typed_trees::types::PrimitiveType::Addr,
            )
            .retain(expression, caller.symbol(), owner, formation_obligations)
        {
            diagnostics.push(
                Diagnostic::error(format!(
                    "anonymous address cast operand cannot prove fit in `addr::Bound` in {owner}",
                ))
                .with_source_span(program.expression_table.source_span(cast.value)),
            );
        }
    }
    // Both ordinary call syntax and an authored operator application consume
    // the selected declaration's public preconditions through this one intake.
    let (contract_owner, arguments, call, application_bindings) = match node {
        ExpressionNode::Call(call) => {
            if crate::mathematical_applications::selected_definition(program, call).is_some() {
                // Mathematical definitions own checked dependent applications,
                // not machine contract telescopes. Their source producer checks
                // the application; the walk above still forms every argument.
                // An authored generic registration does not change that owner.
                return;
            }
            let selected = match caller {
                SpecificationCaller::Declaration(symbol, parameters) => {
                    selected_call_contract_for_parameters(program, call, symbol, parameters)
                }
                SpecificationCaller::Machine(..) => {
                    selected_call_contract(program, caller.symbol(), call)
                }
            };
            let Some(selected) = selected else {
                if call.authored_machine_application.is_valid()
                    && program
                        .expression_table
                        .static_machine_application(call.authored_machine_application)
                        .registration_operation
                        .is_valid()
                {
                    diagnostics.push(Diagnostic::error(format!("cannot form specification call in {owner}: retained public contract owner is unavailable"))
                        .with_source_span(program.expression_table.source_span(expression)));
                }
                return;
            };
            let Some(arguments) = program
                .expression_table
                .expression_handles(call.arguments)
                .get(owner_leading_operands(&selected)..)
            else {
                return;
            };
            let substitution =
                AppliedProofBindings::for_call(program, caller.symbol(), &selected, call);
            (selected, arguments.to_vec(), Some(call), substitution)
        }
        ExpressionNode::Binary(binary) => {
            let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator)
            else {
                return;
            };
            let arguments = vec![binary.left, binary.right];
            let types = arguments
                .iter()
                .map(|argument| caller.expression_type(program, *argument))
                .collect::<Vec<_>>();
            let requirements = caller.selection_requirements(program);
            let scope = crate::OperatorApplicationScope {
                owner: caller.expression_owner(),
                origin: None,
                requires: &requirements,
                expression_type: &|operand| caller.expression_type(program, operand),
            };
            let resolution =
                crate::scoped_operator_application(program, &scope, expression, spelling, &types);
            if resolution.status == checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            {
                crate::value_custody::expression_types::validate_builtin_comparison_operand_landing(
                    program,
                    binary.operator,
                    [(binary.left, types[0]), (binary.right, types[1])],
                    owner,
                    &mut |peer, target| {
                        declaration_anonymous_fit(program, caller, peer, prior_facts, target)
                            .retain(
                                expression,
                                caller.symbol(),
                                "anonymous address comparison operand",
                                formation_obligations,
                            )
                    },
                    diagnostics,
                );
                return;
            }
            let Some(selected) = resolution.into_selected() else {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot form specification operator `{}` in {owner}: no exact selected meaning",
                    spelling.symbol(),
                )).with_source_span(program.expression_table.source_span(expression)));
                return;
            };
            let Some(selected_owner) =
                selected_contract_by_symbol(program, selected.public_symbol())
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot form specification operator `{}` in {owner}: selected public contract is unavailable",
                    spelling.symbol(),
                )));
                return;
            };
            let substitution = AppliedProofBindings::for_operator(program, &selected);
            (selected_owner, arguments, None, substitution)
        }
        _ => return,
    };
    // Type correspondence and carrier formation are distinct. Every selected
    // specification application lands its anonymous actuals before any public
    // Requires or result facts, including machine callers and user operators.
    if let Some(parameters) = owner_parameter_scope(program, &contract_owner) {
        for (argument, parameter) in arguments.iter().zip(parameters.iter()) {
            let required_type = application_bindings
                .as_ref()
                .map_or(parameter.type_reference, |application| {
                    application.project_type(program, parameter.type_reference)
                });
            if program.primitive_type_reference(required_type)
                != Some(typed_trees::types::PrimitiveType::Addr)
                || !crate::has_anonymous_numeric_results(program, *argument)
            {
                continue;
            }
            let before = diagnostics.len();
            crate::proof_contracts::arithmetic_domains::validate_anonymous_integer_primitive_range(
                program,
                typed_trees::types::PrimitiveType::Addr,
                *argument,
                owner,
                diagnostics,
            );
            if diagnostics.len() == before
                && !declaration_anonymous_fit(
                    program,
                    caller,
                    *argument,
                    prior_facts,
                    typed_trees::types::PrimitiveType::Addr,
                )
                .retain(
                    *argument,
                    caller.symbol(),
                    "anonymous address specification argument",
                    formation_obligations,
                )
            {
                diagnostics.push(Diagnostic::error(format!("anonymous address specification argument cannot prove fit in `addr::Bound` in {owner}"))
                    .with_source_span(program.expression_table.source_span(*argument)));
            }
        }
    }
    if let SpecificationCaller::Declaration(symbol, parameters) = caller
        && let Some(call) = call
    {
        validate_declaration_call_arguments(
            program,
            symbol,
            parameters,
            expression,
            call,
            contract_owner,
            content_formation,
            application_bindings.as_ref(),
            owner,
            diagnostics,
        );
    }
    if diagnostics.len() != diagnostics_before {
        return;
    }
    let requirements = applicable_requirements_of(program, &contract_owner);
    if requirements.is_empty() {
        return;
    }
    if !requirements_are_acyclic(program, &contract_owner, &mut Vec::new(), &mut Vec::new()) {
        diagnostics.push(Diagnostic::error(format!(
            "cyclic requires formation for specification call `{}` in {owner}: an application cannot establish the contract needed to form itself",
            owner_name(program, &contract_owner),
        )));
        return;
    }
    let Some(parameter_scope) = owner_parameter_scope(program, &contract_owner) else {
        return;
    };
    let parameters = parameter_scope.as_ref();
    let projected_parameters = application_bindings.as_ref().map(|bindings| {
        parameters
            .iter()
            .map(|parameter| {
                let mut projected = parameter.clone();
                projected.type_reference = bindings.project_type(program, parameter.type_reference);
                projected
            })
            .collect::<Vec<_>>()
    });
    let parameters = projected_parameters.as_deref().unwrap_or(parameters);
    let substitution = parameters
        .iter()
        .zip(&arguments)
        .map(|(parameter, argument)| {
            Some((
                parameter.name.as_str().to_owned(),
                caller.term(program, *argument)?,
            ))
        })
        .collect::<Option<Vec<_>>>();
    // The existing structural prover has name-backed private variables. Do
    // not let two different resolved caller binders acquire one such variable.
    // The arithmetic route below uses exact symbols directly.
    let structural_facts = prior_facts
        .iter()
        .copied()
        .filter(|fact| {
            structurally_substitutable_fact(program, *fact, &|expression| {
                caller.term(program, expression)
            }) && names_are_unambiguous(program, &[*fact], &arguments)
        })
        .collect::<Vec<_>>();
    let structural_names_unique = names_are_unambiguous(program, &structural_facts, &arguments);
    let substitution = substitution.and_then(|mut substitution| {
        let bindings = application_bindings.as_ref()?;
        for binding in &bindings.constants {
            let value = binding.value_term()?;
            let name = program.symbols.name(binding.symbol);
            if substitution.iter().any(|(existing, _)| existing == name) {
                return None;
            }
            substitution.push((name.to_owned(), value));
        }
        Some(substitution)
    });
    let judge = StructuralJudge::from_declaration_requires(
        program,
        caller.symbol(),
        &caller
            .parameters(program)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>(),
        &structural_facts,
        true,
    );
    // Named proposition requirements are an erased evidence lane, not Boolean
    // facts for the structural/arithmetic prover. The checked-tree contract
    // evidence binder validates each supplied term's exact owner, proposition
    // and argument substitution before publication. Defer only the nominal
    // call form that binder handles, with a complete nonempty lane; ordinary
    // requirements below still need independent prior facts.
    let named_evidence_count = requirements.iter().filter(|(_, named)| *named).count();
    let has_named_evidence_lane = matches!(contract_owner, SelectedCallContract::Concrete(..))
        && named_evidence_count != 0
        && call.is_some_and(|call| {
            call.static_requirement_dispatch.is_none()
                && call.quotient_operation.is_none()
                && call.machine_arguments.is_empty()
                && call.evidence_arguments.len() == named_evidence_count
        });
    for (requirement, named_evidence) in requirements {
        if named_evidence
            && has_named_evidence_lane
            && matches!(requirement, ProofFact::Proposition(_))
        {
            continue;
        }
        // Both telescopes belong to the selected public owner. A missing or
        // unsupported static operand never becomes an empty substitution.
        let arguments_select_plainly = application_bindings.is_some()
            && (matches!(
                contract_owner,
                SelectedCallContract::Signature(..) | SelectedCallContract::DynamicSlot { .. }
            ) || call.is_none_or(|call| call.static_requirement_dispatch.is_none()));
        let proven = if let Some(required) = requirement.as_expression()
            && arguments_select_plainly
        {
            let structural =
                requirement_parameters_are_bound(
                    program,
                    *required,
                    parameters,
                    application_bindings.as_ref().expect("complete application"),
                ) && structural_static_binders_are_exact(
                    program,
                    *required,
                    parameters,
                    application_bindings.as_ref().expect("complete application"),
                ) && structurally_substitutable_fact(program, *required, &|expression| {
                    structural_term(
                        program,
                        None,
                        Some((owner_symbol(&contract_owner), parameters)),
                        expression,
                        None,
                    )
                }) && parameters.len() == arguments.len()
                    && structural_names_unique
                    && substitution.as_ref().is_some_and(|substitution| {
                        substitution
                            .iter()
                            .all(|(_, term)| term_has_complete_substitution(program, term))
                            && matches!(
                                instantiated_selected_fact_judgment(
                                    program,
                                    &judge,
                                    owner_symbol(&contract_owner),
                                    parameters,
                                    *required,
                                    substitution,
                                    &application_bindings
                                        .as_ref()
                                        .expect("complete application")
                                        .machines,
                                    caller.state_scope(),
                                ),
                                StructuralJudgment::Proven
                            )
                    });
            structural
                || arithmetic_requirement(
                    program,
                    caller,
                    &contract_owner,
                    prior_facts,
                    *required,
                    &arguments,
                    application_bindings.as_ref().expect("complete application"),
                )
        } else {
            false
        };
        if !proven {
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove requires contract for specification call `{}` in {owner}: establish its selected precondition in an independently formed prior requires fact",
                owner_name(program, &contract_owner),
            )));
        }
    }
}

/// The selected public contract owner of a call's obligations and guarantees.
/// A concrete machine state owns its machine/state contracts; a trait
/// requirement signature owns the public contract a conformance satisfies,
/// reached either by `static_requirement_dispatch` (the realization target is
/// the private rewrite, not the contract owner) or by direct signature-symbol
/// citation in a proof term.
#[derive(Clone, Copy)]
pub(crate) enum SelectedCallContract<'program> {
    Concrete(&'program Machine, &'program State),
    Signature(&'program StateSignature),
    DynamicSlot {
        slot: &'program typed_trees::dynamic_traits::DynamicSlotSignature,
        signature: &'program StateSignature,
    },
    Operator(&'program typed_trees::operator::OperatorDefinition),
}

pub(crate) fn selected_call_contract<'program>(
    program: &'program dyn crate::CheckingInput,
    caller: SymbolHandle,
    call: &TableCallExpression,
) -> Option<SelectedCallContract<'program>> {
    selected_call_site_contract(
        program,
        caller,
        crate::CallSite::Expression {
            expression: Default::default(),
            call,
        },
    )
}

/// Statement and expression completion consume the same selected public owner.
/// A private realization cannot replace its retained requirement contract.
pub(super) fn selected_call_site_contract<'program>(
    program: &'program dyn crate::CheckingInput,
    caller: SymbolHandle,
    site: crate::CallSite<'_>,
) -> Option<SelectedCallContract<'program>> {
    // Contract expressions can name their exact state as value owner. Static
    // selection belongs to that state's machine; abstract signatures retain
    // their independent declaration telescope.
    let caller = program
        .machine_holding_state(caller)
        .map_or(caller, |machine| machine.symbol);
    // The caller owns selection even when the member's own telescope is
    // empty and has no authored machine application. Its exact public view
    // must never be replaced by the executable realization's private contract.
    if let Some((_, signature)) = crate::requirement_contract_signature(program, caller, &site) {
        return Some(SelectedCallContract::Signature(signature));
    }
    if site.static_requirement_dispatch().is_some() {
        // A selected body cannot replace a lost occurrence-local public view.
        return None;
    }
    match site {
        crate::CallSite::Expression { call, .. } => {
            selected_contract_by_symbol(program, call.target_symbol).or_else(|| {
                typed_trees::operator::resolve_named_expression_call(program, call)
                    .map(SelectedCallContract::Operator)
            })
        }
        crate::CallSite::Statement(call) => {
            selected_contract_by_symbol(program, call.target_symbol).or_else(|| {
                let candidates =
                    typed_trees::operator::named_statement_call_candidates(program, call);
                match candidates.as_slice() {
                    [operator] => Some(SelectedCallContract::Operator(operator)),
                    _ => None,
                }
            })
        }
        crate::CallSite::TransitionNamed { .. } => None,
    }
}

/// Resolve an already retained public callable identity. No spelling or
/// realization target substitutes for a missing exact contract owner.
pub(super) fn selected_contract_by_symbol(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<SelectedCallContract<'_>> {
    if !symbol.is_valid() {
        return None;
    }
    if let Some(slot) = program.dynamic_slot_by_symbol(symbol) {
        let signature = program.dynamic_slot_requirement(slot)?;
        return Some(SelectedCallContract::DynamicSlot { slot, signature });
    }
    if let Some(signature) = signature_by_symbol(program, symbol) {
        return Some(SelectedCallContract::Signature(signature));
    }
    let concrete = selected_machine(program, symbol);
    // Only the exact entry has its callable's public interface. A subordinate
    // state retains its own parameter and contract frame, even when its
    // machine also presents an operator declaration.
    let entry_machine = concrete.and_then(|(machine, state)| {
        program
            .machine_states(machine)
            .first()
            .filter(|entry| entry.symbol == state.symbol)
            .map(|_| machine)
    });
    if let Some(signature) =
        entry_machine.and_then(|machine| signature_by_symbol(program, machine.symbol))
    {
        return Some(SelectedCallContract::Signature(signature));
    }
    let operator = typed_trees::operator::declaration_by_symbol(program, symbol).or_else(|| {
        entry_machine.and_then(|machine| {
            typed_trees::operator::declaration_by_symbol(program, machine.symbol)
        })
    });
    operator
        .map(SelectedCallContract::Operator)
        .or_else(|| concrete.map(|(machine, state)| SelectedCallContract::Concrete(machine, state)))
}

/// A proof application retains its own abstract operand telescope before
/// actual substitution. Source semantic overloads select by these established
/// references, sharing the same sealed declaration owner as result typing.
pub(super) fn selected_call_contract_for_parameters<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &TableCallExpression,
    owner: SymbolHandle,
    parameters: &[StateParameter],
) -> Option<SelectedCallContract<'program>> {
    selected_call_contract(program, owner, call).or_else(|| {
        let (symbol, _, _) =
            crate::float_semantic_application_for_parameters(program, call, owner, parameters)?;
        typed_trees::operator::declaration_by_symbol(program, symbol)
            .map(SelectedCallContract::Operator)
    })
}

fn signature_by_symbol(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&StateSignature> {
    crate::selected_requirement_signature(program, symbol)
        .map(|(_, signature)| signature)
        .or_else(|| {
            // A static callable binder owns a public signature just as a
            // trait requirement does. Import that declared contract before
            // considering a selected body's stronger private guarantees.
            program
                .machine_parameter_signature(symbol)
                .map(|(_, signature)| signature)
        })
}

pub(super) fn owner_symbol(owner: &SelectedCallContract<'_>) -> SymbolHandle {
    match owner {
        SelectedCallContract::Concrete(_, state) => state.symbol,
        SelectedCallContract::Signature(signature)
        | SelectedCallContract::DynamicSlot { signature, .. } => signature.symbol,
        SelectedCallContract::Operator(operator) => operator.symbol,
    }
}

pub(super) fn owner_name(
    program: &dyn crate::CheckingInput,
    owner: &SelectedCallContract<'_>,
) -> String {
    match owner {
        SelectedCallContract::Concrete(machine, _) => machine.name.as_str().to_owned(),
        SelectedCallContract::Signature(signature)
        | SelectedCallContract::DynamicSlot { signature, .. } => signature.name.as_str().to_owned(),
        SelectedCallContract::Operator(operator) => {
            program.symbols.name(operator.symbol).to_owned()
        }
    }
}

pub(super) fn owner_parameters<'program>(
    program: &'program dyn crate::CheckingInput,
    owner: &SelectedCallContract<'program>,
) -> &'program [StateParameter] {
    match owner {
        SelectedCallContract::Concrete(_, state) => program.state_parameters(state),
        SelectedCallContract::Signature(signature)
        | SelectedCallContract::DynamicSlot { signature, .. } => {
            program.state_signature_parameters(signature)
        }
        SelectedCallContract::Operator(operator) => program.operator_parameters(operator),
    }
}

/// The public contract keeps its declaration symbols while its types are
/// instantiated by the exact selected slot. Slot symbols belong to invocation
/// flow and cannot replace the original binders in authored contract facts.
pub(super) fn owner_parameter_scope<'program>(
    program: &'program dyn crate::CheckingInput,
    owner: &SelectedCallContract<'program>,
) -> Option<std::borrow::Cow<'program, [StateParameter]>> {
    let declared = owner_parameters(program, owner);
    let SelectedCallContract::DynamicSlot { slot, .. } = owner else {
        return Some(std::borrow::Cow::Borrowed(declared));
    };
    let instantiated = slot.data_parameters();
    if declared.len() != instantiated.len() {
        return None;
    }
    let parameters = declared
        .iter()
        .zip(instantiated)
        .map(|(declared, actual)| {
            let mut parameter = declared.clone();
            parameter.type_reference = actual.type_reference;
            parameter
        })
        .collect();
    Some(std::borrow::Cow::Owned(parameters))
}

pub(super) fn call_parameter_scope<'program>(
    program: &'program dyn crate::CheckingInput,
    caller: SymbolHandle,
    owner: &SelectedCallContract<'program>,
    site: crate::CallSite<'program>,
) -> Option<std::borrow::Cow<'program, [StateParameter]>> {
    if site.static_requirement_dispatch().is_some() {
        // Preconditions, completed guarantees and receiving carriers share
        // the same exact ordinary/public and realized-Value correspondence.
        return crate::machine_calls::calls::executable_parameters(program, caller, &site).ok();
    }
    let declared = owner_parameter_scope(program, owner)?;
    let (application, arguments) = match site {
        crate::CallSite::Expression { call, .. } => (
            call.authored_machine_application,
            call.machine_arguments.as_ref(),
        ),
        crate::CallSite::Statement(call) => (
            call.authored_machine_application,
            call.machine_arguments.as_ref(),
        ),
        crate::CallSite::TransitionNamed { .. } => return Some(declared),
    };
    let mut substitutions = Vec::new();
    if application.is_valid() {
        for binding in program
            .expression_table
            .static_machine_application_bindings(application)
        {
            match binding.value {
                typed_trees::static_applications::StaticMachineBindingValue::Type(reference)
                | typed_trees::static_applications::StaticMachineBindingValue::Const(reference) => {
                    substitutions.push((binding.parameter, reference))
                }
                _ => {}
            }
        }
    } else if let SelectedCallContract::Concrete(machine, _) = owner {
        let binders = program.machine_type_parameters(machine);
        if binders.len() == arguments.len() {
            for (binder, argument) in binders.iter().zip(arguments) {
                let actual = if argument.type_reference.is_valid() {
                    Some(argument.type_reference)
                } else {
                    program
                        .type_reference_table
                        .find_named_type_reference(argument.symbol)
                        .or_else(|| {
                            if !argument.symbol.is_valid() {
                                return None;
                            }
                            program.type_reference_table.references().find_map(
                                |(reference, node)| {
                                    let typed_trees::types::TypeReferenceNode::ConstExpression(
                                        expression,
                                    ) = node
                                    else {
                                        return None;
                                    };
                                    program
                                        .expression_table
                                        .name_path_node(*expression)
                                        .is_some_and(|path| {
                                            path.symbol == argument.symbol
                                                && path.head_symbol == argument.symbol
                                        })
                                        .then_some(reference)
                                },
                            )
                        })
                };
                if let Some(actual) = actual {
                    substitutions.push((binder.symbol, actual));
                }
            }
        }
    }
    if substitutions.is_empty() {
        return Some(declared);
    }
    let mut parameters = declared.into_owned();
    for parameter in &mut parameters {
        let identity = program.type_identity_with_simultaneous_substitutions(
            typed_trees::type_identity::TypeIdentityRequest {
                substitutions: &substitutions,
                ..typed_trees::type_identity::TypeIdentityRequest::ordinary(
                    parameter.type_reference,
                )
            },
        );
        // Keep the exact physical carrier when substitution did not change
        // it. Choosing another equal arena node obscures parameter identity
        // and needlessly searches the entire type table for every formal.
        if program.normalized_type_identity(parameter.type_reference) == identity {
            continue;
        }
        parameter.type_reference =
            program
                .type_reference_table
                .references()
                .find_map(|(reference, _)| {
                    (program.normalized_type_identity(reference) == identity).then_some(reference)
                })?;
    }
    Some(std::borrow::Cow::Owned(parameters))
}

/// Locate an authored contract binder in the selected invocation's operand
/// roster. Dynamic slots retain fresh invocation symbols and a dispatch prefix;
/// their requirement's declaration correspondence, never parameter spelling,
/// determines the data operand position.
pub fn call_contract_parameter_position(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
    parameter: SymbolHandle,
) -> Option<usize> {
    // An invocation binder already has its exact position, including static
    // machine parameters and the generated slot telescope. Authored dynamic
    // requirements instead need their retained declaration correspondence.
    if let Some(parameters) = crate::call_target_parameters(program, target)
        && let Some(position) =
            typed_trees::signature::state_parameter_position(parameters, parameter)
    {
        return Some(position);
    }
    let owner = selected_contract_by_symbol(program, target)?;
    let parameters = owner_parameter_scope(program, &owner)?;
    typed_trees::signature::state_parameter_position(&parameters, parameter)
        .map(|position| position + owner_leading_operands(&owner))
}

pub(super) fn owner_leading_operands(owner: &SelectedCallContract<'_>) -> usize {
    match owner {
        // The selected slot owns this dispatch prefix; its data_parameters
        // API exposes the corresponding complete instantiated telescope.
        SelectedCallContract::DynamicSlot { slot, .. } => {
            slot.parameters.len() - slot.data_parameters().len()
        }
        _ => 0,
    }
}

pub(super) fn owner_return_type(
    owner: &SelectedCallContract<'_>,
) -> typed_trees::types::TypeReferenceHandle {
    match owner {
        SelectedCallContract::Concrete(_, state) => state.return_type,
        SelectedCallContract::Signature(signature) => signature.return_type,
        SelectedCallContract::DynamicSlot { slot, .. } => slot.return_type,
        SelectedCallContract::Operator(operator) => operator.return_type,
    }
}

pub(super) fn owner_contracts<'program>(
    program: &'program dyn crate::CheckingInput,
    owner: &SelectedCallContract<'program>,
) -> impl Iterator<Item = &'program typed_trees::signature::SignatureContract> {
    let (first, second): (&[_], &[_]) = match owner {
        SelectedCallContract::Concrete(machine, state) => (
            if program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == state.symbol)
            {
                program.machine_contracts(machine)
            } else {
                &[]
            },
            program.state_contracts(state),
        ),
        SelectedCallContract::Signature(signature)
        | SelectedCallContract::DynamicSlot { signature, .. } => {
            (program.state_signature_contracts(signature), &[])
        }
        SelectedCallContract::Operator(operator) => (program.operator_contracts(operator), &[]),
    };
    first.iter().chain(second)
}

fn applicable_requirements_of<'program>(
    program: &'program dyn crate::CheckingInput,
    owner: &SelectedCallContract<'program>,
) -> Vec<(&'program ProofFact, bool)> {
    owner_contracts(program, owner)
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| {
            program
                .proof_facts
                .span_or_empty(contract.facts)
                .iter()
                .map(|fact| (fact, contract.binding.is_some()))
        })
        .collect()
}

/// The same authenticated application supplies closed and runtime values.
/// Runtime values come from the completed-operand owner, never source replay.
#[derive(Clone, Copy)]
pub(super) enum RuntimeScalarSubject {
    Symbol(SymbolHandle),
    Expression(ExpressionHandle),
}

/// Whole public contracts read the occurrence's applied signature. Selected
/// executable calls have already substituted closed operands into that view;
/// their runtime Value coordinates are ordinary mapped public formals supplied
/// by `call_parameter_scope`. Rebinding the provider's tuple by the fresh public
/// symbols loses that correspondence. Raw index projections retain their own
/// executable telescope and use `call_scalar_substitution` below instead.
pub(super) fn call_contract_scalar_substitution(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    owner: &SelectedCallContract<'_>,
    site: crate::CallSite<'_>,
    observed: impl FnMut(RuntimeScalarSubject) -> Option<StructuralTerm>,
) -> Option<Vec<(typed_trees::signature::StateParameter, StructuralTerm)>> {
    if site.static_requirement_dispatch().is_some() {
        let (_, selected) = crate::requirement_contract_signature(program, caller, &site)?;
        let SelectedCallContract::Signature(signature) = owner else {
            return None;
        };
        // The declaration symbol alone does not identify a fresh applied view.
        // Both borrows must name this exact retained invocation signature.
        if !std::ptr::eq(*signature, selected) {
            return None;
        }
        crate::selected_call_arguments(program, caller, &site).ok()?;
        return Some(Vec::new());
    }
    call_scalar_substitution(program, caller, owner, site, observed)
}

pub(super) fn call_scalar_substitution(
    program: &dyn crate::CheckingInput,
    caller: SymbolHandle,
    owner: &SelectedCallContract<'_>,
    site: crate::CallSite<'_>,
    mut observed: impl FnMut(RuntimeScalarSubject) -> Option<StructuralTerm>,
) -> Option<Vec<(typed_trees::signature::StateParameter, StructuralTerm)>> {
    let applied = AppliedProofBindings::for_call_site(program, caller, owner, site)?;
    let parameters = match owner {
        SelectedCallContract::Operator(operator) => program.operator_type_parameters(operator),
        SelectedCallContract::Concrete(machine, _) => program.machine_type_parameters(machine),
        SelectedCallContract::Signature(signature)
        | SelectedCallContract::DynamicSlot { signature, .. } => {
            program.state_signature_type_parameters(signature)
        }
    };
    let mut values = applied
        .constants
        .iter()
        .map(|binding| Some((binding.symbol, binding.value_term()?)))
        .collect::<Option<Vec<_>>>()?;
    if !applied.runtime_parameters.is_empty() {
        let executable = call_parameter_scope(program, caller, owner, site)?;
        for binding in &applied.runtime_parameters {
            let mut receiving = executable
                .iter()
                .filter(|parameter| parameter.symbol == binding.symbol);
            if let Some(parameter) = receiving.next() {
                if receiving.next().is_some() || parameter.is_const || parameter.is_mutable {
                    return None;
                }
                // Realization already made this exact Value binder an ordinary
                // trailing formal. Its completed argument owns the substitution;
                // it has no second declaration-header name or source replay.
                continue;
            }
            values.push((binding.symbol, observed(binding.observation_subject()?)?));
        }
    }
    let mut result = Vec::new();
    for (symbol, value) in values {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == symbol)?;
        let (typed_trees::data::TypeParameterKind::Const { type_reference }
        | typed_trees::data::TypeParameterKind::Value { type_reference }) = parameter.kind
        else {
            return None;
        };
        result.push((
            typed_trees::signature::StateParameter {
                symbol,
                name: format!("\0static_{}_{}", symbol.arena_index(), symbol.generation())
                    .as_str()
                    .into(),
                type_reference: applied.project_type(program, type_reference),
                is_const: matches!(
                    parameter.kind,
                    typed_trees::data::TypeParameterKind::Const { .. }
                ),
                ..Default::default()
            },
            value,
        ));
    }
    Some(result)
}

fn call_machine_substitution(
    program: &dyn crate::CheckingInput,
    owner: &SelectedCallContract<'_>,
    arguments: &[typed_trees::expression::StaticMachineArgument],
) -> Option<
    Vec<(
        SymbolHandle,
        typed_trees::types::TypeReferenceHandle,
        typed_trees::expression::StaticMachineArgument,
    )>,
> {
    let SelectedCallContract::Concrete(machine, _) = owner else {
        return arguments.is_empty().then(Vec::new);
    };
    let parameters = program
        .machine_kind_type_parameters(machine)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return None;
    }
    parameters
        .into_iter()
        .zip(arguments.iter())
        .map(|(parameter, selected)| selected_machine_binding(program, parameter.symbol, selected))
        .collect()
}

fn selected_machine_binding(
    program: &dyn crate::CheckingInput,
    parameter: SymbolHandle,
    selected: &typed_trees::expression::StaticMachineArgument,
) -> Option<(
    SymbolHandle,
    typed_trees::types::TypeReferenceHandle,
    typed_trees::expression::StaticMachineArgument,
)> {
    plain_machine_selection(program, selected).then_some(())?;
    Some((
        parameter,
        selected_machine_result_type(program, selected.symbol)?,
        selected.clone(),
    ))
}

fn selected_machine_result_type(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    program
        .machine_parameter_signature(symbol)
        .map(|(_, signature)| signature.return_type)
        .or_else(|| {
            super::structural_terms::selected_application_machine(program, symbol)
                .and_then(|machine| program.machine_states(machine).first())
                .map(|state| state.return_type)
        })
}

fn plain_machine_selection(
    program: &dyn crate::CheckingInput,
    selection: &typed_trees::expression::StaticMachineArgument,
) -> bool {
    selection.symbol.is_valid()
        && selection.application.is_none()
        && selection.evidence_projection.is_none()
        && !selection.type_reference.is_valid()
        && selection.const_literal.is_none()
        && (program
            .machine_parameter_signature(selection.symbol)
            .is_some()
            || super::structural_terms::selected_application_machine(program, selection.symbol)
                .is_some())
}

fn structurally_substitutable_fact(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    term: &impl Fn(ExpressionHandle) -> Option<StructuralTerm>,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if super::structural_terms::is_case_observation(program, expression) {
        // Classification is not a value operand. Exact selected meaning is
        // checked at intake/judgment in the declaration's own scope.
        return term(binary.left)
            .is_some_and(|term| term_has_complete_substitution(program, &term));
    }
    if binary.operator == BinaryOperator::And {
        return structurally_substitutable_fact(program, binary.left, term)
            && structurally_substitutable_fact(program, binary.right, term);
    }
    matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && [binary.left, binary.right].iter().all(|operand| {
        term(*operand).is_some_and(|term| term_has_complete_substitution(program, &term))
    })
}

/// A structural application retains a complete declared value/static telescope.
/// An abstract machine binder is an exact callable identity, not an opaque name;
/// its invocation is compared symbolically without assuming a concrete body.
/// Receiver calls and opaque display text remain outside this judgment.
fn term_has_complete_substitution(
    program: &dyn crate::CheckingInput,
    term: &StructuralTerm,
) -> bool {
    let term = term.semantic_value();
    match term {
        StructuralTerm::ObservedValue { .. } => unreachable!("semantic_value strips observations"),
        StructuralTerm::BoundValue(_)
        | StructuralTerm::CapturedValue { .. }
        | StructuralTerm::BoundProjection { .. }
        | StructuralTerm::IndexedValue { .. }
        | StructuralTerm::MathematicalBinary { .. }
        | StructuralTerm::IntegerEmbedding { .. }
        | StructuralTerm::Opaque(_) => false,
        StructuralTerm::Application {
            target,
            selections,
            arguments,
            ..
        }
        | StructuralTerm::CallProjection {
            target,
            selections,
            arguments,
            ..
        } => {
            let signature = program
                .machine_parameter_signature(*target)
                .map(|(_, signature)| signature);
            let concrete = super::structural_terms::selected_application_machine(program, *target)
                .and_then(|machine| {
                    program
                        .machine_states(machine)
                        .first()
                        .map(|state| (machine, state))
                });
            let (parameter_count, declared_result) = if let Some(signature) = signature {
                if !selections.is_empty() {
                    return false;
                }
                (
                    program.state_signature_parameters(signature).len(),
                    signature.return_type,
                )
            } else if let Some((machine, state)) = concrete {
                let machine_parameters = program.machine_kind_type_parameters(machine).count();
                if machine.attached_data.is_some()
                    || program.machine_type_parameters(machine).len() != machine_parameters
                    || machine_parameters != selections.len()
                    || !selections
                        .iter()
                        .all(|selection| plain_machine_selection(program, selection))
                {
                    return false;
                }
                (program.state_parameters(state).len(), state.return_type)
            } else {
                return false;
            };
            if parameter_count != arguments.len() {
                return false;
            }
            if let StructuralTerm::CallProjection {
                result_type,
                field,
                field_name,
                ..
            } = term
            {
                // The fact-projection validator independently checks denotational
                // eligibility. Substitution must retain this entry's exact result
                // and field, never a namesake's value.
                if declared_result != *result_type {
                    return false;
                }
                let Some(data) =
                    crate::value_custody::places::data_definition_for_type(program, *result_type)
                else {
                    return false;
                };
                if !program.data_members(data).iter().any(|member| {
                    matches!(member, typed_trees::data::DataMember::Field(candidate)
                        if candidate.symbol == *field && candidate.name.as_str() == field_name)
                }) {
                    return false;
                }
            }
            arguments
                .iter()
                .all(|argument| term_has_complete_substitution(program, argument))
        }
        StructuralTerm::CollectionMeasure {
            collection: subject,
            ..
        }
        | StructuralTerm::Projection { subject, .. } => {
            // Term formation retains a selected field or collection measure;
            // its complete subject, not the display path alone, owns identity.
            term_has_complete_substitution(program, subject)
        }
        StructuralTerm::ScalarBinary { left, right, .. } => {
            term_has_complete_substitution(program, left)
                && term_has_complete_substitution(program, right)
        }
        StructuralTerm::Variable(_) | StructuralTerm::Integer(_) | StructuralTerm::Float(_) => true,
        StructuralTerm::Constructor { data, case, fields } => {
            constructor_fields_are_complete(program, data, case, fields)
                && fields
                    .iter()
                    .all(|(_, value)| term_has_complete_substitution(program, value))
        }
    }
}

fn constructor_fields_are_complete(
    program: &dyn crate::CheckingInput,
    data: &str,
    case: &str,
    fields: &[(String, StructuralTerm)],
) -> bool {
    use typed_trees::data::DataMember;
    if data == "bool" {
        return language_semantics::const_value::boolean_literal_spelling(case).is_some()
            && fields.is_empty();
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.name.as_str() == data);
    let Some(definition) = definitions.next() else {
        return false;
    };
    if definitions.next().is_some() {
        return false;
    }
    let mut declared = program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field.name.as_str()),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    if !case.is_empty() {
        let Some(variant) =
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case => Some(variant),
                    _ => None,
                })
        else {
            return false;
        };
        declared.extend(
            program
                .data_payload_fields(variant)
                .iter()
                .map(|field| field.name.as_str()),
        );
    }
    // Every value term must retain its complete field roster. A classifier or
    // partially reconstructed pattern cannot establish a value-call premise.
    fields.len() == declared.len()
        && declared
            .iter()
            .all(|name| fields.iter().filter(|(field, _)| field == name).count() == 1)
}

pub(super) fn fact_roots(
    program: &dyn crate::CheckingInput,
    fact: &ProofFact,
    roots: &mut Vec<ExpressionHandle>,
) {
    match fact {
        ProofFact::Expression(expression) => roots.push(*expression),
        ProofFact::Membership(membership) => roots.push(membership.value),
        ProofFact::Proposition(application) => roots.extend_from_slice(
            program
                .expression_table
                .expression_handles(application.arguments),
        ),
    }
}

/// Declaration validation also checks every requirement's formation, but it
/// cannot justify a cycle of mutually dependent contracts. Check just those
/// dependencies, not body recursion (whose well-foundedness has another owner).
fn requirements_are_acyclic(
    program: &dyn crate::CheckingInput,
    owner: &SelectedCallContract<'_>,
    active: &mut Vec<SymbolHandle>,
    complete: &mut Vec<SymbolHandle>,
) -> bool {
    let symbol = owner_symbol(owner);
    if complete.contains(&symbol) {
        return true;
    }
    if active.contains(&symbol) {
        return false;
    }
    active.push(symbol);
    let mut pending = Vec::new();
    for (fact, _) in applicable_requirements_of(program, owner) {
        fact_roots(program, fact, &mut pending);
    }
    let cyclic = crate::expression_traversal::visit_pre_order(program, pending, |_, node| {
        matches!(node, ExpressionNode::Call(call)
        if selected_call_contract(program, symbol, call).is_some_and(|nested| {
            !requirements_are_acyclic(program, &nested, active, complete)
        }))
    });
    active.pop();
    complete.push(symbol);
    !cyclic
}

fn requirement_parameters_are_bound(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    applied: &AppliedProofBindings,
) -> bool {
    !crate::expression_traversal::visit_pre_order(program, vec![expression], |_, node| {
        let ExpressionNode::Name(path) = node else {
            return false;
        };
        [path.head_symbol, path.symbol].iter().any(|symbol| {
            if let Some(parameter) = program.data_type_parameter_by_symbol(*symbol) {
                // Header binders can remain outside the member-local roster.
                // An unprojected reference still needs its exact applied operand;
                // a caller's same-spelled variable cannot complete that frame.
                return !applied.retains_parameter(parameter);
            }
            program.symbols.get(*symbol).kind == SymbolKind::Parameter
                && !parameters
                    .iter()
                    .any(|parameter| parameter.symbol == *symbol)
                && !applied
                    .constants
                    .iter()
                    .any(|binding| binding.symbol == *symbol)
        })
    })
}

/// The legacy structural term algebra uses private names. Before replacing
/// static operands there, require every same-spelled source reference to
/// resolve to the exact selected formal. Actual argument terms are substituted
/// simultaneously and are never traversed again under this receiving map.
fn structural_static_binders_are_exact(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    applied: &AppliedProofBindings,
) -> bool {
    names_are_unambiguous(program, &[expression], &[])
        && !crate::expression_traversal::visit_pre_order(program, vec![expression], |_, node| {
            let ExpressionNode::Name(path) = node else {
                return false;
            };
            let [name] = program.expression_table.name_path_members(path.members) else {
                return false;
            };
            parameters
                .iter()
                .map(|parameter| (parameter.symbol, parameter.name.as_str()))
                .chain(
                    applied
                        .constants
                        .iter()
                        .map(|binding| (binding.symbol, program.symbols.name(binding.symbol))),
                )
                .any(|(symbol, formal)| formal == name.as_str() && symbol != path.symbol)
        })
}

fn selected_machine(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
) -> Option<(&Machine, &State)> {
    machine_state_by_symbol(program, target).or_else(|| {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == target && target.is_valid())?;
        Some((machine, program.machine_states(machine).first()?))
    })
}

/// Preserve the original declaration telescope by resolved symbol. Only actual
/// integer/address parameters receive numeric atoms; structural arguments stay
/// with their own proof owners rather than becoming numeric by spelling.
fn arithmetic_caller_bindings(
    program: &dyn crate::CheckingInput,
    caller: SpecificationCaller<'_>,
) -> Vec<super::ScopedArithmeticBinding> {
    use super::{ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticValue};
    let mut bindings = Vec::new();
    for parameter in caller.parameters(program) {
        let Some(primitive) =
            crate::value_custody::recasts::exact_primitive_type(program, parameter.type_reference)
        else {
            continue;
        };
        if !primitive.accepts_integer_literal()
            || !parameter.symbol.is_valid()
            || bindings.iter().any(|binding: &ScopedArithmeticBinding| {
                binding.binder == ScopedArithmeticBinder::Symbol(parameter.symbol)
            })
        {
            continue;
        }
        bindings.push(ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
            value: ScopedArithmeticValue::Atom {
                identity: format!("\0specification:{:?}", parameter.symbol),
                unsigned: !primitive.is_signed_integer(),
            },
        });
    }
    bindings
}

fn arithmetic_requirement(
    program: &dyn crate::CheckingInput,
    caller: SpecificationCaller<'_>,
    callee: &SelectedCallContract<'_>,
    prior_facts: &[ExpressionHandle],
    requirement: ExpressionHandle,
    arguments: &[ExpressionHandle],
    applied: &AppliedProofBindings,
) -> bool {
    use super::{
        ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
        ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
    };
    let Some(parameter_scope) = owner_parameter_scope(program, callee) else {
        return false;
    };
    let parameters = parameter_scope.as_ref();
    if parameters.len() != arguments.len() {
        return false;
    }
    let bindings = arithmetic_caller_bindings(program, caller);
    let hypotheses = prior_facts
        .iter()
        .map(|expression| ScopedArithmeticHypothesis {
            proposition: ScopedArithmeticExpression {
                owner: caller.expression_owner(),
                expression: *expression,
                bindings: bindings.clone(),
            },
            holds: true,
        })
        .collect::<Vec<_>>();
    // Separate rosters keep self-call argument substitution from rewriting
    // the caller's hypotheses, even when caller and callee symbols coincide.
    // Call formation still consumes the complete selected telescope. This
    // integer proposition reads only its own exact symbol dependencies: an
    // unused structural argument is not an integer term. The shared reference
    // traversal includes static operands and type-owned expression graphs.
    let dependencies =
        typed_trees::references::collect_expression_symbol_dependencies(program, requirement);
    let mut goal_bindings = applied
        .types
        .iter()
        .map(|(symbol, reference)| ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Type(*symbol),
            value: ScopedArithmeticValue::TypeReference(*reference),
        })
        .collect::<Vec<_>>();
    for constant in &applied.constants {
        if !dependencies.contains(&constant.symbol) {
            continue;
        }
        let Some(value) = applied.constant_integer(constant.symbol) else {
            return false;
        };
        goal_bindings.push(ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Symbol(constant.symbol),
            value: ScopedArithmeticValue::Integer(value),
        });
    }
    goal_bindings.extend(
        parameters
            .iter()
            .zip(arguments)
            .filter(|(parameter, _)| dependencies.contains(&parameter.symbol))
            // An arithmetic requirement cannot interpret a record argument
            // merely because another formal is numeric. Install only integer
            // formals under the selected type application, so an applied
            // integer retains its actual operand. A goal needing an unsupported
            // subject remains unknown rather than acquiring a denotation.
            .filter(|(parameter, _)| {
                crate::value_custody::recasts::exact_primitive_type(
                    program,
                    applied.project_type(program, parameter.type_reference),
                )
                .is_some_and(|primitive| primitive.accepts_integer_literal())
            })
            .map(|(parameter, argument)| ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value: ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                    owner: caller.expression_owner(),
                    expression: *argument,
                    bindings: bindings.clone(),
                }),
            }),
    );
    let goal = ScopedArithmeticExpression {
        owner: owner_symbol(callee),
        expression: requirement,
        bindings: goal_bindings,
    };
    matches!(
        super::scoped_arithmetic::scoped_arithmetic_implication_for_declaration(
            program,
            caller.symbol(),
            &hypotheses,
            &goal
        ),
        StrictArithmeticImplicationJudgment::Proven
    )
}

fn names_are_unambiguous(
    program: &dyn crate::CheckingInput,
    hypotheses: &[ExpressionHandle],
    arguments: &[ExpressionHandle],
) -> bool {
    let pending = hypotheses
        .iter()
        .chain(arguments)
        .copied()
        .collect::<Vec<_>>();
    let mut names: Vec<(String, SymbolHandle)> = Vec::new();
    !crate::expression_traversal::visit_pre_order(program, pending, |expression, node| {
        let ExpressionNode::Name(name) = node else {
            return false;
        };
        if let Some(head) = program
            .expression_table
            .name_path_members(name.members)
            .first()
        {
            if let Some((_, symbol)) = names.iter().find(|(known, _)| known == head.as_str()) {
                if *symbol != name.head_symbol {
                    return true;
                }
            } else {
                names.push((head.as_str().to_owned(), name.head_symbol));
            }
        }
        let spelling = program.expression_table.display_name(expression);
        if let Some((_, symbol)) = names.iter().find(|(known, _)| *known == spelling) {
            *symbol != name.symbol
        } else {
            names.push((spelling, name.symbol));
            false
        }
    })
}
