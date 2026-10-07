//! Reserve the complete structural producer at each authored call operand.
//! Reservation establishes no value: the ordinary structural schedule runs at
//! that operand's position, before the following operand can observe or mutate it.

use super::{CheckedTrees, LoweringError, unsupported};
use crate::expression_preparation::computation_graph::produced_value_root;
use crate::scalar_graph::scalar_graph_lowering::structural_values::Storage;
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument,
    CheckedStructuralValueHandle, CheckedUnitStructuralArgumentSourcePlan, FlowCallFact,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::StructuralTypeDeclaration;
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone)]
pub(crate) struct Slot {
    pub(crate) source_call: arena::Handle<FlowCallFact>,
    pub(crate) argument_ordinal: usize,
    pub(crate) root: CheckedStructuralValueHandle,
    pub(crate) type_reference: TypeReferenceHandle,
    pub(crate) storage: Storage,
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
}

pub(crate) fn prepare(
    checked: &CheckedTrees,
    machines: &[symbols::SymbolHandle],
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .filter_map(|(_, root)| machines.contains(&root.machine).then_some(root.root))
        .collect::<Vec<_>>();
    let mut slots: Vec<Slot> = Vec::new();
    for handle in super::reachable_nodes(checked, &roots)? {
        let CheckedScalarComputationKind::Call {
            source_call,
            structural_arguments,
            ..
        } = plans.nodes.get(handle).kind
        else {
            continue;
        };
        let arguments = plans
            .structural_arguments
            .span(structural_arguments)
            .ok_or(LoweringError::Unsupported(
                "computed producer has stale structural operands",
            ))?;
        for (argument_ordinal, argument) in arguments.iter().enumerate() {
            let CheckedScalarComputationStructuralArgument::Place(argument) = argument else {
                continue;
            };
            if !matches!(
                argument.source,
                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
            ) {
                continue;
            }
            let root = produced_value_root(checked, source_call, argument)?;
            let contract =
                validation::structural_result_contract(&checked.typed, root.type_reference)
                    .map_err(LoweringError::Unsupported)?;
            let identity = checked.normalized_type_identity(contract.carrier);
            let declaration = types
                .iter()
                .find(|declaration| declaration.identity == identity.as_str())
                .ok_or(LoweringError::Unsupported(
                    "computed producer has no structural type namespace",
                ))?;
            if let Some(existing) = slots.iter().find(|slot| {
                slot.source_call == source_call && slot.argument_ordinal == argument_ordinal
            }) {
                if existing.root != root.root
                    || existing.type_reference != root.type_reference
                    || existing.structural_type != declaration.id
                {
                    return unsupported(
                        "computed producer occurrence has conflicting roots or types",
                    );
                }
                continue;
            }
            let storage = Storage::reserve(checked, root.root, next_place)?;
            let place = storage.place(root.root)?;
            slots.push(Slot {
                source_call,
                argument_ordinal,
                root: root.root,
                type_reference: root.type_reference,
                storage,
                place,
                structural_type: declaration.id,
            });
        }
    }
    Ok(slots)
}
