//! Normal-result congruence uses the selected computation, not source spelling
//! or a replay of its body. Both sides enter the immutable entry namespace via
//! exact exit origins before substitution. Unknown leaves remain symbolic;
//! only Boolean congruence and closed connectives establish a truth value.
//! Immutable locals join their earlier source-bound selected computations.
//! These plans exist before contract checking; later execution plans cannot
//! authorize this check. Expansion establishes denotation, not reevaluation:
//! only immutable origins and earlier captured locals may supply operands.
//! Storage reads rejoin their last selected store at the capture coordinate;
//! complete write frames preserve that interval, including writes through loans
//! to otherwise immutable bindings. Later writes do not alter an earlier value.
//! Call-produced locals use exact captured arguments and declared normal-result
//! equations. Reading candidate clauses and expanding their substitutions share
//! one budget; neither callee bodies nor current mutable storage supply values.

use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
};
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

use super::{ExitScalars, ExpressionHandle, exit_return_expression};

mod computations;
mod storage;

enum BooleanSubject {
    Parameter(usize),
    Local(usize),
    Storage(SymbolHandle),
}

impl ExitScalars<'_, '_> {
    pub(super) fn proves_boolean_result(&self, expression: ExpressionHandle) -> Option<bool> {
        let entry = self.program.machine_states(self.machine).first()?;
        if self.program.primitive_type_reference(entry.return_type) != Some(PrimitiveType::Bool) {
            return None;
        }
        let returned = exit_return_expression(self.program, self.exit);
        if !self.return_expression_is_stable(returned) {
            return None;
        }
        let mut remaining = 4096;
        let predicate = crate::values::lower_scalar_contract_predicate(
            self.program,
            &self.facts.operators,
            self.machine,
            expression,
            true,
            &mut remaining,
        )?;
        let state = crate::find_state_in_machine(
            self.program,
            self.exit.machine_symbol,
            self.exit.state_symbol,
        )?;
        let parameters = self.program.state_parameters(entry);
        let scalar_parameters = || {
            parameters.iter().filter(|parameter| {
                self.program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
            })
        };
        // A dense slot is not itself identity. Require a unique origin for this
        // clause and an immutable Boolean binder on both ends of that edge.
        let origin = |position| {
            let parameter = scalar_parameters().nth(position)?;
            if parameter.is_mutable
                || parameter.is_self
                || parameter.is_const
                || self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    != Some(PrimitiveType::Bool)
            {
                return None;
            }
            let mut origins = self
                .facts
                .flow
                .control
                .exit_parameter_origins
                .span_or_empty(self.exit.parameter_origins)
                .iter()
                .filter(|origin| {
                    origin.contract == self.contract && origin.entry_parameter == parameter.symbol
                });
            let origin = origins.next()?;
            if origins.next().is_some() || !origin.state_parameter.is_valid() {
                return None;
            }
            self.program
                .state_parameters(state)
                .iter()
                .find(|parameter| {
                    parameter.symbol == origin.state_parameter
                        && !parameter.is_mutable
                        && !parameter.is_self
                        && !parameter.is_const
                        && self
                            .program
                            .primitive_type_reference(parameter.type_reference)
                            == Some(PrimitiveType::Bool)
                })
                .map(|parameter| parameter.symbol)
        };
        let returned = self.bind_boolean_expression_at(
            u32::try_from(self.exit.statement_index).ok()?,
            self.return_expression_role()?,
            returned,
            &|symbol| {
                let mut positions = (0..scalar_parameters().count())
                    .filter(|position| origin(*position) == Some(symbol));
                let position = positions.next()?;
                positions.next().is_none().then_some(position)
            },
            &mut remaining,
            0,
        )?;
        let predicate = bind_boolean(
            &predicate,
            &mut |subject, remaining, depth| {
                let BooleanSubject::Parameter(position) = subject else {
                    return None;
                };
                if position == scalar_parameters().count() {
                    // Keep substitution inside the same budget, even for
                    // repeated reserved-result occurrences.
                    bind_boolean(
                        &returned,
                        &mut |subject, _, _| {
                            let BooleanSubject::Parameter(position) = subject else {
                                return None;
                            };
                            Some(CheckedBooleanExpression::Parameter { position })
                        },
                        remaining,
                        depth,
                    )
                } else {
                    origin(position).map(|_| CheckedBooleanExpression::Parameter { position })
                }
            },
            &mut remaining,
            0,
        )?;
        match predicate {
            CheckedBooleanExpression::Constant(value) => Some(value),
            _ => None,
        }
    }

