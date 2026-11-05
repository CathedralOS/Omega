//! Bind references to the source invocation when a block's proof inputs live
//! on its machine contract. Authored state blocks keep their own references.

use crate::shared::lowering_error::LoweringError;
use semantic_vocabulary::{MathematicalArgument, ProofFormalScope, ProofTerm};
use terminal_psi::{Block, OperationKind, Terminator};

pub(crate) fn bind_invocation_proof_references(
    block: &mut Block,
    invocation_formal_count: usize,
) -> Result<(), LoweringError> {
    for operation in &mut block.operations {
        match &mut operation.kind {
            OperationKind::Call {
                erased_proof_arguments,
                ..
            }
            | OperationKind::CallUnit {
                erased_proof_arguments,
                ..
            }
            | OperationKind::CallStructuralScalar {
                erased_proof_arguments,
                ..
            }
            | OperationKind::CallStructuralWithScalarArguments {
                erased_proof_arguments,
                ..
            }
            | OperationKind::CallDynamic {
                erased_proof_arguments,
                ..
            } => {
                bind_terms(erased_proof_arguments, invocation_formal_count)?;
            }
            _ => {}
        }
    }
    match &mut block.terminator {
        Terminator::Jump {
            erased_proof_arguments,
            ..
        }
        | Terminator::InvokeNoNormal {
            erased_proof_arguments,
            ..
        } => bind_terms(erased_proof_arguments, invocation_formal_count)?,
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            bind_terms(
                &mut when_true.erased_proof_arguments,
                invocation_formal_count,
            )?;
            bind_terms(
                &mut when_false.erased_proof_arguments,
                invocation_formal_count,
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn bind_terms(
    terms: &mut [ProofTerm],
    invocation_formal_count: usize,
) -> Result<(), LoweringError> {
    let mut pending = terms.iter_mut().collect::<Vec<_>>();
    while let Some(term) = pending.pop() {
        match term {
            ProofTerm::Formal { position } => {
                if *position as usize >= invocation_formal_count {
                    return Err(LoweringError::Unsupported(
                        "proof reference is outside its source invocation parameters",
                    ));
                }
                *term = ProofTerm::InvocationFormal {
                    position: *position,
                };
            }
            ProofTerm::MathematicalApplication {
                arguments,
                formal_scope,
                ..
            } => {
                if arguments.iter().any(|argument| {
                    matches!(argument,
                    MathematicalArgument::Formal { position } if *position as usize >= invocation_formal_count)
                }) {
                    return Err(LoweringError::Unsupported(
                        "mathematical function argument is outside its source invocation parameters",
                    ));
                }
                *formal_scope = ProofFormalScope::Invocation;
            }
            ProofTerm::Array { elements, .. } => pending.extend(elements.iter_mut()),
            ProofTerm::Construction { fields, .. } => {
                pending.extend(fields.iter_mut().map(|field| &mut field.term))
            }
            _ => {}
        }
    }
    Ok(())
}
