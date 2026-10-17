//! Selected call preconditions are obligations of contract term formation.
//!
//! Walk operands before judging their enclosing call. Only independently
//! formed, preceding requires facts are assumptions: neither the fact being
//! formed, another ensures, nor a future body citation can license a partial
//! application. Reflexive equality and body unfolding do not waive this gate.

use super::refuted_requires::instantiated_selected_fact_judgment;
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use super::structural_terms::structural_term;
use crate::expression_traversal::children;
use crate::machine_calls::calls::machine_state_by_symbol;
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

    fn term(
        self,
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<StructuralTerm> {
        match self {
            Self::Machine(machine, Some(state)) => super::structural_terms::structural_term(
                program,
                Some((machine, state, Default::default())),
                None,
                expression,
                None,
            ),
            Self::Machine(machine, None) => super::structural_terms::structural_term(
                program,
                None,
                Some((
                    machine.symbol,
                    program
                        .machine_states(machine)
                        .first()
                        .map(|entry| program.state_parameters(entry))
                        .unwrap_or_default(),
                )),
                expression,
                None,
            ),
            Self::Declaration(symbol, parameters) => super::structural_terms::structural_term(
                program,
                None,
                Some((symbol, parameters)),
                expression,
                None,
            ),
        }
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
        let Some(selected) =
            selected_call_contract_for_parameters(program, call, owner_symbol, parameters)
        else {
            diagnostics.push(
                Diagnostic::error(format!(
                    "{owner} constant expression call has no exact declared contract owner"
                ))
                .with_source_span(program.expression_table.source_span(expression)),
            );
            continue;
        };
        let Some(expected) = owner_parameter_scope(program, &selected) else {
            continue;
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
            continue;
        }
        for (argument, parameter) in actual.iter().zip(expected.iter()) {
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
                    && crate::value_custody::expression_types::argument_matches_type_reference_with_substitutions(program, *argument, parameter.type_reference, &[])
            }, |actual| crate::value_custody::type_references::type_references_match(program, actual, parameter.type_reference));
            if !matches {
                diagnostics.push(Diagnostic::error(format!("{owner} constant expression call argument `{}` does not form parameter `{}` of `{}`",
                    program.expression_table.display_name(*argument), parameter.name, owner_name(program, &selected)))
                    .with_source_span(program.expression_table.source_span(*argument)));
            }
        }
    }
    validate_expression(
        program,
        SpecificationCaller::Declaration(owner_symbol, parameters),
        root,
        prior_facts,
        owner,
        diagnostics,
        &mut Vec::new(),
    );
}

/// Mathematical declarations reuse the same independently formed call
/// precondition judgment under their own exact parameter telescope. They have
/// no synthetic machine, entry state or operational binding environment.
pub(crate) fn validate_mathematical_call_requirements(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::mathematical::MathematicalDefinition,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parameters = crate::mathematical_applications::ordinary_parameters(program, definition);
    validate_expression(
        program,
        SpecificationCaller::Declaration(definition.symbol, &parameters),
        expression,
        &[],
        &format!("mathematical definition `{}`", definition.name),
        diagnostics,
        &mut Vec::new(),
    );
}

