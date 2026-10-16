//! Prepare calls in expression continuations from the published callable.
//! The formal-order schedule already completed every operand. Body and boundary
//! dispatch differ only at emission: argument homes, receiving custody, public
//! result qualifications and normal loan completion use the statement machinery.
//! A returned loan is not a reason to demand an executable requirement body.
use super::{
    CheckedTrees, LoweringError, QualifiedScalarType, storage, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::emission::operation_emission::calls::PreparedStructuralArgument;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_lowering::branch_states::prepared::{EffectCallTarget, LoweredEffectCall};
use crate::unit::attached_unit::bodies::{UnitBody, UnitPlans};
use checked_trees::{CheckedUnitCallRoute, CheckedUnitEffectOperationPlan};

pub(super) struct Prepared {
    pub(super) call: LoweredEffectCall,
}

/// Borrow the actual caller's proof namespace independently of its result.
pub(super) struct Caller<'a> {
    pub(super) state: symbols::SymbolHandle,
    pub(super) erased_proof_parameters: &'a [checked_trees::CheckedErasedProofParameterPlan],
}

impl<'a> Caller<'a> {
    pub(super) fn published(
        checked: &'a CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
    ) -> Result<Self, LoweringError> {
        Ok(Self {
            state,
            erased_proof_parameters: UnitBody::find(
                UnitPlans::published(&checked.facts.flow),
                machine,
            )?
            .state_erased_proof_parameters(state)?,
        })
    }
}

