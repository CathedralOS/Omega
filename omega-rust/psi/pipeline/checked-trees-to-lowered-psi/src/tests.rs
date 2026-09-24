//! Fixtures shared by the lowering tests: reborrow sources, write-line
//! literals and float result assertions. The front-end stages they run first
//! live in `crate::front_end` (`tests/support/front_end.rs`), which the
//! integration `suite` target includes from the same file.

mod attached_unit_cases;
mod borrow_certificate_replay;
mod borrowed_named_results;
mod borrowed_record_leaf_construction;
mod borrowed_view_member_calls;
mod borrowed_whole_view_results;
mod boundary_byte_buffers;
mod byte_extent;
mod byte_sequence_write;
mod byte_write_loop;
mod composed_operand_catalogs;
mod composed_provider_candidates;
mod composed_unit_claims;
mod composed_unit_guarded_jumps;
mod composed_unit_internal_calls;
mod composed_unit_nested_control;
mod composed_unit_prefixed_control;
mod composed_unit_transitive_internal_calls;
mod conditional_result_custody;
mod conditional_return_targets;
mod constructed_case_membership;
mod content_conservation;
mod cyclic_byte_literal_calls;
mod discarded_boundary_results;
mod dynamic_composed_unit;
mod fixed_array_boundary_providers;
mod fixed_byte_array_views;
mod guarded_scalar_returns;
mod indexed_primitive_storage;
mod integer_comparison_replay;
mod lifetime_machine_binders;
mod linear_local_consumers;
mod literal_byte_extent;
mod local_record_reads;
mod operation_crash_contracts;
mod owned_projected_selection;
mod preterminal_optimization;
mod proof_recursion;
mod quotient_correspondence;
mod ranked_value_guarantees;
mod reach_and_scalar_lowering;
mod reborrow_lowering;
mod receiver_free_view_results;
mod scalar_block_invariants;
mod scalar_call_case_arguments;
mod scalar_graph;
mod service_reach_contracts;
mod source_selection;
mod store_lowering;
mod structural_byte_sequence_index_store;
mod structural_byte_sequence_store;
mod structural_case_returns;
mod structural_control_cases;
mod structural_local_bindings;
mod structural_return_cases;
mod structural_scalar_store;
mod suspension_call_plans;
mod unit_cleanup;
mod value_case_dispatch;

use crate::TerminalMachineSelection;
use crate::lower_machine;
use crate::lowering_error::LoweringError;
use crate::retention::reborrow_root_handoff;
use checked_trees::{CheckedScalarExpressionRole, CheckedTrees};
use language_semantics::content::ContentFieldSegment;
use language_semantics::content::{
    ContentAlgebraIdentity as CheckedContentAlgebraIdentity,
    ContentConservationTerm as CheckedContentConservationTerm,
    ContentPlaceRoot as CheckedContentPlaceRoot, ContentPlaceSegment as CheckedContentPlaceSegment,
    ContentPlaceVersion as CheckedContentPlaceVersion,
    ContentStructuralPlace as CheckedContentStructuralPlace,
};
use language_semantics::{PermissionClaimIdentity, PermissionEventSource, SemanticDomainId};
use semantic_vocabulary::{IeeeFloatFormat, ScalarType};
use source::{SourceMap, SourceOrigin};
use std::path::PathBuf;
use symbols::SymbolHandle;
use terminal_psi::{TerminalMachineResult, TerminalModule};
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

/// The toolchain core service declaration, resident so raw-pipeline fixtures
/// can spell `Binding<R>` against the real core declaration. These unit
/// harnesses build a bare `SourceMap` with no package scope, so `use
/// omega::language::core::binding` cannot resolve; installing the source with
/// `SourceOrigin::Toolchain` gives the service classifier the exact identity
/// it requires.
const CORE_SERVICE_OMG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/core/binding.omg"
));

