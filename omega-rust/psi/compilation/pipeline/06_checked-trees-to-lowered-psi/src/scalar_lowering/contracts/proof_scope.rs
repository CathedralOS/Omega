//! Bind references to the source invocation when a block's proof inputs live
//! on its machine contract. Authored state blocks keep their own references.

use semantic_vocabulary::{ProofFormalScope, ProofTerm};
use terminal_psi::{Block, OperationKind, Terminator};

/// Positions were assigned from the checked caller's proof-parameter list.
/// This conversion changes only whether they refer to block or invocation inputs.
pub(crate) fn bind_invocation_proof_references(block: &mut Block) {
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
                bind_terms(erased_proof_arguments);
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
        } => bind_terms(erased_proof_arguments),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            bind_terms(&mut when_true.erased_proof_arguments);
            bind_terms(&mut when_false.erased_proof_arguments);
        }
        _ => {}
    }
}

fn bind_terms(terms: &mut [ProofTerm]) {
    let mut pending = terms.iter_mut().collect::<Vec<_>>();
    while let Some(term) = pending.pop() {
        match term {
            ProofTerm::Formal { position } => {
                *term = ProofTerm::InvocationFormal {
                    position: *position,
                };
            }
            ProofTerm::MathematicalApplication { formal_scope, .. } => {
                *formal_scope = ProofFormalScope::Invocation;
            }
            ProofTerm::Array { elements, .. } => pending.extend(elements.iter_mut()),
            ProofTerm::Construction { fields, .. } => {
                pending.extend(fields.iter_mut().map(|field| &mut field.term))
            }
            _ => {}
        }
    }
}
