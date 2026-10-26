//! Selected contract equations under exact formal and result substitutions.
//!
//! Executed normal returns and total specification applications share this
//! binding owner. Each caller supplies its own admitted result identity and
//! establishes the selected entry premises before importing any equation.
//! Body-wide Boolean constants join every structurally reachable normal exit,
//! including named states and selected target bodies. A returned parameter
//! projection that is the selected requires fact records true for an admitted
//! call; that equation does not establish the premise. This is not execution or
//! caller-actual specialization: an unknown returned value supplies no equation,
//! and storage effects remain the call-frame owner's responsibility.

use super::super::specification_calls::{
    SelectedCallContract, owner_contracts, owner_name, owner_parameters, owner_return_type,
    owner_symbol, selected_call_contract, selected_call_contract_for_parameters,
    selected_call_site_contract, selected_contract_by_symbol,
};
use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use crate::machine_calls::contract_dependencies::independent_callee;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::TypeReferenceHandle;

mod metadata_paths;

/// Keep the original completed producer of an immutable Boolean copy. Only
/// exact local identities participate; parameters, projections and mutable
/// observations cannot acquire a body-wide constant through a saved name.
fn captured_boolean_producer(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    captured_values: &[(SymbolHandle, ExpressionHandle)],
) -> Option<ExpressionHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) | ExpressionNode::Call(_) => Some(expression),
        ExpressionNode::Name(path) if path.symbol == path.head_symbol => captured_values
            .iter()
            .find(|(symbol, _)| *symbol == path.symbol)
            .map(|(_, value)| *value),
        _ => None,
    }
}

/// A joined exit is either a literal or call the constant walker already
/// understands, or a projection that is itself the selected requires fact.
enum RecordedBooleanExit {
    Expression(ExpressionHandle),
    ProjectionPremise,
}

fn followed_exit(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
    captured_values: &[(SymbolHandle, ExpressionHandle)],
) -> ExpressionHandle {
    for _ in 0..32 {
        let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
            break;
        };
        if path.symbol != path.head_symbol {
            break;
        }
        let Some((_, value)) = captured_values
            .iter()
            .find(|(symbol, _)| *symbol == path.symbol)
        else {
            break;
        };
        expression = *value;
    }
    expression
}

fn term_is_parameter_projection(term: &StructuralTerm) -> bool {
    match term {
        StructuralTerm::Projection { subject, .. } => match subject.as_ref() {
            StructuralTerm::Variable(_) => true,
            nested => term_is_parameter_projection(nested),
        },
        _ => false,
    }
}

/// The exit denotes a parameter projection that is one whole requires fact of
/// this state. An admitted call has established that fact, so the returned
/// boolean is true. A different field, a saved literal, or a conjunct is not
/// this fact.
fn projection_premise_exit(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    captured_values: &[(SymbolHandle, ExpressionHandle)],
) -> bool {
    let expression = followed_exit(program, expression, captured_values);
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_)
    ) {
        return false;
    }
    let parameters = program.state_parameters(state);
    let Some(term) = super::super::structural_terms::structural_term(
        program,
        None,
        Some((state.symbol, parameters)),
        expression,
        None,
    ) else {
        return false;
    };
    if !term_is_parameter_projection(&term) {
        return false;
    }
    let owner = SelectedCallContract::Concrete(machine, state);
    owner_contracts(program, &owner)
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .any(|fact| {
            let ProofFact::Expression(required) = fact else {
                return false;
            };
            super::super::structural_terms::structural_term(
                program,
                None,
                Some((state.symbol, parameters)),
                *required,
                None,
            )
            .is_some_and(|required| required == term)
        })
}

fn captured_projection(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    captured_values: &[(SymbolHandle, ExpressionHandle)],
) -> Option<ExpressionHandle> {
    let expression = followed_exit(program, expression, captured_values);
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_)
    )
    .then_some(expression)
}

fn recorded_boolean_exit(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    captured_values: &[(SymbolHandle, ExpressionHandle)],
) -> Option<RecordedBooleanExit> {
    if projection_premise_exit(program, machine, state, expression, captured_values) {
        return Some(RecordedBooleanExit::ProjectionPremise);
    }
    captured_boolean_producer(program, expression, captured_values)
        .map(RecordedBooleanExit::Expression)
}

pub(super) struct SelectedGuarantees {
    pub(super) equations: Vec<(StructuralTerm, StructuralTerm)>,
    pub(super) content: Vec<super::super::content_conservation::ObservedContentEquation>,
}

