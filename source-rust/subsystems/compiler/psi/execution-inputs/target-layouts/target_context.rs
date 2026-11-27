use terminal_psi::{TargetMachineCallable, TerminalPsiIdentity};

/// Bind target-dependent observations to one code artifact. This context
/// does not choose a source body or authorize any boundary provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetContext {
    terminal_psi: TerminalPsiIdentity,
    target: String,
}

/// Scalar result registers supplied by the calling plans of this artifact's assembly machines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyReturnPlans {
    target: TerminalTargetContext,
    results: std::sync::Arc<
        [(
            semantic_vocabulary::MachineId,
            terminal_psi::AsmRegisterView,
        )],
    >,
}

impl AssemblyReturnPlans {
    pub fn new(
        target: &TerminalTargetContext,
        mut results: Vec<(
            semantic_vocabulary::MachineId,
            terminal_psi::AsmRegisterView,
        )>,
    ) -> Result<Self, &'static str> {
        results.sort_unstable_by_key(|(machine, _)| *machine);
        if results.windows(2).any(|pair| pair[0].0 == pair[1].0)
            || results
                .iter()
                .any(|(_, view)| !view.target().accepts_target_name(target.target()))
        {
            return Err("assembly return plans repeat a machine or name another architecture");
        }
        Ok(Self {
            target: target.clone(),
            results: results.into(),
        })
    }

    pub fn target(&self) -> &TerminalTargetContext {
        &self.target
    }

    pub fn admits(
        &self,
        machine: semantic_vocabulary::MachineId,
        view: terminal_psi::AsmRegisterView,
    ) -> bool {
        self.results
            .binary_search_by_key(&machine, |(machine, _)| *machine)
            .is_ok_and(|index| self.results[index].1 == view)
    }
}

impl TerminalTargetContext {
    pub fn resolve_entry(
        selected: Option<&Self>,
        identity: TerminalPsiIdentity,
        module: &terminal_psi::TerminalModule,
        root: TargetMachineCallable,
    ) -> Result<semantic_vocabulary::MachineId, TerminalEntryError> {
        if selected.is_some_and(|selected| selected.terminal_psi != identity) {
            return Err(TerminalEntryError::TerminalIdentityMismatch);
        }
        let TargetMachineCallable::Machine(machine) = root else {
            return Err(TerminalEntryError::NonExecutableEntry(root));
        };
        if module.machine(machine).is_none() {
            return Err(TerminalEntryError::MissingMachine(machine));
        }
        Ok(machine)
    }

    pub fn entry(
        &self,
        identity: TerminalPsiIdentity,
        module: &terminal_psi::TerminalModule,
        root: TargetMachineCallable,
    ) -> Result<semantic_vocabulary::MachineId, TerminalEntryError> {
        Self::resolve_entry(Some(self), identity, module, root)
    }

    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub fn target(&self) -> &str {
        &self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEntryError {
    TerminalIdentityMismatch,
    MissingMachine(semantic_vocabulary::MachineId),
    NonExecutableEntry(TargetMachineCallable),
}

impl std::fmt::Display for TerminalEntryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingMachine(machine) => {
                write!(formatter, "entry machine {machine:?} is absent")
            }
            Self::TerminalIdentityMismatch => {
                formatter.write_str("entry context names a different Psi artifact")
            }
            Self::NonExecutableEntry(root) => {
                write!(formatter, "entry {root:?} is not an executable machine")
            }
        }
    }
}

impl std::error::Error for TerminalEntryError {}

pub fn terminal_target_context<'code>(
    artifact: impl Into<terminal_codec::TerminalExecutionView<'code>>,
    target: &str,
) -> TerminalTargetContext {
    let artifact = artifact.into();
    TerminalTargetContext {
        terminal_psi: artifact.identity(),
        target: target.to_owned(),
    }
}