    fn bind_selected_boolean(
        &self,
        expression: &CheckedBooleanExpression,
        symbols: &[SymbolHandle],
        before_statement: u32,
        entry_position: &dyn Fn(SymbolHandle) -> Option<usize>,
        remaining: &mut usize,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        bind_boolean(
            expression,
            &mut |subject, remaining, depth| {
                let (position, local) = match subject {
                    BooleanSubject::Parameter(position) => (position, false),
                    BooleanSubject::Local(position) => (position, true),
                    BooleanSubject::Storage(symbol) => {
                        return self.bind_boolean_storage_at(
                            symbol,
                            before_statement,
                            entry_position,
                            remaining,
                            depth,
                        );
                    }
                };
                let symbol = *symbols.get(position)?;
                if !local {
                    return entry_position(symbol)
                        .map(|position| CheckedBooleanExpression::Parameter { position });
                }
                let state = crate::find_state_in_machine(
                    self.program,
                    self.exit.machine_symbol,
                    self.exit.state_symbol,
                )?;
                let parameters = self
                    .program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| {
                        self.program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                    })
                    .count();
                let binding_ordinal = u32::try_from(position.checked_sub(parameters)?).ok()?;
                let (statement, local) = self
                    .program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .enumerate()
                    .filter_map(|(statement, node)| {
                        let typed_trees::statement::StatementNode::LocalData(local) = node else {
                            return None;
                        };
                        (!local.is_mutable
                            && local.initial_value.is_valid()
                            && self
                                .program
                                .primitive_type_reference(local.type_reference)
                                .is_some())
                        .then_some((statement, local))
                    })
                    .nth(binding_ordinal as usize)?;
                let statement = u32::try_from(statement).ok()?;
                if local.symbol != symbol
                    || statement >= before_statement
                    || self.program.primitive_type_reference(local.type_reference)
                        != Some(PrimitiveType::Bool)
                {
                    return None;
                }
                self.boolean_local_preserved(symbol, statement, before_statement, remaining)?;
                let plans = &self.facts.values.scalar_expressions;
                let mut definitions = plans.source_bindings.iter().filter(|(_, binding)| {
                    binding.state == self.exit.state_symbol && binding.destination == symbol
                });
                let role = CheckedScalarExpressionRole::LocalInitializer { binding_ordinal };
                if self
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .any(|(_, root)| {
                        root.state == self.exit.state_symbol
                            && root.statement_ordinal == statement
                            && root.role == role
                    })
                {
                    if definitions.next().is_some() {
                        return None;
                    }
                    return self.bind_boolean_expression_at(
                        statement,
                        role,
                        local.initial_value,
                        entry_position,
                        remaining,
                        depth,
                    );
                }
                if matches!(
                    self.program
                        .expression_table
                        .expression(local.initial_value),
                    typed_trees::expression::ExpressionNode::Call(_)
                ) {
                    // Pure direct-call arguments already have occurrence-owned
                    // plans; manufacturing another initializer root would duplicate
                    // the call. The declaration supplies the destination, not its value.
                    if definitions.next().is_some() {
                        return None;
                    }
                    return self.bind_call_boolean(
                        local.initial_value,
                        statement,
                        binding_ordinal,
                        entry_position,
                        remaining,
                        depth,
                    );
                }
                let (_, binding) = definitions.next()?;
                if definitions.next().is_some()
                    || binding.statement_ordinal != statement
                    || binding.expression != local.initial_value
                    || binding.role
                        != (CheckedScalarExpressionRole::LocalInitializer { binding_ordinal })
                {
                    return None;
                }
                let (selected, operands) = self.selected_scalar_expression(
                    binding.statement_ordinal,
                    binding.role,
                    binding.expression,
                )?;
                let CheckedScalarExpression::Boolean(selected) = selected else {
                    return None;
                };
                // Strictly earlier definition coordinates rule out forward/cyclic
                // capture. Mutable reads join their own earlier selected store.
                self.bind_selected_boolean(
                    selected,
                    operands,
                    binding.statement_ordinal,
                    entry_position,
                    remaining,
                    depth,
                )
            },
            remaining,
            depth,
        )
    }

    fn bind_call_boolean(
        &self,
        expression: ExpressionHandle,
        statement: u32,
        binding_ordinal: u32,
        entry_position: &dyn Fn(SymbolHandle) -> Option<usize>,
        remaining: &mut usize,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        if depth >= 64 || *remaining == 0 {
            return None;
        }
        *remaining -= 1;
        let call = self.normal_return_call(expression)?;
        if call.state != self.exit.state_symbol
            || call.fact.statement_index != statement as usize
            || self
                .program
                .primitive_type_reference(call.entry.return_type)
                != Some(PrimitiveType::Bool)
        {
            return None;
        }
        let parameters = self.program.state_parameters(call.entry);
        // The current join has captured pure scalar operands, not structural
        // borrows or mutable post-state snapshots. Every slot must be retained,
        // including arguments unused by the particular guarantee.
        for (position, (parameter, argument)) in parameters.iter().zip(call.arguments).enumerate() {
            if parameter.is_mutable {
                return None;
            }
            let primitive = self
                .program
                .primitive_type_reference(parameter.type_reference)?;
            let (selected, _) = self.selected_scalar_expression(
                statement,
                CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal,
                    argument_ordinal: u32::try_from(position).ok()?,
                },
                *argument,
            )?;
            if crate::values::scalar_expression_type(selected) != Some(primitive) {
                return None;
            }
        }
        self.bind_normal_call_boolean(
            call,
            &mut |position, remaining, depth| {
                let (selected, symbols) = self.selected_scalar_expression(
                    statement,
                    CheckedScalarExpressionRole::CallArgument {
                        binding_ordinal,
                        argument_ordinal: u32::try_from(position).ok()?,
                    },
                    *call.arguments.get(position)?,
                )?;
                let CheckedScalarExpression::Boolean(selected) = selected else {
                    return None;
                };
                self.bind_selected_boolean(
                    selected,
                    symbols,
                    statement,
                    entry_position,
                    remaining,
                    depth,
                )
            },
            remaining,
            depth,
        )
    }

    fn bind_normal_call_boolean(
        &self,
        call: super::calls::NormalReturnCall<'_>,
        argument: &mut dyn FnMut(usize, &mut usize, usize) -> Option<CheckedBooleanExpression>,
        remaining: &mut usize,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        use CheckedBooleanExpression as Boolean;
        let parameters = self.program.state_parameters(call.entry);
        for guarantee in self.normal_call_guarantees(call) {
            if *remaining == 0 {
                return None;
            }
            let Some(predicate) = crate::values::lower_scalar_contract_predicate(
                self.program,
                &self.facts.operators,
                call.callee,
                guarantee,
                true,
                remaining,
            ) else {
                continue;
            };
            let Boolean::Equal { left, right } = &predicate else {
                continue;
            };
            let definition = if **left
                == (Boolean::Parameter {
                    position: parameters.len(),
                }) {
                right
            } else if **right
                == (Boolean::Parameter {
                    position: parameters.len(),
                })
            {
                left
            } else {
                continue;
            };
            // Only declared normal guarantees establish the result. Call requires
            // are checked independently before exit checking; this substitution
            // is never available to prove the call's own requirements.
            if let Some(value) = bind_boolean(
                definition,
                &mut |subject, remaining, depth| {
                    let BooleanSubject::Parameter(position) = subject else {
                        return None;
                    };
                    if self
                        .program
                        .primitive_type_reference(parameters.get(position)?.type_reference)
                        != Some(PrimitiveType::Bool)
                    {
                        return None;
                    }
                    argument(position, remaining, depth)
                },
                remaining,
                depth + 1,
            ) {
                return Some(value);
            }
        }
        None
    }
}

