/*
The structural queries the descriptor and reference graphs need: which places an
operation reads (`operation_place_inputs`), the declared source of a structural
place and the access it grants (`structural_source_contract`), and how a
structural path resolves. Whole-unit validation used to live here as well; it
served only the native abstract-operations optimizer and is archived with it, at
this file's path under `_recycling/`.
*/

pub(crate) mod function_structure {
    mod structural_roots {

        mod availability {

            use abstract_operations::AbstractOperation as O;

            use semantic_vocabulary::PlaceId;

            pub(crate) fn operation_place_accesses(operation: &O) -> Vec<PlaceId> {
                match operation {
                    O::DecomposeStructuralValue { subject, .. } => vec![subject.place],
                    O::ActivateTaskRegister {
                        gdt,
                        task_state,
                        scratch,
                        ..
                    } => vec![gdt.place, task_state.place, scratch.place],
                    O::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                        vec![gdt.place, scratch.place]
                    }
                    O::ReadTaskRegister { .. } | O::ReadSegmentSelector { .. } => vec![],
                    O::GlobalDescriptorTable { source, .. } => vec![source.place],
                    O::AtomicEvent { event, .. } => event.place().into_iter().collect(),
                    O::EstablishByteSequenceConcatenation { left, right, .. } => {
                        vec![left.source(), right.source()]
                    }
                    O::EstablishStructuralArray { elements, .. } => {
                        elements.iter().map(|argument| argument.place).collect()
                    }
                    abstract_operations::establish_case_or_record_fields_pattern!(fields) => fields
                        .iter()
                        .filter_map(|initializer| match &initializer.value {
                            terminal_psi::RecordFieldValue::Structural(argument) => {
                                Some(argument.place)
                            }
                            terminal_psi::RecordFieldValue::Scalar { .. } => None,
                        })
                        .collect(),
                    O::Jump {
                        structural_bindings,
                        ..
                    } => structural_bindings
                        .iter()
                        .map(|binding| binding.argument.place)
                        .collect(),
                    O::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => when_true
                        .structural_bindings
                        .iter()
                        .chain(&when_false.structural_bindings)
                        .map(|binding| binding.argument.place)
                        .collect(),
                    O::PrimitiveLocalStore { destination, .. } => vec![*destination],
                    O::WriteOnlyPrimitiveStore { destination, .. } => vec![destination.place()],
                    O::StructuralScalarFieldStore { destination, .. } => vec![destination.place()],
                    abstract_operations::boundary_or_structural_call_pair_pattern!(
                        structural_arguments,
                        ..
                    ) => structural_arguments
                        .iter()
                        .map(|argument| argument.place)
                        .collect(),
                    abstract_operations::byte_sequence_store_pair_pattern!(
                        destination,
                        source,
                        ..
                    )
                    | O::StructuralByteViewFieldStore {
                        destination,
                        source,
                        ..
                    } => vec![*destination, *source],
                    O::MoveStructuralField { source, .. } => vec![source.place()],
                    O::StoreStructuralField {
                        destination, value, ..
                    } => vec![destination.place(), value.place],
                    O::PackedIntegerUnary { source, .. } => vec![source.place],
                    O::PackedIntegerBinary { left, right, .. } => vec![left.place, right.place],
                    O::StructuralLeafCopy { source, .. } => vec![*source],
                    O::ReleaseActivationClaim { source, .. }
                    | O::SplitActivationClaim { source, .. }
                    | O::MoveStructuralValue { source, .. } => {
                        vec![source.place]
                    }
                    O::MergeActivationClaim { lower, upper, .. } => vec![lower.place, upper.place],
                    O::ByteSequencePredicate { operand, .. } => vec![operand.source()],
                    O::ByteSequenceEqual { left, right, .. } => vec![left.source(), right.source()],
                    O::InvokeNoNormal {
                        structural_arguments,
                        ..
                    }
                    | O::ExternalCompletion {
                        structural_arguments,
                        ..
                    } => structural_arguments
                        .iter()
                        .map(|argument| argument.place)
                        .collect(),
                    O::TransformContent {
                        source, parents, ..
                    } => std::iter::once(source.place)
                        .chain(parents.iter().map(|parent| parent.place))
                        .collect(),
                    O::EstablishReference { source, .. }
                    | O::EstablishAccessReference { source, .. }
                    | O::ByteSequenceSubslice { source, .. }
                    | O::ByteSequenceLength { source, .. }
                    | O::EstablishElementView { source, .. }
                    | O::ElementViewLength { source, .. }
                    | O::ElementViewRead { source, .. }
                    | O::ElementViewSubslice { source, .. }
                    | O::ElementViewFixedArray { source, .. } => {
                        vec![source.place]
                    }
                    O::ElementViewWrite { destination, .. } => vec![destination.place],
                    O::ReleaseReference { source, .. } => vec![*source],
                    abstract_operations::primitive_storage_observation_pattern!(source, ..)
                    | O::ByteSequenceRead { source, .. }
                    | O::ByteSequenceWrite {
                        destination: source,
                        ..
                    }
                    | O::StructuralByteSequenceFieldByteStore {
                        destination: source,
                        ..
                    }
                    | abstract_operations::byte_len_or_field_read_pattern!(source, ..)
                    | O::StructuralScalarFieldRead { source, .. }
                    | O::ReturnStructural { source, .. } => {
                        vec![*source]
                    }
                    O::StructuralCase { source, cases, .. } => std::iter::once(*source)
                        .chain(
                            cases
                                .iter()
                                .flat_map(|case| &case.structural_bindings)
                                .map(|binding| binding.argument.place),
                        )
                        .collect(),
                    O::EstablishActivationClaim { .. }
                    | O::EstablishNativeCodeLayout { .. }
                    | O::EstablishScalarArray { .. }
                    | O::EstablishPrimitiveLocal { .. }
                    | O::EstablishByteSequenceLiteral { .. }
                    | O::EstablishTrivialAffineLocal { .. }
                    | O::EstablishZeroedStructural { .. }
                    | O::EstablishStructuralLocal { .. }
                    | O::SelectImplementation { .. }
                    | O::SelectMachine { .. }
                    | O::PortWrite { .. }
                    | O::RegisterAccess { .. }
                    | O::Call { .. }
                    | O::IntegerConstant { .. }
                    | O::IeeeFloatConstant { .. }
                    | O::FloatResultPolicy { .. }
                    | O::NearestIeeeFloatFusedMultiplyAdd { .. }
                    | O::IeeeFloatCompare { .. }
                    | O::BooleanConstant { .. }
                    | O::BooleanNot { .. }
                    | O::BooleanEqual { .. }
                    | O::IntegerEqual { .. }
                    | O::IntegerLessThan { .. }
                    | O::IntegerLessOrEqual { .. }
                    | O::IntegerBitwiseNot { .. }
                    | O::IntegerWiden { .. }
                    | O::IntegerExactCast { .. }
                    | O::ScalarRepresentationView { .. }
                    | O::IntegerBitwiseAnd { .. }
                    | O::IntegerBitwiseOr { .. }
                    | O::IntegerBitwiseXor { .. }
                    | O::IntegerShift { .. }
                    | O::IntegerArithmetic { .. }
                    | O::TrappingInteger { .. }
                    | O::Return { .. }
                    | O::ReturnUnit { .. }
                    | O::EliminateEmpty { .. }
                    | O::Crash { .. } => Vec::new(),
                }
            }

