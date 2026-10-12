//! Exact-owner identities and the ordered D29 demand rows of one Terminal
//! artifact.
//!
//! The authoritative compiler projection derives each canonical identity from
//! the managed package identity or exact toolchain source digest plus
//! declaration path. Constructing this data carrier grants no coverage or
//! execution authority.

use crate::terminal_module::{TerminalIdentityIndex, TerminalIdentityTable};
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
    pub target: Option<String>,
    pub body: CheckedMachineDeclaration,
}

/// A closed static tuple retained independently of provider selection.
///
/// Empty is an authenticated zero-length declaration telescope. The owning
/// requirement role still distinguishes a D29 operator from a trait member;
/// absence of a trait application never produces this value. Lifetime bindings
/// have their own declaration order and preserve both correspondence subjects.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryApplication {
    Empty,
    Exact {
        arguments: Vec<BoundaryApplicationArgument>,
        lifetimes: Vec<BoundaryApplicationLifetimeBinding>,
    },
}

pub const MAX_BOUNDARY_APPLICATION_DEPTH: usize = 256;

impl BoundaryApplication {
    pub fn exact(arguments: Vec<BoundaryApplicationArgument>) -> Result<Self, &'static str> {
        Self::exact_with_lifetimes(arguments, Vec::new())
    }

    pub fn exact_with_lifetimes(
        arguments: Vec<BoundaryApplicationArgument>,
        lifetimes: Vec<BoundaryApplicationLifetimeBinding>,
    ) -> Result<Self, &'static str> {
        let application = Self::Exact {
            arguments,
            lifetimes,
        };
        application.validate()?;
        Ok(application)
    }

    pub fn arguments(&self) -> &[BoundaryApplicationArgument] {
        match self {
            Self::Empty => &[],
            Self::Exact { arguments, .. } => arguments,
        }
    }

    pub fn lifetimes(&self) -> &[BoundaryApplicationLifetimeBinding] {
        match self {
            Self::Empty => &[],
            Self::Exact { lifetimes, .. } => lifetimes,
        }
    }

    /// Validate the tuple structure once at an untrusted consumption boundary.
    /// Declaration/category admission and execution authority remain separate.
    pub fn validate(&self) -> Result<(), &'static str> {
        self.validate_identity_references(|identity, absent_reason| {
            if identity.is_absent() {
                Err(absent_reason)
            } else {
                Ok(())
            }
        })
    }

    /// Admit every referenced identity against its actual issuing table at
    /// untrusted ingress. This shares structural validation and the complete
    /// nested tuple/lifetime traversal; it grants no declaration or execution
    /// authority and does not reconstruct source semantics from identity text.
    pub fn validate_in_identity_table(
        &self,
        identities: &TerminalIdentityTable,
    ) -> Result<(), &'static str> {
        self.validate_identity_references(|identity, absent_reason| {
            if identity.is_absent() {
                return Err(absent_reason);
            }
            match identities.text(identity) {
                Some(text) if !text.is_empty() => Ok(()),
                _ => Err("boundary application identity is not in its issuing table"),
            }
        })?;
        let mut pending = vec![self];
        while let Some(application) = pending.pop() {
            for argument in application.arguments() {
                if let BoundaryApplicationArgument::Declaration { application, .. } = argument {
                    pending.push(application);
                }
            }
            for binding in application.lifetimes() {
                let formal = identities
                    .lifetime_formal_role(
                        binding.parameter.declaration.identity(),
                        binding.parameter.parameter_ordinal,
                    )
                    .ok_or(
                        "boundary lifetime formal has no original declaration roster membership",
                    )?;
                match &binding.argument {
                    BoundaryLifetimeArgument::AnonymousInvocation
                        if formal != crate::TerminalLifetimeFormalRole::Invocation =>
                    {
                        return Err(
                            "captured boundary lifetime cannot use anonymous invocation erasure",
                        );
                    }
                    BoundaryLifetimeArgument::Named(argument) => {
                        identities.lifetime_formal_role(argument.declaration.identity(), argument.parameter_ordinal)
                            .ok_or("named boundary lifetime actual has no original declaration roster membership")?;
                    }
                    BoundaryLifetimeArgument::AnonymousInvocation => {}
                }
            }
        }
        Ok(())
    }

    fn validate_identity_references(
        &self,
        mut admit_identity: impl FnMut(TerminalIdentityIndex, &'static str) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        let mut pending = vec![(self, 0usize)];
        while let Some((application, depth)) = pending.pop() {
            if depth > MAX_BOUNDARY_APPLICATION_DEPTH {
                return Err("boundary application nesting exceeds the supported depth");
            }
            if matches!(application, Self::Exact { arguments, lifetimes }
                if arguments.is_empty() && lifetimes.is_empty())
            {
                return Err("nonempty boundary application has no bindings");
            }
            for (ordinal, argument) in application.arguments().iter().enumerate() {
                if usize::try_from(argument.binder_ordinal()).ok() != Some(ordinal) {
                    return Err("boundary application arguments are not in exact binder order");
                }
                match argument {
                    BoundaryApplicationArgument::Type { type_identity, .. } => {
                        admit_identity(
                            type_identity.identity(),
                            "boundary application type has an absent identity",
                        )?;
                    }
                    BoundaryApplicationArgument::Const {
                        declared_carrier,
                        value_type,
                        value_encoding,
                        ..
                    }
                    | BoundaryApplicationArgument::Value {
                        declared_carrier,
                        value_type,
                        value_encoding,
                        ..
                    } => {
                        admit_identity(
                            declared_carrier.identity(),
                            "boundary value application has an absent carrier identity",
                        )?;
                        if value_type.is_empty() || value_encoding.is_empty() {
                            return Err(
                                "boundary value application has an empty canonical value identity",
                            );
                        }
                    }
                    BoundaryApplicationArgument::Declaration {
                        declaration,
                        application,
                        ..
                    } => {
                        admit_identity(
                            declaration.identity(),
                            "boundary declaration application has an absent identity",
                        )?;
                        pending.push((application, depth + 1));
                    }
                }
            }
            for (ordinal, binding) in application.lifetimes().iter().enumerate() {
                admit_identity(
                    binding.parameter.declaration.identity(),
                    "boundary application lifetime has an absent declaration identity",
                )?;
                if let BoundaryLifetimeArgument::Named(argument) = &binding.argument {
                    admit_identity(
                        argument.declaration.identity(),
                        "boundary application lifetime has an absent declaration identity",
                    )?;
                }
                if usize::try_from(binding.binder_ordinal).ok() != Some(ordinal) {
                    return Err("boundary application lifetimes are not in exact binder order");
                }
                if application.lifetimes()[..ordinal]
                    .iter()
                    .any(|previous| previous.parameter == binding.parameter)
                {
                    return Err("boundary application repeats a lifetime parameter");
                }
            }
        }
        Ok(())
    }

    /// The currently published D29 owner admits only its original closed
    /// type/constant categories. A general tuple cannot grant a different
    /// requirement role or silently widen that owner's execution vocabulary.
    pub fn validate_operator_application(&self) -> Result<(), &'static str> {
        self.validate()?;
        if !self.lifetimes().is_empty()
            || self.arguments().iter().any(|argument| {
                !matches!(
                    argument,
                    BoundaryApplicationArgument::Type { .. }
                        | BoundaryApplicationArgument::Const { .. }
                )
            })
        {
            return Err("operator application has an unsupported binder category");
        }
        Ok(())
    }
}

