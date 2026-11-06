//! Checked callable descriptions survive independently of executable selection.
//!
//! Pruning may remove a body without removing a condition owned by its
//! declaration. These rows retain the same signature and commitments used for
//! emitted declarations; retaining a row never requests that body's execution.

use arena::{Arena, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::type_identity::{CallableIdentityId, TypeIdentityId};

use super::MachineContractCommitment;

/// Independently decidable callable facts, even when its full description is
/// unavailable. These describe source supply, never executable installation.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCallableHeader {
    pub machine: SymbolHandle,
    pub callable: CallableIdentityId,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub source_target: Option<String>,
    pub product_eligible: bool,
    pub supplies_checked_body: bool,
    /// Remaining source binders, independent of emitted closed specializations.
    pub type_parameter_count: usize,
    pub compiler_evaluation:
        Result<semantic_vocabulary::compiler_evaluation::CompilerEvaluationDeclaration, String>,
    /// The checked realized calls, not the callable's public permission ceiling.
    pub synchronous_invocations: Result<HandleSpan<CheckedSynchronousBoundaryTarget>, String>,
}

impl Default for CheckedCallableHeader {
    fn default() -> Self {
        Self {
            machine: SymbolHandle::default(),
            callable: CallableIdentityId::default(),
            package: None,
            source_target: None,
            product_eligible: false,
            supplies_checked_body: false,
            type_parameter_count: 0,
            compiler_evaluation: Err("checked compiler-evaluation facts are absent".into()),
            synchronous_invocations: Err("checked synchronous invocation facts are absent".into()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedSynchronousBoundaryTarget {
    pub declaration: String,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// None is an explicitly abstract edge, never an inferred application.
    pub application: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedDeclarationParameter {
    pub normalized: TypeIdentityId,
    pub qualified: TypeIdentityId,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedDeclarationCapture {
    pub template_identity: TypeIdentityId,
    pub template_contract: [u8; 32],
    pub specialization: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedDeclarationSpecialization {
    pub template: SymbolHandle,
    pub callable: CallableIdentityId,
    pub commitment: [u8; 32],
    pub capture: Option<CheckedDeclarationCapture>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedMachineDescription {
    pub machine: SymbolHandle,
    pub entry: SymbolHandle,
    pub parameters: HandleSpan<CheckedDeclarationParameter>,
    pub result: TypeIdentityId,
    pub callable: CallableIdentityId,
    pub contract: MachineContractCommitment,
    pub specialization: Option<CheckedDeclarationSpecialization>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedDeclarationDescription {
    #[default]
    Missing,
    Machine(CheckedMachineDescription),
    /// A symbolic or otherwise unpublishable description must not make an
    /// unrelated product fail. Its exact refusal is reported when demanded.
    Unavailable {
        machine: SymbolHandle,
        reason: String,
    },
}

impl CheckedDeclarationDescription {
    pub fn machine(&self) -> SymbolHandle {
        match self {
            Self::Missing => SymbolHandle::default(),
            Self::Machine(description) => description.machine,
            Self::Unavailable { machine, .. } => *machine,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedDeclarationDescriptions {
    pub selections: Arena<CheckedSelectionRequirement>,
    pub evaluation_dependencies: CheckedCompilerEvaluationDependencies,
    pub headers: Arena<CheckedCallableHeader>,
    pub invocation_targets: Arena<CheckedSynchronousBoundaryTarget>,
    pub machines: Arena<CheckedDeclarationDescription>,
    pub parameters: Arena<CheckedDeclarationParameter>,
    /// `machines` sorted by machine symbol, published by the producer once
    /// every row is in place (`index_machines`). Ties keep arena order.
    /// Each lowered declaration asks for its own description; scanning every
    /// machine row per question made lowering quadratic in the machine count.
    pub machine_index: Vec<((u32, u32), arena::Handle<CheckedDeclarationDescription>)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedSelectionRequirement {
    #[default]
    Missing,
    Selection {
        occurrence: typed_trees::AuthoredDeclarationSelectionOccurrenceId,
        source_span: source::SourceSpan,
        excluded: bool,
        request: Result<
            Option<semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest>,
            String,
        >,
    },
}

impl CheckedSelectionRequirement {
    pub fn occurrence_ordinal(&self) -> u64 {
        match self {
            Self::Missing => 0,
            Self::Selection { occurrence, .. } => occurrence.ordinal(),
        }
    }
}

impl Default for CheckedDeclarationDescriptions {
    fn default() -> Self {
        Self {
            selections: Arena::new(),
            evaluation_dependencies: CheckedCompilerEvaluationDependencies::default(),
            headers: Arena::new(),
            invocation_targets: Arena::new(),
            machines: Arena::new(),
            parameters: Arena::new(),
            machine_index: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCompilerEvaluationDependencies {
    pub recursive_components: Arena<CheckedCompilerEvaluationComponent>,
    pub recursive_members: Arena<super::CheckedRuntimeRankMember>,
    pub recursive_calls: Arena<CheckedCompilerEvaluationCall>,
    pub pending_conditions: Arena<SymbolHandle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCompilerEvaluationComponent {
    pub members: HandleSpan<super::CheckedRuntimeRankMember>,
    pub calls: HandleSpan<CheckedCompilerEvaluationCall>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCompilerEvaluationCall {
    pub caller: SymbolHandle,
    pub call_ordinal: u32,
    pub callee: SymbolHandle,
}

impl CheckedDeclarationDescriptions {
    pub fn description(&self, machine: SymbolHandle) -> Option<&CheckedDeclarationDescription> {
        if !machine.is_valid() {
            return None;
        }
        // The published index covers every row only when no row was added
        // after it; otherwise read the rows themselves.
        if self.machine_index.len() != self.machines.len() {
            return self
                .machines
                .iter()
                .find_map(|(_, row)| (row.machine() == machine).then_some(row));
        }
        let key = symbol_key(machine);
        let start = self.machine_index.partition_point(|(row, _)| *row < key);
        self.machine_index[start..]
            .iter()
            .take_while(|(row, _)| *row == key)
            .map(|(_, handle)| self.machines.get(*handle))
            .find(|row| row.machine() == machine)
    }

    /// Publish `machine_index` over the finished `machines` rows.
    pub fn index_machines(&mut self) {
        let mut index = self
            .machines
            .iter()
            .enumerate()
            .map(|(position, (handle, row))| (symbol_key(row.machine()), position, handle))
            .collect::<Vec<_>>();
        index.sort_unstable_by_key(|(key, position, _)| (*key, *position));
        self.machine_index = index
            .into_iter()
            .map(|(key, _, handle)| (key, handle))
            .collect();
    }

    pub fn for_machine(&self, machine: SymbolHandle) -> Result<&CheckedMachineDescription, &str> {
        match self.description(machine) {
            Some(CheckedDeclarationDescription::Machine(description)) => Ok(description),
            Some(CheckedDeclarationDescription::Unavailable { reason, .. }) => Err(reason),
            _ => Err("checked machine lost its retained declaration description"),
        }
    }
}

fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}