pub(crate) fn validate_specification_call_requirements(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    fact: &ProofFact,
    prior_facts: &[ExpressionHandle],
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut roots = Vec::new();
    fact_roots(program, fact, &mut roots);
    let mut visited = Vec::new();
    for root in roots {
        validate_expression(
            program,
            SpecificationCaller::Machine(machine, state),
            root,
            prior_facts,
            owner,
            diagnostics,
            &mut visited,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_expression(
    program: &dyn crate::CheckingInput,
    caller: SpecificationCaller<'_>,
    expression: ExpressionHandle,
    prior_facts: &[ExpressionHandle],
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    visited: &mut Vec<ExpressionHandle>,
) {
    if !expression.is_valid() || visited.contains(&expression) {
        return;
    }
    visited.push(expression);
    let node = program.expression_table.expression(expression);
    let diagnostics_before = diagnostics.len();
    children(program, node, |child| {
        validate_expression(
            program,
            caller,
            child,
            prior_facts,
            owner,
            diagnostics,
            visited,
        );
    });
    if diagnostics.len() != diagnostics_before {
        return;
    }
    let ExpressionNode::Call(call) = node else {
        return;
    };
    // Resolution, not target spelling, selects the declaration. Abstract
    // signature applications arrive through two spellings of the same owner:
    // a static requirement dispatch keeps the declaring trait's requirement
    // identity beside the rewritten realization target, and a direct
    // `Trait::requirement` citation resolves to the signature symbol itself.
    // Both enforce the requirement's own contract under the call's subjects.
    let Some(contract_owner) = selected_call_contract(program, call) else {
        return;
    };
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
    let mut arguments = Vec::new();
    let Some(parameter_scope) = owner_parameter_scope(program, &contract_owner) else {
        return;
    };
    let parameters = parameter_scope.as_ref();
    let Some(actuals) = program
        .expression_table
        .expression_handles(call.arguments)
        .get(owner_leading_operands(&contract_owner)..)
    else {
        return;
    };
    arguments.extend_from_slice(actuals);
    let machine_substitution = call_machine_substitution(program, &contract_owner, call);
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
    let judge = StructuralJudge::from_declaration_requires(
        program,
        caller.symbol(),
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
        && call.static_requirement_dispatch.is_none()
        && call.quotient_operation.is_none()
        && call.machine_arguments.is_empty()
        && named_evidence_count != 0
        && call.evidence_arguments.len() == named_evidence_count;
    for (requirement, named_evidence) in requirements {
        if named_evidence
            && has_named_evidence_lane
            && matches!(requirement, ProofFact::Proposition(_))
        {
            continue;
        }
        // Both telescopes belong to the selected public owner. A missing or
        // unsupported static operand never becomes an empty substitution.
        let arguments_select_plainly = machine_substitution.is_some()
            && (matches!(
                contract_owner,
                SelectedCallContract::Signature(..) | SelectedCallContract::DynamicSlot { .. }
            ) || call.static_requirement_dispatch.is_none());
        let proven = if let Some(required) = requirement.as_expression()
            && arguments_select_plainly
        {
            let structural = requirement_parameters_are_bound(program, *required, parameters)
                && structurally_substitutable_fact(program, *required, &|expression| {
                    structural_term(
                        program,
                        None,
                        Some((owner_symbol(&contract_owner), parameters)),
                        expression,
                        None,
                    )
                })
                && parameters.len() == arguments.len()
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
                                machine_substitution.as_deref().unwrap_or_default(),
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
    call: &TableCallExpression,
) -> Option<SelectedCallContract<'program>> {
    if let Some(dispatch) = &call.static_requirement_dispatch {
        let definition = program.trait_definition_by_symbol(dispatch.declaring_trait)?;
        return program
            .trait_machine_signature_by_symbol(definition, dispatch.contract_symbol())
            .map(SelectedCallContract::Signature);
    }
    selected_contract_by_symbol(program, call.target_symbol).or_else(|| {
        typed_trees::operator::resolve_named_expression_call(program, call)
            .map(SelectedCallContract::Operator)
    })
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
    selected_call_contract(program, call).or_else(|| {
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
    crate::selected_requirement_signature(program, symbol).map(|(_, signature)| signature)
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

/// Bind only the selected owner's machine parameters. Unsupported type/const
/// telescopes remain unavailable rather than being silently erased. Concrete
/// specialization and open modular checking use the same symbol correspondence.
fn call_machine_substitution(
    program: &dyn crate::CheckingInput,
    owner: &SelectedCallContract<'_>,
    call: &TableCallExpression,
) -> Option<
    Vec<(
        SymbolHandle,
        typed_trees::types::TypeReferenceHandle,
        typed_trees::expression::StaticMachineArgument,
    )>,
> {
    let SelectedCallContract::Concrete(machine, _) = owner else {
        return call.machine_arguments.is_empty().then(Vec::new);
    };
    let parameters = program
        .machine_kind_type_parameters(machine)
        .collect::<Vec<_>>();
    if parameters.len() != call.machine_arguments.len()
        || !call
            .machine_arguments
            .iter()
            .all(|selection| plain_machine_selection(program, selection))
    {
        return None;
    }
    parameters
        .into_iter()
        .zip(call.machine_arguments.iter())
        .map(|(parameter, selected)| {
            Some((
                parameter.symbol,
                selected_machine_result_type(program, selected.symbol)?,
                selected.clone(),
            ))
        })
        .collect()
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
        | StructuralTerm::Projection { .. }
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
        if selected_call_contract(program, call).is_some_and(|nested| {
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
) -> bool {
    !crate::expression_traversal::visit_pre_order(program, vec![expression], |_, node| {
        let ExpressionNode::Name(path) = node else {
            return false;
        };
        [path.head_symbol, path.symbol].iter().any(|symbol| {
            program.symbols.get(*symbol).kind == SymbolKind::Parameter
                && !parameters
                    .iter()
                    .any(|parameter| parameter.symbol == *symbol)
        })
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

fn arithmetic_requirement(
    program: &dyn crate::CheckingInput,
    caller: SpecificationCaller<'_>,
    callee: &SelectedCallContract<'_>,
    prior_facts: &[ExpressionHandle],
    requirement: ExpressionHandle,
    arguments: &[ExpressionHandle],
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
    let goal = ScopedArithmeticExpression {
        owner: owner_symbol(callee),
        expression: requirement,
        bindings: parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value: ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                    owner: caller.expression_owner(),
                    expression: *argument,
                    bindings: bindings.clone(),
                }),
            })
            .collect(),
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