/// One original lifetime formal: nominal declaration custody and its
/// declaration-local position, independent of producer arenas or spellings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryLifetimeParameter {
    pub declaration: NominalDeclarationIdentity,
    pub parameter_ordinal: u32,
}

/// Invocation erasure retains an operand class without inventing a nominal
/// source declaration. Exact loan resources remain the checked call's facts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryLifetimeArgument {
    Named(BoundaryLifetimeParameter),
    AnonymousInvocation,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryApplicationLifetimeBinding {
    pub binder_ordinal: u32,
    pub parameter: BoundaryLifetimeParameter,
    pub argument: BoundaryLifetimeArgument,
}

/// A declaration operand keeps its genuine category and complete nested tuple.
/// A conformance is not a selected machine or an assumed relationship receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundaryDeclarationArgumentKind {
    Machine,
    Conformance,
    Proposition,
}

/// One declaration-ordered, category-tagged static binding. Closed runtime-
/// capable value binders stay distinct from compile-time-only const binders.
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
    Value {
        binder_ordinal: u32,
        declared_carrier: BoundaryTypeIdentity,
        value_type: String,
        value_encoding: String,
    },
    Declaration {
        binder_ordinal: u32,
        kind: BoundaryDeclarationArgumentKind,
        declaration: NominalDeclarationIdentity,
        application: BoundaryApplication,
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

    pub fn value_argument(
        binder_ordinal: u32,
        declared_carrier: BoundaryTypeIdentity,
        value_type: String,
        value_encoding: String,
    ) -> Result<Self, &'static str> {
        if value_type.is_empty() || value_encoding.is_empty() {
            return Err("boundary value application has an empty canonical value identity");
        }
        Ok(Self::Value {
            binder_ordinal,
            declared_carrier,
            value_type,
            value_encoding,
        })
    }

    pub const fn binder_ordinal(&self) -> u32 {
        match self {
            Self::Type { binder_ordinal, .. }
            | Self::Const { binder_ordinal, .. }
            | Self::Value { binder_ordinal, .. }
            | Self::Declaration { binder_ordinal, .. } => *binder_ordinal,
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
