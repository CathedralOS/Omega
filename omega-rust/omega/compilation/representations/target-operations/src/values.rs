/*
Target-lowered values pair semantic scalar, Boolean, integer, view, and structural expressions with their resolved ABI parameter placements, byte offsets, and reference custody origins. We consolidate structural parameter/argument records, reference roots and origins, byte/element view descriptors, and scalar/Boolean/integer expression trees into one module.

Keeping `ReferenceRoot` alongside `TargetReferenceOrigin` and view/scalar expressions ensures that target lowering and selection's reference-custody audit share one canonical definition of storage vs. incoming-contents provenance.
*/

use crate::ScalarParameterLocation;
use abstract_operations::{IntegerArithmetic, IntegerShiftDirection, IntegerShiftPolicy};
use calling_conventions::{ValuePlacement, ValueShape};
use semantic_vocabulary::{
    BlockId, IntegerType, IntegerValue, ObligationId, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::StructuralPathSegment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralParameter {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: terminal_psi::StructuralMultiplicity,
    pub access: terminal_psi::StructuralAccess,
    pub projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
    pub shape: ValueShape,
    pub placement: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetStructuralArgument {
    pub place: PlaceId,
    pub access: terminal_psi::StructuralAccess,
    pub path: Vec<StructuralPathSegment>,
    pub root_structural_type: StructuralTypeId,
    pub structural_type: StructuralTypeId,
    pub shape: ValueShape,
    pub source_byte_offset: u32,
    pub fixed_array_length: Option<u64>,
    pub element_stride: Option<u32>,
    pub indices: Vec<crate::TargetStructuralRuntimeIndex>,
    pub source: TargetStructuralArgumentSource,
    pub destination: ValuePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetReferenceResult {
    pub path: Vec<StructuralPathSegment>,
    pub origins: Vec<TargetReferenceOrigin>, // Nonempty, strictly ordered alternatives for one permission carrier.
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TargetReferenceOrigin {
    pub base: TargetReferenceOriginBase,
    pub path: Vec<terminal_psi::ReferenceOriginProjection>,
    pub extent: semantic_vocabulary::ReferenceOriginExtent,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetReferenceOriginBase {
    Storage {
        root: PlaceId,
    },
    IncomingContents {
        root: PlaceId,
        slot: Vec<StructuralPathSegment>,
    },
    Invocation {
        root: PlaceId,
        carrier_type: StructuralTypeId,
    },
    Provider {
        boundary: semantic_vocabulary::BoundaryMachineId,
        output: u32,
        invocations: Vec<(semantic_vocabulary::MachineId, OperationId)>,
        referent: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
    },
}

impl TargetReferenceOriginBase {
    pub fn root_place(&self) -> Option<PlaceId> {
        match self {
            Self::Storage { root } | Self::IncomingContents { root, .. } => Some(*root),
            Self::Invocation { .. } | Self::Provider { .. } => None,
        }
    }
}

impl TargetReferenceOrigin {
    pub fn type_projection(
        &self,
    ) -> std::borrow::Cow<'_, [terminal_psi::ReferenceOriginProjection]> {
        match &self.base {
            TargetReferenceOriginBase::Storage { .. }
            | TargetReferenceOriginBase::Invocation { .. }
            | TargetReferenceOriginBase::Provider { .. } => std::borrow::Cow::Borrowed(&self.path),
            TargetReferenceOriginBase::IncomingContents { slot, .. } => {
                let mut path = slot
                    .iter()
                    .cloned()
                    .map(terminal_psi::ReferenceOriginProjection::Exact)
                    .collect::<Vec<_>>();
                path.push(terminal_psi::ReferenceOriginProjection::Exact(
                    StructuralPathSegment::Referent,
                ));
                path.extend_from_slice(&self.path);
                std::borrow::Cow::Owned(path)
            }
        }
    }

    pub fn storage_envelope(&self) -> Option<(PlaceId, Vec<StructuralPathSegment>, bool, bool)> {
        let root = self.base.root_place()?;
        if let TargetReferenceOriginBase::IncomingContents { slot, .. } = &self.base {
            return Some((root, slot.clone(), false, true));
        }
        let path = self
            .path
            .iter()
            .map_while(|projection| match projection {
                terminal_psi::ReferenceOriginProjection::Exact(segment) => Some(segment.clone()),
                terminal_psi::ReferenceOriginProjection::Element => None,
            })
            .collect::<Vec<_>>();
        let exact = path.len() == self.path.len();
        let indirect = self
            .path
            .contains(&terminal_psi::ReferenceOriginProjection::Exact(
                StructuralPathSegment::Referent,
            ));
        Some((root, path, exact, indirect))
    }

    pub fn exact_path(&self) -> Option<Vec<StructuralPathSegment>> {
        if !matches!(self.base, TargetReferenceOriginBase::Storage { .. })
            || self.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
        {
            return None;
        }
        self.path
            .iter()
            .map(|projection| match projection {
                terminal_psi::ReferenceOriginProjection::Exact(segment) => Some(segment.clone()),
                terminal_psi::ReferenceOriginProjection::Element => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReferenceRoot {
    Place(PlaceId),
    Ingress(PlaceId, Vec<StructuralPathSegment>),
    Alternatives(Vec<TargetReferenceOrigin>),
}

impl ReferenceRoot {
    pub fn incoming_contents(root: PlaceId, slot: Vec<StructuralPathSegment>) -> Self {
        Self::Alternatives(vec![TargetReferenceOrigin {
            base: TargetReferenceOriginBase::IncomingContents { root, slot },
            path: Vec::new(),
            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
        }])
    }

    pub fn projected(&self, suffix: &[StructuralPathSegment]) -> Self {
        self.projected_family(
            &suffix
                .iter()
                .cloned()
                .map(terminal_psi::ReferenceOriginProjection::Exact)
                .collect::<Vec<_>>(),
            semantic_vocabulary::ReferenceOriginExtent::Projected,
        )
    }

    pub fn projected_family(
        &self,
        suffix: &[terminal_psi::ReferenceOriginProjection],
        extent: semantic_vocabulary::ReferenceOriginExtent,
    ) -> Self {
        if suffix.is_empty() && extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
            return self.clone();
        }
        let mut origins = self.origins();
        for origin in &mut origins {
            if origin.extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
                origin.path.extend_from_slice(suffix);
                origin.extent = extent;
            }
        }
        if origins.len() == 1
            && let Some(path) = origins[0].exact_path()
            && let Some(root) = origins[0].base.root_place()
        {
            Self::Ingress(root, path)
        } else {
            origins.sort();
            origins.dedup();
            Self::Alternatives(origins)
        }
    }

    pub fn origins(&self) -> Vec<TargetReferenceOrigin> {
        match self {
            Self::Place(root) => vec![TargetReferenceOrigin {
                base: TargetReferenceOriginBase::Storage { root: *root },
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                path: Vec::new(),
            }],
            Self::Ingress(root, path) => vec![TargetReferenceOrigin {
                base: TargetReferenceOriginBase::Storage { root: *root },
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                path: path
                    .iter()
                    .cloned()
                    .map(terminal_psi::ReferenceOriginProjection::Exact)
                    .collect(),
            }],
            Self::Alternatives(origins) => origins.clone(),
        }
    }

    pub fn contains_place(&self, place: PlaceId) -> bool {
        match self {
            Self::Place(root) | Self::Ingress(root, _) => *root == place,
            Self::Alternatives(origins) => origins
                .iter()
                .any(|origin| origin.base.root_place() == Some(place)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetStructuralArgumentSource {
    StructuralHome {
        psi_operation: OperationId,
    },
    EstablishedPrimitiveLocal {
        psi_operation: semantic_vocabulary::OperationId,
    },
    Placement(ValuePlacement),
    BlockParameter {
        block: semantic_vocabulary::BlockId,
        place: PlaceId,
    },
    EstablishedByteView {
        psi_operation: OperationId,
    },
    EstablishedElementView {
        psi_operation: OperationId,
    },
}

impl From<ValuePlacement> for TargetStructuralArgumentSource {
    fn from(placement: ValuePlacement) -> Self {
        Self::Placement(placement)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetByteView {
    Result {
        psi_operation: OperationId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    Owned {
        psi_operation: OperationId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    Literal {
        psi_operation: OperationId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    BlockParameter {
        block: BlockId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    Parameter {
        place: PlaceId,
        placement: ValuePlacement,
    },
    Subslice {
        psi_operation: OperationId,
        place: PlaceId,
        source: terminal_psi::StructuralArgument,
        indices: Vec<crate::TargetStructuralRuntimeIndex>,
        start: Box<TargetIntegerExpression>,
        end: Box<TargetIntegerExpression>,
        length: ValueId,
        obligation: ObligationId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetElementView {
    Result {
        psi_operation: OperationId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    BlockParameter {
        block: BlockId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    Parameter {
        place: PlaceId,
        placement: ValuePlacement,
    },
    Established {
        psi_operation: OperationId,
        place: PlaceId,
        structural_type: StructuralTypeId,
        element: StructuralTypeId,
        root_structural_type: StructuralTypeId,
        source_byte_offset: u32,
        indices: Vec<crate::TargetStructuralRuntimeIndex>,
        extent: TargetViewExtent,
        region: TargetElementViewRegion,
        element_stride: u32,
        source: TargetStructuralArgumentSource,
    },
    Subslice {
        psi_operation: OperationId,
        place: PlaceId,
        source: terminal_psi::StructuralArgument,
        indices: Vec<crate::TargetStructuralRuntimeIndex>,
        start: Box<TargetIntegerExpression>,
        end: Box<TargetIntegerExpression>,
        length: ValueId,
        obligation: ObligationId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetViewExtent {
    Fixed(u64),
    LiveLength,
    StoredLength,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetElementViewRegion {
    WholeCollection,
    WholeRepresentation {
        source_length: ValueId,
        obligation: ObligationId,
    },
    RepresentationBytes {
        start: Box<TargetIntegerExpression>,
        end: Box<TargetIntegerExpression>,
        obligation: ObligationId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetScalarImmediate {
    Boolean(bool),
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
}

impl TargetScalarImmediate {
    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetScalarExpression {
    IeeeFloat(crate::TargetUnitScalarArgumentSource),
    IeeeFloatElementViewRead {
        format: semantic_vocabulary::IeeeFloatFormat,
        psi_operation: semantic_vocabulary::OperationId,
        source_value: semantic_vocabulary::ValueId,
        source: semantic_vocabulary::PlaceId,
        view: Box<TargetElementView>,
        index: Box<TargetIntegerExpression>,
        length: semantic_vocabulary::ValueId,
        obligation: semantic_vocabulary::ObligationId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        element_byte_offset: u32,
    },
    Boolean(TargetBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        expression: TargetIntegerExpression,
    },
}

impl TargetScalarExpression {
    pub const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::IeeeFloat(source) => source.scalar_type(),
            Self::IeeeFloatElementViewRead { format, .. } => ScalarType::IeeeFloat(*format),
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(*scalar_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetBooleanExpression {
    BlockParameter(TargetScalarBlockValue),
    ScalarHome(crate::TargetUnitScalarHomeRequirement),
    Immediate {
        source_value: ValueId,
        value: bool,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    ElementViewRead {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        view: Box<TargetElementView>,
        index: Box<TargetIntegerExpression>,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        element_byte_offset: u32,
    },
    StructuralField {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        field: StructuralFieldId,
        source_placement: ValuePlacement,
        field_byte_offset: u32,
    },
    Not {
        psi_operation: OperationId,
        operand: Box<TargetBooleanExpression>,
    },
    Equal {
        psi_operation: OperationId,
        left: Box<TargetBooleanExpression>,
        right: Box<TargetBooleanExpression>,
    },
    IntegerEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        right_scalar_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    IntegerLessThan {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        right_scalar_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    IntegerLessOrEqual {
        psi_operation: OperationId,
        scalar_type: IntegerType,
        right_scalar_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    ByteSequencePredicate {
        psi_operation: OperationId,
        source_value: ValueId,
        operand: terminal_psi::control_flow::ByteSequenceOperand,
        predicate: terminal_psi::ByteSequencePredicate,
    },
    ByteSequenceEqual {
        psi_operation: OperationId,
        source_value: ValueId,
        left: terminal_psi::control_flow::ByteSequenceOperand,
        right: terminal_psi::control_flow::ByteSequenceOperand,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetIntegerExpression {
    BlockParameter(TargetScalarBlockValue),
    ScalarHome(crate::TargetUnitScalarHomeRequirement),
    ByteSequenceRead {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        view: Box<TargetByteView>,
        index: Box<TargetIntegerExpression>,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
    },
    ByteSequenceLength {
        psi_operation: OperationId,
        source_value: ValueId,
        source: terminal_psi::StructuralArgument,
        indices: Vec<crate::TargetStructuralRuntimeIndex>,
        length_byte_offset: u32,
    },
    ElementViewRead {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        view: Box<TargetElementView>,
        index: Box<TargetIntegerExpression>,
        length: ValueId,
        obligation: semantic_vocabulary::ObligationId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        element_byte_offset: u32,
    },
    ElementViewLength {
        psi_operation: OperationId,
        source_value: ValueId,
        source: terminal_psi::StructuralArgument,
        indices: Vec<crate::TargetStructuralRuntimeIndex>,
        length_byte_offset: u32,
    },
    Immediate {
        source_value: ValueId,
        value: IntegerValue,
    },
    Parameter {
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    },
    StructuralField {
        psi_operation: OperationId,
        source_value: ValueId,
        source: PlaceId,
        field: StructuralFieldId,
        source_placement: ValuePlacement,
        field_byte_offset: u32,
        integer_type: IntegerType,
    },
    BitwiseNot {
        psi_operation: OperationId,
        operand: Box<TargetIntegerExpression>,
    },
    IntegerWiden {
        psi_operation: OperationId,
        source_type: IntegerType,
        operand: Box<TargetIntegerExpression>,
    },
    IntegerExactCast {
        psi_operation: OperationId,
        obligation: semantic_vocabulary::ObligationId,
        source_type: IntegerType,
        operand: Box<TargetIntegerExpression>,
    },
    BitwiseAnd {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    BitwiseOr {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    BitwiseXor {
        psi_operation: OperationId,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    Shift {
        psi_operation: OperationId,
        policy: IntegerShiftPolicy,
        direction: IntegerShiftDirection,
        count_type: IntegerType,
        value: Box<TargetIntegerExpression>,
        count: Box<TargetIntegerExpression>,
    },
    Arithmetic {
        psi_operation: OperationId,
        arithmetic: IntegerArithmetic,
        left: Box<TargetIntegerExpression>,
        right: Box<TargetIntegerExpression>,
    },
    Trapping {
        psi_operation: OperationId,
        primitive: terminal_psi::TrappingIntegerPrimitive,
        operand_type: IntegerType,
        left: Box<TargetIntegerExpression>,
        right: Option<Box<TargetIntegerExpression>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetScalarBlockValue {
    pub block: BlockId,
    pub value: ValueId,
    pub scalar_type: ScalarType,
}