            pub(crate) fn operation_place_inputs(operation: &O) -> Vec<PlaceId> {
                let mut inputs = operation_place_accesses(operation);
                for (_, actions) in operation.cleanup_rosters() {
                    inputs.extend(actions.iter().map(|action| action.place()));
                }
                inputs
            }

            #[cfg(test)]
            mod tests {
                use super::*;
                use abstract_operations::AbstractResult;
                use semantic_vocabulary::{OperationId, ScalarType, ValueId};
                use terminal_psi::{StructuralAccess, StructuralArgument};

                fn argument(number: u64) -> StructuralArgument {
                    StructuralArgument {
                        place: PlaceId::new(number).unwrap(),
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    }
                }

                #[test]
                fn operation_inputs_retain_all_byte_observations() {
                    let result = AbstractResult {
                        value: ValueId::new(1).unwrap(),
                        scalar_type: ScalarType::Boolean,
                    };
                    let left = terminal_psi::control_flow::ByteSequenceOperand::Whole {
                        source: argument(2),
                        length: ValueId::new(3).unwrap(),
                    };
                    let right = terminal_psi::control_flow::ByteSequenceOperand::Whole {
                        source: argument(4),
                        length: ValueId::new(5).unwrap(),
                    };
                    let predicate = O::ByteSequencePredicate {
                        psi_operation: OperationId::new(1).unwrap(),
                        result,
                        operand: left.clone(),
                        predicate: terminal_psi::ByteSequencePredicate::AsciiOnly,
                    };
                    assert_eq!(operation_place_inputs(&predicate), vec![argument(2).place]);
                    let equal = O::ByteSequenceEqual {
                        psi_operation: OperationId::new(2).unwrap(),
                        result,
                        left,
                        right,
                    };
                    assert_eq!(
                        operation_place_inputs(&equal),
                        vec![argument(2).place, argument(4).place]
                    );
                }

