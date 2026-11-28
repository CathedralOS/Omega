//! The source call and executable authenticated together by the Psi publisher.
#![forbid(unsafe_code)]

use semantic_vocabulary::{InvocationSite, MachineId};
use source_invocation_issuer::PublicationIssuer;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::{ProductSourceIdentity, source_state};

/// Source facts captured for a generated invocation and its accepted lowering.
pub struct SourceInvocationSite<'a> {
    pub source: &'a Arc<source_state::FrozenTypedTrees>,
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
        if self.source.is_for(current.source)
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

/// The publisher lends the exact execution view its source records describe.
#[derive(Clone, Copy)]
pub struct PublishedInvocationProgram<'a>(terminal_codec::TerminalExecutionView<'a>);

impl<'a> PublishedInvocationProgram<'a> {
    pub fn new(_: PublicationIssuer, program: terminal_codec::TerminalExecutionView<'a>) -> Self {
        Self(program)
    }

    pub fn view(self) -> terminal_codec::TerminalExecutionView<'a> {
        self.0
    }
}

/// Keep the checked call paired with its executable until accounting verifies admission.
pub struct PublishedSourceInvocation<'a> {
    call: CheckedSourceCall,
    program: PublishedInvocationProgram<'a>,
}

impl<'a> PublishedSourceInvocation<'a> {
    pub fn into_parts(self) -> (CheckedSourceCall, PublishedInvocationProgram<'a>) {
        (self.call, self.program)
    }
}

impl SourceInvocationSite<'_> {
    pub fn issue<'a>(
        &self,
        _: PublicationIssuer,
        program: PublishedInvocationProgram<'a>,
        caller: SymbolHandle,
        call_site: Option<(MachineId, InvocationSite)>,
        template: Option<SymbolHandle>,
    ) -> PublishedSourceInvocation<'a> {
        let call = CheckedSourceCall {
            source: ProductSourceIdentity::new(self.source),
            request: Arc::downgrade(self.request_source),
            requester: self.requester,
            machine: self.machine,
            caller,
            call: self.call,
            emitted: call_site,
            template,
        };
        PublishedSourceInvocation { call, program }
    }
}
