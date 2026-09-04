#![forbid(unsafe_code)]

//! Source-free D29 boundary-operator application demands.
//!
//! Checked Psi owns the typed source application. This representation is the
//! immutable projection retained beside one canonical Terminal artifact: it
//! names the exact emitted operation, requirement coordinate, and closed
//! tagged application without retaining source paths, arena handles, display
//! spellings, selected plans, or realization authority.

use psi_core::OperationId;

mod coverage;
mod realization;

pub use coverage::{
    BoundaryApplicationCoverageIdentity, OperatorApplicationCoverageRef,
    TerminalBoundaryApplicationCoverage,
};
pub use realization::{
    BoundaryApplicationRealization, BoundaryApplicationRealizationCompanion,
    BoundaryApplicationRealizationRole, TerminalBoundaryApplicationRealizations,
};

/// Exact-owner canonical identity of one nominal declaration.
///
/// The authoritative compiler projection derives these bytes from the managed
/// package identity or exact toolchain source digest plus declaration path.
/// Constructing this data carrier grants no coverage or execution authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryNominalIdentity(String);

impl BoundaryNominalIdentity {
    pub fn new(canonical: String) -> Result<Self, &'static str> {
        if canonical.is_empty() {
            return Err("boundary nominal identity is empty");
        }
        Ok(Self(canonical))
    }

    pub fn canonical(&self) -> &str {
        &self.0
    }
}

/// Exact-owner canonical identity of one structural type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryTypeIdentity(String);

impl BoundaryTypeIdentity {
    pub fn new(canonical: String) -> Result<Self, &'static str> {
        if canonical.is_empty() {
            return Err("boundary type identity is empty");
        }
        Ok(Self(canonical))
    }

    pub fn canonical(&self) -> &str {
        &self.0
    }
}

/// One exact overloaded operator requirement.
///
/// `declaration` prevents same-path declarations from different package or
/// toolchain owners from colliding. `overload` is Psi's canonical signature
/// coordinate for the exact operator declaration, not source spelling.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryOperatorRequirement {
    declaration: BoundaryNominalIdentity,
    overload: String,
}

impl BoundaryOperatorRequirement {
    pub fn new(
        declaration: BoundaryNominalIdentity,
        overload: String,
    ) -> Result<Self, &'static str> {
        if overload.is_empty() {
            return Err("boundary operator overload identity is empty");
        }
        Ok(Self {
            declaration,
            overload,
        })
    }

    pub const fn declaration(&self) -> &BoundaryNominalIdentity {
        &self.declaration
    }

    pub fn overload(&self) -> &str {
        &self.overload
    }
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

/// Complete source-free D29 demand projection for one Terminal artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundaryApplicationDemands {
    terminal_psi: psi_terminal::TerminalPsiIdentity,
    rows: Vec<TerminalBoundaryApplicationDemand>,
}

impl TerminalBoundaryApplicationDemands {
    pub fn new(
        terminal_psi: psi_terminal::TerminalPsiIdentity,
        mut rows: Vec<TerminalBoundaryApplicationDemand>,
    ) -> Result<Self, &'static str> {
        rows.sort_by_key(|row| row.terminal_operation().get());
        if rows
            .windows(2)
            .any(|pair| pair[0].terminal_operation() == pair[1].terminal_operation())
        {
            return Err("boundary application demands repeat a Terminal operation");
        }
        Ok(Self { terminal_psi, rows })
    }

    pub fn validate_for_terminal(
        &self,
        terminal_psi: psi_terminal::TerminalPsiIdentity,
    ) -> Result<(), &'static str> {
        if self.terminal_psi != terminal_psi {
            return Err("boundary application demands belong to different Terminal semantics");
        }
        Ok(())
    }

    pub const fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity {
        self.terminal_psi
    }

    pub fn rows(&self) -> &[TerminalBoundaryApplicationDemand] {
        &self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundaryApplication, BoundaryApplicationArgument, BoundaryTypeIdentity};

    #[test]
    fn exact_application_requires_nonempty_contiguous_tagged_arguments() {
        assert!(BoundaryApplication::exact(Vec::new()).is_err());
        assert!(
            BoundaryApplication::exact(vec![BoundaryApplicationArgument::type_argument(
                1,
                BoundaryTypeIdentity::new("type".to_owned()).unwrap(),
            )])
            .is_err()
        );
        let application = BoundaryApplication::exact(vec![
            BoundaryApplicationArgument::type_argument(
                0,
                BoundaryTypeIdentity::new("type".to_owned()).unwrap(),
            ),
            BoundaryApplicationArgument::const_argument(
                1,
                BoundaryTypeIdentity::new("u64".to_owned()).unwrap(),
                "u64".to_owned(),
                "integer(u64,4)".to_owned(),
            )
            .unwrap(),
        ])
        .unwrap();
        assert_eq!(application.arguments().len(), 2);
    }
}
