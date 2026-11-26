// One bounded decoder for the complete inert calling-policy graph.
// `application` decodes inner fields with the caller's aggregate budgets and framing; the enclosing record validates all structural associations after decoding.

pub(in crate::encoding::recovery) mod opaque;

use super::identity::{nominal, type_identity};
use super::reader::Reader;
use super::{Error, PackagePolicyRecoveryLimits};
use crate::encoding::recovery::policy::conformance;
use crate::encoding::{CALLING_POLICY_MAGIC, PACKAGE_CALLING_POLICY_VERSION};
use crate::record::{
    PackagePolicyCallbackBinder, PackagePolicyCallbackDemand, PackagePolicyCallbackDestination,
    PackagePolicyCallbackInlineField, PackagePolicyCallbackLayout,
    PackagePolicyCallbackLayoutApplication, PackagePolicyCallbackMaterialization,
    PackagePolicyCallbacks, PackagePolicyCallingParameter, PackagePolicyCallingPlan,
    PackagePolicyNativeParameter, PackagePolicyNativeParameterOrigin, PackageReviewBoundaryShape,
    PackageReviewBoundaryShapeClass, PackageReviewBoundaryShapeField,
    PackageReviewBoundaryShapeGraph, PackageReviewRepresentationArchitecture,
    PackageReviewRepresentationObjectFormat, PackageReviewRepresentationTarget,
    PackageReviewRepresentationTargetProfile,
};

impl PackagePolicyCallingPlan {
    super::recover_canonical_component!(
        CALLING_POLICY_MAGIC,
        PACKAGE_CALLING_POLICY_VERSION,
        application,
        validate,
    );
}