/// Exact selected carrier and denotational owner, without importing conclusions.
struct SelectedApplicationCarrier<'program> {
    owner: SelectedCallContract<'program>,
    parameters: &'program [typed_trees::signature::StateParameter],
    result_type: typed_trees::types::TypeReferenceHandle,
}

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn captured_metadata_predicate_judgment(
        &self,
        owner: SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        expression: ExpressionHandle,
        substitution: &[(String, StructuralTerm)],
    ) -> StructuralJudgment {
        let bindings = parameters
            .iter()
            .filter_map(|parameter| {
                substitution
                    .iter()
                    .find(|(name, _)| name == parameter.name.as_str())
                    .map(|(_, value)| (parameter.symbol, value.clone()))
            })
            .collect::<Vec<_>>();
        let Some(term) = bound_term(
            self.program,
            expression,
            owner,
            parameters,
            owner,
            &bindings,
            None,
            0,
        ) else {
            return StructuralJudgment::Unknown;
        };
        fn contains_measure(term: &StructuralTerm) -> bool {
            matches!(term, StructuralTerm::CollectionMeasure { .. })
                || term.any_child(contains_measure)
        }
        if !contains_measure(&term) {
            return StructuralJudgment::Unknown;
        }
        self.judge_observed_predicate(term, 0)
    }

    /// Normalize an exact selected result or a view of a nested selected
    /// result. Equations are instantiated in the callable's own telescope;
    /// this does not execute the application or create a runtime occurrence.
    pub(super) fn selected_contract_normal_form(
        &self,
        value: &StructuralTerm,
        depth: usize,
    ) -> Option<StructuralTerm> {
        if depth >= 32 {
            return None;
        }
        let mut applications = Vec::new();
        collect_applications(value, &mut applications);
        for application in applications {
            let Some(equations) = self.specification_application_equations(application, depth + 1)
            else {
                continue;
            };
            for (left, right) in equations {
                for (pattern, replacement) in [(&left, &right), (&right, &left)] {
                    if pattern == value
                        && replacement != value
                        && !super::super::structural_terms::term_contains(replacement, value)
                    {
                        return Some(replacement.clone());
                    }
                }
            }
        }
        None
    }

    fn selected_application_carrier(
        &self,
        application: &StructuralTerm,
        depth: usize,
    ) -> Option<SelectedApplicationCarrier<'_>> {
        let StructuralTerm::Application {
            target,
            selections,
            result_type,
            arguments,
            ..
        } = application
        else {
            return None;
        };
        if depth >= 32 || !result_type.is_valid() || !selections.is_empty() {
            return None;
        }
        let program = self.program;
        let key = (self.machine_symbol, *target, *result_type, arguments.len());
        let memos = crate::frozen_program::frozen_program_memos(program);
        if let Some(memos) = memos
            && let Ok(carriers) = memos.selected_application_carriers.lock()
            && let Some(verdict) = carriers.get(&key).copied()
        {
            return self.rebuild_selected_carrier(verdict);
        }
        let verdict =
            self.admit_selected_application_carrier(*target, *result_type, arguments.len());
        if let Some(memos) = memos
            && let Ok(mut carriers) = memos.selected_application_carriers.lock()
        {
            carriers.insert(key, verdict);
        }
        self.rebuild_selected_carrier(verdict)
    }

    /// The program-fixed admission behind one selected carrier: ownership,
    /// parameter eligibility, callee independence and declared-carrier
    /// matching. Case-arm leaves re-consult the same applications, so the
    /// verdict is memoized on its (caller, target, result, arity) identity.
    fn admit_selected_application_carrier(
        &self,
        target: symbols::SymbolHandle,
        result_type: typed_trees::types::TypeReferenceHandle,
        arguments_len: usize,
    ) -> crate::frozen_program::SelectedCarrierVerdict {
        use crate::frozen_program::SelectedCarrierVerdict as Verdict;
        let program = self.program;
        let Some(owner) = selected_contract_by_symbol(program, target) else {
            return Verdict::NoCarrier;
        };
        let parameters = owner_parameters(program, &owner);
        if parameters.len() != arguments_len
            || parameters.iter().any(|parameter| {
                parameter.is_mutable
                    || crate::reference_shell(program, parameter.type_reference).is_some()
            })
        {
            return Verdict::NoCarrier;
        }
        let declared_result = match owner {
            SelectedCallContract::Concrete(machine, state) => {
                if !independent_callee(program, self.machine_symbol, machine.symbol) {
                    return Verdict::NoCarrier;
                }
                let plans = crate::operand_call_plans(program, None);
                if crate::machine_calls::denotational_calls::checked_value_call_candidate(
                    program, machine, state, &plans.0, &plans.1,
                )
                .is_err()
                {
                    return Verdict::NoCarrier;
                }
                state.return_type
            }
            SelectedCallContract::Signature(_) | SelectedCallContract::DynamicSlot { .. } => {
                // Public interface selection does not itself establish that
                // an abstract invocation has an observation-free denotation.
                return Verdict::NoCarrier;
            }
            SelectedCallContract::Operator(operator) => {
                if !crate::machine_calls::denotational_calls::selected_operator_has_closed_value_denotation(
                    program, operator,
                ) {
                    let Some((machine, state)) =
                        crate::machine_calls::denotational_calls::selected_operator_authored_body(
                            program, operator,
                        )
                    else {
                        return Verdict::NoCarrier;
                    };
                    if !independent_callee(program, self.machine_symbol, operator.symbol) {
                        return Verdict::NoCarrier;
                    }
                    let plans = crate::operand_call_plans(program, None);
                    if crate::machine_calls::denotational_calls::checked_value_call_candidate(
                        program, machine, state, &plans.0, &plans.1,
                    )
                    .is_err()
                    {
                        return Verdict::NoCarrier;
                    }
                }
                operator.return_type
            }
        };
        if !crate::declared_carriers_match(program, result_type, declared_result)
            || program.arithmetic_domain_for_type_reference(result_type)
                != program.arithmetic_domain_for_type_reference(declared_result)
        {
            return Verdict::NoCarrier;
        }
        match owner {
            SelectedCallContract::Concrete(machine, state) => {
                Verdict::Concrete(machine.symbol, state.symbol)
            }
            SelectedCallContract::Operator(operator) => Verdict::Operator(operator.symbol),
            _ => Verdict::NoCarrier,
        }
    }

    /// Rebuild a memoized carrier from the program tables its verdict names;
    /// the admitted parameter and declared-result rows come back from the
    /// same owner accessors the admission path used.
    fn rebuild_selected_carrier(
        &self,
        verdict: crate::frozen_program::SelectedCarrierVerdict,
    ) -> Option<SelectedApplicationCarrier<'_>> {
        use crate::frozen_program::SelectedCarrierVerdict as Verdict;
        let program = self.program;
        let owner = match verdict {
            Verdict::Concrete(machine_symbol, state_symbol) => {
                let machine = program.machine(machine_symbol)?;
                let state = program.state_in_machine(machine, state_symbol)?;
                SelectedCallContract::Concrete(machine, state)
            }
            Verdict::Operator(symbol) => SelectedCallContract::Operator(
                typed_trees::operator::declaration_by_symbol(program, symbol)?,
            ),
            Verdict::NoCarrier => return None,
        };
        Some(SelectedApplicationCarrier {
            parameters: owner_parameters(program, &owner),
            result_type: owner_return_type(&owner),
            owner,
        })
    }

    /// A case fact may name an admitted mathematical application, but cannot
    /// unfold its body or turn its tag into a whole-value equation. Normalize
    /// metadata from the selected declaration, never a display-name lookup.
    pub(super) fn selected_application_case_subject(
        &self,
        application: &StructuralTerm,
        depth: usize,
    ) -> Option<StructuralTerm> {
        let admitted = self.selected_application_carrier(application, depth)?;
        let StructuralTerm::Application { arguments, .. } = application else {
            return None;
        };
        Some(StructuralTerm::Application {
            result_type: admitted.result_type,
            target: owner_symbol(&admitted.owner),
            selections: Vec::new(),
            machine: owner_name(self.program, &admitted.owner),
            arguments: arguments
                .iter()
                .map(|argument| self.resolve_case_subject(argument.clone(), depth + 1))
                .collect::<Option<Vec<_>>>()?,
        })
    }

    fn specification_application_equations(
        &self,
        application: &StructuralTerm,
        depth: usize,
    ) -> Option<Vec<(StructuralTerm, StructuralTerm)>> {
        let SelectedApplicationCarrier {
            owner, parameters, ..
        } = self.selected_application_carrier(application, depth)?;
        let StructuralTerm::Application { arguments, .. } = application else {
            return None;
        };
        let bindings = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, value)| (parameter.symbol, value.clone()))
            .collect::<Vec<_>>();
        self.selected_guarantees(&owner, &bindings, &bindings, application, depth)
            .map(|guarantees| guarantees.equations)
    }

    /// A portable family guarantees only equations supplied by every member.
    /// Bind each member's formals to the same already-observed operands and
    /// result occurrence before intersecting; private declaration symbols and
    /// differing parameter names never become portable value identities.
    pub(super) fn selected_guarantees(
        &self,
        owner: &SelectedCallContract<'_>,
        entry_bindings: &[(SymbolHandle, StructuralTerm)],
        bindings: &[(SymbolHandle, StructuralTerm)],
        result: &StructuralTerm,
        depth: usize,
    ) -> Option<SelectedGuarantees> {
        let program = self.program;
        if let SelectedCallContract::Concrete(machine, state) = owner
            && machine.target_family_interface
        {
            if program.machine_states(machine).first()?.symbol != state.symbol {
                return None;
            }
            let parameters = program.state_parameters(state);
            let static_parameters = program.machine_type_parameters(machine);
            let static_binders = static_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| (parameter.symbol, format!("static{ordinal}")))
                .collect::<Vec<_>>();
            let members = crate::machine_calls::calls::target_family_members(program, machine)?;
            let mut common: Option<SelectedGuarantees> = None;
            for member in members {
                let machine = program.machine(member.machine)?;
                let entry = program.machine_states(machine).first()?;
                let member_parameters = program.state_parameters(entry);
                let member_static = program.machine_type_parameters(machine);
                if parameters.len() != member_parameters.len()
                    || static_parameters.len() != member_static.len()
                {
                    return None;
                }
                let member_binders = member_static
                    .iter()
                    .enumerate()
                    .map(|(ordinal, parameter)| (parameter.symbol, format!("static{ordinal}")))
                    .collect::<Vec<_>>();
                // The public family and each body own distinct binder symbols.
                // Preserve complete declaration order and exact const carriers
                // before transporting a known static value to a member.
                if !static_parameters.iter().zip(member_static).all(|(parameter, member)| {
                    use typed_trees::data::TypeParameterKind;
                    match (&parameter.kind, &member.kind) {
                        (TypeParameterKind::Type, TypeParameterKind::Type) => true,
                        (TypeParameterKind::Const { type_reference: parameter },
                         TypeParameterKind::Const { type_reference: member }) => {
                            let identity = |reference, binders| {
                                program.type_identity(typed_trees::type_identity::TypeIdentityRequest {
                                    binders,
                                    ..typed_trees::type_identity::TypeIdentityRequest::ordinary(reference)
                                })
                            };
                            let parameter = identity(*parameter, &static_binders);
                            parameter.is_valid() && parameter == identity(*member, &member_binders)
                        }
                        _ => false,
                    }
                }) {
                    return None;
                }
                let member_bindings = |bindings: &[(SymbolHandle, StructuralTerm)]| {
                    parameters
                        .iter()
                        .zip(member_parameters)
                        .filter_map(|(parameter, member)| {
                            bindings
                                .iter()
                                .find(|(symbol, _)| *symbol == parameter.symbol)
                                .map(|(_, value)| (member.symbol, value.clone()))
                        })
                        .chain(static_parameters.iter().zip(member_static).filter_map(
                            |(parameter, member)| {
                                bindings
                                    .iter()
                                    .find(|(symbol, _)| *symbol == parameter.symbol)
                                    .map(|(_, value)| (member.symbol, value.clone()))
                            },
                        ))
                        .collect::<Vec<_>>()
                };
                let guarantees = self.declared_guarantees(
                    &SelectedCallContract::Concrete(machine, entry),
                    &member_bindings(entry_bindings),
                    &member_bindings(bindings),
                    result,
                    depth,
                )?;
                if let Some(common) = &mut common {
                    common.equations.retain(|(left, right)| {
                        guarantees
                            .equations
                            .iter()
                            .any(|(other_left, other_right)| {
                                (left == other_left && right == other_right)
                                    || (left == other_right && right == other_left)
                            })
                    });
                    common.content.retain_mut(|law| {
                        let Some(other) = guarantees
                            .content
                            .iter()
                            .find(|other| law.same_equation(other))
                        else {
                            return false;
                        };
                        // Every portable member owes its own proved source.
                        // Equal substituted equations do not merge authorities.
                        law.retain_family_sources(other);
                        true
                    });
                } else {
                    common = Some(guarantees);
                }
            }
            return common;
        }
        self.declared_guarantees(owner, entry_bindings, bindings, result, depth)
    }

    fn declared_guarantees(
        &self,
        owner: &SelectedCallContract<'_>,
        entry_bindings: &[(SymbolHandle, StructuralTerm)],
        bindings: &[(SymbolHandle, StructuralTerm)],
        result: &StructuralTerm,
        depth: usize,
    ) -> Option<SelectedGuarantees> {
        if depth >= 32 {
            return None;
        }
        let program = self.program;
        let parameters = owner_parameters(program, owner);
        let callable = match owner {
            SelectedCallContract::Concrete(machine, _) => machine.symbol,
            _ => owner_symbol(owner),
        };
        for contract in owner_contracts(program, owner)
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let judgment = match fact {
                    ProofFact::Expression(required) => selected_requirement_judgment(
                        self,
                        *required,
                        callable,
                        parameters,
                        entry_bindings,
                        depth + 1,
                    ),
                    ProofFact::Membership(required) => self.selected_membership_requirement(
                        required,
                        callable,
                        parameters,
                        entry_bindings,
                    ),
                    ProofFact::Proposition(_) => StructuralJudgment::Unknown,
                };
                if !matches!(judgment, StructuralJudgment::Proven) {
                    if super::super::struct_trace() {
                        eprintln!(
                            "STRUCT normal-guarantee requires owner={callable:?} fact={fact:?} judgment={judgment:?} entry_bindings={entry_bindings:?}"
                        );
                    }
                    return self.unconditional_boolean_result(owner, result, depth);
                }
            }
        }
        let mut equations = Vec::new();
        for contract in owner_contracts(program, owner)
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let ProofFact::Expression(guarantee) = fact {
                    capture_guarantees(
                        program,
                        *guarantee,
                        callable,
                        parameters,
                        callable,
                        bindings,
                        result,
                        &mut equations,
                        depth + 1,
                    );
                }
            }
        }
        // A body-wide constant records its literal on the result. Both a
        // match and a direct use read this equation; an authored `ensures`
        // is not required for a result every normal exit already supplies.
        if let Some(literal) = self.recorded_body_boolean(owner, depth + 1) {
            let equation = (result.clone(), literal);
            let present = equations.iter().any(|(left, right)| {
                (left, right) == (&equation.0, &equation.1)
                    || (left, right) == (&equation.1, &equation.0)
            });
            if !present {
                equations.push(equation);
            }
        }
        // Conservation snapshots belong to executed normal returns. A pure
        // specification application has no call-entry storage epoch and must
        // not initialize normalization while forming a projection's own body.
        let content =
            if self.runtime_body_values && matches!(result, StructuralTerm::CapturedValue { .. }) {
                super::super::content_conservation::call_content_guarantees(
                    program,
                    self,
                    owner_symbol(owner),
                    parameters,
                    entry_bindings,
                    bindings,
                    result,
                )
            } else {
                Vec::new()
            };
        Some(SelectedGuarantees { equations, content })
    }

    /// A body-wide constant does not depend on entry premises this judge could
    /// not see. A returned parameter projection that is the selected requires
    /// fact is true once that call is admitted; the equation does not establish
    /// the premise. Formation still owes those premises before the call executes.
    fn unconditional_boolean_result(
        &self,
        owner: &SelectedCallContract<'_>,
        result: &StructuralTerm,
        depth: usize,
    ) -> Option<SelectedGuarantees> {
        let literal = self.recorded_body_boolean(owner, depth + 1)?;
        Some(SelectedGuarantees {
            equations: vec![(result.clone(), literal)],
            content: Vec::new(),
        })
    }

    fn recorded_body_boolean(
        &self,
        owner: &SelectedCallContract<'_>,
        depth: usize,
    ) -> Option<StructuralTerm> {
        let program = self.program;
        if depth >= 32
            || program.primitive_type_reference(owner_return_type(owner))
                != Some(typed_trees::types::PrimitiveType::Bool)
            || matches!(owner, SelectedCallContract::DynamicSlot { .. })
        {
            return None;
        }
        // Join normal exits, not the first return-looking expression. Unknown
        // destinations participate too: omitting a named state, another target
        // body or a fallthrough would turn a conditional value into a theorem.
        // This query is context independent. Cycles revisit no state and cannot
        // manufacture a constant without an actual agreeing literal exit, or a
        // projection exit that is the state's own requires fact.
        let mut pending_states = vec![owner_symbol(owner)];
        let mut visited_states = Vec::new();
        let mut pending_values: Vec<(SymbolHandle, RecordedBooleanExit)> = Vec::new();
        let mut common: Option<bool> = None;
        while !pending_states.is_empty() || !pending_values.is_empty() {
            while let Some(symbol) = pending_states.pop() {
                let machine = program
                    .machine_by_symbol(symbol)
                    .or_else(|| program.machine_holding_state(symbol))?;
                let states = program.machine_states(machine);
                let state = if symbol == machine.symbol {
                    states.first()?
                } else {
                    states.iter().find(|state| state.symbol == symbol)?
                };
                if visited_states.contains(&state.symbol) {
                    continue;
                }
                visited_states.push(state.symbol);
                if machine.target_family_interface {
                    if states.first()?.symbol != state.symbol {
                        return None;
                    }
                    for member in
                        crate::machine_calls::calls::target_family_members(program, machine)?
                    {
                        pending_states.push(member.machine);
                    }
                    continue;
                }
                if !machine.body_is_present
                    || program.primitive_type_reference(state.return_type)
                        != Some(typed_trees::types::PrimitiveType::Bool)
                {
                    return None;
                }
                let statements = program.statement_table.statements(state.statement_nodes);
                // Checked assembly may skip or reorder lexical windows. The
                // shared control owner supplies the actual finite order; an
                // unavailable order is not evidence for a constant result.
                let order = program
                    .statement_table
                    .finite_execution_order(state.statement_nodes)
                    .ok()??;
                // Normalization can name a completed call before returning
                // it. Retain exact immutable Bool producers in execution
                // order, whether authored or introduced by normalization.
                // This is not initializer replay: mutable homes and entry
                // parameters supply no producer, and a copy keeps the
                // producer captured before any later operation runs.
                let mut captured_values = Vec::new();
                for statement_index in &order {
                    let statement = &statements[*statement_index];
                    if let typed_trees::statement::StatementNode::LocalData(local) = statement
                        && !local.is_mutable
                        && program.primitive_type_reference(local.type_reference)
                            == Some(typed_trees::types::PrimitiveType::Bool)
                        && let Some(value) = captured_boolean_producer(
                            program,
                            local.initial_value,
                            &captured_values,
                        )
                        .or_else(|| {
                            captured_projection(program, local.initial_value, &captured_values)
                        })
                    {
                        captured_values.push((local.symbol, value));
                    }
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    if transition.exit != typed_trees::statement::TransitionExit::Ordinary {
                        continue;
                    }
                    for target in transition.targets() {
                        match program.statement_table.transition_target(target) {
                            typed_trees::statement::TransitionTargetNode::Value(expression) => {
                                pending_values.push((
                                    machine.symbol,
                                    recorded_boolean_exit(
                                        program,
                                        machine,
                                        state,
                                        *expression,
                                        &captured_values,
                                    )?,
                                ));
                            }
                            typed_trees::statement::TransitionTargetNode::Named {
                                path, ..
                            } => {
                                pending_states.push(path.symbol);
                            }
                            typed_trees::statement::TransitionTargetNode::Terminal => return None,
                        }
                    }
                }
                // Only the final expression is an implicit return. Earlier
                // expression statements may precede a completely different exit.
                match &statements[*order.last()?] {
                    typed_trees::statement::StatementNode::Expression(expression) => {
                        pending_values.push((
                            machine.symbol,
                            recorded_boolean_exit(
                                program,
                                machine,
                                state,
                                *expression,
                                &captured_values,
                            )?,
                        ));
                    }
                    typed_trees::statement::StatementNode::Call(call) if !call.discards_result => {
                        let selected = selected_call_site_contract(
                            program,
                            machine.symbol,
                            crate::CallSite::Statement(call),
                        )?;
                        if matches!(selected, SelectedCallContract::DynamicSlot { .. }) {
                            return None;
                        }
                        pending_states.push(owner_symbol(&selected));
                    }
                    typed_trees::statement::StatementNode::Transition(transition)
                        if transition.guard
                            == typed_trees::statement::TransitionGuardNode::Always
                            || transition.continuation.is_valid() => {}
                    _ => return None,
                }
            }
            while let Some((caller, exit)) = pending_values.pop() {
                match exit {
                    RecordedBooleanExit::ProjectionPremise => {
                        if common.is_some_and(|existing| !existing) {
                            return None;
                        }
                        common = Some(true);
                    }
                    RecordedBooleanExit::Expression(expression) => {
                        match program.expression_table.expression(expression) {
                            ExpressionNode::Boolean(value) => {
                                if common.is_some_and(|existing| existing != *value) {
                                    return None;
                                }
                                common = Some(*value);
                            }
                            ExpressionNode::Call(call) => {
                                if let Some(realization) =
                                    super::super::structural_terms::realized_requirement_state(call)
                                {
                                    pending_states.push(realization);
                                } else {
                                    let callee = selected_call_contract(program, caller, call)?;
                                    if matches!(callee, SelectedCallContract::DynamicSlot { .. }) {
                                        return None;
                                    }
                                    pending_states.push(owner_symbol(&callee));
                                }
                            }
                            _ => return None,
                        }
                    }
                }
            }
        }
        Some(StructuralTerm::Constructor {
            data: "bool".to_owned(),
            case: common?.to_string(),
            fields: Vec::new(),
        })
    }

    /// Transport an exact declared membership over its still-live subject.
    /// This does not derive membership from a domain's predicate or equate
    /// distinct instances. State entry facts are separate invocations, and a
    /// prior write retires their source observations before this query.
    fn selected_membership_requirement(
        &self,
        required: &typed_trees::domain::ProofMembershipFact,
        callable: SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        bindings: &[(SymbolHandle, StructuralTerm)],
    ) -> StructuralJudgment {
        let program = self.program;
        if !self.runtime_body_values
            || self.case_facts_only
            || !required.semantic_domain.is_valid()
            || !required.domain_symbol.is_valid()
        {
            return StructuralJudgment::Unknown;
        }
        let Some(subject) = bound_term(
            program,
            required.value,
            callable,
            parameters,
            callable,
            bindings,
            None,
            0,
        ) else {
            return StructuralJudgment::Unknown;
        };
        let Some(machine) = program.machine(self.machine_symbol) else {
            return StructuralJudgment::Unknown;
        };
        let state = if self.declaration_state.is_valid() {
            program.state_in_machine(machine, self.declaration_state)
        } else {
            program.machine_states(machine).first()
        };
        let Some(state) = state else {
            return StructuralJudgment::Unknown;
        };
        // Qualification belongs to the admitted value, including a qualified
        // field of an owned aggregate. A local annotation is not an admission.
        // Entry values cease to describe live storage after an overlapping
        // write; completed calls retain their detached occurrence identities.
        for parameter in program.state_parameters(state) {
            if self.declaration_state.is_valid()
                && !self.storage_binding_invalidated(parameter.symbol)
                && self.admitted_value_has_membership(
                    parameter.type_reference,
                    StructuralTerm::Variable(parameter.name.as_str().to_owned()),
                    required,
                    &subject,
                )
            {
                return StructuralJudgment::Proven;
            }
        }
        for (_, value) in self.normal_results.iter() {
            if let StructuralTerm::CapturedValue {
                callable,
                result_type,
                ..
            } = value
                && callable.is_valid()
                && self.admitted_value_has_membership(
                    *result_type,
                    value.clone(),
                    required,
                    &subject,
                )
            {
                return StructuralJudgment::Proven;
            }
        }
        if self.normal_memberships.iter().any(|guarantee| {
            guarantee.domain == required.domain_symbol
                && guarantee.semantic_domain == required.semantic_domain
                && guarantee.subject == subject
        }) {
            return StructuralJudgment::Proven;
        }
        // Only a state-prefix judge authenticates entry admission and replays
        // subsequent writes. A prefix-free runtime judge has no invalidations,
        // but that absence is not evidence that entry membership survived.
        if !self.declaration_state.is_valid() {
            return StructuralJudgment::Unknown;
        }
        for contract in owner_contracts(program, &SelectedCallContract::Concrete(machine, state))
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Membership(premise) = fact else {
                    continue;
                };
                if premise.domain_symbol != required.domain_symbol
                    || premise.semantic_domain != required.semantic_domain
                {
                    continue;
                }
                let mut expressions = Vec::new();
                crate::expression_traversal::collect_expression_tree(
                    program,
                    premise.value,
                    &mut expressions,
                );
                if expressions.iter().any(|expression| {
                    crate::value_custody::places::expression_root_symbol(
                        *expression,
                        &program.expression_table,
                        self.machine_symbol,
                    )
                    .is_some_and(|symbol| self.storage_binding_invalidated(symbol))
                }) {
                    continue;
                }
                if self.declaration_term(premise.value).as_ref() == Some(&subject) {
                    return StructuralJudgment::Proven;
                }
            }
        }
        StructuralJudgment::Unknown
    }

    /// An exact declaration row on an independently admitted value transports
    /// membership, not merely the domain's predicate. Conditional case rows
    /// require the observed constructor. Reference endpoints also require an
    /// authenticated referent: projecting a descriptor does not observe it.
    fn admitted_value_has_membership(
        &self,
        reference: TypeReferenceHandle,
        value: StructuralTerm,
        required: &typed_trees::domain::ProofMembershipFact,
        subject: &StructuralTerm,
    ) -> bool {
        crate::field_domains::declared_type_domain_rows(self.program, reference)
            .iter()
            .filter(|row| {
                row.domain_symbol == required.domain_symbol
                    && row.semantic_domain == required.semantic_domain
            })
            .any(|row| {
                let Some(projected) = super::super::observed_projections::project_observed_value(
                    self.program,
                    reference,
                    value.clone(),
                    &row.path,
                ) else {
                    return false;
                };
                let Some(projected_type) =
                    crate::project_type_reference_from_segments(self.program, reference, &row.path)
                else {
                    return false;
                };
                if let Some(shell) = crate::reference_shell(self.program, projected_type)
                    && (!shell.access.is_readable()
                        || !super::super::structural_terms::scalar_operand_is_observed(
                            self.program,
                            projected_type,
                            &projected,
                        ))
                {
                    return false;
                }
                &projected == subject
            })
    }
}

