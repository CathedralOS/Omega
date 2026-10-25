//! A checked component slot owns real producing instances across suspension.
//!
//! Imports are established before the receiver can bind this component. Their
//! affine roster moves into each invocation and comes back only after checked
//! return acknowledgement. There is no result-producing host callback, adapter
//! cloning, per-call configuration, or second execution of a suspended entry.

use evaluation_accounting::EvaluationInvocationAccount;
use std::collections::BTreeMap;

use semantic_vocabulary::{BoundaryMachineId, OperationId};
use terminal_fuel::TerminalFuelMeter;
use terminal_installation::AdmittedComponentProvider;

use super::{ComponentOwnedReturn, TerminalImportBinding, TerminalInstance};
use crate::program::TerminalProgram;
use crate::results::TerminalExecutionStatus;
use crate::{
    TerminalImportAdapter, TerminalRuntimeSupplies, TerminalScalarValue, TerminalStructuralInputs,
};

/// One immutable checked import/export selection and its retained import era.
/// Only completion of a real producing instance can create its owned return.
/// Imports bind once; subsequent invocations reuse their same affine roster.
pub struct TerminalComponentBinding<'host, 'artifact> {
    selected: AdmittedComponentProvider,
    program: TerminalProgram<'artifact>,
    supplies: TerminalRuntimeSupplies,
    required_imports: Vec<BoundaryMachineId>,
    bindings: BTreeMap<BoundaryMachineId, TerminalImportBinding<'host>>,
    components: ComponentBindings<'host, 'artifact>,
    pending: Option<PendingComponent<'host, 'artifact>>,
}

struct PendingComponent<'host, 'artifact> {
    operation: OperationId,
    arguments: Vec<TerminalScalarValue>,
    structural_operands: Vec<terminal_psi::StructuralArgument>,
    // Keep the original transferred atom tuple beside the real child through
    // suspension. Resumption never resolves the consumed receiver homes again.
    structural_arguments: Vec<crate::TerminalStructuralValue>,
    instance: Option<Box<TerminalInstance<'host, 'artifact>>>,
    completed: Option<ComponentOwnedReturn<'host, 'artifact>>,
}

pub(crate) type ComponentBindings<'host, 'artifact> =
    BTreeMap<BoundaryMachineId, TerminalComponentBinding<'host, 'artifact>>;

impl<'host, 'artifact> TerminalComponentBinding<'host, 'artifact> {
    pub fn new(
        selected: AdmittedComponentProvider,
        program: TerminalProgram<'artifact>,
        supplies: TerminalRuntimeSupplies,
    ) -> Result<Self, String> {
        if program.terminal_psi() != selected.provider()
            || supplies.terminal_psi() != selected.provider()
            || program
                .program
                .resolve_entry(Some(&supplies))
                .map_err(|error| format!("component entry admission: {error:?}"))?
                != selected.candidate()
        {
            return Err("component binding names another artifact or callable entry".into());
        }
        let requirements = program
            .execution_requirements(Some(&supplies))
            .map_err(|error| format!("component closure admission: {error:?}"))?;
        if !requirements.missing_providers.is_empty()
            || !requirements.external_completions.is_empty()
        {
            return Err(
                "component closure needs provider or per-instance external-completion authority"
                    .into(),
            );
        }
        Ok(Self {
            selected,
            program,
            supplies,
            required_imports: requirements.imports,
            bindings: BTreeMap::new(),
            components: BTreeMap::new(),
            pending: None,
        })
    }

