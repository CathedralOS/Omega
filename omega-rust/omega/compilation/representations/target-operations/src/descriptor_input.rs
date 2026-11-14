// Descriptor-reading foreign input guards and contiguous scalar-prefix derivation.
// A native descriptor-reading foreign call may alter only an independently reconstructed scalar prefix before any opaque interval, sum discriminator, atomic resident, Boolean field, or refined domain. Both the producer and receiving audit derive `DescriptorInputGuard` from the admitted ABI and structural layout without trusting caller-proposed bounds; repetition coverage groups by stride so huge fixed arrays are checked in time proportional to declaration recipes.

use crate::{
    NormalizedForeignCallBinding, NormalizedForeignScalarArgument,
    NormalizedForeignStructuralArgument, TargetBoundaryResult,
};
use calling_conventions::{CallSignature, ValueShape, validate_boundary_entry_plan};
use runtime_abi::{
    DescriptorAbi, DescriptorOperation, NativeBorrowedStorageLayout, NativeStorageRepetition,
};
use semantic_vocabulary::{
    ClosedScalarType, IntegerSign, IntegerType, ScalarType, StructuralTypeId,
};
use std::borrow::Borrow;
use terminal_psi::StructuralTypeShape;
use terminal_psi::{
    BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    StructuralAccess, StructuralTypeDeclaration,
};
use terminal_semantics::{canonical_structural_path_tip, observed_field_declaration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DescriptorInputGuard {
    pub count_scalar_argument: usize,
    pub structural_argument: usize,
    pub max_bytes: u64,
    pub count_type: IntegerType,
}

pub fn descriptor_input_guard<Layout: Borrow<NativeBorrowedStorageLayout>>(
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    structural_types: &[StructuralTypeDeclaration],
    layout_of: impl FnOnce(StructuralTypeId) -> Option<Layout>,
) -> Result<Option<DescriptorInputGuard>, &'static str> {
    descriptor_input_guard_with_closed_scalars(
        declaration,
        binding,
        structural_types,
        layout_of,
        |scalar| scalar.try_into().ok(),
    )
}

/// Artifact-backed callers project from their retained type catalog. The
/// callback only closes a supplied identity; it cannot substitute another one.
pub fn descriptor_input_guard_with_closed_scalars<Layout: Borrow<NativeBorrowedStorageLayout>>(
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    structural_types: &[StructuralTypeDeclaration],
    layout_of: impl FnOnce(StructuralTypeId) -> Option<Layout>,
    mut close_scalar: impl FnMut(ScalarType) -> Option<ClosedScalarType>,
) -> Result<Option<DescriptorInputGuard>, &'static str> {
    let [parameter] = declaration.structural_parameters.as_slice() else {
        return Ok(None);
    };
    if !declaration.content_guarantees.is_empty()
        || declaration.completion != BoundaryCompletion::NormalReturn
        || declaration.parameter_order
            != [
                BoundaryParameterKind::Scalar,
                BoundaryParameterKind::Structural,
                BoundaryParameterKind::Scalar,
            ]
    {
        return Ok(None);
    }
    let result = match declaration.result {
        BoundaryMachineResult::Unit => None,
        BoundaryMachineResult::Scalar(scalar) => Some(scalar.scalar_type),
        _ => return Ok(None),
    };
    derive(
        binding,
        structural_types,
        parameter.structural_type,
        parameter.access,
        (&declaration.scalar_parameters, result),
        layout_of,
        &mut close_scalar,
    )
}

pub fn descriptor_input_guard_for_call<Layout: Borrow<NativeBorrowedStorageLayout>>(
    binding: &NormalizedForeignCallBinding,
    structural_types: &[StructuralTypeDeclaration],
    scalar_arguments: &[NormalizedForeignScalarArgument],
    structural_arguments: &[NormalizedForeignStructuralArgument],
    result: &TargetBoundaryResult,
    layout_of: impl FnOnce(StructuralTypeId) -> Option<Layout>,
) -> Result<Option<DescriptorInputGuard>, &'static str> {
    descriptor_input_guard_for_call_with_closed_scalars(
        binding,
        structural_types,
        scalar_arguments,
        structural_arguments,
        result,
        layout_of,
        |scalar| scalar.try_into().ok(),
    )
}