pub(super) fn prepare(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    caller: &Caller<'_>,
    operation: &CheckedUnitEffectOperationPlan,
    bindings: &storage::ScalarBindings,
    arguments: Vec<LoweredDirectExpression>,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    result: Option<terminal_psi::StructuralOperationResult>,
    results: &[(
        u32,
        crate::emission::operation_emission::buffer::StructuralBinding,
    )],
    completed_arguments: &[(usize, PreparedStructuralArgument)],
    family_results: &[semantic_vocabulary::PlaceId],
) -> Result<Prepared, LoweringError> {
    let call = operation.call().ok_or(LoweringError::Unsupported(
        "expression call preparation received a non-call operation",
    ))?;
    let target = UnitPlans::published(&checked.facts.flow).call_parameters(call)?;
    if call.structural_arguments.len() != target.structural.len()
        || call.scalar_operands().len() != target.scalar.len()
        || arguments.len() != target.scalar.len()
        || arguments.iter().zip(target.scalar).any(|(actual, formal)| {
            terminal_scalar_type(formal.primitive_type).ok() != Some(actual.scalar_type())
        })
    {
        return unsupported("expression call disagrees with its published callable");
    }
    match (call.result(), &result) {
        (checked_trees::CheckedUnitCallResult::Unit, None)
        | (checked_trees::CheckedUnitCallResult::Structural { .. }, Some(_)) => {}
        _ => return unsupported("effect call disagrees with its result binding"),
    }
    let endpoint = match call.route() {
        CheckedUnitCallRoute::Module {
            erased_scalar_arguments,
            erased_proof_arguments,
            ..
        } => {
            if !erased_scalar_arguments.is_empty() {
                return unsupported("expression effect calls require erased scalar substitution");
            }
            EffectCallTarget::Module {
                erased_proof_arguments: erased_proof_arguments
                    .iter()
                    .map(|term| {
                        crate::scalar_lowering::contracts::checked_proof_term(
                            identities,
                            checked,
                            term,
                            caller.erased_proof_parameters,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                crash_routes: crate::unit::effective_crash_routes(checked, call.target.machine)?,
            }
        }
        CheckedUnitCallRoute::Boundary => EffectCallTarget::Boundary,
        CheckedUnitCallRoute::Dynamic {
            interface_identity,
            family_operands,
            ..
        } => {
            let slot = checked
                .facts
                .flow
                .terminal_unit_effects
                .dynamic_slots
                .iter()
                .find(|slot| slot.slot == call.target.machine)
                .ok_or(LoweringError::Unsupported(
                    "selector call has no checked slot",
                ))?;
            EffectCallTarget::Dynamic {
                interface_identity: *interface_identity,
                requirement_identity: slot.requirement_identity,
                family_actuals: family_operands
                    .iter()
                    .map(|operand| {
                        let checked_trees::CheckedCallScalarArgument::Pure(expression) = operand
                        else {
                            return unsupported(
                                "dynamic family operand is not a pure scalar expression",
                            );
                        };
                        bindings.expression(
                            identities,
                            &checked.facts.proven_scalar_indexes,
                            expression,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                family_results: family_results.to_vec(),
            }
        }
    };
    let (caller_machine, caller_state) =
        crate::declarations::state_declaration(checked, caller.state)?;
    let mut structural_arguments = Vec::with_capacity(call.structural_arguments.len());
    for (ordinal, (argument, parameter)) in call
        .structural_arguments
        .iter()
        .zip(target.structural)
        .enumerate()
    {
        if parameter.access != argument.access || parameter.type_identity != argument.type_identity
        {
            return unsupported("expression call argument disagrees with its published formal");
        }
        let prepared = if let Some((_, completed)) = completed_arguments
            .iter()
            .find(|(position, _)| *position == ordinal)
        {
            completed.clone()
        } else if argument.access == checked_trees::CheckedStructuralAccess::Owned {
            let source = bindings.owned_call_argument(identities, argument, results)?;
            let root_type = bindings.structural_place_type(source.place).or_else(|| {
                results
                    .iter()
                    .find(|(_, binding)| binding.place() == source.place)
                    .map(|(_, binding)| binding.structural_type())
            });
            PreparedStructuralArgument::from_static(source, root_type)?
        } else if let Some(primitive) = parameter.primitive_referent {
            bindings.prepared_primitive_borrow(
                identities,
                checked,
                argument,
                terminal_scalar_type(primitive)?,
                structural_types,
                checked.state_parameters(caller_state),
            )?
        } else {
            bindings.prepared_borrowed_argument(
                identities,
                checked,
                argument,
                checked.state_parameters(caller_state),
                structural_types,
            )?
        };
        structural_arguments.push(prepared);
    }
    let result = result
        .map(|result| {
            crate::unit::attached_unit::call_results::prepare(
                identities,
                checked,
                call,
                result.place,
                result.structural_type,
                structural_types,
            )
        })
        .transpose()?;
    let coordinate = SourceCallCoordinate {
        state: caller.state,
        statement_index: call.coordinate.statement_index as usize,
        call_ordinal: call.coordinate.call_ordinal as usize,
    };
    let loan_completions = crate::emission::loan_completions::prepare(
        identities,
        checked,
        caller_machine.symbol,
        coordinate,
        structural_types,
        |root| bindings.owned_structural_home(root, checked.state_parameters(caller_state)),
    )?;
    Ok(Prepared {
        call: LoweredEffectCall {
            result,
            endpoint,
            claim_transfers: crate::emission::claim_bindings::prepare_checked_claim_transfers(
                identities,
                checked,
                call.claim_transfers(),
                target.structural,
                structural_types,
            )?,
            loan_completions,
            checked_coordinate: call.coordinate,
            source_coordinate: coordinate,
            source_site: call.source_site().flatten(),
            target_machine: call.target.machine,
            target_state: call.target.state,
            arguments,
            structural_arguments,
        },
    })
}

impl Prepared {
    /// All operands have already completed through the formal-order schedule.
    pub(super) fn finish_completed(
        self,
        value_types: &[QualifiedScalarType],
        target: usize,
        computations: &mut super::computations::Expansion<'_>,
    ) -> usize {
        use super::prepared::{
            LoweredScalarBranchState, LoweredScalarBranchTerminator, LoweredScalarEffect,
        };
        computations.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            parameter_types: value_types.to_vec(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            structural_effects: vec![LoweredScalarEffect::Call(self.call)],
            terminator: LoweredScalarBranchTerminator::jump(
                target,
                super::computations::parameters(value_types),
            ),
        })
    }
}