    pub fn unbound_imports(&self) -> impl Iterator<Item = BoundaryMachineId> + '_ {
        self.required_imports.iter().copied().filter(|boundary| {
            !self.bindings.contains_key(boundary) && !self.components.contains_key(boundary)
        })
    }

    pub fn import(
        &self,
        boundary: BoundaryMachineId,
    ) -> Option<&terminal_psi::BoundaryMachineDeclaration> {
        if !self.required_imports.contains(&boundary) {
            return None;
        }
        self.program.program.boundary_machines().get(&boundary)
    }

    pub fn bind_import(
        &mut self,
        boundary: BoundaryMachineId,
        adapter: impl TerminalImportAdapter + 'host,
    ) -> Result<(), String> {
        let declaration = self
            .import(boundary)
            .ok_or("component slot is not demanded by this entry")?;
        if self.pending.is_some()
            || self.bindings.contains_key(&boundary)
            || self.components.contains_key(&boundary)
        {
            return Err("component import is active or already bound".into());
        }
        if !matches!(
            self.supplies.supplies.get(&boundary),
            None | Some(crate::effects::AdmittedBoundarySupply::HostImport)
        ) {
            return Err("a host adapter cannot replace a checked component provider".into());
        }
        let signature = crate::TerminalImportSignature::from(declaration);
        if adapter.implemented_signature() != signature {
            return Err("component adapter differs from the exact demanded signature".into());
        }
        self.bindings.insert(
            boundary,
            TerminalImportBinding {
                adapter: Box::new(adapter),
            },
        );
        Ok(())
    }

    pub fn bind_component(
        &mut self,
        binding: TerminalComponentBinding<'host, 'artifact>,
    ) -> Result<(), String> {
        let boundary = binding.selected.receiving_boundary();
        if self.pending.is_some()
            || !self.required_imports.contains(&boundary)
            || self.bindings.contains_key(&boundary)
            || self.components.contains_key(&boundary)
            || binding.selected.receiver() != self.program.terminal_psi()
            || self.supplies.supplies.get(&boundary) != Some(&binding.supply())
        {
            return Err("nested component differs from the exact unused receiving slot".into());
        }
        binding.require_ready()?;
        self.components.insert(boundary, binding);
        Ok(())
    }

    pub(crate) fn require_ready(&self) -> Result<(), String> {
        let missing = self.unbound_imports().collect::<Vec<_>>();
        if self.pending.is_some() || !missing.is_empty() {
            return Err(format!(
                "component closure is active or has unbound imports: {missing:?}"
            ));
        }
        for component in self.components.values() {
            component.require_ready()?;
        }
        Ok(())
    }

    pub(crate) fn supply(&self) -> crate::effects::AdmittedBoundarySupply {
        crate::effects::AdmittedBoundarySupply::ComponentProvider {
            provider: self.selected.provider(),
            provider_boundary: self.selected.provider_boundary(),
            candidate: self.selected.candidate(),
        }
    }
    pub(crate) fn selected(&self) -> &AdmittedComponentProvider {
        &self.selected
    }
    pub(crate) fn provider_program(&self) -> &TerminalProgram<'artifact> {
        &self.program
    }
    pub(super) const fn has_pending_invocation(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn pending_at(&self, operation: OperationId) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.operation == operation)
    }

    /// Prepare an actual child before the caller commits its staged owned
    /// input frontier. This performs admission but executes no operation; import
    /// custody moves only after instantiation succeeds.
    pub(crate) fn prepare(
        &mut self,
        operation: OperationId,
        arguments: &[TerminalScalarValue],
        structural_operands: &[terminal_psi::StructuralArgument],
        structural_arguments: &[crate::TerminalStructuralValue],
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<(), String> {
        self.require_ready()?;
        let mut instance = self
            .program
            .instantiate(crate::TerminalInstantiation {
                scalar_arguments: arguments,
                structural_inputs: TerminalStructuralInputs {
                    arguments: structural_arguments,
                    ..TerminalStructuralInputs::default()
                },
                installation: Some(&self.supplies),
                account,
            })
            .map_err(|error| format!("component instance admission: {error:?}"))?;
        if instance.required_imports != self.required_imports {
            return Err("component invocation changed its preflighted import closure".into());
        }
        instance.bindings = std::mem::take(&mut self.bindings);
        instance.components = std::mem::take(&mut self.components);
        self.pending = Some(PendingComponent {
            operation,
            arguments: arguments.to_vec(),
            structural_operands: structural_operands.to_vec(),
            structural_arguments: structural_arguments.to_vec(),
            instance: Some(Box::new(instance)),
            completed: None,
        });
        Ok(())
    }

    /// `None` means normal completion is retained, not yet acknowledged. A
    /// pause retains this child and its transferred input tuple/committed effects.
    pub(crate) fn invoke(
        &mut self,
        operation: OperationId,
        arguments: &[TerminalScalarValue],
        structural_operands: &[terminal_psi::StructuralArgument],
        meter: &mut TerminalFuelMeter,
    ) -> Result<Option<TerminalExecutionStatus>, String> {
        let pending = self
            .pending
            .as_mut()
            .ok_or("component invocation was not prepared")?;
        if pending.operation != operation
            || pending.arguments != arguments
            || pending.structural_operands != structural_operands
            || pending.structural_arguments.len() != structural_operands.len()
        {
            return Err(
                "pending component invocation has another operation or input snapshot".into(),
            );
        }
        if pending.completed.is_some() {
            return Ok(None);
        }
        let instance = pending
            .instance
            .as_mut()
            .ok_or("component invocation lost its live instance")?;
        match instance
            .invoke(meter)
            .map_err(|error| format!("component invocation: {error:?}"))?
        {
            TerminalExecutionStatus::Complete(_) => {
                let instance = *pending.instance.take().expect("borrowed above");
                match instance.into_component_return(&self.selected) {
                    Ok(completed) => pending.completed = Some(completed),
                    Err((instance, error)) => {
                        pending.instance = Some(Box::new(instance));
                        return Err(format!("component return custody: {error:?}"));
                    }
                }
                Ok(None)
            }
            status => Ok(Some(status)),
        }
    }

    pub(crate) fn completed(&self) -> Option<&ComponentOwnedReturn<'host, 'artifact>> {
        self.pending.as_ref()?.completed.as_ref()
    }

    /// After receiving commit, recover this same component era's import roster
    /// for its next fresh activation. The completed activation cannot run again.
    pub(crate) fn acknowledge(&mut self) -> Vec<crate::TerminalEffect> {
        let pending = self
            .pending
            .take()
            .expect("completed invocation was authenticated");
        let completed = pending
            .completed
            .expect("completed return was authenticated");
        let (value, mut owner) = completed.acknowledge();
        drop(value);
        let effects = owner
            .execution
            .as_mut()
            .map_or_else(Vec::new, |execution| std::mem::take(&mut execution.effects));
        self.bindings = std::mem::take(&mut owner.bindings);
        self.components = std::mem::take(&mut owner.components);
        effects
    }
}
