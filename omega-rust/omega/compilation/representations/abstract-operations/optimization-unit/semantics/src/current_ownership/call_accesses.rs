//! Current ordinary callees refine the storage a call can disturb.
//!
//! A mutable formal grants authority, not evidence that every byte is written.
//! Reconstruct writes from the current operation inventory and borrowed-region
//! origins, then substitute the exact actual projection. Never use the immutable
//! Terminal snapshot or a source-language receiver spelling. Missing callees,
//! recursive closures, opaque effects and unresolved local origins retain the
//! ordinary whole-input fence. This proves interference only, not loan closure.

use super::address_joins::borrowed_regions::{BorrowedRegions, Projection};
use super::storage_regions::{storage_cannot_retain_loan, written_or_exclusive_regions};
use crate::unit_validation::operation_contracts::structural_source_contract;
use crate::unit_validation::structural_catalog::resolve_structural_path;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::{
    StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

#[derive(Clone)]
struct ParameterMutation {
    position: usize,
    projection: Projection,
}

#[derive(Clone)]
struct FunctionMutations {
    parameters: usize,
    writes: Vec<ParameterMutation>,
}

/// Prepared once for the current unit; rewritten units reconstruct this table.
#[derive(Default)]
pub(crate) struct CallAccesses {
    functions: BTreeMap<MachineId, Option<FunctionMutations>>,
}

impl CallAccesses {
    pub(crate) fn new(
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Self {
        let mut result = Self::default();
        let mut closed_storage = BTreeMap::new();
        for machine in functions.keys() {
            result.reconstruct(*machine, functions, types, &mut closed_storage);
        }
        result
    }

    fn reconstruct(
        &mut self,
        machine: MachineId,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        closed_storage: &mut BTreeMap<StructuralTypeId, bool>,
    ) {
        if self.functions.contains_key(&machine) {
            return;
        }
        // An in-progress recursive closure has no proved summary yet.
        self.functions.insert(machine, None);
        let Some(function) = functions.get(&machine) else {
            return;
        };
        let origins = BorrowedRegions::origins(function, functions, types);
        let mut writes = Vec::<ParameterMutation>::new();
        for node in function.blocks.iter().flat_map(|block| &block.nodes) {
            if let O::CallStructural { callee, .. } = &node.operation {
                self.reconstruct(*callee, functions, types, closed_storage);
                if !self.functions.get(callee).is_some_and(Option::is_some) {
                    return;
                }
            }
            if matches!(
                node.operation,
                O::CallDynamic { .. }
                    | O::Call { .. }
                    | O::AtomicEvent { .. }
                    | O::PortWrite { .. }
            ) {
                return;
            }
            if let O::BoundaryCall {
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
                ..
            } = &node.operation
                && (structural_arguments.iter().any(|argument| {
                    argument.access == StructuralAccess::Owned
                        || !structural_source_contract(function, argument.place).is_some_and(
                            |source| {
                                argument_storage_is_closed(
                                    types,
                                    source.structural_type,
                                    &argument.path,
                                    argument.access,
                                    closed_storage,
                                )
                            },
                        )
                }) || !completion_claim_sources.is_empty()
                    || !completion_receipts.is_empty())
            {
                // A borrowed record may contain a mutable reference: its
                // boundary callee can then write outside the record's bytes.
                // Exact outer access alone cannot bound that carried authority.
                // Closed referents and direct view payloads use the ordinary
                // backing inventory; other carriers and completion custody
                // require an additional authority mapping.
                return;
            }
            if let O::Return {
                cleanup_actions, ..
            }
            | O::ReturnUnit {
                cleanup_actions, ..
            } = &node.operation
                && cleanup_actions.iter().any(|action| {
                    matches!(
                        action,
                        terminal_psi::TerminalAffineCleanupAction::InvokeNominal(_)
                    )
                })
            {
                // Nominal cleanup executes code after the result is observed;
                // absence of an explicit store node does not make it read-only.
                return;
            }
            for (place, region) in self.written_regions(function, types, &node.operation) {
                let mut projection = Projection::new(&region.path);
                projection.indirect |= region.indirect;
                for region in origins.resolve(place, projection) {
                    let Some(position) = function
                        .structural_parameters
                        .iter()
                        .position(|parameter| Some(parameter.place) == region.root)
                    else {
                        // A local carrier can itself contain borrowed storage.
                        // Absence from the formal roster is not disjointness.
                        return;
                    };
                    if function.structural_parameters[position].access == StructuralAccess::Owned {
                        // Copy/move and carried-reference effects require their
                        // own custody mapping; do not reinterpret owned inputs.
                        return;
                    }
                    let projection = region.projection.without_runtime_coordinates();
                    if !writes.iter().any(|existing| {
                        existing.position == position
                            && existing.projection.path == projection.path
                            && existing.projection.exact == projection.exact
                            && existing.projection.indirect == projection.indirect
                    }) {
                        writes.push(ParameterMutation {
                            position,
                            projection,
                        });
                    }
                }
            }
        }
        self.functions.insert(
            machine,
            Some(FunctionMutations {
                parameters: function.structural_parameters.len(),
                writes,
            }),
        );
    }

    pub(super) fn written_regions(
        &self,
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        operation: &O,
    ) -> Vec<(PlaceId, terminal_semantics::StorageRegion)> {
        if let O::CallStructural {
            callee,
            structural_arguments,
            dynamic_arguments,
            claim_transfers,
            ..
        } = operation
            && dynamic_arguments.is_empty()
            && claim_transfers.is_empty()
            && let Some(Some(summary)) = self.functions.get(callee)
            && summary.parameters == structural_arguments.len()
        {
            return summary
                .writes
                .iter()
                .map(|write| {
                    let actual = &structural_arguments[write.position];
                    let projection = Projection::new(&actual.path).then(&write.projection);
                    (
                        actual.place,
                        terminal_semantics::StorageRegion {
                            path: projection.path,
                            indirect: projection.indirect,
                        },
                    )
                })
                .collect();
        }
        written_or_exclusive_regions(function, types, operation)
    }
}

pub(crate) fn argument_storage_is_closed(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    source: StructuralTypeId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
    established: &mut BTreeMap<StructuralTypeId, bool>,
) -> bool {
    let Some(selected) = resolve_structural_path(types, source, path) else {
        return false;
    };
    // A direct descriptor is already a backing alias in BorrowedRegions.
    // A descriptor nested inside a record is not that record's own backing.
    if path.is_empty() && access != StructuralAccess::Owned {
        match types.get(&selected).map(|declaration| &declaration.shape) {
            Some(StructuralTypeShape::ByteSequence(carrier)) if carrier.is_borrowed() => {
                return true;
            }
            Some(StructuralTypeShape::ElementView { element }) => {
                return storage_cannot_retain_loan(types, *element, established);
            }
            _ => {}
        }
    }
    storage_cannot_retain_loan(types, selected, established)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::{BindingRelevance, StructuralFieldDeclaration, StructuralFieldType};

    #[test]
    fn descriptor_loans_boundary_summary_rejects_nested_carried_authority() {
        let identity = |number| StructuralTypeId::new(number).unwrap();
        let record = |child| StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                identity: "payload".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(identity(child)),
            }],
        };
        let declarations = [
            StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Boolean),
            record(1),
            StructuralTypeShape::Reference {
                referent: identity(1),
                access: StructuralAccess::MutableBorrow,
            },
            record(3),
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::MutableBorrow),
            }),
            record(5),
            StructuralTypeShape::ElementView {
                element: identity(2),
            },
            StructuralTypeShape::ElementView {
                element: identity(4),
            },
            StructuralTypeShape::FusedServiceBinding,
            record(10),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, shape)| StructuralTypeDeclaration {
            id: identity(index as u64 + 1),
            identity: format!("test::Storage{index}"),
            shape,
        })
        .collect::<Vec<_>>();
        let types = declarations.iter().map(|row| (row.id, row)).collect();
        let mut established = BTreeMap::new();
        for (number, closed) in [
            (1, true),
            (2, true),
            (3, false),
            (4, false),
            (5, true),
            (6, false),
            (7, true),
            (8, false),
            (9, false),
            (10, false),
            (11, false),
        ] {
            assert_eq!(
                argument_storage_is_closed(
                    &types,
                    identity(number),
                    &[],
                    StructuralAccess::MutableBorrow,
                    &mut established
                ),
                closed,
                "boundary storage type {number}"
            );
        }
        let path = [StructuralPathSegment::Field("payload".into())];
        for number in [5, 7] {
            assert!(
                !argument_storage_is_closed(
                    &types,
                    identity(number),
                    &[],
                    StructuralAccess::Owned,
                    &mut established,
                ),
                "owning a descriptor does not make its captured loan closed"
            );
        }
        assert!(argument_storage_is_closed(
            &types,
            identity(2),
            &path,
            StructuralAccess::MutableBorrow,
            &mut established,
        ));
        for number in [4, 6] {
            assert!(!argument_storage_is_closed(
                &types,
                identity(number),
                &path,
                StructuralAccess::MutableBorrow,
                &mut established,
            ));
        }
        assert!(!argument_storage_is_closed(
            &types,
            identity(2),
            &[StructuralPathSegment::Field("missing".into())],
            StructuralAccess::MutableBorrow,
            &mut established,
        ));
    }

    #[test]
    fn descriptor_loans_call_summary_does_not_import_callee_selector_ids() {
        use abstract_operations::{
            AbstractResult, AbstractStructuralCallResult, StructuralScalarStoreDestination,
        };
        use semantic_vocabulary::{
            ObligationId, OperationId, ScalarType, StructuralFieldId, StructuralPlaceKind, ValueId,
        };
        use terminal_psi::{
            StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
            StructuralPlaceDeclaration,
        };

        // Raw current-IR receiver fixture: both machines intentionally use the
        // same numeric scalar identifier for different runtime values.
        let mut callee = crate::tests::fixtures::scalar_units::unit()
            .functions
            .remove(0);
        let source = PlaceId::new(91).unwrap();
        let structural_type = StructuralTypeId::new(92).unwrap();
        let selector = ValueId::new(93).unwrap();
        let parameter = StructuralParameterDeclaration {
            place: source,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        callee.structural_parameters = vec![parameter.clone()];
        callee.structural_places = vec![StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }];
        let selected = StructuralPathSegment::RuntimeIndex {
            index: selector,
            obligation: ObligationId::new(94).unwrap(),
        };
        callee.blocks[0].nodes[0].operation = O::StructuralScalarFieldStore {
            psi_operation: OperationId::new(95).unwrap(),
            destination: StructuralScalarStoreDestination::Parameter(parameter),
            path: vec![selected.clone()],
            field: StructuralFieldId::new(96).unwrap(),
            value: AbstractResult {
                value: ValueId::new(97).unwrap(),
                scalar_type: ScalarType::Boolean,
            },
            range_obligation: None,
        };
        let functions = BTreeMap::from([(callee.machine, &callee)]);
        let types = BTreeMap::new();
        let accesses = CallAccesses::new(&functions, &types);
        let summary = accesses.functions[&callee.machine]
            .as_ref()
            .expect("closed current callee");
        assert_eq!(summary.writes.len(), 1);
        assert!(summary.writes[0].projection.path.is_empty());
        assert!(!summary.writes[0].projection.exact);

        let caller = crate::tests::fixtures::scalar_units::unit()
            .functions
            .remove(0);
        let mut call = O::CallStructural {
            psi_operation: OperationId::new(98).unwrap(),
            callee: callee.machine,
            arguments: vec![selector],
            structural_arguments: vec![StructuralArgument {
                place: PlaceId::new(99).unwrap(),
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }],
            dynamic_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            result: AbstractStructuralCallResult::Unit,
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        let writes = accesses.written_regions(&caller, &types, &call);
        assert_eq!(writes.len(), 1);
        assert!(
            writes[0].1.path.is_empty(),
            "callee selector cannot be treated as caller's identically numbered selector"
        );
        let O::CallStructural {
            structural_arguments,
            ..
        } = &mut call
        else {
            panic!()
        };
        structural_arguments[0].path = vec![selected.clone()];
        let writes = accesses.written_regions(&caller, &types, &call);
        assert_eq!(
            writes[0].1.path,
            vec![selected],
            "the actual caller's selected element retains its own coordinate"
        );
    }
}