pub(in crate::encoding::recovery) fn application(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCallingPlan, Error> {
    Ok(PackagePolicyCallingPlan {
        abi: declaration_abi(reader)?,
        boundary_trait: nominal(reader)?,
        boundary_arguments: reader.sequence(8, type_identity)?,
        boundary_lifetime_parameter_count: reader.u32()?,
        boundary_lifetime_arguments: reader.sequence(4, |reader| reader.u32())?,
        application_lifetime_parameter_count: reader.u32()?,
        requirement: nominal(reader)?,
        requirement_trait: nominal(reader)?,
        requirement_arguments: reader.sequence(8, type_identity)?,
        requirement_lifetime_arguments: reader.sequence(4, Reader::u32)?,
        requirement_lifetime_parameter_count: reader.u32()?,
        static_parameters: reader.sequence(1, super::public_api::type_parameter)?,
        target: target(reader)?,
        shape_graph: graph(reader)?,
        semantic_parameters: reader.sequence(20, |reader| {
            Ok(PackagePolicyCallingParameter {
                name: reader.string()?,
                value_type: type_identity(reader)?,
                is_mutable: reader.boolean()?,
                is_const: reader.boolean()?,
                shape_root: reader.u16()?,
            })
        })?,
        semantic_result: reader.option(type_identity)?,
        dispatch_only_formal: reader.option(|reader| reader.u32())?,
        native_parameters: reader.sequence(15, |reader| {
            Ok(PackagePolicyNativeParameter {
                name: reader.string()?,
                origin: super::tag!(reader => {
                    0 => PackagePolicyNativeParameterOrigin::SemanticFormal {
                        formal_ordinal: reader.u32()?,
                        shape_root: reader.u16()?,
                    },
                    1 => PackagePolicyNativeParameterOrigin::PrivateCallback {
                        binder_index: reader.u32()?,
                        byte_size: reader.u16()?,
                        alignment: reader.u16()?,
                    },
                }),
            })
        })?,
        callbacks: decode(reader)?,
        opaque_uses: reader.sequence(1, opaque::decode)?,
        physical: super::physical_calling_policy::physical(reader)?,
    })
}

fn declaration_abi(
    reader: &mut Reader<'_>,
) -> Result<semantic_vocabulary::CallingAbiIdentity, Error> {
    use semantic_vocabulary::{CallingAbiIdentity, CallingOperandKind, CallingStaticOperand};
    let coordinate = reader.u64()?;
    let abi = CallingAbiIdentity {
        requirement_identity: reader.string()?, declaring_application: reader.string()?,
        operands: reader.sequence(13, |reader| Ok(CallingStaticOperand {
            ordinal: reader.u32()?,
            kind: super::tag!(reader => { 0 => CallingOperandKind::Type, 1 => CallingOperandKind::Const, 2 => CallingOperandKind::Value, 3 => CallingOperandKind::Machine, 4 => CallingOperandKind::Implementation, 5 => CallingOperandKind::Proposition, 6 => CallingOperandKind::Proof }),
            identity: reader.string()?,
        }))?,
    };
    if abi.validate().is_err() || abi.id().get() != coordinate {
        return Err(Error::NonCanonicalEncoding);
    }
    Ok(abi)
}

// Bounded recovery for calling-plan callback binders, demands, materializations, and layouts.

pub(in crate::encoding::recovery) fn decode(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCallbacks, Error> {
    Ok(PackagePolicyCallbacks {
        binders: reader.sequence(90, |reader| {
            Ok(PackagePolicyCallbackBinder {
                parameter: nominal(reader)?,
                static_parameter_ordinal: reader.u32()?,
                static_machine_ordinal: reader.u32()?,
                requirement: nominal(reader)?,
            })
        })?,
        demands: reader.sequence(46, |reader| {
            Ok(PackagePolicyCallbackDemand {
                destination: destination(reader)?,
                requirement: nominal(reader)?,
            })
        })?,
        materializations: reader.sequence(9, |reader| {
            Ok(PackagePolicyCallbackMaterialization {
                binder_index: reader.u32()?,
                destination: destination(reader)?,
            })
        })?,
        layouts: reader.sequence(1, |reader| {
            Ok(PackagePolicyCallbackLayout {
                formal_ordinal: reader.u32()?,
                native_ordinal: reader.u32()?,
                root_layout: layout_application(reader)?,
                inline_fields: reader.sequence(1, |reader| {
                    Ok(PackagePolicyCallbackInlineField {
                        field: nominal(reader)?,
                        offset: reader.u64()?,
                        extent: reader.u64()?,
                        alignment: reader.u64()?,
                        child_layout: layout_application(reader)?,
                    })
                })?,
                terminal_slot: conformance::application(reader)?,
                terminal_placements: reader.sequence(9, placement)?,
                terminal_byte_size: reader.u64()?,
                terminal_alignment: reader.u64()?,
                composed_placements: reader.sequence(9, placement)?,
            })
        })?,
    })
}

pub(in crate::encoding::recovery) fn destination(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCallbackDestination, Error> {
    Ok(super::tag!(reader => {
        0 => PackagePolicyCallbackDestination::Parameter {
            native_ordinal: reader.u32()?,
        },
        1 => PackagePolicyCallbackDestination::Field {
            native_ordinal: reader.u32()?,
            layout_index: reader.u32()?,
        },
    }))
}

pub(in crate::encoding::recovery) fn layout_application(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCallbackLayoutApplication, Error> {
    Ok(PackagePolicyCallbackLayoutApplication {
        policy: reader.option(nominal)?,
        schema: type_identity(reader)?,
        byte_size: reader.u64()?,
        alignment: reader.u64()?,
    })
}

pub(in crate::encoding::recovery::policy::calling_application) fn placement(
    reader: &mut Reader<'_>,
) -> Result<layout_plans::LayoutPlacementReport, Error> {
    use layout_plans::{IntegerInterpretation, LayoutPlacementReport};
    Ok(super::tag!(reader => {
        0 => LayoutPlacementReport::At { offset: reader.u64()? },
        1 => LayoutPlacementReport::IntegerAt {
            offset: reader.u64()?,
            stored_width: reader.u64()?,
            interpretation: super::tag!(reader => {
                0 => IntegerInterpretation::Signed,
                1 => IntegerInterpretation::Unsigned,
            }),
        },
        2 => LayoutPlacementReport::Bits {
            container: reader.u64()?,
            container_width: reader.u64()?,
            destination_lsb: reader.u64()?,
            source_lsb: reader.u64()?,
            width: reader.u64()?,
        },
    }))
}

// Bounded recovery for representation targets and boundary shape graphs in calling plans.

pub(in crate::encoding::recovery) fn target(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewRepresentationTarget, Error> {
    Ok(PackageReviewRepresentationTarget {
        profile: super::tag!(reader => {
            0 => PackageReviewRepresentationTargetProfile::LinuxArm64,
            1 => PackageReviewRepresentationTargetProfile::LinuxX64,
            2 => PackageReviewRepresentationTargetProfile::MacosArm64,
            3 => PackageReviewRepresentationTargetProfile::WindowsX64,
            4 => PackageReviewRepresentationTargetProfile::UefiX64,
            5 => PackageReviewRepresentationTargetProfile::CrossPlatformCli,
            6 => PackageReviewRepresentationTargetProfile::LocalUnchecked,
            7 => PackageReviewRepresentationTargetProfile::MacosX64,
            8 => PackageReviewRepresentationTargetProfile::WindowsArm64,
        }),
        architecture: super::tag!(reader => {
            0 => PackageReviewRepresentationArchitecture::Aarch64,
            1 => PackageReviewRepresentationArchitecture::X86_64,
        }),
        object_format: super::tag!(reader => {
            0 => PackageReviewRepresentationObjectFormat::Elf,
            1 => PackageReviewRepresentationObjectFormat::MachO,
            2 => PackageReviewRepresentationObjectFormat::Coff,
        }),
        pointer_size: reader.u16()?,
        pointer_alignment: reader.u16()?,
    })
}

pub(in crate::encoding::recovery) fn graph(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewBoundaryShapeGraph, Error> {
    Ok(PackageReviewBoundaryShapeGraph {
        shapes: reader.sequence(5, |reader| {
            Ok(PackageReviewBoundaryShape {
                class: super::tag!(reader => {
                    0 => PackageReviewBoundaryShapeClass::Integer,
                    1 => PackageReviewBoundaryShapeClass::Float,
                    2 => PackageReviewBoundaryShapeClass::Reference,
                    3 => PackageReviewBoundaryShapeClass::FixedArray {
                        element: reader.u16()?,
                        length: reader.u16()?,
                    },
                    4 => PackageReviewBoundaryShapeClass::Record {
                        first_field: reader.u16()?,
                        field_count: reader.u16()?,
                    },
                }),
                byte_size: reader.u16()?,
                alignment: reader.u16()?,
            })
        })?,
        fields: reader.sequence(4, |reader| {
            Ok(PackageReviewBoundaryShapeField {
                shape: reader.u16()?,
                byte_offset: reader.u16()?,
            })
        })?,
        parameters: reader.sequence(2, Reader::u16)?,
        result: reader.option(Reader::u16)?,
    })
}
