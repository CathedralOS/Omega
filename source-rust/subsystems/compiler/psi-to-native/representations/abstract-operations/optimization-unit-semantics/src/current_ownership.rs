/*
We reconstruct and replay live ownership frontiers across each function's
control-flow graph from its current executable operations and signatures. We
track live claims, non-unrestricted owned places, partial-affine field move
paths, and outstanding reference loans across block entries, operation nodes,
call regions, address joins, and edge transitions, ensuring that every disposal
obligation, loan ceiling, and window restoration contract is satisfied without
trusting cached ownership events.
*/

pub(crate) mod address_joins {
    pub(crate) mod range_relations {
        use abstract_operations::{AbstractOperation as O, ValueBinding};
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{BlockId, MachineId, Proposition, ScalarType, ValueId};
        use std::collections::BTreeMap;
        use terminal_semantics::scalar_ordering::{Binding, Edge, Effect, ScalarOrdering, Step};

        pub(crate) struct RangeRelations(ScalarOrdering);
        impl RangeRelations {
            pub(crate) fn new(
                function: &PsiOptimizationFunction,
                functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
            ) -> Self {
                let positions = function
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        (0..block.nodes.len()).filter_map(|index| {
                            u32::try_from(index).ok().map(|index| (block.id, index))
                        })
                    })
                    .collect::<Vec<_>>();
                let mut steps = Vec::with_capacity(positions.len());
                for position in &positions {
                    let block = function
                        .block(position.0)
                        .expect("position comes from block");
                    let operation = &block.nodes[position.1 as usize].operation;
                    let edge = |target: BlockId, bindings: &[ValueBinding], condition| {
                        let target_index = positions
                            .iter()
                            .position(|position| *position == (target, 0))?;
                        let block = function.block(target)?;
                        Some(Edge {
                            target: target_index,
                            enters_block: true,
                            parameters: block
                                .parameters
                                .iter()
                                .map(|parameter| (parameter.value, parameter.scalar_type))
                                .collect(),
                            bindings: bindings
                                .iter()
                                .map(|binding| Binding {
                                    parameter: binding.parameter,
                                    argument: binding.argument,
                                    scalar_type: binding.scalar_type,
                                })
                                .collect(),
                            condition,
                        })
                    };
                    let successors = match operation {
                        O::Jump {
                            target, bindings, ..
                        } => edge(*target, bindings, None).into_iter().collect(),
                        O::Conditional {
                            condition,
                            when_true,
                            when_false,
                        } => [
                            edge(
                                when_true.target,
                                &when_true.bindings,
                                Some((*condition, true)),
                            ),
                            edge(
                                when_false.target,
                                &when_false.bindings,
                                Some((*condition, false)),
                            ),
                        ]
                        .into_iter()
                        .flatten()
                        .collect(),
                        O::StructuralCase { cases, .. } => cases
                            .iter()
                            .filter_map(|case| edge(case.target, &[], None))
                            .collect(),
                        abstract_operations::return_or_return_unit_pattern!()
                        | abstract_operations::return_structural_or_crash_pattern!() => Vec::new(),
                        _ => positions
                            .iter()
                            .position(|next| *next == (position.0, position.1 + 1))
                            .map(|target| Edge {
                                target,
                                enters_block: false,
                                parameters: Vec::new(),
                                bindings: Vec::new(),
                                condition: None,
                            })
                            .into_iter()
                            .collect(),
                    };
                    steps.push(Step {
                        position: *position,
                        definitions:
                            crate::unit_validation::derived_metadata::expected_definitions(
                                operation, position.0, position.1,
                            )
                            .iter()
                            .map(|definition| (definition.value, definition.scalar_type))
                            .collect(),
                        effect: match operation {
                            O::IntegerLessThan {
                                result,
                                left,
                                right,
                                ..
                            }
                            | O::IntegerLessOrEqual {
                                result,
                                left,
                                right,
                                ..
                            } => Effect::Compare {
                                result: *result,
                                left: *left,
                                right: *right,
                                strict: matches!(operation, O::IntegerLessThan { .. }),
                            },
                            O::BooleanNot {
                                result, operand, ..
                            } => Effect::Not {
                                result: *result,
                                operand: *operand,
                            },
                            O::IntegerConstant {
                                result,
                                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                                ..
                            } => Effect::UnsignedLiteral {
                                result: *result,
                                value: *value,
                            },
                            O::EstablishPrimitiveLocal { result, value, .. } => {
                                Effect::EstablishStorage {
                                    place: result.place,
                                    value: value.value,
                                }
                            }
                            O::PrimitiveLocalStore {
                                destination, value, ..
                            } => Effect::StoreStorage {
                                place: *destination,
                                value: value.value,
                            },
                            O::WriteOnlyPrimitiveStore {
                                destination,
                                path,
                                value,
                                ..
                            } if path.is_empty() => Effect::StoreStorage {
                                place: destination.place(),
                                value: value.value,
                            },
                            O::PrimitiveScalarRead {
                                source,
                                path,
                                result,
                                ..
                            } if path.is_empty() => Effect::ReadStorage {
                                place: *source,
                                result: result.value,
                            },
                            O::EstablishReference { .. }
                            | O::EstablishAccessReference { .. }
                            | O::EstablishElementView { .. } => Effect::None,
                            abstract_operations::call_or_call_structural_pattern!(..)
                            | O::BoundaryCall { .. }
                            | O::ExternalCompletion { .. } => Effect::ForgetStorage,
                            _ if !operation.storage_mutations().is_empty() => Effect::ForgetStorage,
                            _ => Effect::None,
                        },
                        successors,
                    });
                }
                let scalar_types = function
                    .parameters
                    .iter()
                    .chain(function.block_parameters())
                    .map(|parameter| (parameter.value, parameter.scalar_type))
                    .chain(
                        steps
                            .iter()
                            .flat_map(|step| step.definitions.iter().copied()),
                    )
                    .collect::<Vec<_>>();
                let normal_returns = function
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        let scalar_types = &scalar_types;
                        block
                            .nodes
                            .iter()
                            .enumerate()
                            .filter_map(move |(index, node)| {
                                let propositions =
                                    call_guarantees(&node.operation, functions, scalar_types)?;
                                Some(((block.id, u32::try_from(index).ok()?), propositions))
                            })
                    })
                    .collect();
                Self(ScalarOrdering::with_contracts(
                    steps,
                    (function.entry, 0),
                    function
                        .parameters
                        .iter()
                        .map(|parameter| (parameter.value, parameter.scalar_type))
                        .collect(),
                    function
                        .verified_contract
                        .as_ref()
                        .map_or(&[], |contract| contract.requires.as_slice()),
                    normal_returns,
                ))
            }
            pub(crate) fn into_ordering(self) -> ScalarOrdering {
                self.0
            }
        }
        fn call_guarantees(
            operation: &O,
            functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
            scalar_types: &[(ValueId, ScalarType)],
        ) -> Option<Vec<Proposition>> {
            let guarantees = crate::descriptor_results::scalar_call_guarantees(
                operation,
                &|identity| functions.get(&identity).copied(),
                &|identity| {
                    let (_, selected) =
                        scalar_types.iter().find(|(value, _)| *value == identity)?;
                    scalar_types
                        .iter()
                        .all(|(value, candidate)| *value != identity || candidate == selected)
                        .then_some(*selected)
                },
            )?;
            let bindings = scalar_types
                .iter()
                .map(|(value, scalar_type)| {
                    (
                        *value,
                        semantic_vocabulary::ScalarTerm::value(*value, *scalar_type),
                    )
                })
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect::<Vec<_>>();
            Some(
                guarantees
                    .iter()
                    .flat_map(|clause| {
                        terminal_semantics::scalar_ordering::instantiate_contract_ordering(
                            clause, &bindings,
                        )
                    })
                    .collect(),
            )
        }
    }
}
mod call_accesses {

