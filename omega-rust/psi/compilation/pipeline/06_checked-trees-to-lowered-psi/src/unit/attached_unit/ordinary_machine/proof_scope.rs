//! Generated scalar continuations do not rebind an ordinary machine's source
//! inputs. Bind their proof references to the invocation namespace explicitly;
//! authored state-graph formals keep their separate edge-bound block namespace.
//! This changes no certificate context, grant, term, or carrier identity.

use crate::lowering_error::LoweringError;
use semantic_vocabulary::{MathematicalArgument, ProofFormalScope, ProofTerm};
use terminal_psi::{OperationKind, TerminalMachine, Terminator};

pub(super) fn bind(machine: &mut TerminalMachine) -> Result<(), LoweringError> {
    let count = machine.contract.erased_proof_formals.len();
    for block in &mut machine.blocks {
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
                    bind_terms(erased_proof_arguments, count)?;
                }
                _ => {}
            }
        }
        match &mut block.terminator {
            Terminator::Jump {
                erased_proof_arguments,
                ..
            } => bind_terms(erased_proof_arguments, count)?,
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                bind_terms(&mut when_true.erased_proof_arguments, count)?;
                bind_terms(&mut when_false.erased_proof_arguments, count)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn bind_terms(terms: &mut [ProofTerm], count: usize) -> Result<(), LoweringError> {
    let mut pending = terms.iter_mut().collect::<Vec<_>>();
    while let Some(term) = pending.pop() {
        match term {
            ProofTerm::Formal { position } => {
                if *position as usize >= count {
                    return Err(LoweringError::Unsupported(
                        "ordinary proof reference is outside its source invocation roster",
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
                    MathematicalArgument::Formal { position } if *position as usize >= count)
                }) {
                    return Err(LoweringError::Unsupported(
                        "mathematical function argument is outside its source invocation roster",
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
