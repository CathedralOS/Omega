use crate::terminal_module::TerminalIdentityIndex;
use crate::{
    StructuralAccess, StructuralMultiplicity, StructuralPathQualification, StructuralPathSegment,
};
use semantic_vocabulary::{PlaceId, ScalarType, StructuralDomainId, StructuralTypeId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueDeclaration {
    pub id: ValueId,
    pub scalar_type: ScalarType,
    pub qualifications: semantic_vocabulary::ScalarQualificationSetId,
}

impl ValueDeclaration {
    pub const fn value_type(self) -> semantic_vocabulary::QualifiedScalarType {
        semantic_vocabulary::QualifiedScalarType {
            scalar_type: self.scalar_type,
            qualifications: self.qualifications,
        }
    }
}

/// One erased formal whose carrier is proof-only data (for example a
/// recursive proof type with no scalar layout). It has no `ValueId`, no
/// runtime storage, and no operand lane; `ProofTerm::Formal` actuals name it
/// positionally within the owning `erased_proof_formals` roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErasedProofFormal {
    /// Dense zero-based position of the erased authored parameter, used only
    /// for diagnostics.
    pub source_position: u32,
    /// Canonical display-path identity of the proof-only carrier type.
    pub type_identity: TerminalIdentityIndex,
}

/// The normal result shape of one terminal machine.
///
/// Unit is the absence of a runtime value. It therefore has no `ValueId`, no
/// scalar type, and no result pseudo-value that contracts can name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalMachineResult {
    Unit,
    Scalar(ValueDeclaration),
    Structural(StructuralResultDeclaration),
}

