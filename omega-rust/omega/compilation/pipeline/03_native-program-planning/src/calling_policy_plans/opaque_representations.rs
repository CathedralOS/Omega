//! Carried physical movement paths for canonical opaque applications.

pub use calling_conventions::BoundaryOpaqueRepresentationMovementRole;

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
