//! Individual bindings for embedders running an explicitly instantiated PSI program.
use super::{
    BoundaryMachineId, HostContext, TerminalRuntimeError, TerminalRuntimeHostProvisions, bind,
};
impl<'host> HostContext<'host> {
    pub fn bind_instance(
        &self,
        instance: &mut terminal_interpreter::TerminalInstance<'host, '_>,
        provisions: Option<&TerminalRuntimeHostProvisions>,
        imports: &[BoundaryMachineId],
    ) -> Result<(), TerminalRuntimeError> {
        let program = instance.program().clone();
        bind(&program, instance, provisions, imports, self).map(|_| ())
    }

    pub fn prepare_component<'artifact>(
        &self,
        component: &'artifact published_runtime_input::RuntimeComponent,
        x86_flags: Option<terminal_interpreter::TerminalX86FlagsEnvironment>,
        memory_ordering: Option<terminal_interpreter::TerminalMemoryOrderingEnvironment>,
    ) -> Result<
        terminal_interpreter::TerminalComponentBinding<'host, 'artifact>,
        TerminalRuntimeError,
    > {
        crate::execution::components::prepare(component, self, x86_flags, memory_ordering)
    }

    pub fn output(&self) -> Result<(Vec<u8>, Vec<u8>), TerminalRuntimeError> {
        Ok((
            self.output.stdout.snapshot()?,
            self.output.stderr.snapshot()?,
        ))
    }
}
