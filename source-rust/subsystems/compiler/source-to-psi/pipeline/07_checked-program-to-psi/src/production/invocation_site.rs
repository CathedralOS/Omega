//! Authenticate the generated caller, its source call and the emitted site before releasing the trees.
use crate::{AcceptedTerminalSource, ProducedTerminalProgram};
use semantic_vocabulary::{InvocationSite, MachineId};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::{ProductSourceIdentity, source_state};

/// Source facts captured for a generated invocation and its accepted lowering.
pub struct SourceInvocationSite<'a> {
    pub accepted: &'a AcceptedTerminalSource,
    pub request_source: &'a Arc<source_state::TypedTrees>,
    pub machine: symbols::SymbolHandle,
    pub requester: symbols::SymbolHandle,
    pub call: source_state::expression::ExpressionHandle,
}

#[derive(Debug, Clone)]
pub struct CheckedSourceCall {
    source: ProductSourceIdentity,
    request: std::sync::Weak<source_state::TypedTrees>,
    requester: SymbolHandle,
    machine: SymbolHandle,
    caller: SymbolHandle,
    call: source_state::expression::ExpressionHandle,
    emitted: Option<(MachineId, InvocationSite)>,
    template: Option<SymbolHandle>,
}

impl CheckedSourceCall {
    pub fn requesting_source(&self) -> &std::sync::Weak<source_state::TypedTrees> {
        &self.request
    }
    pub const fn requester(&self) -> SymbolHandle {
        self.requester
    }
    pub const fn caller(&self) -> SymbolHandle {
        self.caller
    }
    pub const fn machine(&self) -> SymbolHandle {
        self.machine
    }
    pub const fn machine_template(&self) -> Option<SymbolHandle> {
        self.template
    }
    pub const fn emitted_site(&self) -> Option<(MachineId, InvocationSite)> {
        self.emitted
    }

    pub fn require_site(&self, current: &SourceInvocationSite<'_>) -> Result<(), String> {
        if self.source.is_for(current.accepted.source())
            && self.request.ptr_eq(&Arc::downgrade(current.request_source))
            && self.requester == current.requester
            && self.machine == current.machine
            && self.call == current.call
        {
            Ok(())
        } else {
            Err("evaluation receipt belongs to another source invocation".into())
        }
    }
}

impl SourceInvocationSite<'_> {
    pub fn check(&self, program: &ProducedTerminalProgram) -> Result<CheckedSourceCall, String> {
        if self.request_source.identity.get() != self.accepted.source().identity.get() {
            return Err("source invocation belongs to a different requesting program".into());
        }
        let entry = self
            .accepted
            .entry()
            .ok_or("evaluation receipt lost its accepted entry")?;
        let caller = *self
            .accepted
            .closure()
            .elaboration()
            .machines
            .get(entry)
            .ok_or("evaluation receipt lost its actual caller")?;
        if !self.accepted.is_invocation()
            || self.accepted.source_records() != program.source_records()
            || &self.accepted.closure().elaboration().module != program.view().module()
        {
            return Err("evaluation receipt has another accepted code owner".into());
        }
        let mut sites = self.accepted.source_call_sites(caller, self.call);
        let call_site = sites.next();
        if sites.next().is_some() {
            return Err("evaluation receipt has multiple sites for its actual call".into());
        }
        self.check_call(program, caller, call_site)?;
        let template = self
            .accepted
            .source()
            .machine_instantiation(self.machine)?
            .map(|instantiation| instantiation.template());
        Ok(CheckedSourceCall {
            source: ProductSourceIdentity::new(self.accepted.source()),
            request: Arc::downgrade(self.request_source),
            requester: self.requester,
            machine: self.machine,
            caller,
            call: self.call,
            emitted: call_site,
            template,
        })
    }

    fn call_node(&self) -> Result<&source_state::expression::TableCallExpression, String> {
        if !self
            .accepted
            .source()
            .expression_table
            .expression_is_valid(self.call)
        {
            return Err("source invocation call has another source owner".into());
        }
        self.accepted
            .source()
            .expression_table
            .call_node(self.call)
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
        &self,
        program: &ProducedTerminalProgram,
        caller_symbol: SymbolHandle,
        call_site: Option<(MachineId, InvocationSite)>,
    ) -> Result<(), String> {
        let source = self.accepted.source();
        if source.machine_by_symbol(self.machine).is_none()
            || !source.symbols.symbols().nodes().is_valid(self.requester)
            || source.machine_by_symbol(caller_symbol).is_none()
            || source.symbols.get(caller_symbol).generated_from != self.requester
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
                source_state::statement::TransitionTargetNode::Value(value) if *value == self.call
            )
        {
            return Err("source invocation receipt names another call".into());
        }
        let call = self.call_node()?;
        let source_view = source_state::references::StaticArgumentCall::Expression(self.call, call);
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
        if let Some(invocation) = Self::invocation(program, call_site)? {
            let module = program.view().module();
            let body = module
                .machine(invocation.inputs.callee)
                .ok_or("source invocation lost its actual executable body")?;
            let declared = source
                .machine_by_symbol(self.machine)
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
                        return Err(
                            "direct source invocation was replaced by a selected call".into()
                        );
                    }
                }
                source_state::callable_signature_shapes::CallableDeclaration::Signature(
                    required,
                ) => {
                    let contract = invocation
                        .inputs
                        .required_contract
                        .and_then(|required| {
                            module.invocation_machine_contract(
                                invocation.inputs.callee,
                                Some(required),
                            )
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
                            "selected source invocation changed its exact public application"
                                .into(),
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
            if call.target_symbol != self.machine
                && source
                    .machine_holding_state(call.target_symbol)
                    .is_none_or(|machine| machine.symbol != self.machine)
            {
                return Err("source description call selects another declaration".into());
            }
        }
        Ok(())
    }
}
