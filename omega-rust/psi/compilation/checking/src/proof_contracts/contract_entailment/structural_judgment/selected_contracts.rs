//! Selected contract equations under exact formal and result substitutions.
//!
//! Executed normal returns and total specification applications share this
//! binding owner. Each caller supplies its own admitted result identity and
//! establishes the selected entry premises before importing any equation.

use super::super::specification_calls::{
    SelectedCallContract, owner_contracts, owner_name, owner_parameters, owner_return_type,
    owner_symbol, selected_call_contract, selected_call_contract_for_parameters,
    selected_contract_by_symbol,
};
use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use crate::machine_calls::contract_dependencies::independent_callee;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::TypeReferenceHandle;

/// The value a state returns. The exit is a transition value, not a trailing
/// expression statement. Distinct returned values record nothing.
fn returned_value_expression(
    program: &dyn crate::CheckingInput,
    statements: &[typed_trees::statement::StatementNode],
) -> Option<ExpressionHandle> {
    if let Some(expression) = statements
        .iter()
        .rev()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::Expression(expression) => Some(*expression),
            _ => None,
        })
    {
        return Some(expression);
    }
    for statement in statements {
        let typed_trees::statement::StatementNode::Transition(transition) = statement else {
            continue;
        };
        for target in transition.targets() {
            if let typed_trees::statement::TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(target)
            {
                return Some(*expression);
            }
        }
    }
    None
}

/// Exact selected carrier and denotational owner, without importing conclusions.
struct SelectedApplicationCarrier<'program> {
    owner: SelectedCallContract<'program>,
    parameters: &'program [typed_trees::signature::StateParameter],
    result_type: typed_trees::types::TypeReferenceHandle,
}

impl StructuralJudge<'_> {
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
        self.selected_guarantee_equations(&owner, &bindings, &bindings, application, depth)
    }

    /// A portable family guarantees only equations supplied by every member.
    /// Bind each member's formals to the same already-observed operands and
    /// result occurrence before intersecting; private declaration symbols and
    /// differing parameter names never become portable value identities.
    pub(super) fn selected_guarantee_equations(
        &self,
        owner: &SelectedCallContract<'_>,
        entry_bindings: &[(SymbolHandle, StructuralTerm)],
        bindings: &[(SymbolHandle, StructuralTerm)],
        result: &StructuralTerm,
        depth: usize,
    ) -> Option<Vec<(StructuralTerm, StructuralTerm)>> {
        let program = self.program;
        if let SelectedCallContract::Concrete(machine, state) = owner
            && machine.target_family_interface
        {
            if program.machine_states(machine).first()?.symbol != state.symbol {
                return None;
            }
            let parameters = program.state_parameters(state);
            let members = crate::machine_calls::calls::target_family_members(program, machine)?;
            let mut common: Option<Vec<(StructuralTerm, StructuralTerm)>> = None;
            for member in members {
                let machine = program.machine(member.machine)?;
                let entry = program.machine_states(machine).first()?;
                let member_parameters = program.state_parameters(entry);
                if parameters.len() != member_parameters.len() {
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
                        .collect::<Vec<_>>()
                };
                let equations = self.declared_guarantee_equations(
                    &SelectedCallContract::Concrete(machine, entry),
                    &member_bindings(entry_bindings),
                    &member_bindings(bindings),
                    result,
                    depth,
                )?;
                if let Some(common) = &mut common {
                    common.retain(|(left, right)| {
                        equations.iter().any(|(other_left, other_right)| {
                            (left == other_left && right == other_right)
                                || (left == other_right && right == other_left)
                        })
                    });
                } else {
                    common = Some(equations);
                }
            }
            return common;
        }
        self.declared_guarantee_equations(owner, entry_bindings, bindings, result, depth)
    }

    fn declared_guarantee_equations(
        &self,
        owner: &SelectedCallContract<'_>,
        entry_bindings: &[(SymbolHandle, StructuralTerm)],
        bindings: &[(SymbolHandle, StructuralTerm)],
        result: &StructuralTerm,
        depth: usize,
    ) -> Option<Vec<(StructuralTerm, StructuralTerm)>> {
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
        // A body that is a boolean literal, or a chain of single-expression
        // calls that reaches one, records that literal on the result. The
        // match and the direct use read this equation; an authored `ensures`
        // is not required for a result the body already is.
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
        Some(equations)
    }

    /// The selected entry's result when the body is exactly one boolean
    /// literal, or exactly one call whose own body is. Argument-dependent
    /// bodies are not literals and record nothing here.
    /// A boolean literal body does not depend on entry premises this judge
    /// could not see. Callers still read that recorded result.
    fn unconditional_boolean_result(
        &self,
        owner: &SelectedCallContract<'_>,
        result: &StructuralTerm,
        depth: usize,
    ) -> Option<Vec<(StructuralTerm, StructuralTerm)>> {
        let literal = self.recorded_body_boolean(owner, depth + 1)?;
        Some(vec![(result.clone(), literal)])
    }

    fn recorded_body_boolean(
        &self,
        owner: &SelectedCallContract<'_>,
        depth: usize,
    ) -> Option<StructuralTerm> {
        if depth >= 32 {
            return None;
        }
        match owner {
            SelectedCallContract::Concrete(machine, state) => {
                self.recorded_entry_boolean(machine, state, depth)
            }
            // A spelled operator or its signature view has no body. The
            // literal result is on the concrete machine entry.
            SelectedCallContract::Operator(operator) => {
                self.recorded_symbol_boolean(operator.symbol, depth)
            }
            SelectedCallContract::Signature(signature) => {
                self.recorded_symbol_boolean(signature.symbol, depth)
            }
            SelectedCallContract::DynamicSlot { .. } => None,
        }
    }

    fn recorded_symbol_boolean(
        &self,
        symbol: SymbolHandle,
        depth: usize,
    ) -> Option<StructuralTerm> {
        let machine = self
            .program
            .machine_by_symbol(symbol)
            .or_else(|| self.program.machine_holding_state(symbol))?;
        let state = self
            .program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == symbol)
            .or_else(|| self.program.machine_states(machine).first())?;
        self.recorded_entry_boolean(machine, state, depth)
    }

    fn recorded_entry_boolean(
        &self,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        depth: usize,
    ) -> Option<StructuralTerm> {
        let entry = self.program.machine_states(machine).first()?;
        if entry.symbol != state.symbol {
            return None;
        }
        let statements = typed_trees::statement::reachable_state_statements(
            self.program
                .statement_table
                .statements(state.statement_nodes),
        );
        let Some(expression) = returned_value_expression(self.program, statements) else {
            return None;
        };
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(StructuralTerm::Constructor {
                data: "bool".to_owned(),
                case: value.to_string(),
                fields: Vec::new(),
            }),
            ExpressionNode::Call(call) => {
                if let Some(callee) = selected_call_contract(self.program, call)
                    && let Some(literal) = self.recorded_body_boolean(&callee, depth + 1)
                {
                    return Some(literal);
                }
                self.recorded_symbol_boolean(call.target_symbol, depth + 1)
            }
            _ => None,
        }
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
    judge.judge_observed_predicate(required, depth)
}