fn bind_boolean(
    expression: &CheckedBooleanExpression,
    resolve: &mut dyn FnMut(BooleanSubject, &mut usize, usize) -> Option<CheckedBooleanExpression>,
    remaining: &mut usize,
    depth: usize,
) -> Option<CheckedBooleanExpression> {
    use CheckedBooleanExpression as Boolean;
    if depth >= 64 || *remaining == 0 {
        return None;
    }
    *remaining -= 1;
    Some(match expression {
        Boolean::Constant(value) => Boolean::Constant(*value),
        Boolean::Parameter { position } | Boolean::Local { position } => {
            return resolve(
                if matches!(expression, Boolean::Local { .. }) {
                    BooleanSubject::Local(*position)
                } else {
                    BooleanSubject::Parameter(*position)
                },
                remaining,
                depth + 1,
            );
        }
        Boolean::StorageRead { symbol } => {
            return resolve(BooleanSubject::Storage(*symbol), remaining, depth + 1);
        }
        Boolean::Not(operand) => match bind_boolean(operand, resolve, remaining, depth + 1)? {
            Boolean::Constant(value) => Boolean::Constant(!value),
            Boolean::Not(operand) => *operand,
            operand => Boolean::Not(Box::new(operand)),
        },
        Boolean::Equal { left, right }
        | Boolean::And { left, right }
        | Boolean::Or { left, right } => {
            let left = Box::new(bind_boolean(left, resolve, remaining, depth + 1)?);
            let right = Box::new(bind_boolean(right, resolve, remaining, depth + 1)?);
            match expression {
                Boolean::Equal { .. } if left == right => Boolean::Constant(true),
                Boolean::Equal { .. } => match (&*left, &*right) {
                    (Boolean::Constant(left), Boolean::Constant(right)) => {
                        Boolean::Constant(left == right)
                    }
                    (Boolean::Constant(true), _) => *right,
                    (_, Boolean::Constant(true)) => *left,
                    (Boolean::Constant(false), _) => Boolean::Not(right),
                    (_, Boolean::Constant(false)) => Boolean::Not(left),
                    _ => Boolean::Equal { left, right },
                },
                Boolean::And { .. } => match (&*left, &*right) {
                    (Boolean::Constant(left), Boolean::Constant(right)) => {
                        Boolean::Constant(*left && *right)
                    }
                    (Boolean::Constant(false), _) | (_, Boolean::Constant(false)) => {
                        Boolean::Constant(false)
                    }
                    (Boolean::Constant(true), _) => *right,
                    (_, Boolean::Constant(true)) => *left,
                    _ => Boolean::And { left, right },
                },
                _ => match (&*left, &*right) {
                    (Boolean::Constant(left), Boolean::Constant(right)) => {
                        Boolean::Constant(*left || *right)
                    }
                    (Boolean::Constant(true), _) | (_, Boolean::Constant(true)) => {
                        Boolean::Constant(true)
                    }
                    (Boolean::Constant(false), _) => *right,
                    (_, Boolean::Constant(false)) => *left,
                    _ => Boolean::Or { left, right },
                },
            }
        }
        // Non-Boolean computations need their own retained
        // value/effect evidence; matching their spelling or shape is not proof.
        _ => return None,
    })
}
