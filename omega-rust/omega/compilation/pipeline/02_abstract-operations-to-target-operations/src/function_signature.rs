/*
  Function signature preparation (`prepare_function_signature`,
  `PreparedFunctionSignature`), scalar ABI derivation (`scalar_abi`), structural
  call signatures (`structural_signature`), borrowed storage provenance
  (`borrowed_storage`), and structural type lookup (`StructuralTypeLookup`).

  Evaluates target calling plans for function parameters and results, separates
  owned value placements from `BorrowedReference` descriptors, and binds fixed
  scalar and mixed structural-scalar calling conventions.
*/
use super::control_flow::aggregate_results::result_home_layout;
use super::structural_signature::{StructuralCallPlan, StructuralCallSignature};
use super::unit::known_integers::KnownUnitInteger;
use crate::LoweringError;
use crate::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractFunctionResult};
use calling_conventions::ValueShape;
use semantic_vocabulary::{MachineId, PlaceId, ScalarType, ValueId};
use std::collections::BTreeMap;
use target::NativeTarget;
use target_operations::{ScalarAbiValue, TargetStructuralParameter};

pub(crate) fn integer_parameters(
    machine: MachineId,
    parameters: &[ScalarAbiValue],
) -> Result<BTreeMap<ValueId, KnownUnitInteger>, LoweringError> {
    parameters
        .iter()
        .enumerate()
        .filter_map(|(parameter_index, parameter)| match parameter.scalar_type {
            ScalarType::Callable(_) | ScalarType::Boolean | ScalarType::IeeeFloat(_) => None,
            ScalarType::Integer(scalar_type) => Some(
                u32::try_from(parameter_index)
                    .map(|parameter_index| {
                        (
                            parameter.value,
                            KnownUnitInteger::Parameter {
                                parameter_index,
                                scalar_type,
                            },
                        )
                    })
                    .map_err(|_| LoweringError::UnitFunctionHasScalarParameters(machine)),
            ),
        })
        .collect()
}

pub(crate) struct PreparedFunctionSignature {
    pub(crate) call_plan: StructuralCallPlan,
    pub(crate) scalar_parameters: Vec<ScalarAbiValue>,
    pub(crate) parameters: Vec<TargetStructuralParameter>,
}

impl PreparedFunctionSignature {
    pub(crate) fn parameter(&self, place: PlaceId) -> Option<&TargetStructuralParameter> {
        self.parameters
            .iter()
            .find(|parameter| parameter.place == place)
    }
}

pub(crate) fn parameters_by_place(
    parameters: &[TargetStructuralParameter],
) -> BTreeMap<PlaceId, &TargetStructuralParameter> {
    parameters
        .iter()
        .map(|parameter| (parameter.place, parameter))
        .collect()
}