impl TerminalMachineResult {
    pub const fn scalar(&self) -> Option<ValueDeclaration> {
        match self {
            Self::Scalar(result) => Some(*result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub const fn scalar_ref(&self) -> Option<&ValueDeclaration> {
        match self {
            Self::Scalar(result) => Some(result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    optimization::structural_variant_accessor!(StructuralResultDeclaration; Self::Unit | Self::Scalar(_));
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralParameterDeclaration {
    pub place: PlaceId,
    pub position: u32,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub access: StructuralAccess,
    /// Strictly ordered exact signature preconditions. A parameter does not
    /// establish these facts by declaration: its caller or root installation
    /// must discharge them at invocation.
    pub qualifications: Vec<StructuralDomainId>,
    /// Strictly ordered exact qualification preconditions rooted beneath this
    /// parameter. Whole-root qualifications remain in `qualifications`; every
    /// row here must carry a nonempty path whose resolved structural type is
    /// the declared domain carrier.
    pub projected_qualifications: Vec<StructuralPathQualification>,
}

impl StructuralParameterDeclaration {
    /// The declaration arriving at `place`, when present in `declarations`.
    pub fn find_by_place(declarations: &[Self], place: PlaceId) -> Option<&Self> {
        declarations
            .iter()
            .find(|parameter| parameter.place == place)
    }

    /// Whether the parameter is the callee slot a nominal cleanup binds to:
    /// position zero, the cleanup's structural type and receiver place, and a
    /// mutably writable borrow access.
    pub fn is_cleanup_receiver(&self, cleanup: &crate::NominalAffineCleanup) -> bool {
        self.position == 0
            && self.structural_type == cleanup.structural_type
            && cleanup.cleanup_receiver == Some(self.place)
            && matches!(self.access, crate::exclusive_borrow_pair_pattern!())
    }

    /// Whether the parameter carries an ordinary unqualified root: an
    /// unrestricted or affine multiplicity with no declared or projected
    /// qualifications. Borrowed windows and byte-sequence field stores admit
    /// only such roots — qualifications or linear custody would not survive
    /// the extraction.
    pub fn is_unqualified_root(&self) -> bool {
        matches!(
            self.multiplicity,
            crate::nonlinear_multiplicity_pair_pattern!()
        ) && self.qualifications.is_empty()
            && self.projected_qualifications.is_empty()
    }

    /// Whether the parameter is an owned affine root carrying no declared or
    /// projected qualifications — the shape affine cleanup and transfer
    /// witnesses admit directly.
    pub fn is_unqualified_owned_affine(&self) -> bool {
        self.access == StructuralAccess::Owned
            && self.multiplicity == StructuralMultiplicity::Affine
            && self.qualifications.is_empty()
            && self.projected_qualifications.is_empty()
    }

    /// Whether a scalar read at the given path may use this parameter: an
    /// unrestricted multiplicity, or an affine one once the path projects
    /// below the root, with no residual qualifications.
    pub fn admits_scalar_read_at(&self, path_is_empty: bool) -> bool {
        (self.multiplicity == StructuralMultiplicity::Unrestricted
            || (!path_is_empty && self.multiplicity == StructuralMultiplicity::Affine))
            && self.qualifications.is_empty()
            && self.projected_qualifications.is_empty()
    }
}

/// Exact normal structural result signature. The result place is proof-visible
/// and receives ownership only through a `ReturnStructural` edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralResultDeclaration {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    /// Strictly ordered qualifications transferred with the value.
    pub qualifications: Vec<StructuralDomainId>,
    /// Strictly ordered exact qualifications transferred with nonempty paths
    /// beneath the result root.
    pub projected_qualifications: Vec<StructuralPathQualification>,
    /// Exact reference leaves and their possible formal ingress sources. Verification
    /// reconstructs every returning arm's origin before accepting this map;
    /// callers substitute actual places only after successful completion.
    pub reference_sources: Vec<StructuralReferenceResultSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralReferenceResultSource {
    pub path: Vec<StructuralPathSegment>,
    /// Nonempty, strictly ordered alternatives for this one reference leaf.
    /// These are possible origins, not simultaneous permission carriers.
    pub sources: Vec<StructuralReferenceOrigin>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralReferenceOrigin {
    Parameter(super::ReferenceOriginSource),
    Invocation {
        carrier_type: semantic_vocabulary::StructuralTypeId,
        path: Vec<super::ReferenceOriginProjection>,
        extent: semantic_vocabulary::ReferenceOriginExtent,
        access: crate::StructuralAccess,
    },
}

impl StructuralReferenceOrigin {
    pub fn parameter(&self) -> Option<&super::ReferenceOriginSource> {
        match self {
            Self::Parameter(source) => Some(source),
            Self::Invocation { .. } => None,
        }
    }
    pub fn access(&self) -> crate::StructuralAccess {
        match self {
            Self::Parameter(source) => source.access,
            Self::Invocation { access, .. } => *access,
        }
    }
    pub fn path(&self) -> &[super::ReferenceOriginProjection] {
        match self {
            Self::Parameter(source) => &source.path,
            Self::Invocation { path, .. } => path,
        }
    }
    pub fn extent(&self) -> semantic_vocabulary::ReferenceOriginExtent {
        match self {
            Self::Parameter(source) => source.extent,
            Self::Invocation { extent, .. } => *extent,
        }
    }
}

impl From<super::ReferenceOriginSource> for StructuralReferenceOrigin {
    fn from(source: super::ReferenceOriginSource) -> Self {
        Self::Parameter(source)
    }
}

impl From<StructuralArgument> for StructuralReferenceOrigin {
    fn from(source: StructuralArgument) -> Self {
        Self::Parameter(source.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralArgument {
    pub place: PlaceId,
    pub path: Vec<StructuralPathSegment>,
    pub access: StructuralAccess,
}

impl StructuralArgument {
    /// Spell a whole-home operand with the access its consumer requests.
    /// This constructor supplies no storage or custody authority.
    pub fn whole(place: PlaceId, access: StructuralAccess) -> Self {
        Self {
            place,
            path: Vec::new(),
            access,
        }
    }
}
