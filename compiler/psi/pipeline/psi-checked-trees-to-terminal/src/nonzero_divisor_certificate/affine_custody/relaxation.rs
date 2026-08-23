//! One closed endpoint relaxation after exact affine mapping.

use psi_core::Proposition;
use psi_proof_kernel::{CheckedIntegerAffineForm, IntegerAffineWitness, ProofNode, ProofRule};

mod completion;
mod mapping;

pub(super) use mapping::mapped_bound;

pub(super) fn prove(
    goal: &Proposition,
    form: &CheckedIntegerAffineForm,
    root_bound: &ProofNode,
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
    let mapped_bound = mapped_bound(form, &root_bound.conclusion)?;
    let affine = ProofNode {
        conclusion: mapped_bound,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound.clone()),
            witness,
        },
    };
    completion::prove(goal, affine)
}