/// Discharge a selected entry predicate in its own formal scope. Boolean
/// conjunction combines judgments, rather than asking value equality to fold
/// a formula. Case membership keeps its tag-only authority; scalar leaves use
/// the same exact-symbol substitution as selected normal guarantees.
pub(super) fn selected_requirement_judgment(
    judge: &StructuralJudge<'_>,
    expression: ExpressionHandle,
    callable: SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    bindings: &[(SymbolHandle, StructuralTerm)],
    depth: usize,
) -> StructuralJudgment {
    if depth >= 32 {
        return StructuralJudgment::Unknown;
    }
    let program = judge.program;
    if let Some(binary) = program.expression_table.binary_node(expression)
        && binary.operator == BinaryOperator::And
    {
        // The retained conjunction is not overloadable. Its operands keep
        // their own selected predicate meanings; a nominal leaf must not
        // prevent a neighboring scalar leaf from reaching its judge.
        return match (
            selected_requirement_judgment(
                judge,
                binary.left,
                callable,
                parameters,
                bindings,
                depth + 1,
            ),
            selected_requirement_judgment(
                judge,
                binary.right,
                callable,
                parameters,
                bindings,
                depth + 1,
            ),
        ) {
            (StructuralJudgment::Refuted, _) | (_, StructuralJudgment::Refuted) => {
                StructuralJudgment::Refuted
            }
            (StructuralJudgment::Proven, StructuralJudgment::Proven) => StructuralJudgment::Proven,
            _ => StructuralJudgment::Unknown,
        };
    }
    if super::super::structural_terms::is_case_observation(program, expression) {
        let Some(map) = parameters
            .iter()
            .map(|parameter| {
                let (_, value) = bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == parameter.symbol)?;
                Some((parameter.name.as_str().to_owned(), value.clone()))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return StructuralJudgment::Unknown;
        };
        return judge.instantiated_case_judgment(callable, parameters, expression, &map);
    }
    let Some(required) = bound_term(
        program,
        expression,
        callable,
        parameters,
        callable,
        bindings,
        None,
        depth + 1,
    ) else {
        return StructuralJudgment::Unknown;
    };
    let judgment = judge.judge_observed_predicate(required.clone(), depth);
    if super::super::struct_trace() && !matches!(judgment, StructuralJudgment::Proven) {
        eprintln!(
            "STRUCT selected-requirement owner={callable:?} expression={expression:?} predicate={required:?} judgment={judgment:?}"
        );
    }
    judgment
}

