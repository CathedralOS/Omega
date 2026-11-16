//! Keep the source declarations a program used beside its emitted code. These
//! rows grant no permission on their own: production binds them to the accepted
//! program before a receiver checks which declarations its packages may select.

use crate::compiler_evaluation::DeclarationCustody;
use crate::{
    BlockId, BoundaryMachineId, CallableContractId, CallableInvocationSite, EdgeId, IdTable,
    MachineDescriptorId, MachineId, OperationId, SelectionDeclarationId, SelectionQuestionId,
    StructuralTypeId, TargetMachineCallable, ValueId,
};

/// Operation numbers are local to a machine; the boundary fixes its requirement.
/// The existing Build request and host records share this one representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct BuildCallSite {
    pub machine: MachineId,
    pub operation: OperationId,
    pub boundary: BoundaryMachineId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionDeclaration {
    pub identity: String,
    /// Callable type meaning can be shared by distinct source declarations.
    pub callable_identity: Option<String>,
    pub name: String,
    pub custody: DeclarationCustody,
    pub purpose: SelectionPurpose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SelectionPurpose {
    Build,
    Product,
}

/// Other accepted generic forms have their own local machine/value namespaces.
/// Their semantic questions must not be joined against the published module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SelectionForm {
    Published,
    Semantic { ordinal: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SelectionNode {
    AuthoredSelection {
        occurrence: u64,
        partition: Option<u64>,
    },
    Declaration {
        index: u32,
        generation: u32,
    },
    Expression {
        index: u32,
        generation: u32,
    },
    Statement {
        index: u32,
        generation: u32,
    },
    TypeReference {
        index: u32,
        generation: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectionOccurrence {
    pub lexical_owner: SelectionDeclarationId,
    pub node: SelectionNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SelectionBindingKind {
    Type,
    Value,
    Machine,
    Conformance,
    PropositionMachine,
    Lifetime,
    Proof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionReference {
    Declaration(SelectionDeclarationId),
    /// A runtime call uses its structural callable meaning, without selecting
    /// the package in which an equivalent contract happened to be written.
    RuntimeContract {
        identity: String,
    },
    /// An exact Build operand whose declaration is resolved by Product admission.
    /// Recording this input grants no declaration or execution permission.
    ProductInput {
        input_selection: [u8; 32],
    },
    Intrinsic {
        identity: String,
    },
    LocalBinding {
        owner: SelectionDeclarationId,
        kind: SelectionBindingKind,
        index: u32,
        generation: u32,
    },
}

/// The canonical Build producer authenticates these associations against the
/// actual request and its selected input. Purpose or role alone cannot do so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionProductDesignation {
    pub site: BuildCallSite,
    pub operand: u32,
    pub input_selection: [u8; 32],
    pub static_application: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionRole {
    Declaration,
    Type,
    Contract,
    Executable,
    MachineValue,
    ProductDesignation(SelectionProductDesignation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionQuestion {
    pub form: SelectionForm,
    pub occurrence: SelectionOccurrence,
    pub selected: SelectionReference,
    pub purpose: SelectionPurpose,
    pub role: SelectionRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionCallTarget {
    Declared(TargetMachineCallable),
    Selected {
        contract: CallableContractId,
        selector: ValueId,
    },
}

/// A physical join always belongs to the published module. Source-only generic
/// questions remain in the packet and need no fabricated physical association.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionJoin {
    Machine {
        declaration: SelectionDeclarationId,
        machine: MachineId,
        head: BlockId,
    },
    Boundary {
        declaration: SelectionDeclarationId,
        boundary: BoundaryMachineId,
    },
    Invocation {
        question: SelectionQuestionId,
        original_caller: SelectionDeclarationId,
        machine: MachineId,
        site: CallableInvocationSite,
        target: SelectionCallTarget,
    },
    Descriptor {
        question: SelectionQuestionId,
        machine: MachineId,
        operation: OperationId,
        descriptor: MachineDescriptorId,
        selected: SelectionDeclarationId,
    },
    FoldedCall {
        question: SelectionQuestionId,
        original_caller: SelectionDeclarationId,
        original_target: SelectionDeclarationId,
        machine: MachineId,
        edge: EdgeId,
        head: BlockId,
    },
    Structure {
        declaration: SelectionDeclarationId,
        structural_type: StructuralTypeId,
        storage_identity: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectionQuestions {
    pub declarations: IdTable<SelectionDeclarationId, SelectionDeclaration>,
    pub questions: IdTable<SelectionQuestionId, SelectionQuestion>,
    pub joins: Vec<SelectionJoin>,
}
