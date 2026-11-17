//! Carried physical movement paths for canonical opaque applications.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryOpaqueRepresentationMovement {
    pub(super) role: BoundaryOpaqueRepresentationMovementRole,
    pub(super) path: Vec<BoundaryOpaqueRepresentationPathElement>,
    pub(super) placement: calling_conventions::ValuePlacement,
}

// The boundary-signature tests read these to check the movement plan; no
// production caller reads them back.
#[cfg(test)]
impl BoundaryOpaqueRepresentationMovement {
    pub const fn role(&self) -> BoundaryOpaqueRepresentationMovementRole {
        self.role
    }

    pub const fn placement(&self) -> &calling_conventions::ValuePlacement {
        &self.placement
    }
}
