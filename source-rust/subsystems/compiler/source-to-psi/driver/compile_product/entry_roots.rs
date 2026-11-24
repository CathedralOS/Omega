//! The declarations a Product nominates and the entry they become after preparation.
use crate::RootBinding;
use diagnostics::Diagnostic;
use std::borrow::Cow;
use typed_trees::TypedTrees;
use typed_trees_to_prepared_typed_trees::{EntryApplication, PreparedTypedTrees};

pub(crate) enum ProductRoots {
    Direct(Vec<RootBinding>),
    Applied {
        test: RootBinding,
        application: EntryApplication,
    },
}

impl ProductRoots {
    /// An applied entry has no executable declaration until stage 04 closes it.
    pub(super) fn direct(&self) -> &[RootBinding] {
        match self {
            Self::Direct(bindings) => bindings,
            Self::Applied { .. } => &[],
        }
    }

    /// Both authored bodies can demand target layouts before specialization.
    pub(super) fn layout_origins<'a>(
        &'a self,
        typed: &TypedTrees,
    ) -> Result<Cow<'a, [RootBinding]>, Vec<Diagnostic>> {
        match self {
            Self::Direct(bindings) => Ok(Cow::Borrowed(bindings)),
            Self::Applied { test, application } => {
                let handle = application.in_source(typed).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "test adapter application does not belong to this Product's typed source",
                    )]
                })?;
                let adapter = typed
                    .expression_table
                    .static_machine_application(handle)
                    .registration_operation;
                Ok(Cow::Owned(vec![
                    test.clone(),
                    binding_for(typed, &test.slot, adapter),
                ]))
            }
        }
    }

    pub(super) fn applications(&self) -> &[EntryApplication] {
        match self {
            Self::Direct(_) => &[],
            Self::Applied { application, .. } => std::slice::from_ref(application),
        }
    }

    pub(super) fn specialized(
        &self,
        prepared: &PreparedTypedTrees,
    ) -> Result<Option<RootBinding>, Vec<Diagnostic>> {
        let Self::Applied { test, application } = self else {
            return Ok(None);
        };
        let entry = prepared
            .entry_for_application(*application)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "source preparation lost the selected test adapter specialization",
                )]
            })?;
        Ok(Some(binding_for(prepared.trees(), &test.slot, entry)))
    }
}

fn binding_for(
    typed: &TypedTrees,
    slot: &str,
    implementation: symbols::SymbolHandle,
) -> RootBinding {
    RootBinding {
        slot: slot.to_owned(),
        implementation: typed.symbols.display_path(implementation, "::"),
        implementation_symbol: implementation,
        source_span: typed.symbols.symbol_source_span(implementation),
    }
}

/// Attribute a normal return to the selected satisfier only when the checked
/// entry invokes that exact machine on every path that can return normally.
pub(super) fn check_test_invocation(
    publication: &crate::driver::judge_lowered_source::SourcePublication,
    test: &RootBinding,
    application: EntryApplication,
    target: &str,
) -> Result<(), Vec<Diagnostic>> {
    use terminal_semantics::control_graph::{InvocationOnReturn, invocation_on_return};
    let accepted = publication.accepted()?;
    let typed = accepted.source();
    let handle = application.in_source(typed).ok_or_else(|| {
        vec![Diagnostic::error(
            "accepted test adapter application lost its source",
        )]
    })?;
    let adapter = typed
        .expression_table
        .static_machine_application(handle)
        .registration_operation;
    let refuse = |why| {
        let mut error = Diagnostic::error(format!(
            "test adapter `{}` {why} selected test `{}`",
            typed.symbols.display_path(adapter, "::"),
            test.implementation,
        ));
        if let Some(span) = typed.symbols.symbol_source_span(adapter) {
            error = error.with_source_span(span);
        }
        vec![error]
    };
    let selected = accepted
        .machine_of(test.implementation_symbol)
        .ok_or_else(|| refuse("does not invoke"))?;
    let module = accepted.module();
    let entry = module
        .program_entry_contract(target)
        .map_err(|why| vec![Diagnostic::error(why)])?
        .entry;
    match invocation_on_return(module, entry, selected).map_err(|why| {
        vec![Diagnostic::error(format!(
            "cannot verify the checked control flow of the test adapter: {why:?}"
        ))]
    })? {
        InvocationOnReturn::Required => Ok(()),
        InvocationOnReturn::Bypassed => Err(refuse("can return without invoking")),
        InvocationOnReturn::Unreached => Err(refuse("does not invoke")),
    }
}
