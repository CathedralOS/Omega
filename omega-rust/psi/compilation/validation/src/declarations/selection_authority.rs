//! Package authority for authored declaration selections.
//!
//! Checking normalizes exact finalized occurrence custody and exclusions once.
//! The caller supplies reconciled live dependency authority for those requests.
//! Loading a transitive package does not make its declarations selectable.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use semantic_vocabulary::PackageKeyIdentity;
use source::SourceOrigin;
use symbols::FixedCoreDeclaration;

/// Package-neutral authority consulted before the compiler executes an
/// authored machine or admits checked declaration selections.
///
/// Psi owns provenance and call-closure discovery. Omega supplies the exact
/// reconciled direct-dependency predicate without leaking resolver or lockfile
/// structures into the language layer.
pub use semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority;

/// Validate exact checked declaration selections under supplied package authority.
/// The producer retained exact Product exclusions and normalized requests once.
/// This asks the live authority; loaded source or a stored scope grants nothing.
pub fn validate_authored_declaration_selections(
    program: &CheckedTrees,
    packages: &dyn DeclarationSelectionAuthority,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    if program
        .facts
        .authored_declaration_selections()
        .into_iter()
        .len()
        != program.facts.declaration_descriptions.selections.len()
    {
        return Err(vec![Diagnostic::error(
            "checked declaration selections lost their normalized authority obligations",
        )]);
    }
    for (_, selection) in program.facts.declaration_descriptions.selections.iter() {
        let checked_trees::CheckedSelectionRequirement::Selection {
            source_span,
            excluded,
            request,
            ..
        } = selection
        else {
            return Err(vec![Diagnostic::error(
                "checked declaration-selection requirement is absent",
            )]);
        };
        if *excluded {
            continue;
        }
        match request {
            Ok(Some(request))
                if !packages.allows_declaration_selection(request.requester, request.owner) =>
            {
                diagnostics.push(Diagnostic::error(format!(
                    "package {} selects a declaration owned by package {} without declaring that package as a direct dependency",
                    packages.package_label(request.requester), packages.package_label(request.owner),
                )).with_source_span(*source_span));
            }
            Err(reason) => {
                diagnostics.push(Diagnostic::error(reason.clone()).with_source_span(*source_span))
            }
            _ => {}
        }
    }
    crate::program_validation::finish_strict_diagnostics(diagnostics)
}

/// Normalize finalized occurrence custody once while checking owns the exact
/// declarations. The resulting package pairs are obligations, not authority;
/// every execution consumer still asks its live predicate. Excluded Product
/// descriptors remain explicitly uncovered rather than silently authorized.
pub fn checked_selection_requirements(
    program: &CheckedTrees,
) -> arena::Arena<checked_trees::CheckedSelectionRequirement> {
    let skipped =
        crate::queries::execution_projection::descriptor_selection_occurrences(&program.typed);
    let mut result = Vec::new();
    for selection in program.facts.authored_declaration_selections() {
        let selected_is_pruned = match selection.target() {
            AuthoredDeclarationSelectionTarget::Resolved(selected) => {
                let selected = program
                    .closed_requirement_application(selected.selected_symbol())
                    .map_or(selected.selected_symbol(), |application| {
                        application.template
                    });
                !typed_trees::visibility::owning_machine_is_retained(
                    program.product_declarations(),
                    selected,
                )
            }
            _ => false,
        };
        let excluded = skipped.contains(&selection.occurrence_id()) || selected_is_pruned;
        result.push(checked_trees::CheckedSelectionRequirement::Selection {
            occurrence: selection.occurrence_id(),
            source_span: selection.source_span(),
            excluded,
            request: normalize_checked_selection(program, *selection),
        });
    }
    result.sort_by_key(checked_trees::CheckedSelectionRequirement::occurrence_ordinal);
    let mut retained = arena::Arena::new();
    retained.insert_many(result);
    retained
}

