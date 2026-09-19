//! Fixtures shared by the lowering tests: checked sources, reborrow
//! sources, write-line literals and float result assertions.

mod attached_unit_cases;
mod boundary_byte_buffers;
mod byte_extent;
mod byte_sequence_write;
mod byte_write_loop;
mod composed_operand_catalogs;
mod composed_provider_candidates;
mod composed_unit_claims;
mod composed_unit_internal_calls;
mod composed_unit_nested_control;
mod composed_unit_prefixed_control;
mod composed_unit_transitive_internal_calls;
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
mod scalar_block_invariants;
mod scalar_call_case_arguments;
mod scalar_graph;
mod service_reach_contracts;
mod source_selection;
mod store_lowering;
mod structural_byte_sequence_index_store;
mod structural_byte_sequence_store;
mod structural_control_cases;
mod structural_return_cases;
mod structural_scalar_store;
mod unit_cleanup;

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
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use symbols::SymbolHandle;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_psi::{TerminalMachineResult, TerminalModule};
use tokens_to_syntax_trees::{
    parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id,
};
use typed_trees_to_checked_trees::lower_typed_trees;

fn checked_source(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn checked_scalar_suspension_fixture() -> checked_trees::CheckedTrees {
    checked_source(
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
    const FLOAT_MEANING: &str = "data FloatMeaning { }";
    const FLOAT_PROJECTIONS: &str = r#"
        operator Float::meaning32(value: f32) -> FloatMeaning;
        operator Float::meaning64(value: f64) -> FloatMeaning;
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
    let meaning_tokens = Lexer::new(FLOAT_MEANING)
        .tokenize()
        .expect("tokenize meaning");
    let mut syntax =
        parse_syntax_trees_with_id(meaning_source, &meaning_tokens).expect("parse meaning");
    let projection_tokens = Lexer::new(FLOAT_PROJECTIONS)
        .tokenize()
        .expect("tokenize projections");
    parse_syntax_trees_into_with_id(&mut syntax, projection_source, &projection_tokens)
        .expect("parse projections");
    let user_tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
    parse_syntax_trees_into_with_id(&mut syntax, user_source, &user_tokens).expect("parse fixture");
    let resolved = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve source-aware fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn reborrow_source(child_access: &str) -> checked_trees::CheckedTrees {
    checked_source(&format!(
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
    checked_source(&format!(
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
    checked_source(
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
    checked_source(&format!(
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
    checked_source(
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
    checked_source(&format!(
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
    let empty = checked_source(
        r#"
            data Empty {}
            machine Empty::run() {}
        "#,
    );
    let mut module = lower_machine(&empty, "Empty::run")
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
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
    let lowered = lower_machine(&checked, "result").expect("lower direct float result owner");
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
    assert!(machine.contract.ensures.is_empty());
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
    checked_source(
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
    let checked = checked_source(
        r#"
            let double(x: u64): u64 = x;

            machine main() {}
        "#,
    );
    assert_eq!(checked.facts.proof.mathematical_declarations.len(), 1);
    let error =
        lower_machine(&checked, "main").expect_err("mathematical declarations refuse at lowering");
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
