//! Keep the source declarations a program used beside its emitted code. These
//! rows grant no permission on their own: production binds them to the accepted
//! program before a receiver checks which declarations its packages may select.

use crate::compiler_evaluation::DeclarationCustody;
use crate::{
    BlockId, BoundaryMachineId, CallableContractId, CallableInvocationSite, EdgeId,
    MachineDescriptorId, MachineId, OperationId, SelectionDeclarationId, SelectionQuestionId,
    StructuralTypeId, TargetMachineCallable, ValueId,
};
use arena::IdTable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, record_storage::Record)]
pub enum ProductTarget {
    LinuxArm64,
    LinuxX86_64,
    MacosArm64,
    MacosX86_64,
    WindowsX86_64,
    UefiX86_64,
    CrossPlatformCli,
    LocalUnchecked,
    AlphaBootstrap,
    WindowsArm64,
}

impl ProductTarget {
    /// A target's code is its position here, so new targets append.
    pub const ALL: [Self; 10] = [
        Self::LinuxArm64,
        Self::LinuxX86_64,
        Self::MacosArm64,
        Self::MacosX86_64,
        Self::WindowsX86_64,
        Self::UefiX86_64,
        Self::CrossPlatformCli,
        Self::LocalUnchecked,
        Self::AlphaBootstrap,
        Self::WindowsArm64,
    ];

    pub fn from_code(code: u64) -> Option<Self> {
        usize::try_from(code)
            .ok()
            .and_then(|index| Self::ALL.get(index))
            .copied()
    }

    pub fn from_canonical_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|target| target.canonical_name() == name)
    }

    pub fn code(self) -> u64 {
        Self::ALL
            .into_iter()
            .position(|target| target == self)
            .expect("complete target catalog") as u64
    }

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::LinuxArm64 => "linux_arm64",
            Self::LinuxX86_64 => "linux_x86_64",
            Self::MacosArm64 => "macos_arm64",
            Self::MacosX86_64 => "macos_x86_64",
            Self::WindowsX86_64 => "windows_x86_64",
            Self::UefiX86_64 => "uefi_x86_64",
            Self::CrossPlatformCli => "cross_platform_cli",
            Self::LocalUnchecked => "local_unchecked",
            Self::AlphaBootstrap => "alpha_bootstrap",
            Self::WindowsArm64 => "windows_arm64",
        }
    }

    pub const fn source_case(self) -> &'static str {
        match self {
            Self::LinuxArm64 => "LinuxArm64",
            Self::LinuxX86_64 => "LinuxX86_64",
            Self::MacosArm64 => "MacosArm64",
            Self::MacosX86_64 => "MacosX86_64",
            Self::WindowsX86_64 => "WindowsX86_64",
            Self::UefiX86_64 => "UefiX86_64",
            Self::CrossPlatformCli => "CrossPlatformCli",
            Self::LocalUnchecked => "LocalUnchecked",
            Self::AlphaBootstrap => "AlphaBootstrap",
            Self::WindowsArm64 => "WindowsArm64",
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record,
)]
pub enum CompositionMode {
    #[default]
    Fused,
    Independent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBindingProducer {
    pub package: crate::PackageKeyIdentity,
    pub declaration_path: String,
    pub callable_identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, record_storage::Record)]
pub enum WireCompatibilityFact {
    Readable,
    Writable,
    PreserveUnknown,
    Canonical,
    CompleteMigration,
}

impl WireCompatibilityFact {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Readable" => Some(Self::Readable),
            "Writable" => Some(Self::Writable),
            "PreserveUnknown" => Some(Self::PreserveUnknown),
            "Canonical" => Some(Self::Canonical),
            "CompleteMigration" => Some(Self::CompleteMigration),
            _ => None,
        }
    }
}

/// Operation numbers are local to a machine; the boundary fixes its requirement.
/// The existing Build request and host records share this one representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
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

impl SelectionPurpose {
    /// Imported library declarations retain their Product namespace even
    /// when checked for a consuming Build. Only that Build's actual package
    /// owns declarations in its Build namespace.
    pub fn for_declaring_package(
        self,
        build_root: Option<crate::PackageKeyIdentity>,
        declaring: crate::PackageKeyIdentity,
    ) -> Self {
        if build_root == Some(declaring) {
            self
        } else {
            Self::Product
        }
    }
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
    /// The emitted Build call that makes this request. Only the published
    /// module emits one. A generic Build helper's own check form never runs
    /// its request, so its designation has no site; each application of the
    /// helper that does run is published with its own.
    pub site: Option<BuildCallSite>,
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
        /// The declaration that authored the choice: the lexical caller for
        /// ordinary calls, or the exact carrier for generated cleanup calls.
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
    pub dependency_calls: crate::dependency_reach::DependencyCallOrigins,
    pub declarations: IdTable<SelectionDeclarationId, SelectionDeclaration>,
    pub questions: IdTable<SelectionQuestionId, SelectionQuestion>,
    pub joins: Vec<SelectionJoin>,
}

#[cfg(test)]
mod tests {
    use super::ProductTarget;

    #[test]
    fn saved_product_target_codes_keep_their_published_meanings() {
        for (code, name) in [
            (0, "linux_arm64"),
            (1, "linux_x86_64"),
            (2, "macos_arm64"),
            (3, "macos_x86_64"),
            (4, "windows_x86_64"),
            (5, "uefi_x86_64"),
            (6, "cross_platform_cli"),
            (7, "local_unchecked"),
            (8, "alpha_bootstrap"),
        ] {
            let target = ProductTarget::from_code(code).unwrap();
            assert_eq!(target.canonical_name(), name);
            assert_eq!(ProductTarget::from_canonical_name(name), Some(target));
            assert_eq!(target.code(), code);
        }
        assert_eq!(ProductTarget::from_code(u64::MAX), None);
    }
}
