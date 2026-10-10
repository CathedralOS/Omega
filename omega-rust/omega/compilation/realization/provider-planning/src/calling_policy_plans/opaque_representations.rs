pub use layout::BoundaryOpaqueRepresentationUse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundaryOpaqueRepresentationMovementRole {
    Parameter {
        formal_ordinal: u32,
        native_ordinal: u32,
    },
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryOpaqueRepresentationPathElement {
    FixedArrayElement,
    RecordField { ordinal: u16 },
}

/// Exact target movement assigned to one opaque occurrence by the
/// replay-validated boundary entry plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryOpaqueRepresentationMovement {
    pub(super) role: BoundaryOpaqueRepresentationMovementRole,
    pub(super) path: Vec<BoundaryOpaqueRepresentationPathElement>,
    pub(super) placement: calling_conventions::ValuePlacement,
}

impl BoundaryOpaqueRepresentationMovement {
    pub const fn role(&self) -> BoundaryOpaqueRepresentationMovementRole {
        self.role
    }

    pub const fn placement(&self) -> &calling_conventions::ValuePlacement {
        &self.placement
    }

    pub fn path(&self) -> &[BoundaryOpaqueRepresentationPathElement] {
        &self.path
    }
}