fn collect_applications<'term>(
    term: &'term StructuralTerm,
    applications: &mut Vec<&'term StructuralTerm>,
) {
    let term = term.semantic_value();
    term.for_each_child(|child| collect_applications(child, applications));
    if matches!(term, StructuralTerm::Application { .. }) {
        applications.push(term);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture_guarantees(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    scope: SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    callable: SymbolHandle,
    bindings: &[(SymbolHandle, StructuralTerm)],
    result: &StructuralTerm,
    equations: &mut Vec<(StructuralTerm, StructuralTerm)>,
    depth: usize,
) {
    if depth >= 64 {
        return;
    }
    let binary = program.expression_table.binary_node(expression);
    if let Some(binary) = binary
        && binary.operator == BinaryOperator::And
    {
        // Decompose the compiler-owned conjunction. Each leaf retains its
        // selected meaning; a tag guarantee grants no scalar identity and
        // cannot suppress a neighboring predicate.
        capture_guarantees(
            program,
            binary.left,
            scope,
            parameters,
            callable,
            bindings,
            result,
            equations,
            depth + 1,
        );
        capture_guarantees(
            program,
            binary.right,
            scope,
            parameters,
            callable,
            bindings,
            result,
            equations,
            depth + 1,
        );
        return;
    }
    if let Some(binary) = binary
        && binary.operator == BinaryOperator::Equal
        && super::super::structural_terms::has_identity_equality_meaning(
            program, scope, parameters, expression,
        )
        && let (Some(left), Some(right)) = (
            bound_term(
                program,
                binary.left,
                scope,
                parameters,
                callable,
                bindings,
                Some(result),
                0,
            ),
            bound_term(
                program,
                binary.right,
                scope,
                parameters,
                callable,
                bindings,
                Some(result),
                0,
            ),
        )
    {
        let references = [binary.left, binary.right].map(|operand| {
            crate::parameter_expression_result_type_reference(program, scope, parameters, operand)
        });
        if let Some(references) =
            crate::value_custody::expression_types::builtin_scalar_operand_references(
                program, binary, references, None,
            )
            && super::super::structural_terms::scalar_operand_is_observed(
                program,
                references[0],
                &left,
            )
            && super::super::structural_terms::scalar_operand_is_observed(
                program,
                references[1],
                &right,
            )
        {
            equations.push((left, right));
            return;
        }
    }
    let Some(predicate) = bound_term(
        program,
        expression,
        scope,
        parameters,
        callable,
        bindings,
        Some(result),
        depth + 1,
    ) else {
        return;
    };
    let boolean =
        crate::parameter_expression_result_type_reference(program, scope, parameters, expression)
            .and_then(|reference| program.primitive_type_reference(reference))
            == Some(typed_trees::types::PrimitiveType::Bool);
    if boolean || predicate.is_mathematical_predicate() {
        // Normal return establishes this exact public predicate. Keeping its
        // truth does not turn IEEE equality into identity or select an arm of
        // a disjunction. Each later consumer authenticates the selected meaning.
        equations.push((predicate, super::value_predicates::boolean_term(true)));
    }
}

/// Formal substitution uses selected symbols. Names are only the existing
/// structural judge's diagnostic payload; no caller spelling chooses a formal.
#[allow(clippy::too_many_arguments)]
pub(in crate::proof_contracts::contract_entailment) fn bound_term(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    scope: SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    callable: SymbolHandle,
    bindings: &[(SymbolHandle, StructuralTerm)],
    result: Option<&StructuralTerm>,
    depth: usize,
) -> Option<StructuralTerm> {
    if depth >= 64 {
        return None;
    }
    if let Some(value) = super::super::proof_integer::anonymous_integer_value(program, expression) {
        return Some(StructuralTerm::Integer(value));
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if let Some((_, value)) = bindings
                .iter()
                .find(|(symbol, _)| symbol.is_valid() && *symbol == path.symbol)
            {
                return Some(value.clone());
            }
            if crate::reserved_result_owner(program, expression)
                .is_some_and(|owner| owner.0 == callable)
            {
                return result.cloned();
            }
            if path.symbol.is_valid()
                && !matches!(
                    program.symbols.get(path.symbol).kind,
                    symbols::SymbolKind::Parameter | symbols::SymbolKind::TypeParameter
                )
            {
                return Some(StructuralTerm::BoundValue(path.symbol));
            }
            None
        }
        ExpressionNode::Member(member) => {
            let reference = crate::parameter_expression_result_type_reference(
                program,
                scope,
                parameters,
                member.receiver,
            )?;
            if let Some(measure) = super::super::structural_terms::collection_measure_term(
                program,
                member,
                reference,
                || {
                    bound_term(
                        program,
                        member.receiver,
                        scope,
                        parameters,
                        callable,
                        bindings,
                        result,
                        depth + 1,
                    )
                },
            ) {
                return Some(measure);
            }
            if let Some(measure) = super::super::structural_terms::collection_measure_term(
                program,
                member,
                reference,
                || {
                    metadata_paths::bound_descriptor(
                        program,
                        member.receiver,
                        scope,
                        parameters,
                        callable,
                        bindings,
                        result,
                        depth + 1,
                    )
                },
            ) {
                return Some(measure);
            }
            let carrier =
                crate::value_custody::places::unwrapped_type_reference(program, reference)?;
            // The applied contract's fields belong to its selected data
            // application, not a same-named field on the generic base.
            let data = crate::value_custody::places::data_definition_for_type(program, carrier)?;
            let field = crate::value_custody::places::exact_data_member_field(
                program,
                data,
                member.member_symbol,
                member.member.as_str(),
                member.case_variant.as_ref().map(|case| case.as_str()),
            )?;
            let value = bound_term(
                program,
                member.receiver,
                scope,
                parameters,
                callable,
                bindings,
                result,
                depth + 1,
            )?;
            let mut path = Vec::new();
            crate::value_custody::places::push_field_place_segments(
                program,
                &mut path,
                field.symbol,
            );
            super::super::observed_projections::project_observed_value(
                program, reference, value, &path,
            )
        }
        ExpressionNode::Indexed(indexed) => super::super::structural_terms::indexed_term_in_scope(
            program,
            None,
            Some((scope, parameters)),
            expression,
            bound_term(
                program,
                indexed.collection,
                scope,
                parameters,
                callable,
                bindings,
                result,
                depth + 1,
            )?,
            bound_term(
                program,
                indexed.index,
                scope,
                parameters,
                callable,
                bindings,
                result,
                depth + 1,
            )?,
        ),
        ExpressionNode::Call(call) => {
            if let Some((_, argument)) = crate::integer_embedding_argument(program, expression) {
                let reference = crate::parameter_expression_result_type_reference(
                    program, scope, parameters, argument,
                )?;
                let value = bound_term(
                    program,
                    argument,
                    scope,
                    parameters,
                    callable,
                    bindings,
                    result,
                    depth + 1,
                )?;
                return super::super::structural_terms::integer_embedding_term(
                    program, expression, reference, value,
                );
            }
            let selected = selected_call_contract_for_parameters(program, call, scope, parameters)?;
            let realization = super::super::structural_terms::realized_requirement_state(call);
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| {
                    bound_term(
                        program,
                        *argument,
                        scope,
                        parameters,
                        callable,
                        bindings,
                        result,
                        depth + 1,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            let result_type = crate::parameter_expression_result_type_reference(
                program, scope, parameters, expression,
            )
            .unwrap_or_default();
            Some(StructuralTerm::Application {
                result_type,
                target: realization.unwrap_or_else(|| owner_symbol(&selected)),
                selections: if realization.is_some() {
                    Vec::new()
                } else {
                    call.machine_arguments.to_vec()
                },
                machine: super::super::structural_terms::structural_call_machine_name(
                    call.target.as_str(),
                    &call.machine_arguments,
                    &[],
                ),
                arguments,
            })
        }
        ExpressionNode::StructLiteral(literal) => {
            super::super::structural_terms::constructor_literal_term(
                program,
                literal,
                |_, field| {
                    let field = field?;
                    bound_term(
                        program,
                        field.value,
                        scope,
                        parameters,
                        callable,
                        bindings,
                        result,
                        depth + 1,
                    )
                },
            )
        }
        ExpressionNode::Unary(unary) => super::super::structural_terms::logical_not_term_in_scope(
            program,
            None,
            Some((scope, parameters)),
            expression,
            bound_term(
                program,
                unary.operand,
                scope,
                parameters,
                callable,
                bindings,
                result,
                depth + 1,
            )?,
        ),
        // Public guarantees observe the admitted conversion's value through
        // the same denotation rule as ordinary contract expressions. Conversion
        // formation remains a separate obligation at its authored occurrence.
        ExpressionNode::Cast(cast) => {
            let source = crate::parameter_expression_result_type_reference(
                program, scope, parameters, cast.value,
            )?;
            super::super::structural_terms::denotation_preserving_conversion(program, cast, source)
                .then(|| {
                    bound_term(
                        program,
                        cast.value,
                        scope,
                        parameters,
                        callable,
                        bindings,
                        result,
                        depth + 1,
                    )
                })
                .flatten()
        }
        ExpressionNode::Integer(_) | ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => {
            super::super::structural_terms::structural_term(program, None, None, expression, None)
        }
        ExpressionNode::Binary(binary) => {
            // A sealed primitive float comparison is an IEEE Boolean, not
            // identity and not a reason to drop the substituted operands.
            // Other non-builtin operators stay unbound here.
            if !crate::has_builtin_parameter_bound_expression_meaning(
                program, scope, parameters, expression,
            ) {
                let types = [binary.left, binary.right].map(|operand| {
                    crate::parameter_expression_result_type_reference(
                        program, scope, parameters, operand,
                    )
                });
                super::super::structural_terms::sealed_primitive_float_operator(
                    program, scope, expression, &types,
                )?;
            }
            let left = bound_term(
                program,
                binary.left,
                scope,
                parameters,
                callable,
                bindings,
                result,
                depth + 1,
            )?;
            let right = bound_term(
                program,
                binary.right,
                scope,
                parameters,
                callable,
                bindings,
                result,
                depth + 1,
            )?;
            super::super::structural_terms::binary_term_in_scope(
                program,
                None,
                Some((scope, parameters)),
                expression,
                left,
                right,
            )
        }
        _ => None,
    }
}
