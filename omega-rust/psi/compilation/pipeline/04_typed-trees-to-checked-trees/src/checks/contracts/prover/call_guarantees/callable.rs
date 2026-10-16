//! Contract arithmetic retains its declaration's parameter identities: a
//! machine, a bodyless requirement, or a transparent proposition. Neither a
//! requirement nor a proposition acquires a manufactured executable body.

use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContract, StateParameter, StateSignature};
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone, Copy)]
pub(in crate::checks) enum Callable<'program> {
    Machine {
        machine: &'program Machine,
        state: &'program State,
    },
    Requirement {
        signature: &'program StateSignature,
    },
    Proposition {
        declaration: &'program typed_trees::proposition::PropositionDefinition,
    },
}

impl<'program> Callable<'program> {
    pub(super) fn resolve(
        program: &'program dyn ::validation::CheckingInput,
        target: SymbolHandle,
    ) -> Option<Self> {
        if !target.is_valid() {
            return None;
        }
        let machines = validation::find_machine_head(program, target)
            .map(|(machine, state)| Self::Machine { machine, state });
        let requirements = validation::selected_requirement_signature(program, target)
            .map(|(_, signature)| Self::Requirement { signature });
        let mut candidates = machines.into_iter().chain(requirements);
        let selected = candidates.next()?;
        let result_type = match selected {
            Self::Machine { state, .. } => state.return_type,
            Self::Requirement { signature } => signature.return_type,
            Self::Proposition { .. } => TypeReferenceHandle::invalid(),
        };
        // An omitted result is ordinary Unit and still has call requirements.
        // It cannot authorize reserved result use; that reader checks its type.
        (candidates.next().is_none()
            && (!result_type.is_valid()
                || program
                    .type_reference_table
                    .contains_type_reference(result_type)))
        .then_some(selected)
    }

    pub(in crate::checks) fn owner_symbol(self) -> SymbolHandle {
        match self {
            Self::Machine { machine, .. } => machine.symbol,
            Self::Requirement { signature } => signature.symbol,
            Self::Proposition { declaration } => declaration.symbol,
        }
    }

    pub(in crate::checks) fn target_symbol(self) -> SymbolHandle {
        match self {
            Self::Machine { state, .. } => state.symbol,
            Self::Requirement { signature } => signature.symbol,
            Self::Proposition { declaration } => declaration.symbol,
        }
    }

    pub(in crate::checks) fn parameters(
        self,
        program: &'program dyn ::validation::CheckingInput,
    ) -> &'program [StateParameter] {
        match self {
            Self::Machine { state, .. } => program.state_parameters(state),
            Self::Requirement { signature } => program.state_signature_parameters(signature),
            Self::Proposition { declaration } => program.proposition_parameters(declaration),
        }
    }

    pub(super) fn result_type(self) -> TypeReferenceHandle {
        match self {
            Self::Machine { state, .. } => state.return_type,
            Self::Requirement { signature } => signature.return_type,
            Self::Proposition { .. } => TypeReferenceHandle::invalid(),
        }
    }

    pub(in crate::checks) fn contracts(
        self,
        program: &'program dyn ::validation::CheckingInput,
    ) -> impl Iterator<Item = &'program SignatureContract> {
        let (first, second): (&[SignatureContract], &[SignatureContract]) = match self {
            Self::Machine { machine, state } => (
                if program
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == state.symbol)
                {
                    program.machine_contracts(machine)
                } else {
                    // Machine contracts bind entry parameters, not a later
                    // state's independently declared parameter scope.
                    &[]
                },
                program.state_contracts(state),
            ),
            Self::Requirement { signature } => (program.state_signature_contracts(signature), &[]),
            Self::Proposition { .. } => (&[], &[]),
        };
        first.iter().chain(second)
    }

    pub(in crate::checks) fn scalar_reference(
        self,
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        if let Some(result) = validation::reserved_result_place(program, expression) {
            return (result.machine_symbol == self.owner_symbol()).then_some(result.type_reference);
        }
        match self {
            Self::Machine { machine, state } => {
                validation::expression_result_type_reference(program, machine, state, expression)
            }
            Self::Proposition { declaration } => {
                validation::parameter_expression_result_type_reference(
                    program,
                    declaration.symbol,
                    program.proposition_parameters(declaration),
                    expression,
                )
            }
            Self::Requirement { signature } => {
                validation::parameter_expression_result_type_reference(
                    program,
                    signature.symbol,
                    program.state_signature_parameters(signature),
                    expression,
                )
            }
        }
    }

    pub(in crate::checks) fn builtin_meaning(
        self,
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        match self {
            Self::Machine { machine, state } => validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ),
            Self::Proposition { declaration } => {
                validation::has_builtin_parameter_bound_expression_meaning(
                    program,
                    declaration.symbol,
                    program.proposition_parameters(declaration),
                    expression,
                )
            }
            Self::Requirement { signature } => {
                validation::has_builtin_parameter_bound_expression_meaning(
                    program,
                    signature.symbol,
                    program.state_signature_parameters(signature),
                    expression,
                )
            }
        }
    }

    pub(super) fn decomposed_builtin_meaning(
        self,
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        match self {
            Self::Machine { machine, state } => validation::has_builtin_decomposed_guard_meaning(
                program,
                machine,
                Some(state),
                expression,
            ),
            Self::Requirement { .. } | Self::Proposition { .. } => {
                if matches!(program.expression_table.expression(expression),
                    typed_trees::expression::ExpressionNode::Binary(binary)
                        if matches!(binary.operator, typed_trees::expression::BinaryOperator::And
                            | typed_trees::expression::BinaryOperator::Or))
                {
                    return true;
                }
                self.builtin_meaning(program, expression)
            }
        }
    }
}