pub(crate) fn prepare_function_signature(
    function: &AbstractFunction,
    target: NativeTarget,
    structural_types: &StructuralTypeLookup<'_>,
) -> Result<PreparedFunctionSignature, LoweringError> {
    let scalar_parameter_shapes = function
        .parameters
        .iter()
        .map(|parameter| {
            ValueShape::for_scalar(parameter.scalar_type).ok_or(
                LoweringError::UnitFunctionHasScalarParameters(function.machine),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = StructuralCallSignature::derive(
        &scalar_parameter_shapes,
        function
            .structural_parameters
            .iter()
            .map(|parameter| (parameter.structural_type, parameter.access)),
        match &function.result {
            AbstractFunctionResult::Structural(result) => {
                Some(result_home_layout(result, structural_types)?.shape())
            }
            _ => function
                .result
                .scalar()
                .map(|result| super::scalar::scalar_shape(result.value, result.scalar_type, false))
                .transpose()?,
        },
        structural_types,
    )?;
    let parameter_shapes = signature.structural_shapes().to_vec();
    let call_plan = signature.plan(target)?;
    let expected_parameter_count = function.parameters.len() + function.structural_parameters.len();
    if call_plan.parameters.len() != expected_parameter_count {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: expected_parameter_count,
            actual: call_plan.parameters.len(),
        });
    }
    let scalar_parameters = function
        .parameters
        .iter()
        .zip(&scalar_parameter_shapes)
        .zip(&call_plan.parameters)
        .map(|((parameter, expected_shape), placement)| {
            if placement.shape != *expected_shape {
                return Err(LoweringError::UnsupportedScalarParameterPlacement(
                    parameter.value,
                ));
            }
            Ok(ScalarAbiValue {
                value: parameter.value,
                scalar_type: parameter.scalar_type,
                placement: placement.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let parameters = function
        .structural_parameters
        .iter()
        .zip(parameter_shapes.iter().copied())
        .zip(call_plan.parameters.iter().skip(function.parameters.len()))
        .map(
            |((parameter, shape), placement)| TargetStructuralParameter {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                projected_qualifications: parameter.projected_qualifications.clone(),
                shape,
                placement: placement.clone(),
            },
        )
        .collect();
    Ok(PreparedFunctionSignature {
        call_plan,
        scalar_parameters,
        parameters,
    })
}

pub(crate) mod borrowed_storage {
    use crate::structural_layout;
    use abstract_operations::AbstractOperationPlan;
    use calling_conventions::terminal_layout::StructuralLayouts;
    use runtime_abi::{NativeBorrowedStorageLayout, NativeStorageCaseTag, NativeStorageRepetition};
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment as Path, ScalarType, StructuralCaseId, StructuralTypeId,
    };
    use std::collections::BTreeSet;
    use structural_layout::{direct_scalar_field_geometry, structural_shape};
    use terminal_psi::{
        StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment, StructuralTypeShape,
    };

    pub(crate) fn capture(
        plan: &AbstractOperationPlan,
        declarations: &StructuralLayouts<'_>,
    ) -> Vec<NativeBorrowedStorageLayout> {
        let mut requested: BTreeSet<_> = plan
            .boundary_machines
            .iter()
            .flat_map(|boundary| {
                boundary
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.structural_type)
                    .chain(match &boundary.result {
                        terminal_psi::BoundaryMachineResult::Structural(result) => {
                            Some(result.structural_type)
                        }
                        _ => None,
                    })
            })
            .collect();
        let needs_original_roots = plan
            .boundary_machines
            .iter()
            .flat_map(|boundary| &boundary.structural_parameters)
            .any(|parameter| {
                matches!(
                    parameter.access,
                    terminal_psi::reading_borrow_pair_pattern!()
                )
            });
        if needs_original_roots {
            for function in &plan.functions {
                requested.extend(
                    function
                        .structural_parameters
                        .iter()
                        .chain(function.block_structural_parameters())
                        .filter(|parameter| {
                            parameter.access == terminal_psi::StructuralAccess::Owned
                        })
                        .map(|parameter| parameter.structural_type),
                );
                requested.extend(
                    function
                        .operations
                        .iter()
                        .filter_map(|operation| operation.materialized_structural_result())
                        .map(|(_, result)| result.structural_type),
                );
            }
        }
        requested
            .into_iter()
            .filter_map(|root| capture_type(root, declarations))
            .collect()
    }

    pub(crate) fn capture_type(
        root: StructuralTypeId,
        declarations: &StructuralLayouts<'_>,
    ) -> Option<NativeBorrowedStorageLayout> {
        let shape = structural_shape(root, declarations).ok()?;
        let mut recipes = StorageRecipes {
            declarations,
            fields: Vec::new(),
            discriminators: Vec::new(),
            opaque_regions: Vec::new(),
        };
        recipes.collect(root, 0, None, &mut Vec::new(), &mut Vec::new())?;
        Some(NativeBorrowedStorageLayout::new(
            root,
            shape.byte_size,
            shape.alignment,
            recipes.fields,
            recipes.discriminators,
            recipes.opaque_regions,
        ))
    }

    type Field = (
        Vec<Path>,
        Vec<NativeStorageRepetition>,
        u32,
        ScalarType,
        ScalarType,
    );
    type Discriminator = (
        Vec<Path>,
        Vec<NativeStorageRepetition>,
        StructuralTypeId,
        u32,
        Vec<NativeStorageCaseTag>,
    );
    type OpaqueRegion = (Vec<Path>, Vec<NativeStorageRepetition>, u32, u32);

    struct StorageRecipes<'declarations> {
        declarations: &'declarations StructuralLayouts<'declarations>,
        fields: Vec<Field>,
        discriminators: Vec<Discriminator>,
        opaque_regions: Vec<OpaqueRegion>,
    }

    impl StorageRecipes<'_> {
        fn collect(
            &mut self,
            root: StructuralTypeId,
            offset: u32,
            occurrence_stride: Option<u32>,
            path: &mut Vec<Path>,
            repetitions: &mut Vec<NativeStorageRepetition>,
        ) -> Option<()> {
            let declaration = self.declarations.get(root).copied()?;
            match &declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => {
                    self.fields
                        .push((path.clone(), repetitions.clone(), offset, *scalar, *scalar))
                }
                StructuralTypeShape::AtomicCell(cell) => {
                    let scalar = cell.scalar_type();
                    self.fields
                        .push((path.clone(), repetitions.clone(), offset, scalar, scalar));
                }
                StructuralTypeShape::Reference { .. }
                | StructuralTypeShape::ByteSequence(_)
                | StructuralTypeShape::ElementView { .. } => {
                    let shape =
                        structural_layout::structural_shape(root, self.declarations).ok()?;
                    self.opaque_regions.push((
                        path.clone(),
                        repetitions.clone(),
                        offset,
                        shape.byte_size,
                    ));
                }
                StructuralTypeShape::Record { fields, .. } => {
                    self.collect_members(root, fields, None, offset, path, repetitions)?;
                }
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    let common = match &declaration.shape {
                        StructuralTypeShape::Mixed { fields, .. } => fields.as_slice(),
                        _ => &[],
                    };
                    if common
                        .iter()
                        .chain(cases.iter().flat_map(|case| &case.fields))
                        .any(|field| !field.relevance.is_erased() && field.storage.is_some())
                    {
                        return None;
                    }
                    let layout =
                        structural_layout::structural_sum_layout(root, self.declarations).ok()?;
                    let mappings = cases
                        .iter()
                        .enumerate()
                        .map(|(ordinal, case)| {
                            Some(NativeStorageCaseTag {
                                case: case.id,
                                native_tag: i32::try_from(ordinal).ok()?,
                            })
                        })
                        .collect::<Option<Vec<_>>>()?;
                    self.discriminators.push((
                        path.clone(),
                        repetitions.clone(),
                        root,
                        offset.checked_add(layout.tag_byte_offset)?,
                        mappings,
                    ));
                    self.collect_members(root, common, None, offset, path, repetitions)?;
                    for case in cases {
                        path.push(Path::Case(case.id));
                        self.collect_members(
                            root,
                            &case.fields,
                            Some(case.id),
                            offset,
                            path,
                            repetitions,
                        )?;
                        path.pop();
                    }
                }
                StructuralTypeShape::FixedArray { element, length } => {
                    let length = length.literal()?;
                    let (_, stride) =
                        structural_layout::root_array_projection_metadata(root, self.declarations)
                            .ok()?;
                    repetitions.push(NativeStorageRepetition {
                        path_position: path.len(),
                        length,
                        byte_stride: occurrence_stride.or(stride)?,
                    });
                    path.push(Path::FixedIndex(0));
                    self.collect(*element, offset, None, path, repetitions)?;
                    path.pop();
                    repetitions.pop();
                }
            }
            Some(())
        }

        fn collect_members(
            &mut self,
            root: StructuralTypeId,
            members: &[StructuralFieldDeclaration],
            case: Option<StructuralCaseId>,
            offset: u32,
            path: &mut Vec<Path>,
            repetitions: &mut Vec<NativeStorageRepetition>,
        ) -> Option<()> {
            for (position, member) in members
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .enumerate()
            {
                if matches!(member.field_type, StructuralFieldType::Erased { .. }) {
                    continue;
                }
                path.push(Path::Field(member.id));
                if let Some(scalar) = member.field_type.scalar_type() {
                    let (displacement, _) = if let Some(case) = case {
                        structural_layout::static_field_geometry(
                            root,
                            &[StructuralPathSegment::Case(case)],
                            member.id,
                            scalar,
                            self.declarations,
                        )
                        .ok()?
                    } else {
                        direct_scalar_field_geometry(root, member.id, scalar, self.declarations)
                            .ok()?
                    };
                    if matches!(
                        member.storage,
                        Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_))
                    ) {
                        return None;
                    }
                    let stored = member
                        .storage
                        .as_ref()
                        .and_then(|storage| storage.stored_scalar())
                        .unwrap_or(scalar);
                    self.fields.push((
                        path.clone(),
                        repetitions.clone(),
                        offset.checked_add(displacement)?,
                        scalar,
                        stored,
                    ));
                } else if let Some(child) = member.field_type.as_structural() {
                    let (_, _, displacement) = if let Some(case) = case {
                        structural_layout::case_projection(
                            root,
                            &[Path::Case(case), Path::Field(member.id)],
                            self.declarations,
                        )
                        .ok()?
                    } else {
                        structural_layout::resolve_structural_projection_path(
                            root,
                            &[StructuralPathSegment::Field(member.identity)],
                            self.declarations,
                        )
                        .ok()?
                    };
                    let stride = member
                        .storage
                        .as_ref()
                        .and_then(semantic_vocabulary::AuthoredFieldStorage::element_stride)
                        .map(u32::try_from)
                        .transpose()
                        .ok()?;
                    self.collect(
                        child,
                        offset.checked_add(displacement)?,
                        stride,
                        path,
                        repetitions,
                    )?;
                } else {
                    let carrier = member.field_type.byte_sequence_carrier()?;
                    let (displacement, byte_size) =
                        self.inline_byte_geometry(root, member, position, case, carrier)?;
                    self.opaque_regions.push((
                        path.clone(),
                        repetitions.clone(),
                        offset.checked_add(displacement)?,
                        byte_size,
                    ));
                }
                path.pop();
            }
            Some(())
        }

        fn inline_byte_geometry(
            &self,
            root: StructuralTypeId,
            member: &StructuralFieldDeclaration,
            position: usize,
            case: Option<StructuralCaseId>,
            carrier: terminal_psi::ByteSequenceCarrier,
        ) -> Option<(u32, u32)> {
            let declaration = self.declarations.get(root)?;
            if let StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } =
                &declaration.shape
            {
                let layout =
                    structural_layout::structural_sum_layout(root, self.declarations).ok()?;
                let field = if let Some(case) = case {
                    let case_position = terminal_psi::structural_case_position(cases, case)?;
                    layout.cases.get(case_position)?.fields.get(position)?
                } else {
                    layout.common_fields.get(position)?
                };
                return Some((field.byte_offset, field.shape.byte_size));
            }

            let member_path = [StructuralPathSegment::Field(member.identity)];
            let displacement = match carrier {
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => {
                    structural_layout::bounded_byte_field_geometry(
                        root,
                        &member_path,
                        self.declarations,
                    )
                    .ok()??
                    .0
                }
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                | terminal_psi::ByteSequenceCarrier::BoundedView { .. } => {
                    structural_layout::borrowed_view_field_offset(
                        root,
                        &member_path,
                        self.declarations,
                    )
                    .ok()??
                }
            };
            let shape = structural_layout::byte_sequence_shape(carrier, root).ok()?;
            Some((displacement, shape.byte_size))
        }
    }
}

