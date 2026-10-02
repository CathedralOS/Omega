//! Erased scope follows the private blocks created for one authored state.
//!
//! The construction range is the scope witness. Equal carrier types or proof
//! roster lengths never establish that two independently authored states share
//! bindings. Only edges into this private range get identity actuals; outgoing
//! authored substitutions and the shared return sink retain their own contract.

use super::{LoweredDirectExpression, LoweredScalarBranchState, LoweredScalarBranchTerminator};
use crate::LoweringError;
use crate::scalar_graph::scalar_contracts::LoweredProofTerm;
use std::ops::Range;

pub(super) fn retain(
    base: usize,
    private_start: usize,
    entry: &mut LoweredScalarBranchState,
    states: &mut [LoweredScalarBranchState],
) -> Result<(), LoweringError> {
    let start = base
        .checked_add(private_start)
        .ok_or(LoweringError::Unsupported(
            "erased scope private state identity overflows",
        ))?;
    let end = base
        .checked_add(states.len())
        .ok_or(LoweringError::Unsupported(
            "erased scope private state identity overflows",
        ))?;
    let private = states
        .get_mut(private_start..)
        .ok_or(LoweringError::Unsupported(
            "erased scope private range is outside its expansion",
        ))?;
    let scalar_actuals = entry
        .erased_formal_types
        .iter()
        .enumerate()
        .map(
            |(position, value_type)| LoweredDirectExpression::ErasedParameter {
                position,
                scalar_type: value_type.scalar_type,
            },
        )
        .collect::<Vec<_>>();
    let proof_actuals = entry
        .erased_proof_formals
        .iter()
        .enumerate()
        .map(|(position, _)| {
            u32::try_from(position)
                .map(|position| LoweredProofTerm::Formal { position })
                .map_err(|_| LoweringError::Unsupported("erased proof scope exceeds u32"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for state in private {
        state.erased_formal_types = entry.erased_formal_types.clone();
        state.erased_proof_formals = entry.erased_proof_formals.clone();
        forward(
            &mut state.terminator,
            start..end,
            &scalar_actuals,
            &proof_actuals,
        )?;
    }
    forward(
        &mut entry.terminator,
        start..end,
        &scalar_actuals,
        &proof_actuals,
    )
}

fn forward(
    terminator: &mut LoweredScalarBranchTerminator,
    private: Range<usize>,
    scalar_actuals: &[LoweredDirectExpression],
    proof_actuals: &[LoweredProofTerm],
) -> Result<(), LoweringError> {
    let edge = |target: usize,
                scalar: &mut Vec<LoweredDirectExpression>,
                proof: &mut Vec<LoweredProofTerm>| {
        if !private.contains(&target) {
            return Ok(());
        }
        if (!scalar.is_empty() && scalar != scalar_actuals)
            || (!proof.is_empty() && proof != proof_actuals)
        {
            return Err(LoweringError::Unsupported(
                "private erased edge substitutes its source scope",
            ));
        }
        *scalar = scalar_actuals.to_vec();
        *proof = proof_actuals.to_vec();
        Ok(())
    };
    match terminator {
        LoweredScalarBranchTerminator::Jump {
            target,
            erased_arguments,
            erased_proof_arguments,
            ..
        }
        | LoweredScalarBranchTerminator::Qualify {
            target,
            erased_arguments,
            erased_proof_arguments,
            ..
        } => edge(*target, erased_arguments, erased_proof_arguments),
        LoweredScalarBranchTerminator::Conditional {
            when_true_target,
            when_true_erased_arguments,
            when_true_erased_proof_arguments,
            when_false_target,
            when_false_erased_arguments,
            when_false_erased_proof_arguments,
            ..
        }
        | LoweredScalarBranchTerminator::CaseDispatch {
            when_true_target,
            when_true_erased_arguments,
            when_true_erased_proof_arguments,
            when_false_target,
            when_false_erased_arguments,
            when_false_erased_proof_arguments,
            ..
        } => {
            edge(
                *when_true_target,
                when_true_erased_arguments,
                when_true_erased_proof_arguments,
            )?;
            edge(
                *when_false_target,
                when_false_erased_arguments,
                when_false_erased_proof_arguments,
            )
        }
        LoweredScalarBranchTerminator::CaseDispatchSplit {
            armed,
            fallback_target,
            fallback_erased_arguments,
            fallback_erased_proof_arguments,
            ..
        } => {
            for arm in armed {
                edge(
                    arm.target,
                    &mut arm.erased_arguments,
                    &mut arm.erased_proof_arguments,
                )?;
            }
            edge(
                *fallback_target,
                fallback_erased_arguments,
                fallback_erased_proof_arguments,
            )
        }
        LoweredScalarBranchTerminator::Return { .. } | LoweredScalarBranchTerminator::Crash(_) => {
            Ok(())
        }
    }
}
