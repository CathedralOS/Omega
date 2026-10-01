//! Owned aggregate actuals retain exact type, custody, and current value backing.
//! Arrays, records, completed sums, and bounded byte owners share transport,
//! not semantic identity. A byte owner transports its live-length word and
//! backing together; presenting it as a borrowed view would change that ABI.
//! An owned receiver uses that same transport: `is_self` identifies its source
//! role, not a different ABI or an exemption from the home custody checks.
use super::LiveDefinitions;
use crate::LoweringError;
use abstract_operations::AbstractFunction;
use semantic_vocabulary::StructuralTypeId;
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetStructuralArgument, TargetStructuralParameter};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralTypeDeclaration, StructuralTypeShape,
};

pub(super) fn is_owned_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    parameter.access == StructuralAccess::Owned
        && parameter.multiplicity != StructuralMultiplicity::Linear
        && parameter.projected_qualifications.is_empty()
        && value_layout(parameter.structural_type, types).is_ok()
}

pub(super) fn argument(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let invalid = || LoweringError::UnsupportedStructuralArray(declaration.structural_type);
    let expected_layout = value_layout(declaration.structural_type, types)?;
    let expected_shape = expected_layout.shape();
    if !is_owned_parameter(declaration, types)
        || argument.access != StructuralAccess::Owned
        || destination.shape != expected_shape
    {
        return Err(invalid());
    }
    let (source, root_structural_type, source_byte_offset) = if let Some(home) =
        live.structural_homes.get(&argument.place)
    {
        // A field snapshot has no qualification roster of its own. The
        // canonical call judgment reconstructs its captured byte predicates;
        // this leg transports that exact owned carrier, not a borrowed view.
        // Geometry cannot establish a predicate from the formal's roster.
        let copied_byte_field = argument.path.is_empty()
            && !declaration.is_self
            && declaration.multiplicity == StructuralMultiplicity::Unrestricted
            && home.qualifications().is_empty()
            && types.get(&declaration.structural_type).is_some_and(|row| {
                matches!(row.shape, StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                ))
            })
            && home.operation_result().is_some_and(|(producer, result)| {
                result.qualification_establishments.is_empty()
                    && function.operations.iter().any(|operation| matches!(operation,
                        abstract_operations::AbstractOperation::StructuralLeafCopy {
                            psi_operation, result: actual,
                            path: abstract_operations::StructuralCopyPath::Ordinary(path), ..
                        } if *psi_operation == producer && actual == result
                            && matches!(path.last(), Some(terminal_psi::StructuralPathSegment::Field(_)))
                    ))
            });
        if home.access() != StructuralAccess::Owned
            || home.multiplicity() != declaration.multiplicity
            || home.has_claims()
            || (home.qualifications() != declaration.qualifications.as_slice()
                && !copied_byte_field)
            || home.projected_qualifications() != declaration.projected_qualifications.as_slice()
        {
            return Err(invalid());
        }
        // Partial owned transfer selects bytes from the completed root home;
        // the verified call and residual cleanup retain the root's custody.
        // A root qualification cannot silently become a payload qualification.
        let offset = if argument.path.is_empty() {
            if home.structural_type() != declaration.structural_type
                || home.layout != expected_layout
            {
                return Err(invalid());
            }
            0
        } else {
            if !home.qualifications().is_empty() {
                return Err(invalid());
            }
            let (selected, shape, offset) =
                crate::lowering::structural_layout::resolve_structural_projection_path(
                    home.structural_type(),
                    &argument.path,
                    types,
                    &mut BTreeMap::new(),
                    &mut BTreeSet::new(),
                )?;
            if selected != declaration.structural_type || shape != expected_shape {
                return Err(invalid());
            }
            offset
        };
        // A state arrival owns its destination home, not an incoming
        // operation's home. Preserve that identity through calls just as
        // through reads and loans; independent source replay checks both
        // coordinates against the caller's block declaration.
        let source = match &home.origin {
            target_operations::TargetStructuralHomeOrigin::OperationResult {
                operation, ..
            } => target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: *operation,
            },
            target_operations::TargetStructuralHomeOrigin::BlockParameter {
                block,
                declaration,
            } => target_operations::TargetStructuralArgumentSource::BlockParameter {
                block: *block,
                place: declaration.place,
            },
        };
        (source, home.structural_type(), offset)
    } else if let Some((establishment, structural_type)) =
        live.trivial_affine_locals.get(&argument.place)
    {
        // An empty-record local has no bytes to transport; its establishment
        // is the producer the call replays. It is affine and unqualified.
        if !argument.path.is_empty()
            || *structural_type != declaration.structural_type
            || declaration.multiplicity != StructuralMultiplicity::Affine
            || !declaration.qualifications.is_empty()
            || expected_shape.byte_size != 0
        {
            return Err(invalid());
        }
        (
            target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: *establishment,
            },
            *structural_type,
            0,
        )
    } else {
        let parameter = prepared
            .parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .ok_or_else(invalid)?;
        // The callee's whole-root preconditions transfer only when the
        // caller's declared roster on this place equals them exactly.
        let caller_qualifications = function
            .structural_parameters
            .iter()
            .find(|source| source.place == argument.place)
            .map(|source| source.qualifications.as_slice())
            .unwrap_or(&[]);
        if !argument.path.is_empty()
            || parameter.access != StructuralAccess::Owned
            || parameter.multiplicity != declaration.multiplicity
            || parameter.structural_type != declaration.structural_type
            || parameter.shape != expected_shape
            || caller_qualifications != declaration.qualifications.as_slice()
        {
            return Err(invalid());
        }
        (
            parameter.placement.clone().into(),
            parameter.structural_type,
            0,
        )
    };
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type,
        structural_type: declaration.structural_type,
        shape: expected_shape,
        source_byte_offset,
        fixed_array_length: None,
        element_stride: None,
        indices: Vec::new(),
        source,
        destination: destination.placement.clone(),
    })
}

fn value_layout(
    structural_type: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<target_operations::TargetStructuralHomeLayout, LoweringError> {
    use target_operations::TargetStructuralHomeLayout;
    match types
        .get(&structural_type)
        .map(|declaration| &declaration.shape)
    {
        Some(
            StructuralTypeShape::Record { .. }
            | StructuralTypeShape::FixedArray { .. }
            | StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                ..
            }),
        ) => crate::lowering::structural_layout::structural_shape(
            structural_type,
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )
        .map(TargetStructuralHomeLayout::Aggregate),
        Some(StructuralTypeShape::Sum { .. }) => {
            // Equal byte sizes do not establish equal tag and payload placement.
            // Retain the same declared sum layout used by construction and reads.
            crate::lowering::structural_layout::structural_sum_layout(
                structural_type,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )
            .map(TargetStructuralHomeLayout::Sum)
        }
        _ => super::scalar_arrays::shape(structural_type, types)
            .map(|(_, _, shape)| TargetStructuralHomeLayout::Aggregate(shape)),
    }
}
