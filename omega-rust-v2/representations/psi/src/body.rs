use crate::Site;
use crate::{IntegerOperation, TypeId};
use num_bigint::BigInt;
use std::num::NonZeroUsize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockId(pub NonZeroUsize);

impl BlockId {
    pub fn get(self) -> usize {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EdgeId(pub NonZeroUsize);

impl EdgeId {
    pub fn get(self) -> usize {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ValueId(pub NonZeroUsize);

impl ValueId {
    pub fn get(self) -> usize {
        self.0.get()
    }

    pub fn index(self) -> usize {
        self.get() - 1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaceId(pub NonZeroUsize);

impl PlaceId {
    pub fn get(self) -> usize {
        self.0.get()
    }

    pub fn index(self) -> usize {
        self.get() - 1
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct OperationId(pub NonZeroUsize);

impl OperationId {
    pub fn get(self) -> usize {
        self.0.get()
    }

    pub fn index(self) -> usize {
        self.get() - 1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Parameter<Scalar = ValueId, Structural = PlaceId> {
    Scalar(Scalar),
    Structural(Structural),
}

/// Runtime order and lane of the declared formal before any invocation supplies an operand.
pub type ParameterType = Parameter<TypeId, TypeId>;

impl ParameterType {
    pub fn carrier(self) -> TypeId {
        match self {
            Self::Scalar(id) | Self::Structural(id) => id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    pub ty: TypeId,
    pub origin: ValueOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueOrigin {
    Parameter,
    BlockParameter(BlockId),
    Component { operation: OperationId, position: usize },
    Operation(OperationId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PsiOperation {
    pub id: OperationId,
    pub result: OperationResult,
    pub kind: OperationKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationKind {
    BeginAssembly(Box<crate::AssemblyEntry>),
    AssemblyInstruction(Box<crate::AssemblyInstruction>),
    Constant(ScalarLiteral),
    Boolean {
        primitive: BooleanPrimitive,
        operands: Vec<ValueId>,
    },
    ExactIntegerCast {
        operand: ValueId,
    },
    /// The operation itself stops with Trap if its primitive fails on these input
    /// carriers. Only the successful value reaches the following operation or edge.
    TrappingInteger {
        primitive: crate::TrappingIntegerPrimitive,
        left: ValueId,
        right: ValueId,
    },
    Integer {
        operation: IntegerOperation,
        operands: Vec<ValueId>,
    },
    Call {
        callee: crate::CallTarget,
        arguments: Vec<CallArgument>,
        requirements: Vec<crate::CallRequirement>,
    },
    /// Construct a descriptor for this packaged machine under `contract`.
    /// The checker must establish the machine's refinement of the contract.
    ConstructCallableDescriptor {
        machine: crate::MachineId,
        contract: crate::CallableContractId,
    },
    /// An erased proof application of a logical theorem. Its result is always
    /// `OperationResult::Unit`; see `TheoremApplication`.
    ApplyTheorem(Box<crate::TheoremApplication>),
    /// The same descriptor under the destination contract. The checker judges
    /// that the source value's contract refines `contract`; no target changes.
    RefineCallableDescriptor {
        source: ValueId,
        contract: crate::CallableContractId,
    },
    ByteSequenceLength {
        source: PlaceId,
    },
    ByteSequenceRead {
        source: PlaceId,
        index: ValueId,
        length: ValueId,
        obligation: crate::BoundsObligationId,
    },
    /// Form a reference descriptor to the original place. A referent projection
    /// through a loan records the parent descriptor of this reborrow.
    BorrowPlace {
        source: ProjectedPlace,
        access: crate::ReferenceAccess,
    },
    /// Same-backing shared scalar view. This operation owns the obligation
    /// to compare the selected layouts of its two referents and prove source
    /// validity implies destination validity at this place. It supplies no
    /// compatibility fact. The source and result descriptors retain their
    /// respective referent types, with unchanged authority and lifetime.
    ScalarBitRecast {
        source: PlaceId,
        source_referent: crate::ScalarType,
        destination_referent: crate::ScalarType,
    },
    /// Same-backing exclusive scalar view. `lender` is the input descriptor
    /// and identifies the loan to which valid storage is owed at handback,
    /// including obligations of the underlying declared place. The result
    /// carries this debt even when forwarded. Immediate established use needs
    /// compatible geometry, exclusive custody and validity in both directions;
    /// this operation neither proves those facts nor performs restoration.
    MutableScalarBitRecast {
        lender: PlaceId,
        source_referent: crate::ScalarType,
        destination_referent: crate::ScalarType,
    },
    /// Take the live value through an exclusive loan. The source path must be
    /// re-established before the loan can be released.
    BorrowedMove {
        source: ProjectedPlace,
    },
    /// Dispose of the previous live value, when present, then install the replacement at the captured destination.
    StructuralStore {
        destination: ProjectedPlace,
        value: PlaceId,
        disposition: StructuralStoreDisposition,
    },
    /// Exact `u64` element count of this shared or exclusive element
    /// descriptor; never a byte count.
    ElementViewLength {
        source: PlaceId,
    },
    /// The scalar at `path` inside element `index`, bounded by `length`: the
    /// same descriptor's immediately preceding element count. An empty path
    /// selects a scalar element.
    ElementViewRead {
        source: PlaceId,
        index: ValueId,
        length: ValueId,
        obligation: crate::BoundsObligationId,
        path: Vec<crate::ElementLeafStep>,
    },
    /// Zero every leaf of fresh storage for the result's nominal record. The
    /// operation carries the binding's `ZeroValue` obligation: the checker judges
    /// each default-domain clause of that schema, and of each schema it contains,
    /// for the all-zero value. Lowering neither discharges nor assumes them.
    EstablishZeroedStructural,
    /// Owes every default-domain clause of the result's nominal schema at
    /// this operation, with `self` formed from this case and its payload.
    EstablishCase {
        case: crate::DeclarationId,
        fields: Vec<FieldInitializer>,
    },
    /// Owes every default-domain clause of the result's nominal schema at
    /// this operation, with `self` formed from these field operands.
    EstablishRecord {
        fields: Vec<FieldInitializer>,
    },
    /// Read the selected unrestricted structural value into independent storage.
    CopyStructural {
        source: ProjectedPlace,
    },
    /// Transfers custody of the selected place into the operation result.
    MoveStructural {
        source: ProjectedPlace,
    },
    /// Consuming decomposition publishes the exact field partition of the owning declaration.
    DecomposeRecord {
        source: ProjectedPlace,
        declaration: crate::DeclarationId,
        fields: Vec<DecomposedField>,
    },
    StructuralScalarFieldStore {
        destination: ProjectedPlace,
        field: crate::DeclarationId,
        value: ValueId,
    },
    /// Replace the scalar in the original storage this place selects: storage
    /// reached through an exclusive loan, or, at an empty path, a primitive
    /// home listed in `MachineBody::mutable_homes`.
    ProjectedScalarStore {
        destination: ProjectedPlace,
        value: ValueId,
    },
    StructuralCaseMembership {
        source: ProjectedPlace,
        case: crate::DeclarationId,
    },
    /// Read a copyable scalar from the original storage reached through this place.
    ProjectedScalarRead {
        source: ProjectedPlace,
    },
    PrimitiveScalarRead {
        source: PlaceId,
        path: Vec<crate::RuntimeIndex>,
    },
    StructuralLeafCopy {
        source: PlaceId,
        path: Vec<crate::RuntimeIndex>,
    },
    EstablishScalarArray {
        elements: Vec<ValueId>,
    },
    /// Home an initialized primitive local so it can be borrowed. The result
    /// is an unrestricted, unqualified, claim-free place of the value's exact
    /// primitive type; every loan of the local borrows this one home.
    /// Later reads of the local are `PrimitiveScalarRead`s of the home, not
    /// equality with `value`. A mutable local's home is in
    /// `MachineBody::mutable_homes`, and each assignment is a
    /// `ProjectedScalarStore` into it at an empty path.
    EstablishPrimitiveLocal {
        value: ValueId,
    },
    /// Each place supplies one complete element, in source order. An owned
    /// element transfers once and continues beneath `FixedIndex(ordinal)`.
    EstablishStructuralArray {
        elements: Vec<PlaceId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StructuralStoreDisposition {
    FillVacancy,
    ReplaceUnrestricted,
    Dispose(Vec<crate::CleanupAction>),
}

/// A complete field input, in the declaration order of its record or case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldInitializer {
    pub field: crate::DeclarationId,
    pub operand: ValueOperand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BooleanPrimitive {
    Not,
    Equal,
    NotEqual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationResult {
    Unit,
    Scalar(ValueId),
    Structural(PlaceId),
}

/// A complete consuming pattern installs each component in an independent result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecomposedField {
    pub field: crate::DeclarationId,
    pub result: OperationResult,
}

pub type CallArgument = ValueOperand;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueOperand {
    Scalar(ValueId),
    /// Copy the complete unrestricted value. A shared descriptor retains its borrowed backing.
    CopyWhole(PlaceId),
    /// Transfers the complete structural operand into the formal.
    MoveWhole(PlaceId),
    /// Borrow this original place for the duration of the call.
    Borrow {
        place: ProjectedPlace,
        access: crate::ReferenceAccess,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedPlace {
    pub root: PlaceId,
    pub path: Vec<PlaceProjection>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PlaceProjection {
    Referent,
    /// Namespace of the active payload, established by the discriminator.
    Case(crate::DeclarationId),
    Field(crate::DeclarationId),
    /// Canonical zero-based literal ordinal below the reached array's declared
    /// nonzero extent. It is a static claim segment, never a runtime selector.
    FixedIndex(u64),
    /// One element below an exclusive element view's `Referent`. The view owns
    /// none of its elements and has no static extent: the element is selected
    /// by this occurrence's evaluated selector, owed against `length`, the same
    /// view's dominating `ElementViewLength` observation. A literal selector
    /// still uses this form; it never becomes a `FixedIndex`.
    ViewElement {
        index: crate::RuntimeIndex,
        length: ValueId,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ScalarLiteral {
    Integer(BigInt),
    Boolean(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Place {
    pub ty: TypeId,
    pub origin: PlaceOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaceOrigin {
    Parameter,
    BlockParameter(BlockId),
    Result,
    Component { operation: OperationId, position: usize },
    Operation(OperationId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReturnEdge {
    Unit,
    Scalar(ValueId),
    Structural { source: PlaceId, result: PlaceId },
}

#[derive(Debug, Eq, PartialEq)]
pub struct MachineBody {
    /// The descriptor at each explicit binding occurrence and its destination
    /// ordinal in `LoweredMachine::regions`. The checker judges the relation.
    pub lifetime_bindings: Vec<(PlaceId, usize)>,
    pub call_acknowledgements: Vec<crate::CallAcknowledgement>,
    pub suspension_sites: Vec<crate::SuspensionSite>,
    pub blocks: Vec<crate::BranchBlock>,
    pub operations: Vec<PsiOperation>,
    pub parameters: Vec<Parameter>,
    /// Owned record homes and primitive local homes whose source binding
    /// permits mutation.
    pub mutable_homes: Vec<PlaceId>,
    pub places: Vec<Place>,
    pub values: Vec<Value>,
    pub diagnostics: EntryDiagnostics,
}

#[derive(Debug, Eq, PartialEq)]
pub struct EntryDiagnostics {
    pub body: Site,
    pub parameters: Vec<Site>,
    pub operations: Vec<Site>,
    pub edges: Vec<Site>,
    pub bounds: Vec<Site>,
}