fn normalize_checked_selection(
    program: &CheckedTrees,
    selection: typed_trees::AuthoredDeclarationSelection,
) -> Result<Option<semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest>, String> {
    let source_span = selection.source_span();
    let source_file = program
        .symbols
        .source_file(source_span)
        .ok_or("authored declaration selection has no compiler-owned source custody")?;
    if source_file.origin == SourceOrigin::Toolchain {
        return Ok(None);
    }
    let requester = source_file.package_identity.ok_or_else(|| {
        format!(
            "authored declaration selection in {} has no reconciled requesting package identity",
            source_file.path.display()
        )
    })?;
    let selected = match selection.target() {
        AuthoredDeclarationSelectionTarget::Intrinsic(_) => return Ok(None),
        AuthoredDeclarationSelectionTarget::LateBound(binding) => {
            return Err(format!(
                "package-authored declaration selection remained unresolved after successful checking: {:?} ({binding:?})",
                selection.kind()
            ));
        }
        AuthoredDeclarationSelectionTarget::Resolved(selected) => selected.selected_symbol(),
    };
    let selected = program
        .closed_requirement_application(selected)
        .map_or(selected, |application| application.template);
    if program.symbols.builtin_type_atom(selected).is_some()
        || program
            .symbols
            .builtin_function_for_symbol(selected)
            .is_some()
        || program.symbols.builtin_type_member(selected).is_some()
        || FixedCoreDeclaration::ALL.into_iter().any(|declaration| {
            program.symbols.fixed_core_declaration_symbol(declaration) == Some(selected)
        })
    {
        return Ok(None);
    }
    let selected_kind = program.symbols.get(selected).kind;
    if typed_trees::visibility::requires_declaration_visibility(selected_kind) {
        let visibility = typed_trees::visibility::authored_selection_visibility(
            program.product_declarations(),
            selected,
        )
        .ok_or_else(|| {
            format!(
                "selected {:?} `{}` has no retained declaration visibility",
                selected_kind,
                program.symbols.display_path(selected, "::")
            )
        })?;
        if !visibility.is_public()
            && program.symbols.symbol_package_identity(selected) != Some(requester)
        {
            return Err(format!(
                "package {requester:?} selects private {} `{}`",
                visibility.kind(),
                program.symbols.display_path(selected, "::")
            ));
        }
    }
    if let Some(owner) = program.symbols.symbol_package_identity(selected) {
        return Ok(Some(
            semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest {
                requester,
                owner,
            },
        ));
    }
    match program.symbols.symbol_source_origin(selected) {
        Some(SourceOrigin::Toolchain) => Ok(None),
        Some(SourceOrigin::User) => Err(format!(
            "selected user declaration `{}` has no reconciled owning package identity",
            program.symbols.display_path(selected, "::")
        )),
        None => Err(format!(
            "selected declaration `{}` has no package or toolchain provenance",
            program.symbols.display_path(selected, "::")
        )),
    }
}

/// Comparison data from a source checkpoint. It deliberately implements no
/// authority trait: decoded scope can reject reuse, never authorize execution.
#[derive(Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum RecordedDeclarationAuthority {
    Absent,
    Packages(std::collections::BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>),
}
impl RecordedDeclarationAuthority {
    pub fn capture(
        authority: Option<&dyn DeclarationSelectionAuthority>,
    ) -> Result<Self, &'static str> {
        let Some(authority) = authority else {
            return Ok(Self::Absent);
        };
        let mut scope = authority
            .declaration_selection_scope()
            .ok_or("declaration selection predicate does not export a complete comparison scope")?;
        for owners in scope.values_mut() {
            owners.sort_unstable();
            owners.dedup();
        }
        Ok(Self::Packages(scope))
    }
    pub fn has_authority(&self) -> bool {
        matches!(self, Self::Packages(_))
    }
    pub fn allows(&self, requester: PackageKeyIdentity, owner: PackageKeyIdentity) -> Option<bool> {
        match self {
            Self::Absent => None,
            Self::Packages(scope) => scope.get(&requester).map(|owners| owners.contains(&owner)),
        }
    }
    pub fn require_current(
        &self,
        authority: Option<&dyn DeclarationSelectionAuthority>,
        sources: &source::SourceMap,
    ) -> Result<(), &'static str> {
        self.require_requesters(
            authority,
            sources
                .selected_files()
                .filter(|file| file.origin != source::SourceOrigin::Toolchain)
                .filter_map(|file| file.package_identity),
        )
    }
    /// Compare the complete owner sets of the operation's actual declaration
    /// domain. Lookup may also expose unrelated callers; availability does not
    /// make those callers requesters of an already projected operation.
    pub fn require_current_sites(
        &self,
        authority: Option<&dyn DeclarationSelectionAuthority>,
        program: &typed_trees::TypedTrees,
        sites: impl Iterator<Item = source::SourceSpan>,
    ) -> Result<(), &'static str> {
        let mut requesters = std::collections::BTreeSet::new();
        for site in sites {
            let file = program
                .symbols
                .source_file(site)
                .ok_or("retained pending invocation has no source custody")?;
            if file.origin != source::SourceOrigin::Toolchain
                && let Some(requester) = file.package_identity
            {
                requesters.insert(requester);
            }
        }
        self.require_requesters(authority, requesters.into_iter())
    }
    fn require_requesters(
        &self,
        authority: Option<&dyn DeclarationSelectionAuthority>,
        requesters: impl Iterator<Item = PackageKeyIdentity>,
    ) -> Result<(), &'static str> {
        match (self, Self::capture(authority)?) {
            (Self::Absent, Self::Absent) => Ok(()),
            (Self::Packages(previous), Self::Packages(current)) => {
                for requester in requesters {
                    let old = previous
                        .get(&requester)
                        .ok_or("retained declaration scope does not cover a requesting package")?;
                    let new = current
                        .get(&requester)
                        .ok_or("current declaration scope does not cover a requesting package")?;
                    if old != new {
                        return Err("requesting package declaration scope changed");
                    }
                }
                Ok(())
            }
            _ => Err("presence of declaration selection authority changed"),
        }
    }
}