pub fn descriptor_input_guard_for_call_with_closed_scalars<
    Layout: Borrow<NativeBorrowedStorageLayout>,
>(
    binding: &NormalizedForeignCallBinding,
    structural_types: &[StructuralTypeDeclaration],
    scalar_arguments: &[NormalizedForeignScalarArgument],
    structural_arguments: &[NormalizedForeignStructuralArgument],
    result: &TargetBoundaryResult,
    layout_of: impl FnOnce(StructuralTypeId) -> Option<Layout>,
    mut close_scalar: impl FnMut(ScalarType) -> Option<ClosedScalarType>,
) -> Result<Option<DescriptorInputGuard>, &'static str> {
    if !matches!(
        selected_descriptor(binding),
        Some((_, DescriptorOperation::Read))
    ) {
        return Ok(None);
    }
    let [argument] = structural_arguments else {
        return Ok(None);
    };
    if scalar_arguments.len() != 2
        || scalar_arguments[0].parameter_index != 0
        || scalar_arguments[1].parameter_index != 2
    {
        return Ok(None);
    }
    let declared_tip = terminal_semantics::runtime_structural_path_tip(
        structural_types.iter(),
        argument.root_structural_type,
        &argument.path,
    )
    .map(|declaration| declaration.id);
    let selected = declared_tip
        .or_else(|| {
            terminal_semantics::primitive_projection_declaration(
                structural_types.iter(),
                argument.root_structural_type,
                &argument.path,
            )
        })
        .ok_or("descriptor input argument has no exact structural type")?;
    let scalars = scalar_arguments
        .iter()
        .map(|argument| argument.source.scalar_type())
        .collect::<Vec<_>>();
    let result = match result {
        TargetBoundaryResult::Unit => None,
        TargetBoundaryResult::Scalar(home) => Some(home.scalar_type),
        TargetBoundaryResult::Structural(_) => return Ok(None),
    };
    derive(
        binding,
        structural_types,
        selected,
        argument.access,
        (&scalars, result),
        layout_of,
        &mut close_scalar,
    )
}

fn derive<Layout: Borrow<NativeBorrowedStorageLayout>>(
    binding: &NormalizedForeignCallBinding,
    types: &[StructuralTypeDeclaration],
    buffer: StructuralTypeId,
    access: StructuralAccess,
    scalar_signature: (&[ScalarType], Option<ScalarType>),
    layout_of: impl FnOnce(StructuralTypeId) -> Option<Layout>,
    close_scalar: &mut impl FnMut(ScalarType) -> Option<ClosedScalarType>,
) -> Result<Option<DescriptorInputGuard>, &'static str> {
    let (scalars, result) = scalar_signature;
    let operation = selected_descriptor(binding);
    let Some((abi, DescriptorOperation::Read)) = operation else {
        return Ok(None);
    };
    if access != StructuralAccess::MutableBorrow {
        return Ok(None);
    }
    let Some(layout) = layout_of(buffer) else {
        return Ok(None);
    };
    let layout = layout.borrow();
    if layout.opaque_regions().is_empty() {
        return Ok(None);
    }
    let invalid = "descriptor input guard requires its exact admitted native ABI";
    let width = if abi == DescriptorAbi::WindowsCrt {
        32
    } else {
        64
    };
    let descriptor = IntegerType::new(IntegerSign::Signed, 32).map_err(|_| invalid)?;
    let count = IntegerType::new(IntegerSign::Unsigned, width).map_err(|_| invalid)?;
    let returned = IntegerType::new(IntegerSign::Signed, width).map_err(|_| invalid)?;
    if scalars != [ScalarType::Integer(descriptor), ScalarType::Integer(count)]
        || result.is_some_and(|scalar| scalar != ScalarType::Integer(returned))
        || !binding.has_valid_entry_mechanism()
        || binding.provider_plan_report_identity() == 0
        || !binding
            .boundary_entry_plan
            .call
            .callback_materializations
            .is_empty()
    {
        return Err(invalid);
    }
    validate_boundary_entry_plan(
        binding.boundary_entry_plan.clone(),
        &CallSignature {
            parameters: vec![
                ValueShape::integer(4, 4),
                ValueShape::integer(8, 8),
                ValueShape::integer(u32::from(width / 8), width / 8),
            ],
            result: result.map(|_| ValueShape::integer(u32::from(width / 8), width / 8)),
        },
    )
    .map_err(|_| invalid)?;
    Ok(Some(DescriptorInputGuard {
        count_scalar_argument: 1,
        structural_argument: 0,
        max_bytes: u64::from(scalar_prefix(layout, types, close_scalar)?),
        count_type: count,
    }))
}