                #[test]
                fn operation_inputs_keep_content_parents_even_when_descriptor_query_selects_source()
                {
                    let result = terminal_psi::StructuralOperationResult {
                        place: PlaceId::new(6).unwrap(),
                        structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    };
                    let transform = O::TransformContent {
                        psi_operation: OperationId::new(1).unwrap(),
                        result,
                        source: argument(2),
                        parents: vec![argument(3), argument(4)],
                        derivations: Vec::new(),
                    };
                    assert_eq!(
                        operation_place_inputs(&transform),
                        vec![argument(2).place, argument(3).place, argument(4).place]
                    );
                }
            }
        }
        pub(crate) use availability::operation_place_inputs;
    }
    pub(crate) use structural_roots::operation_place_inputs;
}
pub(crate) mod operation_contracts {

    mod structural_access {
        use abstract_operations::AbstractOperation as O;

        use abstract_operations::element_view_or_byte_subslice_pattern;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{PlaceId, StructuralTypeId};

        use terminal_psi::StructuralAccess;

        use terminal_psi::StructuralOperationResult;

        #[derive(Debug, Clone, Copy)]
        pub(crate) struct StructuralSourceContract<'a> {
            pub(crate) structural_type: StructuralTypeId,
            pub(crate) multiplicity: terminal_psi::StructuralMultiplicity,
            pub(crate) access: StructuralAccess,
            operation_result: Option<&'a StructuralOperationResult>,
        }

