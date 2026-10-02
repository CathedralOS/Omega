//! Structural operands retain their storage namespace even when no source
//! parameter or local declares their computed result type. All scalar/Unit
//! catalog entrances use these same roots; reservation still grants no custody.
use super::{CheckedTrees, LoweringError};
use checked_trees::{
    CheckedScalarComputationKind as Computation,
    CheckedScalarComputationStructuralArgument as Argument,
};

pub(crate) fn type_roots(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<String>, LoweringError> {
    let mut types = super::cases::type_roots(checked, &[machine])?;
    types.extend(super::fields::type_roots(checked, machine)?);
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
        .collect::<Vec<_>>();
    for handle in super::reachable_nodes(checked, &roots)? {
        match &plans.nodes.get(handle).kind {
            Computation::Call {
                structural_arguments,
                ..
            } => {
                for argument in plans
                    .structural_arguments
                    .span(*structural_arguments)
                    .ok_or(LoweringError::Unsupported(
                        "computed operand type roots lost their argument roster",
                    ))?
                {
                    if let Argument::Place(subject) = argument {
                        types.push(subject.type_identity.clone());
                    }
                }
            }
            Computation::ByteSequenceLiteralComparison { subject, .. }
            | Computation::ByteSequencePredicate { subject, .. }
            | Computation::CaseMembership {
                subject: Argument::Place(subject),
                ..
            } => {
                types.push(subject.type_identity.clone());
            }
            _ => {}
        }
    }
    types.sort();
    types.dedup();
    Ok(types)
}
