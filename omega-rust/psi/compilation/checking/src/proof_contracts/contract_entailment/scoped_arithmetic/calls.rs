//! Published result guarantees under a call occurrence's simultaneous roster.
//!
//! Specification call formation has already discharged the selected requires.
//! This transport reads only independently checked published ensures, never a
//! body, a precondition, or the contract currently being established. A result
//! has a private occurrence identity: equal arguments do not equate independent
//! effectful calls. Actuals retain their caller roster before formal binding.
//! Bounded expansion is optional proof strength, not a second formation route.

use super::super::specification_calls::{
    SelectedCallContract, fact_roots, owner_contracts, owner_parameters, owner_symbol,
    selected_call_contract, selected_call_site_contract,
};
use super::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, ScopedCallCompletion,
};
use crate::expression_traversal::children;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;

mod collections;

/// Expression handles name value occurrences; statement calls additionally
/// need their exact state/ordinal because no expression result identifies them.
#[derive(Clone, Copy)]
enum CallOccurrence {
    Expression(ExpressionHandle),
    Statement { state: SymbolHandle, ordinal: usize },
}

impl CallOccurrence {
    fn post_atom(
        self,
        context: SymbolHandle,
        scope: usize,
        owner: SymbolHandle,
        formal: SymbolHandle,
    ) -> String {
        match self {
            Self::Expression(expression) => {
                format!("\0scoped-call-formal:{scope}:{expression:?}:{owner:?}:{formal:?}")
            }
            Self::Statement { state, ordinal } => format!(
                "\0scoped-statement-formal:{scope}:{context:?}:{state:?}:{ordinal}:{owner:?}:{formal:?}"
            ),
        }
    }
}

#[derive(Clone)]
pub(in crate::proof_contracts::contract_entailment) struct ScopedCalls<'program> {
    program: &'program dyn crate::CheckingInput,
    context: SymbolHandle,
    remaining: usize,
    depth: usize,
    scopes: Vec<(ScopedArithmeticExpression, ScopedArithmeticExpression)>,
    guarantees: Vec<ScopedArithmeticHypothesis>,
    lengths: Vec<(ScopedArithmeticExpression, ScopedArithmeticValue)>,
}

impl<'program> ScopedCalls<'program> {
    pub(super) fn new(program: &'program dyn crate::CheckingInput, context: &Machine) -> Self {
        Self::for_declaration(program, context.symbol)
    }

    pub(in crate::proof_contracts::contract_entailment) fn for_declaration(
        program: &'program dyn crate::CheckingInput,
        context: SymbolHandle,
    ) -> Self {
        Self {
            program,
            context,
            remaining: 4096,
            depth: 0,
            scopes: Vec::new(),
            guarantees: Vec::new(),
            lengths: Vec::new(),
        }
    }

    fn step(&mut self) -> Option<()> {
        self.remaining = self.remaining.checked_sub(1)?;
        Some(())
    }

    pub(super) fn value(&mut self, value: &ScopedArithmeticValue) -> Option<ScopedArithmeticValue> {
        self.step()?;
        Some(match value {
            ScopedArithmeticValue::CollectionLength(term) => self.collection_length(term)?,
            ScopedArithmeticValue::Term(term) => {
                ScopedArithmeticValue::Term(self.expression(term)?)
            }
            ScopedArithmeticValue::AdmittedIntegerRemainder(term) => {
                ScopedArithmeticValue::AdmittedIntegerRemainder(self.expression(term)?)
            }
            ScopedArithmeticValue::IntegerOperation {
                operator,
                left,
                right,
            } => ScopedArithmeticValue::IntegerOperation {
                operator: *operator,
                left: Box::new(self.value(left)?),
                right: Box::new(self.value(right)?),
            },
            ScopedArithmeticValue::BoundedInteger {
                value,
                minimum,
                maximum,
            } => ScopedArithmeticValue::BoundedInteger {
                value: Box::new(self.value(value)?),
                minimum: minimum.clone(),
                maximum: maximum.clone(),
            },
            _ => value.clone(),
        })
    }

