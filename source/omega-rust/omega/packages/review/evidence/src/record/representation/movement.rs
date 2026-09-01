//! Exact replay-validated physical movement for one opaque occurrence.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewBoundaryValueClass {
    Integer,
    Float,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewBoundaryCallingPolicy {
    MicrosoftX64,
    SystemVAMD64,
    Aapcs64,
    LinuxSyscallX86_64,
    LinuxSyscallAarch64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewMachineRegister {
    X86Rax,
    X86Rcx,
    X86Rdx,
    X86Rbx,
    X86Rsp,
    X86Rbp,
    X86Rsi,
    X86Rdi,
    X86R8,
    X86R9,
    X86R10,
    X86R11,
    X86R12,
    X86R13,
    X86R14,
    X86R15,
    X86Xmm(u8),
    Aarch64X(u8),
    Aarch64V(u8),
}

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
        value_byte_offset: u16,
        byte_size: u16,
    },
    Stack {
        stack_byte_offset: u32,
        value_byte_offset: u16,
        byte_size: u16,
        alignment: u16,
    },
    Indirect {
        pointer: PackageReviewIndirectPointerLocation,
        copy_stack_byte_offset: Option<u32>,
        byte_size: u16,
        alignment: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewBoundaryValueShape {
    pub(crate) class: PackageReviewBoundaryValueClass,
    pub(crate) byte_size: u16,
    pub(crate) alignment: u16,
}

impl PackageReviewBoundaryValueShape {
    pub const fn class(self) -> PackageReviewBoundaryValueClass {
        self.class
    }

    pub const fn byte_size(self) -> u16 {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewOpaqueRepresentationMovementRole {
    Parameter {
        formal_ordinal: u32,
        native_ordinal: u32,
    },
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewOpaqueRepresentationPathElement {
    FixedArrayElement,
    RecordField { ordinal: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewOpaqueRepresentationOccurrence {
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
