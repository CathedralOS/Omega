//! Published scalar guarantees under a call occurrence's simultaneous roster.
//!
//! Specification call formation has already discharged the selected requires.
//! This transport reads only independently checked published ensures, never a
//! body, a precondition, or the contract currently being established. A result
//! has a private occurrence identity: equal arguments do not equate independent
//! effectful calls. Actuals retain their caller roster before formal binding.
//! Bounded expansion is optional proof strength, not a second formation route.

use super::super::specification_calls::{
    SelectedCallContract, fact_roots, owner_contracts, owner_parameters, owner_symbol,
    selected_call_contract,
};
use super::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue,
};
use crate::expression_traversal::children;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::PrimitiveType;

pub(super) struct ScopedCalls<'program> {
    program: &'program dyn crate::CheckingInput,
    context: SymbolHandle,
    remaining: usize,
    depth: usize,
    scopes: Vec<(ScopedArithmeticExpression, ScopedArithmeticExpression)>,
    guarantees: Vec<ScopedArithmeticHypothesis>,
}

impl<'program> ScopedCalls<'program> {
    pub(super) fn new(program: &'program dyn crate::CheckingInput, context: &Machine) -> Self {
        Self {
            program,
            context: context.symbol,
            remaining: 4096,
            depth: 0,
            scopes: Vec::new(),
            guarantees: Vec::new(),
        }
    }

    fn step(&mut self) -> Option<()> {
        self.remaining = self.remaining.checked_sub(1)?;
        Some(())
    }

    pub(super) fn value(&mut self, value: &ScopedArithmeticValue) -> Option<ScopedArithmeticValue> {
        self.step()?;
        Some(match value {
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

    pub(super) fn hypotheses(
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
            let mut engine = super::Engine::strict_with_symbol_bindings(self.program, machine, &[]);
            prepared.extend(self.guarantees.drain(..).filter(|hypothesis| {
                engine.install_scoped_bindings(&hypothesis.proposition.bindings)
            }));
        }
        Some(prepared)
    }

    pub(super) fn expression(
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
            let result_type = match owner {
                SelectedCallContract::Concrete(_, state) => state.return_type,
                SelectedCallContract::Signature(signature) => signature.return_type,
            };
            let Some(primitive) = self
                .program
                .primitive_type_reference(result_type)
                .filter(|primitive| primitive.is_fixed_integer())
            else {
                continue;
            };
            let independent = self
                .independent(owner, &mut Vec::new(), &mut Vec::new())
                .unwrap_or(false);
            let interval = if independent {
                crate::proof_contracts::arithmetic_domains::call_result_interval(self.program, call)
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
                    PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
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
                let mut bindings = vec![ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Result,
                    value: result.clone(),
                }];
                // Noninteger formal subjects remain outside the integer engine;
                // it may still read unrelated scalar conjuncts of the contract.
                for (parameter, argument) in parameters.iter().zip(arguments) {
                    if self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some_and(|primitive| primitive.is_fixed_integer())
                    {
                        bindings.push(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                            // The source owner captures admitted argument
                            // denotations before contract substitution. An
                            // unobserved actual remains opaque; its spelling
                            // cannot supply a polynomial or unfold a body.
                            value: prepared
                                .bindings
                                .iter()
                                .find(|binding| {
                                    binding.binder == ScopedArithmeticBinder::Occurrence(*argument)
                                })
                                .map(|binding| binding.value.clone())
                                .unwrap_or_else(|| ScopedArithmeticValue::Atom {
                                    identity: format!("\0scoped-actual:{scope}:{argument:?}"),
                                    unsigned: self
                                        .program
                                        .primitive_type_reference(parameter.type_reference)
                                        .is_some_and(|primitive| {
                                            matches!(
                                                primitive,
                                                PrimitiveType::U8
                                                    | PrimitiveType::U16
                                                    | PrimitiveType::U32
                                                    | PrimitiveType::U64
                                            )
                                        }),
                                }),
                        });
                    }
                }
                let guarantees: Vec<_> = owner_contracts(self.program, &owner)
                    .filter(|contract| {
                        contract.kind == SignatureContractKind::Ensures
                            && contract.binding.is_none()
                    })
                    .flat_map(|contract| self.program.proof_facts.span_or_empty(contract.facts))
                    .filter_map(|fact| match fact {
                        ProofFact::Expression(expression) => Some(*expression),
                        _ => None,
                    })
                    .collect();
                for guarantee in guarantees {
                    let builtin = match owner {
                        SelectedCallContract::Concrete(machine, state) => {
                            crate::has_builtin_bound_expression_meaning(
                                self.program,
                                machine,
                                Some(state),
                                guarantee,
                            )
                        }
                        SelectedCallContract::Signature(signature) => {
                            crate::has_builtin_parameter_bound_expression_meaning(
                                self.program,
                                signature.symbol,
                                parameters,
                                guarantee,
                            )
                        }
                    };
                    if builtin {
                        let proposition = self.expression(&ScopedArithmeticExpression {
                            expression: guarantee,
                            bindings: bindings.clone(),
                        })?;
                        self.guarantees.push(ScopedArithmeticHypothesis {
                            proposition,
                            holds: true,
                        });
                    }
                }
            }
            prepared.bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Occurrence(expression),
                value: result,
            });
        }
        self.scopes[scope].1 = prepared.clone();
        Some(prepared)
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
