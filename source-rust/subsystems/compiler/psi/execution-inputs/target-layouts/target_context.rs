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
    /// Resolve the ordinary calling convention for the target and check each demanded body.
    /// A host supplies artifact-local machine identities. Register choices come from the body
    /// and the convention's own direct integer result bank.
    pub fn admit<'code>(
        artifact: impl Into<terminal_codec::TerminalExecutionView<'code>>,
        target: &TerminalTargetContext,
        machines: impl IntoIterator<Item = semantic_vocabulary::MachineId>,
    ) -> Result<Self, &'static str> {
        use language_core::native_call::{NativeCallingConvention, target_platform};
        use terminal_psi::{AssemblyExitConstraint, TerminalMachineResult};
        let artifact = artifact.into();
        if target.terminal_psi() != artifact.identity() {
            return Err("assembly calling plan belongs to another artifact");
        }
        let (architecture, format) = target_platform(target.target())
            .ok_or("assembly return requires a concrete native target")?;
        let native = NativeCallingConvention::for_target(architecture, format);
        let module = artifact.module();
        let mut results = Vec::new();
        for id in machines {
            let machine = module
                .machine(id)
                .ok_or("assembly calling plan names an absent machine")?;
            if !machine.structural_parameters.is_empty()
                || !machine.entry_claims.is_empty()
                || module
                    .interfaces
                    .provider_candidates
                    .iter()
                    .any(|row| row.candidate == id)
            {
                return Err(
                    "assembly return requires its selected boundary or structural calling plan",
                );
            }
            let TerminalMachineResult::Scalar(result) = &machine.result else {
                return Err("assembly register return requires a scalar result");
            };
            if !matches!(result.scalar_type, semantic_vocabulary::ScalarType::Integer(integer)
                if !integer.is_address() && integer.bits() == Some(32))
            {
                return Err("assembly register return requires a fixed 32-bit integer result");
            }
            let mut returns =
                machine
                    .blocks
                    .iter()
                    .filter_map(|block| match &block.assembly_exit {
                        Some(AssemblyExitConstraint::ScalarReturn(view)) => Some(*view),
                        _ => None,
                    });
            let view = returns
                .next()
                .ok_or("machine does not declare an assembly register return")?;
            if returns.any(|other| other != view)
                || !view.target().accepts_target_name(target.target())
                || Some(&view.parent()) != native.integer_result_registers().first()
            {
                return Err(
                    "authored assembly register does not hold this calling convention's scalar result",
                );
            }
            results.push((id, view));
        }
        results.sort_unstable_by_key(|(id, _)| *id);
        if results.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("duplicate machine in assembly return plans");
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