/// Check `source` with `core/binding.omg` resident as a Toolchain source and
/// one fused-service erasure authorization bound per declared boundary trait —
/// the settled-state input `build_evaluation` produces before checking when a
/// Fused provider is selected. Fixtures exercising service-carrier semantics
/// spell `Binding<R>` fields; the requirement trait they close over must be
/// `pub`. The digest is a stand-in; nothing here compares it against a
/// realized plan.
fn checked_source_with_core_service(source: &str) -> checked_trees::CheckedTrees {
    let mut sources = SourceMap::default();
    let service_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/binding.omg"),
            CORE_SERVICE_OMG.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let user_source_id = sources
        .add(PathBuf::from("tests/main.omg"), source.to_owned())
        .source_id;
    let mut typed = crate::front_end::typed_program_from_source_map(
        sources,
        &[
            (service_source_id, CORE_SERVICE_OMG),
            (user_source_id, source),
        ],
    );
    let authorizations = typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .map(
            |definition| typed_trees::typed_trees::FusedServiceErasureAuthorization {
                requirement: definition.symbol,
                provider_plan_digest: [0x5a; 32],
            },
        )
        .collect();
    typed
        .bind_fused_service_erasures(authorizations)
        .expect("fixture boundary traits admit fused service authorizations");
    lower_typed_trees(typed, &CheckingRequest::settled()).expect("check")
}

fn checked_scalar_suspension_fixture() -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(
        r#"
            machine wait(value: bool) -> bool
            requires true == true
            ensures true == true
            { value }

            machine root(parameter: bool) -> bool
            requires true == true
            ensures true == true
            {
                let local: bool = true;
                let parked: bool = wait(local);
                parameter && local
            }
        "#,
    )
}

fn scalar_fixture_call_coordinate(
    checked: &checked_trees::CheckedTrees,
) -> (SymbolHandle, SymbolHandle, usize, usize, SymbolHandle) {
    let root = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "root")
        .expect("root machine");
    let state = checked.machine_states(root).first().expect("root state");
    let target = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "wait")
        .expect("wait machine")
        .symbol;
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .find(|graph| graph.machine == root.symbol)
        .expect("root scalar graph");
    let computations = &checked.facts.values.scalar_computations;
    let root_computation = computations.roots.iter().map(|(_, root)| root).find(|computation| {
        computation.machine == root.symbol
            && computation.state == state.symbol
            && matches!(computation.role, checked_trees::CheckedScalarExpressionRole::LocalInitializer { .. })
            && matches!(computations.nodes.get(computation.root).kind,
                checked_trees::CheckedScalarComputationKind::Call { target_machine, .. } if target_machine == target)
    }).expect("real checked wait call computation");
    let binding = graph.states[0]
        .bindings
        .iter()
        .find(|binding| binding.statement_ordinal == root_computation.statement_ordinal)
        .expect("wait computation has a scalar binding");
    assert_eq!(
        binding.value,
        checked_trees::CheckedScalarBindingValue::Computation
    );
    let checked_trees::CheckedScalarComputationKind::Call {
        call_ordinal,
        source_call,
        ..
    } = computations.nodes.get(root_computation.root).kind
    else {
        panic!("wait initializer retains its call")
    };
    let occurrence = checked.facts.flow.control.calls.get(source_call);
    assert_eq!(
        occurrence.statement_index,
        binding.statement_ordinal as usize
    );
    assert_eq!(occurrence.call_ordinal, call_ordinal as usize);
    (
        root.symbol,
        state.symbol,
        usize::try_from(binding.statement_ordinal).unwrap(),
        usize::try_from(call_ordinal).unwrap(),
        target,
    )
}

