//! Fixed task stack layouts and carry obligations.

use checked_trees::{CheckedTrees, SuspensionCrossingStorage};
use diagnostics::Diagnostic;
use language_semantics::{CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use layout::TypeLayout;
use target::NativeTarget;
use task_plans::{ActivationCarryObligations, CanonicalSuspensionCrossing};

/// Current fixed-stack layout bridge: persistent machine storage plus the
/// largest canonical park frontier and entry resume overhead. Whole-call-graph
/// WCSU composition will replace this local sizing pass before provider stack
/// reservation is enabled.
pub(crate) fn fixed_stack_layout(
    program: &CheckedTrees,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    layouts: &layout::LayoutPlan,
    machine: &checked_trees::machine::Machine,
    crossings: &[&checked_trees::SuspensionCrossingCarryFact],
) -> Result<(u64, u64), Vec<Diagnostic>> {
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find_map(|(_, layout)| (layout.symbol == machine.symbol).then_some(layout.layout))
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "task activation target `{}` has no concrete machine layout",
                machine.name
            ))]
        })?;
    // Every activation needs an explicit resume-state word even when the
    // selected machine has no stored fields and never parks.
    let state_word = TypeLayout {
        size: target.pointer_size,
        alignment: target.pointer_alignment,
    };
    let mut base_size = 0usize;
    let mut base_alignment = 1usize;
    append_layout(&mut base_size, &mut base_alignment, state_word)?;
    append_layout(&mut base_size, &mut base_alignment, machine_layout)?;
    base_size = align_to(base_size, base_alignment)?;

    let mut maximum_size = base_size;
    let mut maximum_alignment = base_alignment;
    for crossing in crossings {
        let mut size = base_size;
        let mut alignment = base_alignment;
        for live in &crossing.live_values {
            if live.storage == SuspensionCrossingStorage::Persistent {
                continue;
            }
            let layout = layout::layout_type_reference(
                program,
                target,
                opaque_representation_selections,
                live.type_reference,
            )
            .map_err(|error| vec![error])?;
            append_layout(&mut size, &mut alignment, layout)?;
        }
        size = align_to(size, alignment)?;
        maximum_size = maximum_size.max(size);
        maximum_alignment = maximum_alignment.max(alignment);
    }
    maximum_size = align_to(maximum_size, maximum_alignment)?;
    Ok((
        u64::try_from(maximum_size)
            .map_err(|_| vec![Diagnostic::error("task continuation size exceeds u64")])?,
        u64::try_from(maximum_alignment)
            .map_err(|_| vec![Diagnostic::error("task continuation alignment exceeds u64")])?,
    ))
}

fn append_layout(
    size: &mut usize,
    alignment: &mut usize,
    layout: TypeLayout,
) -> Result<(), Vec<Diagnostic>> {
    let field_alignment = layout.alignment.max(1);
    *size = align_to(*size, field_alignment)?;
    *size = size
        .checked_add(layout.size)
        .ok_or_else(|| vec![Diagnostic::error("task continuation layout size overflow")])?;
    *alignment = (*alignment).max(field_alignment);
    Ok(())
}

fn align_to(value: usize, alignment: usize) -> Result<usize, Vec<Diagnostic>> {
    value
        .checked_add(alignment.saturating_sub(1))
        .map(|rounded| rounded / alignment * alignment)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task continuation layout alignment overflow",
            )]
        })
}

pub(crate) fn carry_obligations(policy: CarryPolicy) -> ActivationCarryObligations {
    ActivationCarryObligations {
        preserve_cpu: policy.cpu == CarryCpu::Origin,
        preserve_host_thread: policy.host_thread == CarryHostThread::Origin,
    }
}

pub(crate) fn canonical_suspension_crossing(
    program: &CheckedTrees,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
) -> Result<CanonicalSuspensionCrossing, Vec<Diagnostic>> {
    Ok(CanonicalSuspensionCrossing {
        identity: checked_trees::canonical_suspension_crossing_id(program, crossing).ok_or_else(
            || {
                vec![Diagnostic::error(
                    "task activation carry crossing source identity must resolve exactly",
                )]
            },
        )?,
        suspension_allowed: crossing.effective.suspension == CarrySuspension::Allowed,
        preserve_cpu: crossing.effective.cpu == CarryCpu::Origin,
        preserve_host_thread: crossing.effective.host_thread == CarryHostThread::Origin,
    })
}
