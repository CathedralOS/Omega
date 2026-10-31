//! Exact-owner identities and the ordered D29 demand rows of one Terminal
//! artifact.
//!
//! The authoritative compiler projection derives each canonical identity from
//! the managed package identity or exact toolchain source digest plus
//! declaration path. Constructing this data carrier grants no coverage or
//! execution authority.

use crate::terminal_module::TerminalIdentityIndex;
use crate::{CheckedMachineDeclaration, NominalDeclarationIdentity};
use semantic_vocabulary::OperationId;

/// Exact-owner canonical identity of one structural type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryTypeIdentity(TerminalIdentityIndex);

impl BoundaryTypeIdentity {
    pub fn new(identity: TerminalIdentityIndex) -> Self {
        Self(identity)
    }

    pub fn identity(&self) -> TerminalIdentityIndex {
        self.0
    }
}

/// One exact overloaded operator requirement.
///
/// `declaration` prevents same-path declarations from different package or
/// toolchain owners from colliding. `overload` is Psi's canonical signature
/// coordinate for the exact operator declaration, not source spelling.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryOperatorRequirement {
    declaration: NominalDeclarationIdentity,
    overload: String,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
}

impl BoundaryOperatorRequirement {
    pub fn new(
        declaration: NominalDeclarationIdentity,
        overload: String,
        package: Option<semantic_vocabulary::PackageKeyIdentity>,
    ) -> Result<Self, &'static str> {
        if overload.is_empty() {
            return Err("boundary operator overload identity is empty");
        }
        Ok(Self {
            declaration,
            overload,
            package,
        })
    }

    pub const fn declaration(&self) -> &NominalDeclarationIdentity {
        &self.declaration
    }

    pub fn overload(&self) -> &str {
        &self.overload
    }

    pub const fn package(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.package
    }
}

/// A checked body that can satisfy a closed requirement. Psi retains candidates;
/// a target's Build selects one later. No source handle or selected plan belongs
/// in this record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryApplicationCandidate {
    pub requirement: BoundaryOperatorRequirement,
    pub application: BoundaryApplication,
    pub selection_identity: TerminalIdentityIndex,
    pub selection_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub body: CheckedMachineDeclaration,
}

/// Canonical D29 application shape. Empty means a real zero-length operator
/// telescope; it never stands for an ordinary boundary-trait invocation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryApplication {
    Empty,
    Exact(Vec<BoundaryApplicationArgument>),
}

impl BoundaryApplication {
    pub fn exact(arguments: Vec<BoundaryApplicationArgument>) -> Result<Self, &'static str> {
        if arguments.is_empty() {
            return Err("nonempty boundary application has no arguments");
        }
        for (ordinal, argument) in arguments.iter().enumerate() {
            let expected = u32::try_from(ordinal)
                .map_err(|_| "boundary application exceeds the supported ordinal range")?;
            if argument.binder_ordinal() != expected {
                return Err("boundary application arguments are not in exact binder order");
            }
        }
        Ok(Self::Exact(arguments))
    }

    pub fn arguments(&self) -> &[BoundaryApplicationArgument] {
        match self {
            Self::Empty => &[],
            Self::Exact(arguments) => arguments,
        }
    }
}

/// One declaration-ordered, category-tagged static binding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryApplicationArgument {
    Type {
        binder_ordinal: u32,
        type_identity: BoundaryTypeIdentity,
    },
    Const {
        binder_ordinal: u32,
        declared_carrier: BoundaryTypeIdentity,
        value_type: String,
        value_encoding: String,
    },
}

impl BoundaryApplicationArgument {
    pub fn type_argument(binder_ordinal: u32, type_identity: BoundaryTypeIdentity) -> Self {
        Self::Type {
            binder_ordinal,
            type_identity,
        }
    }

    pub fn const_argument(
        binder_ordinal: u32,
        declared_carrier: BoundaryTypeIdentity,
        value_type: String,
        value_encoding: String,
    ) -> Result<Self, &'static str> {
        if value_type.is_empty() || value_encoding.is_empty() {
            return Err("boundary const application has an empty canonical value identity");
        }
        Ok(Self::Const {
            binder_ordinal,
            declared_carrier,
            value_type,
            value_encoding,
        })
    }

    pub const fn binder_ordinal(&self) -> u32 {
        match self {
            Self::Type { binder_ordinal, .. } | Self::Const { binder_ordinal, .. } => {
                *binder_ordinal
            }
        }
    }
}

/// One exact source-free D29 demand in a canonical Terminal artifact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalBoundaryApplicationDemand {
    terminal_operation: OperationId,
    requirement: BoundaryOperatorRequirement,
    application: BoundaryApplication,
}

impl TerminalBoundaryApplicationDemand {
    pub fn new(
        terminal_operation: OperationId,
        requirement: BoundaryOperatorRequirement,
        application: BoundaryApplication,
    ) -> Self {
        Self {
            terminal_operation,
            requirement,
            application,
        }
    }

    pub const fn terminal_operation(&self) -> OperationId {
        self.terminal_operation
    }

    pub const fn requirement(&self) -> &BoundaryOperatorRequirement {
        &self.requirement
    }

    pub const fn application(&self) -> &BoundaryApplication {
        &self.application
    }
}