fn checked_float_projection_source(source: &str) -> checked_trees::CheckedTrees {
    // A proof-only carrier like the real enum: holding builtin `Int` inline
    // classifies it proof-only, so ensures facts mentioning it route to the
    // structural judge (closed_float_meaning_equality) instead of the
    // polynomial engine.
    const FLOAT_MEANING: &str = "pub data FloatMeaning { value: Int }";
    const FLOAT_FORMAT: &str = r#"
        pub data FloatSpecialValues [copy] {
            signed_zero: bool;
            subnormals: bool;
            infinity: bool;
            nan: bool;
        }
        pub data FloatFormat [copy] {
            radix: u32;
            precision: u32;
            minimum_normal_exponent: i32;
            maximum_normal_exponent: i32;
            minimum_subnormal_exponent: i32;
            specials: FloatSpecialValues;
            rounds_to_nearest_ties_to_even: bool;
        }
        pub const FloatFormat::BINARY32: FloatFormat = FloatFormat {
            radix: 2,
            precision: 24,
            minimum_normal_exponent: -126,
            maximum_normal_exponent: 127,
            minimum_subnormal_exponent: -149,
            specials: FloatSpecialValues {
                signed_zero: true,
                subnormals: true,
                infinity: true,
                nan: true,
            },
            rounds_to_nearest_ties_to_even: true,
        };
        pub const FloatFormat::BINARY64: FloatFormat = FloatFormat {
            radix: 2,
            precision: 53,
            minimum_normal_exponent: -1022,
            maximum_normal_exponent: 1023,
            minimum_subnormal_exponent: -1074,
            specials: FloatSpecialValues {
                signed_zero: true,
                subnormals: true,
                infinity: true,
                nan: true,
            },
            rounds_to_nearest_ties_to_even: true,
        };
    "#;
    const FLOAT_PROJECTIONS: &str = r#"
        machine Float::meaning32(value: f32) -> FloatMeaning;
        machine Float::meaning64(value: f64) -> FloatMeaning;
        pub machine FloatSemantics::add(
            format: FloatFormat,
            left: FloatMeaning,
            right: FloatMeaning
        ) -> FloatMeaning;
    "#;

    let mut sources = SourceMap::default();
    let meaning_source = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/float_meaning.omg"),
            FLOAT_MEANING.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let format_source = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/float_format.omg"),
            FLOAT_FORMAT.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let projection_source = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/float_operations.omg"),
            FLOAT_PROJECTIONS.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let user_source = sources
        .add(
            PathBuf::from("tests/float_projection/main.omg"),
            source.to_owned(),
        )
        .source_id;
    crate::front_end::checked_program_from_source_map(
        sources,
        &[
            (meaning_source, FLOAT_MEANING),
            (format_source, FLOAT_FORMAT),
            (projection_source, FLOAT_PROJECTIONS),
            (user_source, source),
        ],
    )
}

fn reborrow_source(child_access: &str) -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(&format!(
        r#"
            data Cell {{ value: i32; }}
            data Main {{ cell: Cell; }}
            machine Main::exercise(&mut self) {{
                let parent: &mut Cell = &mut self.cell;
                let child: {child_access} Cell = {child_access} parent;
            }}
        "#
    ))
}

fn reborrow_restored_call_source(child_access: &str) -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(&format!(
        r#"
            data Harness {{}}
            data Sink {{}}
            machine Sink::mutate(value: &mut i32) {{ value = 2; }}
            machine Harness::exercise(root: &mut i32) {{
                let parent: &mut i32 = &mut root;
                let child: {child_access} i32 = {child_access} parent;
                Sink::mutate(parent);
            }}
        "#
    ))
}

fn shared_reborrow_restored_call_source() -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(
        r#"
            data Harness {}
            data Sink {}
            machine Sink::mutate(value: &mut i32) { value = 2; }
            machine Harness::exercise(root: &mut i32) {
                let parent: &mut i32 = &mut root;
                let child: &i32 = &parent;
                Sink::mutate(parent);
            }
        "#,
    )
}

fn two_shared_reborrow_restored_call_source() -> checked_trees::CheckedTrees {
    two_shared_reborrow_restored_call_source_with_observations("Sink::observe(left, right);")
}

fn two_shared_reborrow_restored_call_source_with_observations(
    observations: &str,
) -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(&format!(
        r#"
            data Harness {{}}
            data Sink {{}}
            machine Sink::observe(left: &i32, right: &i32) {{}}
            machine Sink::mutate(value: &mut i32) {{ value = 2; }}
            machine Harness::exercise(root: &mut i32) {{
                let parent: &mut i32 = &mut root;
                let left: &i32 = &parent;
                let right: &i32 = &parent;
                {observations}
                Sink::mutate(parent);
            }}
        "#
    ))
}