        pub(crate) fn structural_source_contract(
            caller: &PsiOptimizationFunction,
            place: PlaceId,
        ) -> Option<StructuralSourceContract<'_>> {
            caller
                .arrival_parameter(place)
                .map(|parameter| StructuralSourceContract {
                    structural_type: parameter.structural_type,
                    multiplicity: parameter.multiplicity,
                    access: parameter.access,
                    operation_result: None,
                })
                .or_else(|| {
                    caller
                        .block_nodes()
                        .filter_map(|node| {
                            structural_operation_result_contract(caller, &node.operation)
                        })
                        .find(|source| {
                            source
                                .operation_result
                                .is_some_and(|result| result.place == place)
                        })
                })
                .or_else(|| trivial_affine_local_contract(caller, place))
                .or_else(|| {
                    caller.block_nodes().find_map(|node| {
                        let O::EstablishByteSequenceLiteral {
                            place: declaration,
                            structural_type,
                            ..
                        } = &node.operation
                        else {
                            return None;
                        };
                        (declaration.id == place).then_some(StructuralSourceContract {
                            structural_type: structural_type.id,
                            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                            access: StructuralAccess::SharedBorrow,
                            operation_result: None,
                        })
                    })
                })
        }

        fn trivial_affine_local_contract(
            caller: &PsiOptimizationFunction,
            place: PlaceId,
        ) -> Option<StructuralSourceContract<'_>> {
            caller.block_nodes().find_map(|node| match &node.operation {
                O::EstablishTrivialAffineLocal {
                    place: declaration,
                    structural_type,
                    ..
                } if declaration.id == place => Some(StructuralSourceContract {
                    structural_type: structural_type.id,
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    operation_result: None,
                }),
                _ => None,
            })
        }

        pub(crate) fn structural_operation_result_contract<'a>(
            caller: &'a PsiOptimizationFunction,
            operation: &'a O,
        ) -> Option<StructuralSourceContract<'a>> {
            let metadata = operation.structural_result_metadata()?;
            let access = match metadata.role {
                abstract_operations::StructuralResultRole::OwnedValue => StructuralAccess::Owned,
                abstract_operations::StructuralResultRole::PrimitiveLocalStorage => {
                    StructuralAccess::MutableBorrow
                }
                abstract_operations::StructuralResultRole::ViewRecipe => {
                    view_access(caller, operation)?
                }
            };
            let result = metadata.result;
            Some(StructuralSourceContract {
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                access,
                operation_result: Some(result),
            })
        }

        fn view_access(
            caller: &PsiOptimizationFunction,
            operation: &O,
        ) -> Option<StructuralAccess> {
            use semantic_vocabulary::StructuralPlaceKind;
            use terminal_psi::{
                StructuralAccess as Access, StructuralMultiplicity as Multiplicity,
            };

            let (_, initial) = operation.structural_result()?;
            let mut place = initial.place;
            let mut current = Some(operation);
            let mut expected_type = Some(initial.structural_type);
            let mut requested = None;
            let mut required = Access::SharedBorrow;
            for _ in 0..caller.structural_places.len() {
                let mut declarations = caller
                    .structural_places
                    .iter()
                    .filter(|row| row.id == place);
                let declaration = declarations.next()?;
                if declarations.next().is_some() {
                    return None;
                }
                if let Some(parameter) = caller.arrival_parameter(place) {
                    if expected_type.is_some_and(|identity| identity != parameter.structural_type)
                        || (expected_type.is_some()
                            && !matches!(
                                parameter.access,
                                Access::SharedBorrow | Access::MutableBorrow
                            ))
                        || !structural_access_can_supply(parameter.access, required)
                    {
                        return None;
                    }
                    return requested;
                }
                let (producer, structural_type) = match declaration.kind {
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } => (producer, structural_type),
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    } if required == Access::SharedBorrow
                        && expected_type.is_none_or(|identity| identity == structural_type) =>
                    {
                        return requested;
                    }
                    _ => return None,
                };
                let recorded = caller
                    .block_nodes()
                    .find(|node| node.operation.psi_operation() == Some(producer))?;
                let operation = current.take().unwrap_or(&recorded.operation);
                if operation != &recorded.operation {
                    return None;
                }
                let (actual_producer, result) = operation.structural_result()?;
                if producer != actual_producer
                    || result.place != place
                    || result.structural_type != structural_type
                    || expected_type.is_some_and(|identity| identity != structural_type)
                {
                    return None;
                }
                match operation {
                    element_view_or_byte_subslice_pattern!(source, ..) => {
                        if !result.qualifications.is_empty()
                            || result
                                .projected_qualifications
                                .iter()
                                .any(|row| !row.is_element_membership())
                            || !result.qualification_establishments.is_empty()
                            || !result.claims.is_empty()
                        {
                            return None;
                        }
                        let access = match result.multiplicity {
                            Multiplicity::Unrestricted => Access::SharedBorrow,
                            Multiplicity::Affine => Access::MutableBorrow,
                            _ => return None,
                        };
                        if !structural_access_can_supply(access, required) {
                            return None;
                        }
                        requested.get_or_insert(access);
                        required = access;
                        if !structural_access_can_supply(source.access, required) {
                            return None;
                        }
                        expected_type = if source.path.is_empty() {
                            Some(
                                if let Some(parameter) = caller.arrival_parameter(source.place) {
                                    parameter.structural_type
                                } else {
                                    let declaration = caller
                                        .structural_places
                                        .iter()
                                        .find(|declaration| declaration.id == source.place)?;
                                    match declaration.kind {
                                        StructuralPlaceKind::OperationResult {
                                            structural_type,
                                            ..
                                        }
                                        | StructuralPlaceKind::ByteSequenceLiteral {
                                            structural_type,
                                            ..
                                        } => structural_type,
                                        _ => return None,
                                    }
                                },
                            )
                        } else {
                            None
                        };
                        place = source.place;
                    }
                    O::EstablishElementView {
                        source,
                        destination,
                        ..
                    } => {
                        if *destination != result.place
                            || !matches!(
                                (source.access, result.multiplicity),
                                (Access::SharedBorrow, Multiplicity::Unrestricted)
                                    | (Access::MutableBorrow, Multiplicity::Affine)
                            )
                            || !structural_access_can_supply(source.access, required)
                            || !result.qualifications.is_empty()
                            || result
                                .projected_qualifications
                                .iter()
                                .any(|row| !row.is_element_membership())
                            || !result.qualification_establishments.is_empty()
                            || !result.claims.is_empty()
                        {
                            return None;
                        }
                        requested.get_or_insert(source.access);
                        required = source.access;
                        expected_type = None;
                        place = source.place;
                    }
                    _ => {
                        let source = structural_operation_result_contract(caller, operation)?;
                        let access = if expected_type.is_some() && source.access == Access::Owned {
                            operation.materialized_structural_result()?;
                            match source.multiplicity {
                                Multiplicity::Unrestricted => Access::SharedBorrow,
                                Multiplicity::Affine => Access::MutableBorrow,
                                Multiplicity::Linear => return None,
                            }
                        } else {
                            source.access
                        };
                        return if structural_access_can_supply(access, required) {
                            requested
                        } else {
                            None
                        };
                    }
                }
            }
            None
        }

        pub(crate) fn structural_access_can_supply(
            source: StructuralAccess,
            presented: StructuralAccess,
        ) -> bool {
            match source {
                StructuralAccess::Owned => true,
                StructuralAccess::SharedBorrow => presented == StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow => matches!(
                    presented,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                ),
                StructuralAccess::WriteOnlyBorrow => presented == StructuralAccess::WriteOnlyBorrow,
            }
        }
    }
    pub(crate) use structural_access::structural_source_contract;
}
pub(crate) mod structural_catalog {