fn selected_descriptor(
    binding: &NormalizedForeignCallBinding,
) -> Option<(DescriptorAbi, DescriptorOperation)> {
    if let Some(kernel) = binding.system_call() {
        runtime_abi::descriptor_system_call(kernel.target, kernel.number)
    } else {
        binding
            .import()
            .and_then(|(locator, _)| runtime_abi::descriptor_import(locator))
    }
}

fn scalar_prefix(
    layout: &NativeBorrowedStorageLayout,
    types: &[StructuralTypeDeclaration],
    close_scalar: &mut impl FnMut(ScalarType) -> Option<ClosedScalarType>,
) -> Result<u32, &'static str> {
    let mut limit = layout.byte_size;
    for region in layout.opaque_regions() {
        if region.byte_size != 0
            && layout
                .opaque_region_repetitions(region)
                .iter()
                .all(|dimension| dimension.length != 0)
        {
            limit = limit.min(region.byte_offset);
        }
    }
    for discriminator in layout.discriminators() {
        if layout
            .discriminator_repetitions(discriminator)
            .iter()
            .all(|dimension| dimension.length != 0)
        {
            limit = limit.min(discriminator.byte_offset);
        }
    }
    for field in layout.fields() {
        if !layout
            .field_repetitions(field)
            .iter()
            .all(|dimension| dimension.length != 0)
        {
            continue;
        }
        let path = layout.field_path(field).collect::<Vec<_>>();
        let unrestricted = if let Some((
            semantic_vocabulary::CanonicalStructuralPathSegment::Field(identity),
            prefix,
        )) = path.split_last()
        {
            observed_field_declaration(types.iter(), layout.structural_type, prefix, *identity)
                .is_some_and(|field| {
                    matches!(
                        field.field_type,
                        terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(_))
                            | terminal_psi::StructuralFieldType::Scalar(ScalarType::IeeeFloat(_))
                            | terminal_psi::StructuralFieldType::IeeeFloat(_)
                    )
                })
        } else {
            canonical_structural_path_tip(types.iter(), layout.structural_type, &path).is_some_and(
                |declaration| {
                    declaration.scalar_restriction.is_none()
                        && matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::PrimitiveScalar(
                                ScalarType::Integer(_)
                            ) | StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(_))
                        )
                },
            )
        };
        if !unrestricted || field.logical_scalar != field.stored_scalar {
            limit = limit.min(field.byte_offset);
        }
    }
    let mut recipes = Vec::new();
    for field in layout.fields() {
        let bytes = scalar_prefix_byte_width(field.stored_scalar, close_scalar)?;
        if field.byte_offset < limit {
            recipes.push(PrefixRecipe {
                offset: field.byte_offset,
                bytes,
                path: layout.field_path(field).collect(),
                repetitions: layout.field_repetitions(field).to_vec(),
            });
        }
    }
    let mut intervals = coverage(&recipes.iter().collect::<Vec<_>>(), 0)?;
    intervals.sort_unstable();
    let mut end = 0;
    for (start, stop) in intervals {
        if start > end {
            break;
        }
        end = end.max(stop).min(limit);
        if end == limit {
            break;
        }
    }
    Ok(end)
}

/// The public classifier accepts independent raw layout metadata. A concrete
/// descriptor must exist before a supplied field can contribute byte coverage.
fn scalar_prefix_byte_width(
    scalar: ScalarType,
    close_scalar: &mut impl FnMut(ScalarType) -> Option<ClosedScalarType>,
) -> Result<u32, &'static str> {
    let closed =
        close_scalar(scalar).ok_or("descriptor scalar prefix requires a closed scalar type")?;
    if closed.as_checking() != scalar {
        return Err("descriptor scalar prefix closure changes its supplied type");
    }
    Ok(match closed {
        ClosedScalarType::Boolean => 1,
        ClosedScalarType::IeeeFloat(format) => u32::from(format.byte_size()),
        ClosedScalarType::Integer(integer) if !integer.is_address() && integer.bits() % 8 == 0 => {
            u32::from(integer.bits() / 8)
        }
        _ => return Err("descriptor scalar prefix has no native byte codec"),
    })
}

