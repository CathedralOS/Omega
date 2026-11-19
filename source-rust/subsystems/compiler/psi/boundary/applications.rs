use crate::terminal_module::TerminalIdentityIndex;
use crate::{CheckedMachineDeclaration, NominalDeclarationIdentity};
use semantic_vocabulary::OperationId;

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryApplicationCandidate {
    pub requirement: BoundaryOperatorRequirement,
    pub application: BoundaryApplication,
    pub selection_identity: TerminalIdentityIndex,
    pub selection_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub target: Option<String>,
    pub body: CheckedMachineDeclaration,
}

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

impl crate::TerminalModule {
    /// Select the demands of the given machines. An operation in dormant code
    /// still counts when checking that each demand has exactly one owner.
    pub fn boundary_application_demands_for_machines(
        &self,
        machines: &[semantic_vocabulary::MachineId],
    ) -> Result<Vec<TerminalBoundaryApplicationDemand>, &'static str> {
        if !machines.is_sorted() {
            return Err("boundary demand machine roster is not ordered");
        }
        if self.boundary_application_demands.windows(2).any(|rows| {
            rows[0].terminal_operation().get() >= rows[1].terminal_operation().get()
        }) {
            return Err("Terminal boundary demand catalog is not strictly ordered");
        }
        let catalog = &self.boundary_application_demands;
        if catalog.is_empty() {
            return Ok(Vec::new());
        }
        // Only operations named by a demand need an owner. Still visit every
        // machine so a duplicate in dormant code cannot satisfy the check.
        let mut owners = vec![None; catalog.len()];
        for machine in &self.machines {
            for operation in machine.operations() {
                if let Ok(position) = catalog
                    .binary_search_by_key(&operation.id.get(), |demand| {
                        demand.terminal_operation().get()
                    })
                    && owners[position].replace(machine.id).is_some()
                {
                    return Err("Terminal boundary demand has no unique operation owner");
                }
            }
        }
        let mut demands = Vec::new();
        for (demand, owner) in catalog.iter().zip(owners) {
            let owner = owner.ok_or("Terminal boundary demand has no operation owner")?;
            if machines.binary_search(&owner).is_ok() {
                demands.push(demand.clone());
            }
        }
        Ok(demands)
    }
}