    mod paths {
        use crate::StructuralTypes;
        use semantic_vocabulary::StructuralTypeId;
        use std::collections::BTreeMap;

        use terminal_psi::StructuralFieldType;
        use terminal_psi::StructuralPathSegment;
        use terminal_psi::StructuralTypeDeclaration;

        pub(crate) fn resolve_structural_path(
            types: &(impl StructuralTypes + ?Sized),
            structural_type: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Option<StructuralTypeId> {
            resolve_path(
                types.declarations(),
                |identity| types.declaration(identity),
                structural_type,
                path,
                StructuralFieldType::canonical_leaf_type,
            )
        }

        use structural_storage_queries::storage_projection::resolve_path;

        impl StructuralTypes for BTreeMap<StructuralTypeId, &StructuralTypeDeclaration> {
            fn declaration(&self, id: StructuralTypeId) -> Option<&StructuralTypeDeclaration> {
                self.get(&id).copied()
            }

            fn declarations(&self) -> impl Iterator<Item = &StructuralTypeDeclaration> + Clone {
                self.values().copied()
            }
        }
    }
    mod storage_projection {
        pub use structural_storage_queries::storage_projection::{
            StructuralStorageProjection, structural_borrowed_storage_projection,
        };
    }

    pub use storage_projection::{
        StructuralStorageProjection, structural_borrowed_storage_projection,
    };

    pub(crate) use paths::resolve_structural_path;
}