fn collect_applications<'term>(
    term: &'term StructuralTerm,
    applications: &mut Vec<&'term StructuralTerm>,
) {
    let term = term.semantic_value();
    match term {
        StructuralTerm::Application { arguments, .. } => {
            for argument in arguments {
                collect_applications(argument, applications);
            }
            applications.push(term);
        }
        StructuralTerm::CollectionMeasure {
            collection: subject,
            ..
        }
        | StructuralTerm::Projection { subject, .. }
        | StructuralTerm::IntegerEmbedding { value: subject, .. } => {
            collect_applications(subject, applications)
        }
        StructuralTerm::IndexedValue {
            collection, index, ..
        } => {
            collect_applications(collection, applications);
            collect_applications(index, applications);
        }
        StructuralTerm::ScalarBinary { left, right, .. }
        | StructuralTerm::MathematicalBinary { left, right, .. } => {
            collect_applications(left, applications);
            collect_applications(right, applications);
        }
        StructuralTerm::Constructor { fields, .. } => {
            for (_, field) in fields {
                collect_applications(field, applications);
            }
        }
        StructuralTerm::CallProjection { arguments, .. } => {
            for argument in arguments {
                collect_applications(argument, applications);
            }
        }
        _ => {}
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
pub(super) fn bound_term(
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
                && program.symbols.get(path.symbol).kind != symbols::SymbolKind::Parameter
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
            let carrier =
                crate::value_custody::places::unwrapped_type_reference(program, reference)?;
            let owner = crate::value_custody::type_shapes::head_data_symbol(program, carrier)?;
            let data = program.data_definition_by_symbol(owner)?;
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
                target: owner_symbol(&selected),
                selections: call.machine_arguments.to_vec(),
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
            if !crate::has_builtin_parameter_bound_expression_meaning(
                program, scope, parameters, expression,
            ) {
                return None;
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
