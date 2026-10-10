//! Exact replay-validated physical movement for one opaque occurrence.

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
    pub const fn class(self) -> PackageReviewBoundaryValueClass {
        self.class
    }

    pub const fn byte_size(self) -> u32 {
        self.byte_size
    }

    pub const fn alignment(self) -> u16 {
        self.alignment
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryValuePlacement {
    pub(crate) shape: PackageReviewBoundaryValueShape,
    pub(crate) locations: Vec<PackageReviewBoundaryValueLocation>,
}

impl PackageReviewBoundaryValuePlacement {
    pub const fn shape(&self) -> PackageReviewBoundaryValueShape {
        self.shape
    }

    pub fn locations(&self) -> &[PackageReviewBoundaryValueLocation] {
        &self.locations
    }
}

pub(crate) type PackageReviewOpaqueRepresentationMovementRole = provider_planning::calling_policy_plans::BoundaryOpaqueRepresentationMovementRole;


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
    pub const fn carrier_shape_root(&self) -> u16 {
        self.carrier_shape_root
    }

    pub const fn role(&self) -> PackageReviewOpaqueRepresentationMovementRole {
        self.role
    }

    pub fn path(&self) -> &[PackageReviewOpaqueRepresentationPathElement] {
        &self.path
    }

    pub const fn placement(&self) -> &PackageReviewBoundaryValuePlacement {
        &self.placement
    }
}