    use super::storage_regions::storage_cannot_retain_loan;

    use crate::unit_validation::structural_catalog::resolve_structural_path;

    use semantic_vocabulary::StructuralTypeId;
    use std::collections::BTreeMap;
    use terminal_psi::{
        StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    };

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
        if path.is_empty() && access != StructuralAccess::Owned {
            match types.get(&selected).map(|declaration| &declaration.shape) {
                Some(StructuralTypeShape::ByteSequence(carrier)) if carrier.is_borrowed() => {
                    return true;
                }
                Some(StructuralTypeShape::ElementView { element, .. }) => {
                    return storage_cannot_retain_loan(types, *element, established);
                }
                _ => {}
            }
        }
        storage_cannot_retain_loan(types, selected, established)
    }
}
pub(crate) use call_accesses::argument_storage_is_closed;
mod storage_regions {

    use semantic_vocabulary::StructuralTypeId;
    use std::collections::BTreeMap;
    use terminal_psi::{
        StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration,
        StructuralTypeShape,
    };

    pub(crate) fn storage_cannot_retain_loan(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
        established: &mut BTreeMap<StructuralTypeId, bool>,
    ) -> bool {
        if let Some(result) = established.get(&structural_type) {
            return *result;
        }
        established.insert(structural_type, false);
        let Some(declaration) = types.get(&structural_type) else {
            return false;
        };
        let result = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::AtomicCell(_) => true,
            StructuralTypeShape::ByteSequence(carrier) => !carrier.is_borrowed(),
            StructuralTypeShape::Record { fields, .. } => {
                fields_cannot_retain_loan(types, fields, established)
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                storage_cannot_retain_loan(types, *element, established)
            }
            StructuralTypeShape::Sum { cases } => cases
                .iter()
                .all(|case| fields_cannot_retain_loan(types, &case.fields, established)),
            StructuralTypeShape::Mixed { fields, cases } => {
                fields_cannot_retain_loan(types, fields, established)
                    && cases
                        .iter()
                        .all(|case| fields_cannot_retain_loan(types, &case.fields, established))
            }
            StructuralTypeShape::TypeParameter
            | StructuralTypeShape::Opaque
            | StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ElementView { .. } => false,
        };
        established.insert(structural_type, result);
        result
    }

    fn fields_cannot_retain_loan(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        fields: &[StructuralFieldDeclaration],
        established: &mut BTreeMap<StructuralTypeId, bool>,
    ) -> bool {
        fields.iter().all(|field| match &field.field_type {
            StructuralFieldType::Scalar(_)
            | StructuralFieldType::Atomic(_)
            | StructuralFieldType::AtomicBoundedInteger(_)
            | StructuralFieldType::RefinedScalar(_)
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::IeeeFloat(_) => true,
            StructuralFieldType::ByteSequence(carrier)
            | StructuralFieldType::PredicateByteSequence { carrier, .. } => !carrier.is_borrowed(),
            StructuralFieldType::Structural(child) => {
                storage_cannot_retain_loan(types, *child, established)
            }
            StructuralFieldType::Erased { .. } => field.relevance.is_erased(),
        })
    }
}