    pub(in crate::proof_contracts::contract_entailment) fn hypotheses(
        &mut self,
        hypotheses: &[ScopedArithmeticHypothesis],
    ) -> Option<Vec<ScopedArithmeticHypothesis>> {
        let mut prepared = Vec::with_capacity(hypotheses.len());
        for hypothesis in hypotheses {
            prepared.push(ScopedArithmeticHypothesis {
                proposition: self.expression(&hypothesis.proposition)?,
                holds: hypothesis.holds,
            });
        }
        // Transported guarantees are optional premises. An unread argument
        // cannot invalidate the caller's unrelated, fully bound hypotheses.
        if let Some(machine) = self.program.machine_by_symbol(self.context) {
            let mut engine = super::Engine::strict(self.program, machine, &[]);
            prepared.extend(
                self.guarantees
                    .drain(..)
                    .filter(|hypothesis| engine.install_scoped_expression(&hypothesis.proposition)),
            );
        }
        Some(prepared)
    }

    pub(in crate::proof_contracts::contract_entailment) fn expression(
        &mut self,
        source: &ScopedArithmeticExpression,
    ) -> Option<ScopedArithmeticExpression> {
        if self.depth >= 64 {
            return None;
        }
        self.depth += 1;
        let result = self.expression_inner(source);
        self.depth -= 1;
        result
    }

