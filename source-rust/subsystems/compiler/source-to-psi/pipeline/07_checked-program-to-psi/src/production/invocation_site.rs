//! Authenticate the generated caller and its emitted site before issuing a receipt input.
use crate::{AcceptedTerminalSource, ProducedTerminalProgram};
use published_source_invocation::{
    PublishedInvocationProgram, PublishedSourceInvocation, SourceInvocationSite,
};
use semantic_vocabulary::{InvocationSite, MachineId};
use source_invocation_issuer::PUBLICATION;
use symbols::SymbolHandle;
use typed_trees::source_state;

impl ProducedTerminalProgram {
    pub fn invocation_program(&self) -> PublishedInvocationProgram<'_> {
        PublishedInvocationProgram::new(PUBLICATION, self.view())
    }
}

impl AcceptedTerminalSource {
    pub fn check_invocation<'a>(
        &self,
        site: &SourceInvocationSite<'_>,
        program: &'a ProducedTerminalProgram,
    ) -> Result<PublishedSourceInvocation<'a>, String> {
        if !std::sync::Arc::ptr_eq(site.source, self.source()) {
            return Err("evaluation receipt has another accepted code owner".into());
        }
        if site.request_source.identity.get() != self.source().identity.get() {
            return Err("source invocation belongs to a different requesting program".into());
        }
        let entry = self
            .entry()
            .ok_or("evaluation receipt lost its accepted entry")?;
        let caller = *self
            .closure()
            .elaboration()
            .machines
            .get(entry)
            .ok_or("evaluation receipt lost its actual caller")?;
        if !self.is_invocation()
            || self.source_records() != program.source_records()
            || &self.closure().elaboration().module != program.view().module()
        {
            return Err("evaluation receipt has another accepted code owner".into());
        }
        let mut sites = self.source_call_sites(caller, site.call);
        let call_site = sites.next();
        if sites.next().is_some() {
            return Err("evaluation receipt has multiple sites for its actual call".into());
        }
        check_call(site, program, caller, call_site)?;
        let template = self
            .source()
            .machine_instantiation(site.machine)?
            .map(|instantiation| instantiation.template());
        Ok(site.issue(
            PUBLICATION,
            program.invocation_program(),
            caller,
            call_site,
            template,
        ))
    }
}

fn call_node<'a>(
    site: &'a SourceInvocationSite<'_>,
) -> Result<&'a source_state::expression::TableCallExpression, String> {
    if !site.source.expression_table.expression_is_valid(site.call) {
        return Err("source invocation call has another source owner".into());
    }
    site.source
        .expression_table
        .call_node(site.call)
        .ok_or_else(|| "source invocation lost its retained call".into())
}
fn invocation<'program>(
    program: &'program ProducedTerminalProgram,
    call_site: Option<(MachineId, InvocationSite)>,
) -> Result<Option<terminal_psi::ModuleInvocation<'program>>, String> {
    let Some((caller, site)) = call_site else {
        return Ok(None);
    };
    let module = program.view().module();
    if module.entry != Some(semantic_vocabulary::TargetMachineCallable::Machine(caller)) {
        return Err("source invocation receipt names another canonical caller".into());
    }
    let invocation = module
        .machine(caller)
        .and_then(|machine| {
            machine
                .module_invocations()
                .find(|invocation| invocation.site == site)
        })
        .ok_or("source invocation receipt lost its exact emitted call site")?;
    Ok(Some(invocation))
}

fn check_call(
    site: &SourceInvocationSite<'_>,
    program: &ProducedTerminalProgram,
    caller_symbol: SymbolHandle,
    call_site: Option<(MachineId, InvocationSite)>,
) -> Result<(), String> {
    let source = site.source;
    if source.machine_by_symbol(site.machine).is_none()
        || !source.symbols.symbols().nodes().is_valid(site.requester)
        || source.machine_by_symbol(caller_symbol).is_none()
        || source.symbols.get(caller_symbol).generated_from != site.requester
        || program
            .view()
            .module()
            .identities
            .invocation_scope()
            .is_none()
        || program.evidence().is_none()
    {
        return Err("source invocation lost its accepted source/code custody".into());
    }
    let caller = source
        .machine_by_symbol(caller_symbol)
        .ok_or("source invocation lost its caller declaration")?;
    let Some([state]) = source.machine_states(caller).as_array() else {
        return Err("source invocation lost its unique caller entry".into());
    };
    let Some(source_state::statement::StatementNode::Transition(transition)) = source
        .statement_table
        .statements(state.statement_nodes)
        .last()
    else {
        return Err("source invocation lost its actual return call".into());
    };
    if !source
        .statement_table
        .transition_target_is_valid(transition.target)
        || !matches!(
            source.statement_table.transition_target(transition.target),
            source_state::statement::TransitionTargetNode::Value(value) if *value == site.call
        )
    {
        return Err("source invocation receipt names another call".into());
    }
    let call = call_node(site)?;
    let source_view = source_state::references::StaticArgumentCall::Expression(site.call, call);
    if source_view.applications().any(|application| {
        !source
            .expression_table
            .static_machine_application_is_valid(application)
    }) {
        return Err("source invocation has a stale application in its retained call".into());
    }
    let lexical_owner = source_view.lexical_owner(source);
    let lexical_owner = if lexical_owner.is_valid() {
        lexical_owner
    } else {
        caller_symbol
    };
    if let Some(invocation) = invocation(program, call_site)? {
        let module = program.view().module();
        let body = module
            .machine(invocation.inputs.callee)
            .ok_or("source invocation lost its actual executable body")?;
        let declared = source
            .machine_by_symbol(site.machine)
            .ok_or("source invocation lost its executable source declaration")?;
        let expected = source
            .machine_callable_identity_text(declared)
            .filter(|identity| !identity.is_empty())
            .ok_or("source invocation body has no complete source callable identity")?;
        if body
            .declaration
            .as_ref()
            .and_then(|declaration| module.identities.text(declaration.callable))
            != Some(expected.as_str())
        {
            return Err("source invocation site names another executable declaration".into());
        }
        match source.source_call_declaration(lexical_owner, source_view)? {
            source_state::callable_signature_shapes::CallableDeclaration::Machine(_) => {
                if invocation.inputs.required_contract.is_some() {
                    return Err("direct source invocation was replaced by a selected call".into());
                }
            }
            source_state::callable_signature_shapes::CallableDeclaration::Signature(required) => {
                let contract = invocation
                    .inputs
                    .required_contract
                    .and_then(|required| {
                        module.invocation_machine_contract(invocation.inputs.callee, Some(required))
                    })
                    .ok_or("selected source invocation lost its actual public contract")?;
                let (owner, _) = source
                    .trait_machine_signature(required.symbol)
                    .ok_or("selected source invocation lost its public source declaration")?;
                let expected = source.trait_requirement_callable_identity_text(owner, required);
                if contract
                    .declaration
                    .as_ref()
                    .and_then(|declaration| module.identities.text(declaration.callable))
                    != Some(expected.as_str())
                    || contract.selected_application.is_none()
                {
                    return Err(
                        "selected source invocation changed its exact public application".into(),
                    );
                }
            }
        }
    } else {
        if source_state::reflection::construction::declaration_query_argument_role(
            source,
            source_view,
        ) != source_state::references::StaticArgumentRole::DeclarationDescription
        {
            return Err("source invocation lost its actual emitted call association".into());
        }
        if call.target_symbol != site.machine
            && source
                .machine_holding_state(call.target_symbol)
                .is_none_or(|machine| machine.symbol != site.machine)
        {
            return Err("source description call selects another declaration".into());
        }
    }
    Ok(())
}
