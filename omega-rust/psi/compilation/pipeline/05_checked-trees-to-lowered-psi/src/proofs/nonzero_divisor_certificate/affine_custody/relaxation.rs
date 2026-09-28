//! One closed endpoint relaxation after exact affine mapping.

use std::rc::Rc;

use proof_admission::{CheckedIntegerAffineForm, IntegerAffineWitness, ProofNode, ProofRule};
use semantic_vocabulary::Proposition;

use super::DefinitionIndex;

mod completion;

/// The mapped endpoint relation this form establishes over the proved root
/// bound, memoized by the shared table because the same word meets the same
/// root bound under many goals.
pub(super) fn mapped_bound(
    definitions: &DefinitionIndex,
    form: &Rc<CheckedIntegerAffineForm>,
    root_bound: &Proposition,
) -> Option<Proposition> {
    definitions.affine_mapped(form, root_bound)
}

pub(super) fn prove(
    goal: &Proposition,
    definitions: &DefinitionIndex,
    form: &Rc<CheckedIntegerAffineForm>,
    root_bound: &ProofNode,
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
    let mapped_bound = mapped_bound(definitions, form, &root_bound.conclusion)?;
    let affine = ProofNode {
        conclusion: mapped_bound,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound.clone()),
            witness,
        },
    };
    completion::prove(goal, affine)
}