fn three_shared_reborrow_restored_call_source() -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(
        r#"
            data Harness {}
            data Sink {}
            machine Sink::observe(left: &i32, middle: &i32, right: &i32) {}
            machine Sink::mutate(value: &mut i32) { value = 2; }
            machine Harness::exercise(root: &mut i32) {
                let parent: &mut i32 = &mut root;
                let left: &i32 = &parent;
                let middle: &i32 = &parent;
                let right: &i32 = &parent;
                Sink::observe(left, middle, right);
                Sink::mutate(parent);
            }
        "#,
    )
}

fn multihop_reborrow_source(middle_access: &str, leaf_access: &str) -> checked_trees::CheckedTrees {
    let leaf_call = if leaf_access == "&mut" {
        "mutate(leaf);"
    } else {
        "leaf.value = 1;"
    };
    crate::front_end::checked_program(&format!(
        r#"
            data Cell {{ value: i32; }}
            data Main {{ cell: Cell; }}
            machine mutate(value: &mut Cell) {{ value.value = 1; }}
            machine Main::exercise(&mut self) {{
                let root: &mut Cell = &mut self.cell;
                let middle: {middle_access} Cell = {middle_access} root;
                let leaf: {leaf_access} Cell = {leaf_access} middle;
                {leaf_call}
            }}
        "#
    ))
}

fn lower_reborrow_rows(
    checked: &checked_trees::CheckedTrees,
) -> Result<Vec<terminal_psi::TerminalReborrowRootHandoff>, LoweringError> {
    let source_machine = checked
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .next()
        .expect("real source retains one child resource")
        .1
        .machine_symbol;
    let mut rows = Vec::new();
    reborrow_root_handoff::retain_selected_reborrow_root_handoffs(
        checked,
        source_machine,
        semantic_vocabulary::MachineId::new(1).expect("nonzero machine"),
        &mut rows,
    )?;
    Ok(rows)
}

fn terminal_module_with_reborrow(
    checked: &checked_trees::CheckedTrees,
) -> terminal_psi::TerminalModule {
    let empty = crate::front_end::checked_program(
        r#"
            data Empty {}
            machine Empty::run() {}
        "#,
    );
    let mut module = lower_machine(&empty, TerminalMachineSelection::Name("Empty::run"))
        .expect("empty terminal baseline")
        .semantic_module;
    let mut rows = lower_reborrow_rows(checked).expect("real checked handoff");
    for row in &mut rows {
        row.machine = module.entry;
    }
    module.reborrow_root_handoffs = rows;
    module
}

fn checked_write_line_literal() -> checked_trees::CheckedTrees {
    let source = r#"
        boundary trait Console {
            machine write_line(text: &[u8])
            reaches Console;
        }

        data Root {}
        machine Root::enter()
        reaches Console
        {
            Console::write_line("\x80A");
        }
    "#;
    crate::front_end::checked_program(source)
}

