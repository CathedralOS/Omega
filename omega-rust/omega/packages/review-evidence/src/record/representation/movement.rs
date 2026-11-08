//! Exact replay-validated physical movement for one opaque occurrence.

use crate::record::accessors::field_projection_accessors;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewBoundaryValueClass {
    BorrowedScalarReference,
    Integer,
    Float,
    BorrowedReference,
    HomogeneousFloatAggregate {
        members: u8,
    },
    SystemVAggregate {
        first: PackageReviewSystemVEightbyteClass,
        second: PackageReviewSystemVEightbyteClass,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSystemVEightbyteClass {
    Integer,
    Sse,
}

pub type PackageReviewBoundaryCallingPolicy = calling_conventions::CallingPolicy;

pub type PackageReviewMachineRegister = calling_conventions::MachineRegister;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewIndirectPointerLocation {
    Register(PackageReviewMachineRegister),
    Stack {
        stack_byte_offset: u32,
        alignment: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewBoundaryValueLocation {
    Register {
        register: PackageReviewMachineRegister,
        value_byte_offset: u32,
        byte_size: u32,
    },
    Stack {
        stack_byte_offset: u32,
        value_byte_offset: u32,
        byte_size: u32,
        alignment: u16,
    },
    Indirect {
        pointer: PackageReviewIndirectPointerLocation,
        copy_stack_byte_offset: Option<u32>,
        byte_size: u32,
        alignment: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryValueShape {
    pub(crate) class: PackageReviewBoundaryValueClass,
    pub(crate) byte_size: u32,
    pub(crate) alignment: u16,
}

impl PackageReviewBoundaryValueShape {
    field_projection_accessors!(valued class: PackageReviewBoundaryValueClass, byte_size: u32);
    field_projection_accessors!(valued alignment: u16);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryValuePlacement {
    pub(crate) shape: PackageReviewBoundaryValueShape,
    pub(crate) locations: Vec<PackageReviewBoundaryValueLocation>,
}

impl PackageReviewBoundaryValuePlacement {
    field_projection_accessors!(copied shape: PackageReviewBoundaryValueShape);
    field_projection_accessors!(sliced locations: &[PackageReviewBoundaryValueLocation]);
}

pub(crate) type PackageReviewOpaqueRepresentationMovementRole =
    provider_planning::calling_policy_plans::BoundaryOpaqueRepresentationMovementRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageReviewOpaqueRepresentationPathElement {
    FixedArrayElement,
    RecordField { ordinal: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewOpaqueRepresentationOccurrence {
    pub(crate) carrier_shape_root: u16,
    pub(crate) role: PackageReviewOpaqueRepresentationMovementRole,
    pub(crate) path: Vec<PackageReviewOpaqueRepresentationPathElement>,
    pub(crate) placement: PackageReviewBoundaryValuePlacement,
}

impl PackageReviewOpaqueRepresentationOccurrence {
    field_projection_accessors!(copied carrier_shape_root: u16);
    field_projection_accessors!(copied role: PackageReviewOpaqueRepresentationMovementRole);
    field_projection_accessors!(sliced path: &[PackageReviewOpaqueRepresentationPathElement]);
    field_projection_accessors!(borrowed placement: &PackageReviewBoundaryValuePlacement);
}
