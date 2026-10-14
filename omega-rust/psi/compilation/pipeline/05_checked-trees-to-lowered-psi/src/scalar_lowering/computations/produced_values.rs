//! Reserve the complete structural producer at each authored call operand.
//! Reservation establishes no value: the ordinary structural schedule runs at
//! that operand's position, before the following operand can observe or mutate it.
//! Checked value roots own admission, including borrowed descriptor custody.
//! Reservation resolves their published carrier; it does not reapply an
//! owned-result contract to a descriptor that lends original backing.

use super::{CheckedTrees, LoweringError, unsupported};
use crate::expression_preparation::computation_graph::produced_value_root;
use crate::scalar_lowering::branch_states::structural_values::Storage;
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
    /// None is the separately evaluated authority, never a formal argument.
    pub(crate) argument_ordinal: Option<usize>,
    pub(crate) root: CheckedStructuralValueHandle,
    pub(crate) type_reference: TypeReferenceHandle,
    pub(crate) storage: Storage,
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
}

pub(crate) fn prepare(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machines: &[symbols::SymbolHandle],
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Slot>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans.roots_in_machines(machines);
    let mut slots: Vec<Slot> = Vec::new();
    for handle in super::reachable_nodes(checked, &roots)? {
        let CheckedScalarComputationKind::Call {
            source_call,
            structural_arguments,
            ref callee,
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
        let mut operands = Vec::new();
        let authority = callee.authority_value();
        if authority.is_valid() {
            let node = checked.facts.values.structural_values.nodes.get(authority);
            let root = checked
                .facts
                .values
                .structural_values
                .root_for_expression(node.state, node.statement_index, node.expression)
                .ok_or(LoweringError::Unsupported(
                    "computed authority lost its value root",
                ))?;
            if root.root != authority {
                return unsupported("computed authority has conflicting value roots");
            }
            operands.push((None, root));
        }
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
            operands.push((Some(argument_ordinal), root));
        }
        for (argument_ordinal, root) in operands {
            let identity = validation::stored_structural_type_identity(
                &checked.typed,
                root.type_reference,
                &[],
                &[],
            );
            let declaration = crate::terminal_identities::structural_declaration_by_identity(
                types,
                identities,
                checked,
                identity,
                "computed producer has no structural type namespace",
            )?;
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
            let storage = Storage::reserve(identities, checked, root.root, types, next_place)?;
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