pub(crate) mod scalar_abi {
    use crate::LoweringError;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::{AbstractFunction, AbstractFunctionResult};
    use calling_conventions::ValueShape;
    use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
    use semantic_vocabulary::{IntegerType, ScalarType};
    use target::NativeTarget;
    use target_operations::{MixedStructuralScalarFunctionAbi, ScalarAbiValue, ScalarFunctionAbi};

    pub(crate) fn derive_mixed_structural_scalar_function_abi(
        function: &AbstractFunction,
        target: NativeTarget,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> Result<Option<MixedStructuralScalarFunctionAbi>, LoweringError> {
        if function.structural_parameters.is_empty() {
            return Ok(None);
        }
        let Some(result) = function.result.scalar() else {
            return Ok(None);
        };
        if fixed_native_scalar_shape(result.scalar_type).is_none() {
            return Ok(None);
        }
        let Ok(prepared) = crate::function_signature::prepare_function_signature(
            function,
            target,
            structural_types,
        ) else {
            return Ok(None);
        };
        let result_placement = prepared.call_plan.result.clone().ok_or(
            LoweringError::FixedIntegerScalarAbiPlanMissingResult(function.machine),
        )?;
        Ok(Some(MixedStructuralScalarFunctionAbi {
            call_plan: prepared.call_plan.into_call_plan(),
            scalar_parameters: prepared.scalar_parameters,
            structural_parameters: prepared.parameters,
            result: ScalarAbiValue {
                value: result.value,
                scalar_type: result.scalar_type,
                placement: result_placement,
            },
        }))
    }

    pub(crate) fn derive_fixed_scalar_function_abi(
        function: &AbstractFunction,
        target: NativeTarget,
    ) -> Result<Option<ScalarFunctionAbi>, LoweringError> {
        if !function.structural_parameters.is_empty() || !function.entry_claims.is_empty() {
            return Ok(None);
        }
        let result_shape = match function.result {
            AbstractFunctionResult::Unit => None,
            AbstractFunctionResult::Scalar(result) => {
                let Some(shape) = fixed_native_scalar_shape(result.scalar_type) else {
                    return Ok(None);
                };
                Some(shape)
            }
            AbstractFunctionResult::Structural(_) => return Ok(None),
        };

        let parameter_types_and_shapes = function
            .parameters
            .iter()
            .map(|parameter| {
                let scalar_type = parameter.scalar_type;
                Some((scalar_type, fixed_native_scalar_shape(scalar_type)?))
            })
            .collect::<Option<Vec<_>>>();
        let Some(parameter_types_and_shapes) = parameter_types_and_shapes else {
            return Ok(None);
        };
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: parameter_types_and_shapes
                    .iter()
                    .map(|(_, shape)| *shape)
                    .collect(),
                result: result_shape,
            },
        )
        .map_err(LoweringError::AbiPlan)?;
        let result = match (function.result.scalar(), call_plan.result.as_ref()) {
            (None, None) => None,
            (Some(result), Some(placement)) if Some(placement.shape) == result_shape => {
                Some(ScalarAbiValue {
                    value: result.value,
                    scalar_type: result.scalar_type,
                    placement: placement.clone(),
                })
            }
            _ => {
                return Err(LoweringError::FixedIntegerScalarAbiPlanMissingResult(
                    function.machine,
                ));
            }
        };
        if call_plan.parameters.len() != function.parameters.len() {
            return Err(LoweringError::AbiParameterCountMismatch {
                expected: function.parameters.len(),
                actual: call_plan.parameters.len(),
            });
        }
        let parameters = function
            .parameters
            .iter()
            .zip(parameter_types_and_shapes)
            .zip(&call_plan.parameters)
            .map(|((parameter, (scalar_type, expected_shape)), placement)| {
                if placement.shape != expected_shape {
                    return Err(LoweringError::UnsupportedScalarParameterPlacement(
                        parameter.value,
                    ));
                }
                Ok(ScalarAbiValue {
                    value: parameter.value,
                    scalar_type,
                    placement: placement.clone(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(ScalarFunctionAbi {
            call_plan,
            parameters,
            result,
        }))
    }

    pub(crate) fn fixed_native_scalar_shape(scalar_type: ScalarType) -> Option<ValueShape> {
        ValueShape::for_scalar(scalar_type)
    }
    pub(crate) fn fixed_native_integer_shape(scalar_type: IntegerType) -> Option<ValueShape> {
        ValueShape::for_integer(scalar_type)
    }
}

pub(crate) mod structural_signature {
    use calling_conventions::terminal_layout::StructuralLayouts;
    use std::ops::Deref;

    use calling_conventions::{
        CallPlan, CallSignature, CallingPolicy, ValueShape, evaluate_call_plan,
    };
    use semantic_vocabulary::StructuralTypeId;
    use target::NativeTarget;
    use terminal_psi::StructuralAccess;

    use crate::LoweringError;
    use crate::structural_layout::{scalar_storage_type, structural_shape};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ParameterLane {
        Internal,
        Foreign,
    }

    pub(crate) struct StructuralCallSignature {
        signature: CallSignature,
        scalar_parameter_count: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct StructuralCallPlan(CallPlan);

    impl StructuralCallPlan {
        pub(crate) fn into_call_plan(self) -> CallPlan {
            self.0
        }
    }

    impl Deref for StructuralCallPlan {
        type Target = CallPlan;

        fn deref(&self) -> &CallPlan {
            &self.0
        }
    }

    impl StructuralCallSignature {
        pub(crate) fn parameter_shape(
            lane: ParameterLane,
            structural_type: StructuralTypeId,
            referent: ValueShape,
            access: StructuralAccess,
            declarations: &StructuralLayouts<'_>,
        ) -> ValueShape {
            match access {
                StructuralAccess::Owned => referent,
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow => {
                    if lane == ParameterLane::Internal
                        && scalar_storage_type(structural_type, declarations).is_some()
                    {
                        ValueShape::borrowed_scalar_reference()
                    } else {
                        ValueShape::borrowed_reference(referent.byte_size, referent.alignment)
                    }
                }
            }
        }

        pub(crate) fn derive(
            scalar_shapes: &[ValueShape],
            formals: impl IntoIterator<Item = (StructuralTypeId, StructuralAccess)>,
            result: Option<ValueShape>,
            declarations: &StructuralLayouts<'_>,
        ) -> Result<Self, LoweringError> {
            let mut shapes = scalar_shapes.to_vec();
            for (structural_type, access) in formals {
                let referent = structural_shape(structural_type, declarations)?;
                shapes.push(Self::parameter_shape(
                    ParameterLane::Internal,
                    structural_type,
                    referent,
                    access,
                    declarations,
                ));
            }
            Ok(Self {
                signature: CallSignature {
                    parameters: shapes,
                    result,
                },
                scalar_parameter_count: scalar_shapes.len(),
            })
        }

        pub(crate) fn structural_shapes(&self) -> &[ValueShape] {
            &self.signature.parameters[self.scalar_parameter_count..]
        }

        pub(crate) fn plan(
            &self,
            target: NativeTarget,
        ) -> Result<StructuralCallPlan, LoweringError> {
            evaluate_call_plan(CallingPolicy::native_for_target(target), &self.signature)
                .map(StructuralCallPlan)
                .map_err(LoweringError::AbiPlan)
        }
    }
}

pub(crate) mod structural_type_lookup {
    use std::ops::Deref;

    use abstract_operations::StructuralTypeCatalog;
    use calling_conventions::terminal_layout::StructuralLayouts;
    use semantic_vocabulary::{NonDenseIdentities, StructuralTypeId};
    use terminal_psi::StructuralPathSegment;

    pub(crate) struct StructuralTypeLookup<'a> {
        layouts: StructuralLayouts<'a>,
        catalog: &'a StructuralTypeCatalog,
    }

    impl<'a> StructuralTypeLookup<'a> {
        pub(crate) fn new(source: &'a StructuralTypeCatalog) -> Result<Self, NonDenseIdentities> {
            Ok(Self {
                layouts: StructuralLayouts::new(source.iter())?,
                catalog: source,
            })
        }

        pub(crate) fn catalog(&self) -> &StructuralTypeCatalog {
            self.catalog
        }

        pub(crate) fn subtree(
            &self,
            root: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Option<StructuralTypeId> {
            if path.iter().any(|segment| {
                !matches!(
                    segment,
                    StructuralPathSegment::Field(_)
                        | StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::RuntimeIndex { .. }
                        | StructuralPathSegment::Case(_)
                )
            }) {
                return None;
            }
            terminal_semantics::runtime_structural_path_tip(
                self.layouts.values().copied(),
                root,
                path,
            )
            .map(|declaration| declaration.id)
        }
    }

    impl<'a> Deref for StructuralTypeLookup<'a> {
        type Target = StructuralLayouts<'a>;

        fn deref(&self) -> &Self::Target {
            &self.layouts
        }
    }
}