struct PrefixRecipe {
    offset: u32,
    bytes: u32,
    path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    repetitions: Vec<NativeStorageRepetition>,
}

fn coverage(recipes: &[&PrefixRecipe], depth: usize) -> Result<Vec<(u32, u32)>, &'static str> {
    let invalid = "descriptor prefix repetition exceeds native storage";
    let mut intervals = Vec::new();
    let mut groups = std::collections::BTreeMap::<_, Vec<&PrefixRecipe>>::new();
    for recipe in recipes {
        if let Some(dimension) = recipe.repetitions.get(depth) {
            if dimension.length == 0 {
                continue;
            }
            let path = recipe
                .path
                .get(..dimension.path_position)
                .ok_or(invalid)?
                .to_vec();
            groups
                .entry((path, dimension.byte_stride, dimension.length))
                .or_default()
                .push(*recipe);
        } else {
            intervals.push((
                recipe.offset,
                recipe.offset.checked_add(recipe.bytes).ok_or(invalid)?,
            ));
        }
    }
    for ((_, stride, length), group) in groups {
        let mut element = coverage(&group, depth + 1)?;
        merge(&mut element);
        for (start, end) in element {
            let end = if end - start >= stride {
                let displacement = (length - 1)
                    .checked_mul(u64::from(stride))
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or(invalid)?;
                end.checked_add(displacement).ok_or(invalid)?
            } else {
                end
            };
            intervals.push((start, end));
        }
    }
    merge(&mut intervals);
    Ok(intervals)
}

fn merge(intervals: &mut Vec<(u32, u32)>) {
    intervals.sort_unstable();
    let mut merged: Vec<(u32, u32)> = Vec::with_capacity(intervals.len());
    for (start, end) in intervals.drain(..) {
        if let Some(previous) = merged.last_mut()
            && start <= previous.1
        {
            previous.1 = previous.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    *intervals = merged;
}

#[cfg(test)]
mod width_closure_tests {
    use super::*;
    use semantic_vocabulary::{
        ClosedIntegerType, IeeeFloatFormat, MachineId, ValueId, WidthParameter,
    };

    #[test]
    fn descriptor_scalar_geometry_keeps_exact_width_and_projection_identity() {
        let mut independent = |scalar: ScalarType| scalar.try_into().ok();
        for bits in [8, 24, 64, 128] {
            let integer = IntegerType::new(IntegerSign::Signed, bits).unwrap();
            assert_eq!(
                scalar_prefix_byte_width(ScalarType::Integer(integer), &mut independent),
                Ok(u32::from(bits / 8))
            );
        }
        for (scalar, bytes) in [
            (ScalarType::Boolean, 1),
            (ScalarType::IeeeFloat(IeeeFloatFormat::Binary32), 4),
            (ScalarType::IeeeFloat(IeeeFloatFormat::Binary64), 8),
        ] {
            assert_eq!(
                scalar_prefix_byte_width(scalar, &mut independent),
                Ok(bytes)
            );
        }
        for integer in [
            IntegerType::new(IntegerSign::Unsigned, 13).unwrap(),
            IntegerType::address(64).unwrap(),
        ] {
            assert!(
                scalar_prefix_byte_width(ScalarType::Integer(integer), &mut independent).is_err()
            );
        }
        let symbolic = IntegerType::parameterized(
            IntegerSign::Unsigned,
            WidthParameter {
                owner: MachineId::new(1).unwrap(),
                value: ValueId::new(2).unwrap(),
            },
        );
        assert_eq!(
            scalar_prefix_byte_width(ScalarType::Integer(symbolic), &mut independent),
            Err("descriptor scalar prefix requires a closed scalar type")
        );
        let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
        assert!(scalar_prefix_byte_width(scalar, &mut |_| None).is_err());
        let other =
            ClosedScalarType::Integer(ClosedIntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        assert_eq!(
            scalar_prefix_byte_width(scalar, &mut |_| Some(other)),
            Err("descriptor scalar prefix closure changes its supplied type")
        );
    }
}
