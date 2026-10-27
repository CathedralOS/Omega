//! Receiving controls mutate only the source-produced mixed application. The
//! trusted checked source inventory, carrier keys and grant policy stay fixed.

use std::collections::BTreeSet;

use semantic_vocabulary::{
    IntegerSign, IntegerType, MathematicalArgument, ProofFormalScope, ProofTerm, ScalarTerm,
    ScalarType,
};
use terminal_codec::MathematicalProofActualError;
use terminal_psi::{OperationKind, TerminalModule, Terminator};

pub(super) fn check(
    module: &TerminalModule,
    expected: &TerminalModule,
    carriers: &[(String, Vec<u8>)],
    grants: &BTreeSet<u32>,
) -> Result<usize, String> {
    let mut probe = module.clone();
    if first_mixed(&mut probe).is_none() {
        return Ok(0);
    }
    let mut count = 0;
    for name in [
        "scalar-source-slot",
        "scalar-source-carrier",
        "scalar-type",
        "erased-source-scope",
        "erased-tail-slot",
        "missing-argument",
        "argument-order",
    ] {
        let mut changed = module.clone();
        let term = first_mixed(&mut changed).ok_or("source mixed application disappeared")?;
        let ProofTerm::MathematicalApplication {
            arguments,
            formal_scope,
            ..
        } = term
        else {
            unreachable!("first_mixed returns a mathematical application")
        };
        let has_formal = arguments
            .iter()
            .any(|argument| matches!(argument, MathematicalArgument::Formal { .. }));
        if (!has_formal && matches!(name, "erased-source-scope" | "erased-tail-slot"))
            || (arguments.len() < 2 && matches!(name, "missing-argument" | "argument-order"))
        {
            continue;
        }
        match name {
            "scalar-source-slot" | "scalar-source-carrier" | "scalar-type" => {
                let (type_identity, term) = arguments
                    .iter_mut()
                    .find_map(|argument| {
                        if let MathematicalArgument::Scalar {
                            type_identity,
                            term,
                        } = argument
                        {
                            Some((type_identity, term))
                        } else {
                            None
                        }
                    })
                    .ok_or("mixed application has no scalar input")?;
                let ScalarTerm::Value { id, scalar_type } = term else {
                    return Err(
                        "source mixed application did not retain a scalar observation".into(),
                    );
                };
                match name {
                    "scalar-source-slot" => {
                        *id = module
                            .machines
                            .iter()
                            .flat_map(|owner| {
                                owner
                                    .parameters
                                    .iter()
                                    .chain(owner.blocks.iter().flat_map(|block| &block.parameters))
                            })
                            .find(|value| value.id != *id && value.scalar_type == *scalar_type)
                            .map(|value| value.id)
                            .unwrap_or(
                                semantic_vocabulary::ValueId::new(u64::MAX)
                                    .expect("nonzero foreign observation"),
                            );
                    }
                    "scalar-source-carrier" => type_identity.push_str("::foreign-source-carrier"),
                    _ => {
                        *scalar_type = if *scalar_type == ScalarType::Boolean {
                            ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 64)
                                    .expect("fixed u64 descriptor"),
                            )
                        } else {
                            ScalarType::Boolean
                        }
                    }
                }
            }
            "erased-source-scope" => {
                *formal_scope = match formal_scope {
                    ProofFormalScope::Block => ProofFormalScope::Invocation,
                    ProofFormalScope::Invocation => ProofFormalScope::Block,
                }
            }
            "erased-tail-slot" => {
                let position = arguments
                    .iter_mut()
                    .find_map(|argument| match argument {
                        MathematicalArgument::Formal { position } => Some(position),
                        _ => None,
                    })
                    .ok_or("mixed source control needs an erased child")?;
                *position = position.checked_add(1).ok_or("proof position overflow")?;
            }
            "missing-argument" => {
                arguments.pop().ok_or("empty mixed application")?;
            }
            "argument-order" => {
                let last = arguments
                    .len()
                    .checked_sub(1)
                    .ok_or("empty mixed application")?;
                arguments.swap(0, last);
            }
            _ => unreachable!(),
        }
        super::indexed_family_controls::must_refuse_actual(
            &changed,
            expected,
            carriers,
            grants,
            name,
            |error| matches!(error, MathematicalProofActualError::SourceCarrierMismatch),
        )?;
        eprintln!(
            "corpus_runner: mixed_mathematical_application_control {name}=SourceCarrierMismatch"
        );
        count += 1;
    }
    Ok(count)
}

fn first_mixed(module: &mut TerminalModule) -> Option<&mut ProofTerm> {
    fn find(terms: &mut [ProofTerm]) -> Option<&mut ProofTerm> {
        terms.iter_mut().find(|term| matches!(term,
            ProofTerm::MathematicalApplication { arguments, .. }
                if arguments.iter().any(|argument| matches!(argument, MathematicalArgument::Scalar { .. }))))
    }
    for owner in &mut module.machines {
        for block in &mut owner.blocks {
            for operation in &mut block.operations {
                let terms = match &mut operation.kind {
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
                    } => erased_proof_arguments,
                    _ => continue,
                };
                if let Some(term) = find(terms) {
                    return Some(term);
                }
            }
            let edges = match &mut block.terminator {
                Terminator::Jump {
                    erased_proof_arguments,
                    ..
                }
                | Terminator::InvokeNoNormal {
                    erased_proof_arguments,
                    ..
                } => [Some(erased_proof_arguments), None],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => [
                    Some(&mut when_true.erased_proof_arguments),
                    Some(&mut when_false.erased_proof_arguments),
                ],
                _ => [None, None],
            };
            for terms in edges.into_iter().flatten() {
                if let Some(term) = find(terms) {
                    return Some(term);
                }
            }
        }
    }
    None
}
