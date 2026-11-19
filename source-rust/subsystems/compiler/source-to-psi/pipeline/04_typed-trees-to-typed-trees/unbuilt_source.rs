//! Source inspection for an explicit request that does not execute Build.
//! This returns frozen source for diagnostics and explicit checker tests; it
//! grants neither acceptance nor an executable product.
use crate::{SourcePreparationFailure, TypedFrontier};
use diagnostics::Diagnostic;

pub struct UnbuiltSourceInputs<'a> {
    pub root_bindings: &'a [declaration_contracts::selection::RootBinding],
    pub provider_selections: &'a [crate::provider_choice_inputs::ProviderSelection],
    pub targets: &'a [crate::source_targets::SourceTarget],
    pub subject: Option<semantic_vocabulary::PackageKeyIdentity>,
}

pub fn prepare_unbuilt_frontier(
    typed: crate::source_state::TypedTrees,
    accounting: crate::evaluation::SourceCompletionAccounting,
    input: UnbuiltSourceInputs<'_>,
) -> Result<TypedFrontier, SourcePreparationFailure> {
    let (layout_scopes, entries, bindings, subject) = (|| -> Result<_, Vec<Diagnostic>> {
        // Like the product check's first preparation: no target has chosen a
        // dependency default yet, so none is a layout root.
        let layout_scopes = crate::product_source::layout_target_scopes(
            &typed,
            input.root_bindings,
            input.provider_selections,
            &[],
            &[],
            input.targets,
        )?;
        let entries = crate::product_source::program_roots(input.root_bindings);
        let bindings = crate::entry_shapes::bound_program_entry_shapes(
            &typed,
            input.root_bindings,
            input.targets,
        )?;
        // The root of a stated graph is the subject. A source given no graph is
        // its own subject: the one package identity its authored product files
        // carry.
        let mut subject = input.subject;
        if input.subject.is_none()
            && let Some(sources) = typed.symbols.source_map()
        {
            for file in sources.files().filter(|file| {
                file.origin == source_admission::SourceOrigin::User
                    && file.dependency_scope
                        == source_provenance::DependencyScope::Product
            }) {
                match (subject, file.package_identity) {
                    (None, identity) => subject = identity,
                    (Some(known), Some(identity)) if known == identity => {}
                    _ => {
                        return Err(vec![Diagnostic::error(
                            "a program judged without its Build has authored sources of more than \
                         one package",
                        )]);
                    }
                }
            }
        }

        Ok((layout_scopes, entries, bindings, subject))
    })()
    .map_err(|diagnostics| SourcePreparationFailure {
        diagnostics,
        accounting: Default::default(),
    })?;
    // `complete_product_frontend` finishes Psi 03's authored bodies; it has
    // not specialized or frozen them. Run the shared Psi 04 preparation once,
    // exactly as the product path does, and keep the real result. Entries and
    // the subject belong to subsequent structural judgment, not a deleted
    // checker projection or an interrupted source-checking request.
    let prepared = crate::SourcePreparationRequest {
        program: typed,
        accounting,
        compilation: crate::TerminalSourceCompilation {
            calling: None,
            providers: None,

            declaration_metadata: &[],
            entries: &entries,
            subjects: &[subject],
            purpose: source_provenance::DependencyScope::Product,
            selected: &[],
            bindings: &bindings,
            supplies: &[],
        },
        admission: crate::SourceAdmission {
            invocation_sources: None,
            external_roots: &[],
            layout_scopes: &layout_scopes,
            layout_selection_authority: None,
            representations: crate::ProductRepresentationInput::default(),
            wire_demands: &[],
            declarations: None,
        },
    }
    .prepare()?;
    Ok(TypedFrontier {
        normalization: Default::default(),
        supplies: Vec::new(),
        typed: prepared.program.as_ref().clone(),
        source_completion_accounting: prepared.accounting,
        entries,
        subject,
        selected: Vec::new(),
        bindings,
    })
}