    fn expression_inner(
        &mut self,
        source: &ScopedArithmeticExpression,
    ) -> Option<ScopedArithmeticExpression> {
        if let Some((_, prepared)) = self.scopes.iter().find(|(original, _)| original == source) {
            return Some(prepared.clone());
        }
        self.step()?;
        if !self
            .program
            .expression_table
            .expression_is_valid(source.expression)
        {
            return None;
        }
        let mut prepared = source.clone();
        for binding in &mut prepared.bindings {
            binding.value = self.value(&binding.value)?;
        }
        let scope = self.scopes.len();
        // Reserve the scope identity before processing nested guarantees.
        self.scopes.push((source.clone(), prepared.clone()));
        let mut occurrences = Vec::new();
        self.postorder(source.expression, &mut occurrences)?;
        for expression in occurrences {
            if prepared
                .bindings
                .iter()
                .any(|binding| binding.binder == ScopedArithmeticBinder::Occurrence(expression))
            {
                continue;
            }
            let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
            else {
                continue;
            };
            if crate::integer_embedding_argument(self.program, expression).is_some() {
                continue;
            }
            let Some(owner) = selected_call_contract(self.program, call) else {
                continue;
            };
            // A selected dynamic slot is a runtime occurrence. Its public
            // guarantees belong to the executed normal-return path, not a
            // replayable mathematical application of selector + operands.
            if matches!(owner, SelectedCallContract::DynamicSlot { .. }) {
                continue;
            }
            let result_type = super::super::specification_calls::owner_return_type(&owner);
            let Some(primitive) = self
                .program
                .primitive_type_reference(result_type)
                .filter(|primitive| primitive.is_fixed_integer())
            else {
                continue;
            };
            let independent = self
                .independent(owner, &mut Vec::new(), &mut Vec::new())
                .unwrap_or(false)
                && crate::machine_calls::contract_dependencies::independent_call_application(
                    self.program,
                    self.context,
                    owner_symbol(&owner),
                    typed_trees::references::StaticArgumentCall::Expression(expression, call),
                );
            let interval = if independent {
                crate::proof_contracts::arithmetic_domains::call_result_interval(
                    self.program,
                    self.context,
                    expression,
                    call,
                )
            } else {
                None
            }
            .or_else(|| crate::proof_contracts::arithmetic_domains::primitive_range(primitive));
            let atom = ScopedArithmeticValue::Atom {
                identity: format!(
                    "\0scoped-call:{scope}:{expression:?}:{:?}",
                    owner_symbol(&owner)
                ),
                unsigned: matches!(
                    primitive,
                    typed_trees::unsigned_integer_primitive_pattern!()
                ),
            };
            let result = ScopedArithmeticValue::BoundedInteger {
                value: Box::new(atom),
                minimum: interval
                    .and_then(|interval| interval.low())
                    .map(numerics::bignum::BigInt::from_i64),
                maximum: interval
                    .and_then(|interval| interval.high())
                    .map(numerics::bignum::BigInt::from_i64),
            };
            let parameters = owner_parameters(self.program, &owner);
            let arguments = self
                .program
                .expression_table
                .expression_handles(call.arguments);
            if independent && parameters.len() == arguments.len() {
                let bindings = self.completed_bindings(
                    owner,
                    arguments,
                    CallOccurrence::Expression(expression),
                    &prepared.bindings,
                    Some(result.clone()),
                    scope,
                )?;
                self.transport_guarantees(owner, &bindings, None)?;
            }
            prepared.bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Occurrence(expression),
                value: result,
            });
        }
        self.scopes[scope].1 = prepared.clone();
        Some(prepared)
    }

    /// Bind one completed result through its selected public contract. The
    /// caller supplies captured actuals; this owner supplies return-phase
    /// formal identities, contract independence and arithmetic adaptation.
    pub(super) fn completed_guarantees(
        &mut self,
        machine: &Machine,
        completion: ScopedCallCompletion<'_>,
    ) -> Option<Vec<ScopedArithmeticHypothesis>> {
        let ScopedCallCompletion {
            state,
            statement_index,
            site,
            actuals,
            machine_values,
            result,
        } = completion;
        let occurrence = match site {
            crate::CallSite::Expression { expression, call } => {
                if self.program.expression_table.call_node(expression)? != call
                    || crate::integer_embedding_argument(self.program, expression).is_some()
                    || result.is_none()
                {
                    return None;
                }
                CallOccurrence::Expression(expression)
            }
            crate::CallSite::Statement(call) => {
                if self
                    .program
                    .statement_table
                    .statement_at(state, statement_index)?
                    .as_call()?
                    != call
                    || result.is_some()
                {
                    return None;
                }
                CallOccurrence::Statement {
                    state: state.symbol,
                    ordinal: statement_index,
                }
            }
            crate::CallSite::TransitionNamed { .. } => return None,
        };
        let selected_call = match site {
            crate::CallSite::Statement(call) => {
                typed_trees::references::StaticArgumentCall::Statement(call)
            }
            crate::CallSite::Expression { expression, call } => {
                typed_trees::references::StaticArgumentCall::Expression(expression, call)
            }
            crate::CallSite::TransitionNamed { .. } => return None,
        };
        let owner = selected_call_site_contract(self.program, site)?;
        let arguments = crate::call_site_argument_expressions(self.program, &site);
        if matches!(owner, SelectedCallContract::DynamicSlot { .. })
            || !self.independent(owner, &mut Vec::new(), &mut Vec::new())?
            || !crate::machine_calls::contract_dependencies::independent_call_application(
                self.program,
                self.context,
                owner_symbol(&owner),
                selected_call,
            )
            || owner_parameters(self.program, &owner).len() != arguments.len()
        {
            return None;
        }
        if let crate::CallSite::Expression { expression, .. } = site {
            let declared = super::super::specification_calls::owner_return_type(&owner);
            let observed =
                crate::expression_result_type_reference(self.program, machine, state, expression)?;
            if !self
                .program
                .primitive_type_reference(declared)?
                .is_fixed_integer()
                || !crate::declared_carriers_match(self.program, observed, declared)
                || self.program.arithmetic_domain_for_type_reference(observed)
                    != self.program.arithmetic_domain_for_type_reference(declared)
            {
                return None;
            }
        }
        let mut bindings =
            self.completed_bindings(owner, arguments, occurrence, actuals, result, 0)?;
        for (symbol, value) in machine_values {
            self.step()?;
            if bindings
                .iter()
                .any(|binding| binding.binder == ScopedArithmeticBinder::Symbol(*symbol))
            {
                return None;
            }
            bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(*symbol),
                value: self.value(value)?,
            });
        }
        self.transport_guarantees(owner, &bindings, None)?;
        self.hypotheses(&[])
    }

    fn completed_bindings(
        &mut self,
        owner: SelectedCallContract<'program>,
        arguments: &[ExpressionHandle],
        occurrence: CallOccurrence,
        actuals: &[ScopedArithmeticBinding],
        result: Option<ScopedArithmeticValue>,
        scope: usize,
    ) -> Option<Vec<ScopedArithmeticBinding>> {
        let mut bindings = Vec::new();
        if let Some(value) = result {
            bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Result,
                value,
            });
        }
        let program = self.program;
        for (parameter, argument) in owner_parameters(program, &owner).iter().zip(arguments) {
            self.step()?;
            // An owned mutable formal names the callee's post-state value.
            // Its incoming argument is a different observation, even when the
            // actual is a literal. Immutable descriptor observations remain
            // available to the shared collection-measure binder.
            let captured = (!parameter.is_mutable)
                .then(|| {
                    actuals
                        .iter()
                        .find(|binding| {
                            binding.binder == ScopedArithmeticBinder::Occurrence(*argument)
                        })
                        .map(|binding| binding.value.clone())
                })
                .flatten();
            let value = if let Some(value) = captured {
                value
            } else {
                let Some(primitive) = program
                    .primitive_type_reference(parameter.type_reference)
                    .filter(|primitive| primitive.is_fixed_integer())
                else {
                    continue;
                };
                ScopedArithmeticValue::Atom {
                    identity: occurrence.post_atom(
                        self.context,
                        scope,
                        owner_symbol(&owner),
                        parameter.symbol,
                    ),
                    unsigned: matches!(
                        primitive,
                        typed_trees::unsigned_integer_primitive_pattern!()
                    ),
                }
            };
            bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value,
            });
        }
        Some(bindings)
    }

    fn transport_guarantees(
        &mut self,
        owner: SelectedCallContract<'program>,
        bindings: &[ScopedArithmeticBinding],
        result_length: Option<&ScopedArithmeticValue>,
    ) -> Option<()> {
        let parameters = owner_parameters(self.program, &owner);
        let guarantees: Vec<_> = owner_contracts(self.program, &owner)
            .filter(|contract| {
                contract.kind == SignatureContractKind::Ensures && contract.binding.is_none()
            })
            .flat_map(|contract| self.program.proof_facts.span_or_empty(contract.facts))
            .filter_map(|fact| match fact {
                ProofFact::Expression(expression) => Some(*expression),
                _ => None,
            })
            .collect();
        let mut pending = guarantees;
        while let Some(guarantee) = pending.pop() {
            self.step()?;
            // Only the authenticated conjunction grants each independent
            // conjunct. An unsupported neighboring subject need not erase a
            // usable guarantee about the completed result.
            if let Some(binary) = self.program.expression_table.binary_node(guarantee)
                && binary.operator == typed_trees::expression::BinaryOperator::And
                && match owner {
                    SelectedCallContract::Concrete(machine, state) => {
                        crate::has_builtin_decomposed_guard_meaning(
                            self.program,
                            machine,
                            Some(state),
                            guarantee,
                        )
                    }
                    SelectedCallContract::Signature(signature)
                    | SelectedCallContract::DynamicSlot { signature, .. } => {
                        crate::has_builtin_parameter_bound_expression_meaning(
                            self.program,
                            signature.symbol,
                            parameters,
                            guarantee,
                        )
                    }
                    SelectedCallContract::Operator(operator) => {
                        crate::has_builtin_parameter_bound_expression_meaning(
                            self.program,
                            operator.symbol,
                            parameters,
                            guarantee,
                        )
                    }
                }
            {
                pending.push(binary.right);
                pending.push(binary.left);
                continue;
            }
            let builtin = match owner {
                SelectedCallContract::Concrete(machine, state) => {
                    crate::has_builtin_bound_expression_meaning(
                        self.program,
                        machine,
                        Some(state),
                        guarantee,
                    )
                }
                SelectedCallContract::Signature(signature)
                | SelectedCallContract::DynamicSlot { signature, .. } => {
                    crate::has_builtin_parameter_bound_expression_meaning(
                        self.program,
                        signature.symbol,
                        parameters,
                        guarantee,
                    )
                }
                SelectedCallContract::Operator(operator) => {
                    crate::has_builtin_parameter_bound_expression_meaning(
                        self.program,
                        operator.symbol,
                        parameters,
                        guarantee,
                    )
                }
            };
            if builtin {
                if result_length.is_none()
                    && !bindings
                        .iter()
                        .any(|binding| binding.binder == ScopedArithmeticBinder::Result)
                {
                    let mut occurrences = Vec::new();
                    self.postorder(guarantee, &mut occurrences)?;
                    if occurrences.iter().any(|expression| {
                        self.program
                            .expression_table
                            .names_reserved_result(*expression)
                    }) {
                        continue;
                    }
                }
                let mut roster = bindings.to_vec();
                self.bind_length_projections(owner, guarantee, result_length, &mut roster)?;
                let proposition = self.expression(&ScopedArithmeticExpression {
                    owner: owner_symbol(&owner),
                    expression: guarantee,
                    bindings: roster,
                })?;
                self.guarantees.push(ScopedArithmeticHypothesis {
                    proposition,
                    holds: true,
                });
            }
        }
        Some(())
    }

    fn postorder(
        &mut self,
        expression: ExpressionHandle,
        ordered: &mut Vec<ExpressionHandle>,
    ) -> Option<()> {
        let mut pending = vec![(expression, false)];
        while let Some((expression, returning)) = pending.pop() {
            self.step()?;
            if !expression.is_valid() || ordered.contains(&expression) {
                continue;
            }
            if returning {
                ordered.push(expression);
                continue;
            }
            if !self
                .program
                .expression_table
                .expression_is_valid(expression)
            {
                return None;
            }
            pending.push((expression, true));
            children(
                self.program,
                self.program.expression_table.expression(expression),
                |child| pending.push((child, false)),
            );
        }
        Some(())
    }

    /// Published guarantees cannot establish themselves, even through another
    /// declaration. Check the guarantee dependency graph before importing any
    /// clause; ordinary recursive bodies do not create this dependency.
    fn independent(
        &mut self,
        owner: SelectedCallContract<'program>,
        active: &mut Vec<SymbolHandle>,
        complete: &mut Vec<SymbolHandle>,
    ) -> Option<bool> {
        self.step()?;
        if active.len() >= 64 {
            return Some(false);
        }
        let symbol = owner_symbol(&owner);
        if active.contains(&symbol)
            || matches!(owner, SelectedCallContract::Concrete(machine, _) if machine.symbol == self.context)
        {
            return Some(false);
        }
        if complete.contains(&symbol) {
            return Some(true);
        }
        active.push(symbol);
        let mut pending = Vec::new();
        for contract in owner_contracts(self.program, &owner)
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in self.program.proof_facts.span_or_empty(contract.facts) {
                fact_roots(self.program, fact, &mut pending);
            }
        }
        let mut visited = Vec::new();
        while let Some(expression) = pending.pop() {
            self.step()?;
            if !expression.is_valid() || visited.contains(&expression) {
                continue;
            }
            visited.push(expression);
            let node = self.program.expression_table.expression(expression);
            if let ExpressionNode::Call(call) = node
                && let Some(nested) = selected_call_contract(self.program, call)
                && !self.independent(nested, active, complete)?
            {
                return Some(false);
            }
            children(self.program, node, |child| pending.push(child));
        }
        active.pop();
        complete.push(symbol);
        Some(true)
    }
}
