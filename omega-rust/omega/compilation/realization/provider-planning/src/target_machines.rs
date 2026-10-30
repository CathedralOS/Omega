//! Select exact provider members from Psi's retained declaration inventory.
//!
//! Psi owns product-family formation and retains every authored target.
//! Only Build source selects an execution target before resolution. Neither
//! route carries a parallel inventory joined back by names or source spans.
//! Product provider selection remains immutable here; moving the remaining
//! provider settlement behind Terminal Psi is separate work.

use crate::SettledTargetMachineDeclarations;
use diagnostics::Diagnostic;
use language_semantics::direct_calls::DirectCallDeclarations;
use symbols::SymbolHandle;
use target::TargetProfile;

mod authored_providers;
pub use authored_providers::{AuthoredTargetProviders, TargetProviderInventoryRequest};
mod provider_publication;
pub use provider_publication::{
    PreparedTargetProviderPublication, TargetProviderPreparationRequest, provider_selection_roots,
};

/// Project retained declarations onto one realization using their exact handles.
pub fn settle_provider_defaults(
    origins: &[crate::SelectedTargetMachineOrigin],
    retained: &[crate::TargetProviderDefaults],
    target: TargetProfile,
) -> SettledTargetMachineDeclarations {
    let matches_target = |name: &str| {
        TargetProfile::from_canonical_target_name(name).is_ok_and(|resolved| resolved == target)
    };
    let provider_defaults = retained
        .iter()
        .filter(|row| row.target.as_ref().is_none_or(|name| matches_target(name)))
        .flat_map(|row| row.selections.iter().cloned())
        .collect();
    let origins = origins
        .iter()
        .filter(|origin| matches_target(&origin.target))
        .cloned()
        .collect();
    SettledTargetMachineDeclarations {
        provider_defaults,
        origins,
    }
}

/// Refuse the realized product's calls into bodies its target does not have.
///
/// A family implemented only by other targets keeps its declaration as an
/// inert sibling with its callers, so the call still resolves and checking
/// accepts it. Psi checks every body whether or not a realization reaches it,
/// so a missing body is a realization question, never a checking error for
/// the program (wiki/spec/language/machines.md, target-scoped families):
/// only a call the realized product reaches demands an implementation.
///
/// Reach starts at the selected program entry, the root every executable
/// product requires. It follows explicit calls and named transitions, and a
/// call to a requirement continues into the realization each selected
/// provider row binds for it, as the native execution scope does. It does not
/// treat a portable family head as an executable body: exact target membership
/// selects the body whose calls must be visited. A missing member keeps the
/// authored family at the diagnostic boundary rather than importing a sibling.
/// It never enters an inert body, so a caller scoped to the sibling's own target
/// goes inert with it and is never reported. A compilation without a selected
/// entry realizes nothing: a dependency package's candidate check and a
/// targetless check refuse no portable code, such as std's hosted console and
/// filesystem adapters checked for `uefi_x86_64`. Machine-valued arguments are
/// not explicit callees; a missing body reached only through one is refused
/// when the native execution scope reaches the absent member.
pub fn reject_reached_inert_sibling_calls(
    declarations: &DirectCallDeclarations<SymbolHandle>,
    target: TargetProfile,
    entry: Option<symbols::SymbolHandle>,
    selected_providers: &[crate::SelectedProviderReviewProvenance],
) -> Result<(), Vec<Diagnostic>> {
    let provider_rows = selected_providers
        .iter()
        .flat_map(|selected| {
            selected
                .provider
                .row_requirements
                .iter()
                .copied()
                .zip(&selected.provider.row_realizations)
                .filter_map(|(required, owner)| owner.authored().map(|machine| (required, machine)))
        })
        .collect::<Vec<_>>();
    provider_selection::reject_reached_inert_calls(
        declarations,
        target.target_name(),
        entry.as_ref(),
        &provider_rows,
    )
}

/// The same exact family selection supplies closed source evaluation. These
/// rows name checked bodies; they supply no invocation or access permission.
pub fn selected_constant_target_bodies(
    context: crate::ProviderPlanningContext<'_>,
    target: TargetProfile,
) -> Result<Vec<build_time_evaluation::SelectedBuildTimeTargetBody>, Vec<Diagnostic>> {
    if context.scope != source::DependencyScope::Product {
        return Ok(Vec::new());
    }
    let mut selected = Vec::new();
    for family in &context.typed.target_machine_families {
        if !context.contains(family.canonical) {
            continue;
        }
        let Some(member) = context
            .typed
            .target_family_member(family.canonical, target.target_name())
        else {
            continue;
        };
        // This roster transports ordinary checked bodies only. An imported
        // intrinsic or foreign family keeps its own execution owner; merely
        // being realized for this target cannot turn it into checked source.
        if !member.supplies_checked_body()
            || !realizes_machine(context, member, Some(target.target_name()))
        {
            continue;
        }
        selected.push(
            build_time_evaluation::SelectedBuildTimeTargetBody::prepare(
                context.typed,
                family.canonical,
                member.symbol,
                target.target_name(),
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?,
        );
    }
    Ok(selected)
}

/// The machine a provider row realizes on the selected target. A row names
/// the machine resolution bound its authored path to: a portable body, an
/// exact target member, or a target family's head when the authored name is
/// a family (`public macos_arm64 machine sleep` in a library). The head is the
/// family's declaration, not a body, so the row realizes the member the
/// catalog retains for the selected target; a family without a body for that
/// target leaves the row, and its map, inert on that target exactly like a
/// member authored for another target. A Build-scope context realizes only
/// portable bodies, as `realizes_machine` states.
pub(crate) fn realized_row_machine<'program>(
    context: crate::ProviderPlanningContext<'program>,
    named: symbols::SymbolHandle,
    selected_target: Option<&str>,
) -> Option<&'program checked_trees::typed_trees::machine::Machine> {
    let typed = context.typed;
    let machine = typed.machine_by_symbol(named)?;
    let realization = if machine.target_family_interface {
        let target = selected_target
            .filter(|target| !target.is_empty())
            .unwrap_or_else(|| TargetProfile::host().target_name());
        typed.target_family_member(named, target)?
    } else {
        machine
    };
    realizes_machine(context, realization, selected_target).then_some(realization)
}

/// Select declaration candidates without changing the shared source program.
/// Build has its own admitted execution scope; its selected declaration marker
/// is resolved during Build preparation, not from the product target.
pub(crate) fn realizes_machine(
    context: crate::ProviderPlanningContext<'_>,
    machine: &checked_trees::typed_trees::machine::Machine,
    target: Option<&str>,
) -> bool {
    // A portable family interface has no implementation or satisfies edges.
    // Sharing the members' callable signature does not make it a second
    // checked adapter. Entry/family dispatch selects a retained member through
    // the explicit family catalog instead of this provider-candidate query.
    if machine.target_family_interface || !context.contains(machine.symbol) {
        return false;
    }
    if context.scope == source::DependencyScope::Build {
        return machine.target.is_none();
    }
    let target = target
        .filter(|target| !target.is_empty())
        .unwrap_or_else(|| TargetProfile::host().target_name());
    context
        .typed
        .source_target_context(machine)
        .is_none_or(|authored| authored == target)
}
