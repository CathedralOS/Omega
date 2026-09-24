use crate::tests::front_end::{checked_program, checked_program_result};

use crate::CheckingRequest;
use crate::lower_typed_trees;
use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
};

mod attached_receiver_shapes;
mod borrowed_view_returns;
mod borrowed_window_pairs;
mod boundary_result_operands;
mod call_argument_casts;
mod call_result_field_stores;
mod callable_composed;
pub(crate) mod calls;
mod cleanup;
mod closed_case_pairs;
mod composed_call_arguments;
mod composed_claims;
mod composed_internal_calls;
mod composed_nested_control;
mod composed_prefixed_control;
mod composed_transitive_internal_calls;
mod discarded_results;
mod free_scalar_parameters;
mod linear_local_consumers;
mod nested_boundary_results;
mod primitive_locals;
mod receiver_stores;
mod repro_cyclic;
mod returns;
mod returns_primitive_effects;
mod scalar_boundary_targets;
mod scalar_primitive_targets;
mod scalar_sequences;
mod shared_convergence;
mod shared_result_borrows;
mod slice_view_locals;
mod state_graph_carriers;
mod state_graph_crash_exit;
mod state_graph_forwarded_views;
mod state_graph_guarded_jumps;
mod state_graph_literals;
mod state_graph_scalars;
mod state_graph_subslices;
mod structural_call_arguments;
mod structural_local_bindings;
mod tail_calls;

use checked_trees::{
    CheckedBoundaryMachineResultPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape,
};
use language_core::BindingRelevance;
use language_semantics::Multiplicity;
use typed_trees::types::PrimitiveType;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let source = format!("boundary trait PortIo {{}}\n{source}");
    checked_program(&source)
}

/// Same prelude as `checked`, with the toolchain `core/binding.omg` resident
/// so fixtures can hold `Binding<R>` carriers, plus the settled fused-service
/// erasure authorizations `bind_fixture_fused_service_erasures` supplies.
/// Boundary traits closed over by a `Binding<R>` field must be declared `pub`
/// in the fixture.
fn checked_with_service(source: &str) -> checked_trees::CheckedTrees {
    let source = format!("boundary trait PortIo {{}}\n{source}");
    let mut typed = crate::tests::front_end::typed_program_with_core_service(&source);
    crate::tests::bind_fixture_fused_service_erasures(&mut typed);
    lower_typed_trees(typed, &CheckingRequest::settled()).expect("check")
}

fn contextual_cleanup_diagnostics(source: &str) -> Vec<diagnostics::Diagnostic> {
    checked_program_result(source)
        .expect_err("contextual cleanup requirement-set mismatch must reject at its return edge")
}

fn machine_named(checked: &checked_trees::CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == name || machine.name.as_str().ends_with(&format!("::{name}"))
        })
        .unwrap_or_else(|| panic!("missing machine `{name}`"))
        .symbol
}

fn record_fields(
    shape: &checked_trees::CheckedUnitStructuralTypePlan,
) -> &[CheckedUnitStructuralFieldPlan] {
    let CheckedUnitStructuralTypeShape::Record { fields } = &shape.shape else {
        panic!("expected record structural shape")
    };
    fields
}