fn assert_source_direct_float_result(primitive: &str, projection: &str, format: IeeeFloatFormat) {
    let source = format!(
        r#"
            machine result(value: {primitive}) -> {primitive}
            ensures
                Float::{projection}(result) == Float::{projection}(result);
            {{ value }}
        "#,
    );
    let checked = checked_float_projection_source(&source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("result"))
        .expect("lower direct float result owner");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let TerminalMachineResult::Scalar(result) = machine.result else {
        panic!("entry should retain its scalar result")
    };
    assert_eq!(result.scalar_type, ScalarType::IeeeFloat(format));
    assert!(machine.contract.requires.is_empty());
    // The authored proof-only meaning equality keeps its claim: one `Atom`
    // clause citing the module's dense checked equality row.
    let [clause] = machine.contract.ensures.as_slice() else {
        panic!("the authored ensures clause lowers to one contract clause")
    };
    assert_eq!(
        clause.proposition,
        semantic_vocabulary::Proposition::Atom(
            terminal_psi::float_meaning_equality_proposition_id(0)
        )
    );
    assert!(machine.contract.outcome_specific_ensures.is_empty());
    assert!(machine.contract.crash_routes.is_empty());
    let [projection] = lowered.semantic_module.float_meaning_projections.as_slice() else {
        panic!("one source-derived FloatMeaning projection expected")
    };
    assert_eq!(
        projection.source,
        terminal_psi::FloatMeaningSource::DirectMachineResult(
            terminal_psi::DirectMachineFloatResult {
                owner: machine.id,
                result: result.id,
                format,
            }
        )
    );
    assert_eq!(
        lowered.semantic_module.float_meaning_equalities,
        vec![terminal_psi::FloatMeaningEqualityProposition {
            id: terminal_psi::ProofPropositionId(0),
            left: projection.result.id,
            right: projection.result.id,
        }]
    );
    terminal_verifier::validate_module(&lowered.semantic_module).expect("verify direct result");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module)
    );
}

fn unit_claim_at(
    machine: SymbolHandle,
    state: SymbolHandle,
    ordinal: u32,
) -> PermissionClaimIdentity {
    PermissionClaimIdentity::Established {
        machine_symbol: machine,
        state_symbol: state,
        source: PermissionEventSource::StateEntry,
        ordinal,
    }
}

fn hard_root_checked_fixture() -> CheckedTrees {
    crate::front_end::checked_program(
        r#"
        boundary trait PortIo {}
        pub data Evidence { case Only; }
        pub data Acknowledgement [linear] {
            sequence: u64;
        }
        pub domain Acknowledgement::Pending;

        boundary machine Acknowledgement::settle(self)
        reaches PortIo
        requires self in Acknowledgement::Pending
        ensures true;

        data Root { proof [erased]: Evidence; }
        machine Root::enter(acknowledgement: Acknowledgement in Pending)
        reaches PortIo
        {
            Helper::run(acknowledgement);
        }

        pub data Helper {}
        machine Helper::run(acknowledgement: Acknowledgement in Pending)
        reaches PortIo
        {
            asm { out 1016, 90 }
            acknowledgement.settle();
        }
        "#,
    )
}

#[test]
fn mathematical_declarations_refuse_at_terminal_lowering() {
    // Checked `let`/`boundary let` records reach this stage on
    // `ProofFacts::mathematical_declarations`; no Terminal evidence encoding
    // carries them yet, so production refuses loudly rather than emit a
    // module that silently omits them (PROOF-CONTRACT-MIGRATION).
    let checked = crate::front_end::checked_program(
        r#"
            let double(x: u64): u64 = x;

            machine main() {}
        "#,
    );
    assert_eq!(checked.facts.proof.mathematical_declarations.len(), 1);
    let error = lower_machine(&checked, TerminalMachineSelection::Name("main"))
        .expect_err("mathematical declarations refuse at lowering");
    assert!(
        matches!(
            error,
            LoweringError::Unsupported(reason) if reason.contains("PROOF-CONTRACT-MIGRATION")
        ),
        "unexpected lowering error: {error:?}"
    );
}

fn source_projection(
    version: CheckedContentPlaceVersion,
    root: CheckedContentPlaceRoot,
    fields: &[(&str, u32)],
    semantic_domain: SemanticDomainId,
) -> CheckedContentConservationTerm {
    CheckedContentConservationTerm::Projection {
        domain: SymbolHandle::from_arena_index(70),
        semantic_domain,
        projection_machine: SymbolHandle::from_arena_index(71),
        projection_report_fingerprint: 0xfeed,
        subject: CheckedContentStructuralPlace {
            version,
            root,
            segments: fields
                .iter()
                .map(|(name, symbol)| {
                    CheckedContentPlaceSegment::Field(ContentFieldSegment {
                        symbol: SymbolHandle::from_arena_index(*symbol),
                        name: (*name).to_owned(),
                    })
                })
                .collect(),
        },
    }
}
