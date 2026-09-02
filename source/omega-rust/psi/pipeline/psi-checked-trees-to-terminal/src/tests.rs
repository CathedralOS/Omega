//! Root-level checked-to-terminal producer regressions.

use super::*;
use psi_language_semantics::{
    PermissionEventSource, SemanticDomainId,
    content::{
        ContentCaseSegment, ContentConservationEquation, ContentConservationOwnerKind,
        ContentFieldSegment,
    },
};
use psi_source::{SourceMap, SourceOrigin};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_symbols::SymbolHandle;
use psi_syntax_trees_to_symbol_resolved_trees::{
    lower_syntax_trees, lower_syntax_trees_with_sources,
};
use psi_terminal::BindingRelevance;
use psi_tokens_to_syntax_trees::{
    parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id,
};
use psi_typed_trees_to_checked_trees::lower_typed_trees;
use std::{path::PathBuf, sync::Arc};

mod attached_unit_cases;
mod composed_unit_claims;
mod composed_unit_internal_calls;
mod composed_unit_nested_control;
mod composed_unit_prefixed_control;
mod composed_unit_transitive_internal_calls;
mod content_conservation;
mod dynamic_composed_unit;
mod proof_recursion;
mod quotient_correspondence;
mod scalar_graph;
mod service_reach_contracts;
mod structural_control_cases;
mod structural_return_cases;
mod unit_cleanup;

fn checked_source(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn checked_float_projection_source(source: &str) -> psi_checked_trees::CheckedTrees {
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
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve source-aware fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn reborrow_source(child_access: &str) -> psi_checked_trees::CheckedTrees {
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

fn reborrow_restored_call_source(child_access: &str) -> psi_checked_trees::CheckedTrees {
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

fn shared_reborrow_restored_call_source() -> psi_checked_trees::CheckedTrees {
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

fn two_shared_reborrow_restored_call_source() -> psi_checked_trees::CheckedTrees {
    two_shared_reborrow_restored_call_source_with_observations("Sink::observe(left, right);")
}

fn two_shared_reborrow_restored_call_source_with_observations(
    observations: &str,
) -> psi_checked_trees::CheckedTrees {
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

fn three_shared_reborrow_restored_call_source() -> psi_checked_trees::CheckedTrees {
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

fn multihop_reborrow_source(
    middle_access: &str,
    leaf_access: &str,
) -> psi_checked_trees::CheckedTrees {
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
    checked: &psi_checked_trees::CheckedTrees,
) -> Result<Vec<psi_terminal::TerminalReborrowRootHandoff>, LoweringError> {
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
        psi_core::MachineId::new(1).expect("nonzero machine"),
        &mut rows,
    )?;
    Ok(rows)
}

fn terminal_module_with_reborrow(
    checked: &psi_checked_trees::CheckedTrees,
) -> psi_terminal::TerminalModule {
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

#[test]
fn terminal_reborrow_root_handoff_lowers_mutable_and_write_only_children() {
    for child_access in ["&mut", "&write"] {
        let checked = reborrow_source(child_access);
        let rows = lower_reborrow_rows(&checked)
            .expect("one-hop state-exit reborrow publishes root custody");
        let [row] = rows.as_slice() else {
            panic!("one exact root handoff")
        };
        assert_eq!(
            row.direct_root_access,
            psi_terminal::StructuralAccess::MutableBorrow
        );
        let [step] = row.lineage.as_slice() else {
            panic!("one exact child edge")
        };
        assert_eq!(
            step.child_access,
            if child_access == "&mut" {
                psi_terminal::StructuralAccess::MutableBorrow
            } else {
                psi_terminal::StructuralAccess::WriteOnlyBorrow
            }
        );
        assert_eq!(step.formation_boundary, step.child_activation);
        assert_eq!(
            row.direct_root_lifetime_identity,
            row.direct_root_place.root_identity
        );
    }
}

#[test]
fn terminal_reborrow_restored_call_use_lowers_exclusive_and_sole_shared_children() {
    for (label, checked, machine_name, expected_access) in [
        (
            "mutable",
            reborrow_restored_call_source("&mut"),
            "Harness::exercise",
            psi_terminal::StructuralAccess::MutableBorrow,
        ),
        (
            "write-only",
            reborrow_restored_call_source("&write"),
            "Harness::exercise",
            psi_terminal::StructuralAccess::WriteOnlyBorrow,
        ),
        (
            "sole-shared",
            shared_reborrow_restored_call_source(),
            "Harness::exercise",
            psi_terminal::StructuralAccess::SharedBorrow,
        ),
    ] {
        let lowered = lower_machine(&checked, machine_name).unwrap_or_else(|error| {
            panic!("{label} restored-parent mutating call lowers to Terminal Psi: {error:?}")
        });
        let [use_row] = lowered
            .semantic_module
            .reborrow_restored_call_uses
            .as_slice()
        else {
            panic!("one exact restored-parent call use")
        };
        assert_eq!(use_row.machine, lowered.semantic_module.entry);
        assert_eq!(use_row.child_access, expected_access);
        assert_eq!(
            use_row.restoration_class,
            if expected_access == psi_terminal::StructuralAccess::SharedBorrow {
                psi_terminal::TerminalReborrowRestorationClass::SharedFreezeRestoration
            } else {
                psi_terminal::TerminalReborrowRestorationClass::ExclusiveReactivation
            }
        );
        if expected_access == psi_terminal::StructuralAccess::SharedBorrow {
            let [member] = use_row.shared_cohort.as_slice() else {
                panic!("sole shared restoration retains one cohort member")
            };
            assert_eq!(member.child_owner_identity, use_row.child_owner_identity);
            assert_eq!(member.child_owner_path, use_row.child_owner_path);
            assert_eq!(member.child_place, use_row.child_place);
            assert_eq!(member.child_access, use_row.child_access);
            assert_eq!(member.child_activation, use_row.child_activation);
            assert_eq!(member.child_weakening, use_row.child_weakening);
        } else {
            assert!(use_row.shared_cohort.is_empty());
        }
        let psi_terminal::TerminalBorrowBoundarySource::Call {
            statement_index,
            call_ordinal,
            target_identity,
        } = &use_row.call_boundary
        else {
            panic!("restored use retains one exact source call coordinate")
        };
        assert_eq!(*call_ordinal, 0);
        assert!(!target_identity.is_empty());
        let caller = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == use_row.machine)
            .expect("restored-use caller");
        let operation = caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == use_row.operation)
            .expect("restored-use operation");
        let psi_terminal::OperationKind::CallUnit { callee, .. } = operation.kind else {
            panic!("restored-use operation is CallUnit")
        };
        assert_eq!(use_row.call_target_machine, callee);
        let certificate = checked
            .facts
            .borrow
            .reborrow_restored_call_use_certificates
            .iter()
            .next()
            .expect("checked restored use")
            .1;
        let psi_checked_trees::FlowInvalidationSource::Statement {
            statement_index: child_end,
        } = checked
            .facts
            .borrow
            .reborrow_loan_resources
            .get(certificate.child_resource)
            .weakening_source
        else {
            panic!("restored child weakens at one statement")
        };
        assert_eq!(
            *statement_index,
            u64::try_from(child_end).expect("statement range")
        );
        assert_eq!(
            use_row.direct_root_lifetime_identity,
            use_row.direct_root_place.root_identity
        );
        assert_eq!(use_row.formation_boundary, use_row.child_activation);
        assert!(lowered.source_call_occurrences.iter().any(|occurrence| {
            occurrence.terminal_operation == use_row.operation
                && occurrence.source_state
                    == checked
                        .facts
                        .borrow
                        .reborrow_restored_call_use_certificates
                        .iter()
                        .next()
                        .expect("checked restored use")
                        .1
                        .state_symbol
        }));
        psi_terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("restored call use verifies");
        let encoded = psi_terminal_codec::encode_module(&lowered.semantic_module)
            .expect("restored call use encodes");
        assert_eq!(
            psi_terminal_codec::decode_module(&encoded).expect("restored call use decodes"),
            lowered.semantic_module
        );
    }
}

#[test]
fn terminal_reborrow_restored_call_use_lowers_exact_two_member_shared_cohort() {
    let checked = two_shared_reborrow_restored_call_source();
    let exercise = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| checked.typed.symbols.name(plan.machine) == "Harness::exercise")
        .expect("the exact alias-erased exercise plan");
    let [
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate: observation_coordinate,
            structural_arguments: observation_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate: mutation_coordinate,
            structural_arguments: mutation_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::ReturnUnit { .. },
    ] = exercise.operations.as_slice()
    else {
        panic!("one observation call, one restored mutation, and Unit return")
    };
    assert_eq!(observation_coordinate.statement_index, 3);
    assert_eq!(mutation_coordinate.statement_index, 4);
    assert_eq!(observation_arguments.len(), 2);
    assert!(observation_arguments.iter().all(|argument| {
        argument.source_parameter_index() == Some(0)
            && argument.path.is_empty()
            && argument.access == psi_checked_trees::CheckedStructuralAccess::SharedBorrow
    }));
    let [mutation_argument] = mutation_arguments.as_slice() else {
        panic!("one restored whole-parent mutation argument")
    };
    assert_eq!(mutation_argument.source_parameter_index(), Some(0));
    assert!(mutation_argument.path.is_empty());
    assert_eq!(
        mutation_argument.access,
        psi_checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    let lowered = lower_machine(&checked, "Harness::exercise")
        .expect("two-member shared cohort lowers to Terminal Psi");
    let [row] = lowered
        .semantic_module
        .reborrow_restored_call_uses
        .as_slice()
    else {
        panic!("one restored-parent call publication")
    };
    assert_eq!(
        row.restoration_class,
        psi_terminal::TerminalReborrowRestorationClass::SharedFreezeRestoration
    );
    let [left, right] = row.shared_cohort.as_slice() else {
        panic!("the exact two-member shared-freeze roster")
    };
    assert_ne!(left, right);
    for member in [left, right] {
        assert_eq!(
            member.child_access,
            psi_terminal::StructuralAccess::SharedBorrow
        );
        assert_eq!(member.child_weakening, row.child_weakening);
    }
    assert!(row.shared_cohort.iter().any(|member| {
        member.child_owner_identity == row.child_owner_identity
            && member.child_owner_path == row.child_owner_path
            && member.child_place == row.child_place
            && member.child_activation == row.child_activation
    }));
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("two-member restored call use verifies");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("two-member restored call use encodes");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("two-member restored call use decodes"),
        lowered.semantic_module
    );
}

#[test]
fn terminal_reborrow_restored_call_use_lowers_exact_three_member_shared_cohort() {
    let checked = three_shared_reborrow_restored_call_source();
    let exercise = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| checked.typed.symbols.name(plan.machine) == "Harness::exercise")
        .expect("the exact alias-erased exercise plan");
    let [
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate: observation_coordinate,
            structural_arguments: observation_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate: mutation_coordinate,
            structural_arguments: mutation_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::ReturnUnit { .. },
    ] = exercise.operations.as_slice()
    else {
        panic!("one observation call, one restored mutation, and Unit return")
    };
    assert_eq!(observation_coordinate.statement_index, 4);
    assert_eq!(mutation_coordinate.statement_index, 5);
    assert_eq!(observation_arguments.len(), 3);
    assert!(observation_arguments.iter().all(|argument| {
        argument.source_parameter_index() == Some(0)
            && argument.path.is_empty()
            && argument.access == psi_checked_trees::CheckedStructuralAccess::SharedBorrow
    }));
    let [mutation_argument] = mutation_arguments.as_slice() else {
        panic!("one restored whole-parent mutation argument")
    };
    assert_eq!(mutation_argument.source_parameter_index(), Some(0));
    assert!(mutation_argument.path.is_empty());
    assert_eq!(
        mutation_argument.access,
        psi_checked_trees::CheckedStructuralAccess::MutableBorrow
    );

    let lowered = lower_machine(&checked, "Harness::exercise")
        .expect("three-member shared cohort lowers to Terminal Psi");
    let [row] = lowered
        .semantic_module
        .reborrow_restored_call_uses
        .as_slice()
    else {
        panic!("one restored-parent call publication")
    };
    let [left, middle, right] = row.shared_cohort.as_slice() else {
        panic!("the exact three-member shared-freeze roster")
    };
    assert_ne!(left, middle);
    assert_ne!(left, right);
    assert_ne!(middle, right);
    for member in [left, middle, right] {
        assert_eq!(
            member.child_access,
            psi_terminal::StructuralAccess::SharedBorrow
        );
        assert_eq!(member.child_weakening, row.child_weakening);
    }
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("three-member restored call use verifies");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("three-member restored call use encodes");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("three-member restored call use decodes"),
        lowered.semantic_module
    );

    let certificate = checked
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .next()
        .expect("three-member checked restored use")
        .1
        .clone();
    let mut duplicate = checked;
    let disposition = duplicate
        .facts
        .borrow
        .reborrow_disposition_events
        .get_mut(certificate.disposition);
    disposition.shared_cohort[2] = disposition.shared_cohort[0];
    assert!(
        lower_machine(&duplicate, "Harness::exercise").is_err(),
        "nonadjacent duplicate cohort members must fail checked-to-Terminal replay"
    );
}

#[test]
fn terminal_two_shared_restored_call_aliasing_fences_reordered_and_extra_observations() {
    for observations in [
        "Sink::observe(right, left);",
        "Sink::observe(left, right); Sink::observe(left, right);",
    ] {
        let checked = two_shared_reborrow_restored_call_source_with_observations(observations);
        assert!(
            checked
                .facts
                .borrow
                .reborrow_restored_call_use_certificates
                .is_empty(),
            "unsupported shared observation layout must not gain checked authority"
        );
        assert!(
            lower_machine(&checked, "Harness::exercise").is_err(),
            "unsupported shared observation layout must remain outside the Unit plan"
        );
    }
}

#[test]
fn terminal_shared_restored_call_use_rejects_checked_cohort_drift() {
    let baseline = shared_reborrow_restored_call_source();
    let certificate = baseline
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .next()
        .expect("shared restored use")
        .1
        .clone();

    let mut missing = baseline.clone();
    missing
        .facts
        .borrow
        .reborrow_disposition_events
        .get_mut(certificate.disposition)
        .shared_cohort
        .clear();
    assert!(lower_machine(&missing, "Harness::exercise").is_err());

    let mut wrong_disposition = baseline.clone();
    wrong_disposition
        .facts
        .borrow
        .reborrow_disposition_events
        .get_mut(certificate.disposition)
        .disposition = psi_checked_trees::CheckedReborrowResourceDisposition::Reactivate;
    assert!(lower_machine(&wrong_disposition, "Harness::exercise").is_err());

    let mut wrong_containment = baseline;
    wrong_containment
        .facts
        .borrow
        .reborrow_containment_certificates
        .get_mut(certificate.containment)
        .containment = psi_checked_trees::CheckedReborrowContainmentKind::ExclusiveSuspension;
    assert!(lower_machine(&wrong_containment, "Harness::exercise").is_err());

    let mut wrong_parent_status = shared_reborrow_restored_call_source();
    wrong_parent_status
        .facts
        .borrow
        .reborrow_loan_resources
        .get_mut(certificate.child_resource)
        .parent_end_status
        .status = psi_checked_trees::ParentLexicalStatusAtChildEnd::RetiredWithChild;
    assert!(lower_machine(&wrong_parent_status, "Harness::exercise").is_err());

    let mut wrong_parent_weakening = shared_reborrow_restored_call_source();
    wrong_parent_weakening
        .facts
        .borrow
        .reborrow_containment_certificates
        .get_mut(certificate.containment)
        .parent_weakening = certificate.child_weakening;
    assert!(lower_machine(&wrong_parent_weakening, "Harness::exercise").is_err());

    let mut invalid_formation_constraint = shared_reborrow_restored_call_source();
    invalid_formation_constraint
        .facts
        .borrow
        .reborrow_loan_resources
        .get_mut(certificate.child_resource)
        .parent_suspension
        .parent_entry_constraint = psi_arena::Handle::invalid();
    assert!(lower_machine(&invalid_formation_constraint, "Harness::exercise").is_err());
}

#[test]
fn terminal_reborrow_root_handoff_lowers_finite_linear_exclusive_lineages() {
    for (middle_access, leaf_access, expected) in [
        (
            "&mut",
            "&mut",
            [
                psi_terminal::StructuralAccess::MutableBorrow,
                psi_terminal::StructuralAccess::MutableBorrow,
            ],
        ),
        (
            "&mut",
            "&write",
            [
                psi_terminal::StructuralAccess::MutableBorrow,
                psi_terminal::StructuralAccess::WriteOnlyBorrow,
            ],
        ),
        (
            "&write",
            "&write",
            [
                psi_terminal::StructuralAccess::WriteOnlyBorrow,
                psi_terminal::StructuralAccess::WriteOnlyBorrow,
            ],
        ),
    ] {
        let checked = multihop_reborrow_source(middle_access, leaf_access);
        let rows = lower_reborrow_rows(&checked)
            .expect("a finite linear exclusive lineage publishes root custody");
        let [row] = rows.as_slice() else {
            panic!("one exact multihop root handoff")
        };
        assert_eq!(
            row.direct_root_access,
            psi_terminal::StructuralAccess::MutableBorrow
        );
        assert_eq!(
            row.lineage
                .iter()
                .map(|step| step.child_access)
                .collect::<Vec<_>>(),
            expected,
        );
        assert!(row.lineage.iter().all(|step| {
            step.formation_boundary == step.child_activation
                && step.child_place.root_identity == row.direct_root_place.root_identity
        }));
        assert_eq!(
            row.direct_root_lifetime_identity,
            row.direct_root_place.root_identity
        );
    }
}

#[test]
fn terminal_reborrow_root_handoff_fences_shared_and_branched_lineages() {
    let shared = reborrow_source("&");
    assert!(lower_reborrow_rows(&shared).is_err());

    let branched = checked_source(
        r#"
            data Cell { value: i32; }
            data Main { cell: Cell; }
            machine use_mut(value: &mut Cell) { value.value = 1; }
            machine Main::exercise(&mut self) {
                let root: &mut Cell = &mut self.cell;
                let first: &mut Cell = &mut root;
                use_mut(first);
                let second: &mut Cell = &mut root;
            }
        "#,
    );
    assert!(lower_reborrow_rows(&branched).is_err());
}

#[test]
fn terminal_multihop_root_handoff_rejects_missing_or_reordered_checked_edges() {
    let baseline = multihop_reborrow_source("&mut", "&mut");
    let containment = baseline
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .next()
        .expect("first containment edge")
        .0;
    let mut missing = baseline.clone();
    let retained = missing
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .filter(|(handle, _)| *handle != containment)
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    missing
        .facts
        .borrow
        .reborrow_containment_certificates
        .reset_retain_capacity();
    for row in retained {
        missing
            .facts
            .borrow
            .reborrow_containment_certificates
            .insert(row);
    }
    assert!(lower_reborrow_rows(&missing).is_err());

    let mut reordered = baseline;
    let event = reordered
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .find(|(_, event)| {
            event.disposition
                == psi_checked_trees::CheckedReborrowResourceDisposition::StateExitDirectRootHandoff
        })
        .expect("multihop root disposition")
        .0;
    reordered
        .facts
        .borrow
        .reborrow_disposition_events
        .get_mut(event)
        .retired_parent_path
        .swap(0, 1);
    assert!(lower_reborrow_rows(&reordered).is_err());
}

#[test]
fn terminal_reborrow_root_handoff_rejects_tampered_checked_joins() {
    let baseline = reborrow_source("&mut");
    let event_handle = baseline
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .next()
        .expect("disposition")
        .0;
    let certificate_handle = baseline
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .next()
        .expect("containment")
        .0;

    let mut wrong_phase = baseline.clone();
    wrong_phase
        .facts
        .borrow
        .reborrow_disposition_events
        .get_mut(event_handle)
        .boundary_phase = psi_checked_trees::CheckedBorrowResourceLifecyclePhase::Activation;
    assert!(lower_reborrow_rows(&wrong_phase).is_err());

    let mut wrong_disposition = baseline.clone();
    wrong_disposition
        .facts
        .borrow
        .reborrow_disposition_events
        .get_mut(event_handle)
        .disposition = psi_checked_trees::CheckedReborrowResourceDisposition::Reactivate;
    assert!(lower_reborrow_rows(&wrong_disposition).is_err());

    let mut wrong_containment = baseline.clone();
    wrong_containment
        .facts
        .borrow
        .reborrow_containment_certificates
        .get_mut(certificate_handle)
        .containment = psi_checked_trees::CheckedReborrowContainmentKind::SharedFreeze;
    assert!(lower_reborrow_rows(&wrong_containment).is_err());

    let child_handle = baseline
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .next()
        .expect("child resource")
        .0;
    let mut wrong_effect = baseline.clone();
    wrong_effect
        .facts
        .borrow
        .reborrow_loan_resources
        .get_mut(child_handle)
        .access_effect = psi_checked_trees::CheckedReborrowAccessEffect::SharedFreeze;
    assert!(lower_reborrow_rows(&wrong_effect).is_err());

    let mut missing_event = baseline.clone();
    missing_event.facts.borrow.reborrow_disposition_events = psi_arena::Arena::new();
    assert!(lower_reborrow_rows(&missing_event).is_err());
}

#[test]
fn terminal_reborrow_root_handoff_codec_and_verifier_reject_tampering() {
    let checked = reborrow_source("&mut");
    let module = terminal_module_with_reborrow(&checked);
    psi_terminal_verifier::validate_module(&module).expect("exact handoff verifies");
    let encoded = psi_terminal_codec::encode_module(&module).expect("handoff encodes");
    assert_eq!(
        psi_terminal_codec::decode_module(&encoded).expect("handoff decodes"),
        module
    );

    let mut amplified = module.clone();
    amplified.reborrow_root_handoffs[0].direct_root_access = psi_terminal::StructuralAccess::Owned;
    assert!(psi_terminal_verifier::validate_module(&amplified).is_err());

    let mut redirected = module.clone();
    redirected.reborrow_root_handoffs[0].lineage[0].formation_boundary =
        psi_terminal::TerminalBorrowBoundarySource::Statement {
            statement_index: u64::MAX,
        };
    assert!(psi_terminal_verifier::validate_module(&redirected).is_err());

    let mut duplicated = module.clone();
    duplicated
        .reborrow_root_handoffs
        .push(duplicated.reborrow_root_handoffs[0].clone());
    assert!(psi_terminal_verifier::validate_module(&duplicated).is_err());

    let mut observed_invalid_access_tag = false;
    for (index, byte) in encoded.iter().enumerate() {
        if *byte != 3 {
            continue;
        }
        let mut tampered = encoded.clone();
        tampered[index] = 0xff;
        if matches!(
            psi_terminal_codec::decode_module(&tampered),
            Err(psi_terminal_codec::CodecError::InvalidTag(
                "StructuralAccess",
                0xff
            ))
        ) {
            observed_invalid_access_tag = true;
            break;
        }
    }
    assert!(
        observed_invalid_access_tag,
        "codec rejects a corrupted custody access tag"
    );
}

#[test]
fn terminal_multihop_root_handoff_round_trips_and_rejects_lineage_drift() {
    let checked = multihop_reborrow_source("&mut", "&mut");
    let module = terminal_module_with_reborrow(&checked);
    assert_eq!(module.reborrow_root_handoffs[0].lineage.len(), 2);
    psi_terminal_verifier::validate_module(&module).expect("exact multihop handoff verifies");
    let encoded = psi_terminal_codec::encode_module(&module).expect("multihop handoff encodes");
    assert_eq!(
        psi_terminal_codec::decode_module(&encoded).expect("multihop handoff decodes"),
        module
    );

    let mut empty = module.clone();
    empty.reborrow_root_handoffs[0].lineage.clear();
    assert!(psi_terminal_verifier::validate_module(&empty).is_err());

    let mut amplified = module.clone();
    amplified.reborrow_root_handoffs[0].lineage[0].child_access =
        psi_terminal::StructuralAccess::WriteOnlyBorrow;
    amplified.reborrow_root_handoffs[0].lineage[1].child_access =
        psi_terminal::StructuralAccess::MutableBorrow;
    assert!(psi_terminal_verifier::validate_module(&amplified).is_err());

    let mut retargeted = module;
    retargeted.reborrow_root_handoffs[0].lineage[1]
        .projection_remainder
        .push(psi_terminal::TerminalBorrowPlaceSegment::FixedIndex(0));
    assert!(psi_terminal_verifier::validate_module(&retargeted).is_err());
}

#[test]
fn callback_custody_crosses_terminal_production_in_exact_order_and_returns_on_rejection() {
    let checked = checked_source(
        r#"
            data Main {}
            machine Main::launch() {}
        "#,
    );
    let custody = vec![(11u64, "first"), (29u64, "second")];
    let produced =
        produce_terminal_artifact_with_callback_custody(&checked, "Main::launch", custody.clone())
            .expect("opaque callback custody crosses canonical Terminal production");
    assert_eq!(produced.callback_custody(), &custody);
    produced.artifact().validate().expect("canonical artifact");
    let (_, _, returned, _) = produced.into_parts();
    assert_eq!(returned, custody);

    let swapped = vec![(29u64, "second"), (11u64, "first")];
    let produced =
        produce_terminal_artifact_with_callback_custody(&checked, "Main::launch", swapped.clone())
            .expect("opaque callback custody preserves caller-provided order");
    assert_eq!(produced.callback_custody(), &swapped);

    let rejected =
        produce_terminal_artifact_with_callback_custody(&checked, "Main::missing", custody.clone())
            .expect_err("missing Terminal machine rejects transactionally");
    assert!(matches!(
        rejected.error(),
        TerminalArtifactProductionError::Lowering(LoweringError::MachineNotFound(_))
    ));
    let (_, returned) = rejected.into_parts();
    assert_eq!(returned, custody);
}

#[test]
fn checked_boundary_operator_scope_rejects_terminal_artifact_substitution() {
    let first = checked_source(
        r#"
            data Main {}
            machine Main::launch() {}
        "#,
    );
    let produced =
        produce_terminal_artifact_with_checked_boundary_operator_scope(&first, "Main::launch")
            .expect("checked Terminal production");

    let second = checked_source(
        r#"
            data Helper {}
            machine Helper::touch() {}
            data Main {}
            machine Main::launch() { Helper::touch(); }
        "#,
    );
    let substituted = produce_terminal_artifact(&second, "Main::launch")
        .expect("distinct canonical Terminal artifact");
    assert!(
        produced
            .boundary_operator_scope()
            .validate_for_artifact(&substituted)
            .is_err()
    );
}

#[test]
fn checked_boundary_operator_scope_retains_the_complete_exact_demand_roster() {
    let demand_source = checked_source(
        r#"
            boundary operator == Number::equal(left: i32, right: i32) -> bool;

            machine launch(left: i32, right: i32) -> bool {
                left == right
            }
        "#,
    );
    let [expected] = demand_source
        .facts
        .operators
        .boundary_applications
        .as_slice()
    else {
        panic!("one exact checked boundary-operator demand")
    };
    let expected = expected.clone();
    // This milestone closes scope custody only. Source-free operation matching
    // remains the next D29/D32 join, so use an independently lowerable Terminal
    // fixture and verify that its companion retains the exact checked row.
    let mut checked = checked_source(
        r#"
            data Main {}
            machine Main::launch() {}
        "#,
    );
    checked.facts.operators.boundary_applications = vec![expected.clone()];
    let produced =
        produce_terminal_artifact_with_checked_boundary_operator_scope(&checked, "Main::launch")
            .expect("checked Terminal production retains exact D29 demand custody");

    assert_eq!(
        produced.boundary_operator_scope().applications(),
        std::slice::from_ref(&expected)
    );
    assert!(produced.boundary_operator_scope().occurrences().is_empty());
    assert!(!produced.boundary_operator_scope().is_empty());
}

#[test]
fn program_entry_receipt_binds_checked_source_to_canonical_terminal_entry() {
    let checked = checked_source(
        r#"
            data Main {}
            machine Main::launch() {}
        "#,
    );
    let source_signature_identity = [0x5a; 32];
    let produced = produce_program_entry_terminal_artifact(
        &checked,
        "Main::launch",
        source_signature_identity,
    )
    .expect("produce checked Unit ProgramEntry artifact");
    let receipt = produced.receipt();
    let decoded = psi_terminal_codec::decode_module(produced.artifact().semantic_bytes())
        .expect("decode canonical semantic module");

    assert_eq!(
        receipt.source_signature_identity(),
        source_signature_identity
    );
    assert_eq!(receipt.source_machine_name(), "Main::launch");
    assert_eq!(receipt.terminal_entry(), decoded.entry);
    assert_eq!(
        receipt.terminal_psi_identity(),
        produced.artifact().manifest().semantic()
    );
    assert!(
        decoded
            .machines
            .iter()
            .any(|machine| machine.id == receipt.terminal_entry()
                && machine.result == TerminalMachineResult::Unit)
    );
    produced
        .artifact()
        .validate()
        .expect("receipt-coupled artifact replays");
}

#[test]
fn program_entry_receipt_retains_two_granted_extent_roots_and_their_boundary_handoff() {
    let checked = checked_source(
        r#"
            data Extent [linear] {
                base: addr;
                length: u64;
            }

            boundary machine no_wrap(base: addr, length: u64) -> bool;

            domain Extent::Granted
            requires
                no_wrap(self.base, self.length)
            established by
                ProgramStorageEntry::enter;

            boundary trait ProgramStorageEntry {
                machine enter(
                    image: Extent in Granted,
                    initial_storage: Extent in Granted
                );
            }

            data ProgramLocalProducer {}
            machine ProgramLocalProducer::handoff<machine Enter>(
                image: Extent in Granted,
                initial_storage: Extent in Granted
            )
            where machine Enter satisfies ProgramStorageEntry::enter;
            {
                Enter(image, initial_storage);
            }
        "#,
    );
    let source_signature_identity = [0xa5; 32];
    let produced = produce_program_entry_terminal_artifact(
        &checked,
        "ProgramLocalProducer::handoff",
        source_signature_identity,
    )
    .expect("produce exact two-root Unit ProgramEntry artifact");
    let receipt = produced.receipt();
    let decoded = psi_terminal_codec::decode_module(produced.artifact().semantic_bytes())
        .expect("decode canonical two-root semantic module");

    assert_eq!(
        receipt.source_signature_identity(),
        source_signature_identity
    );
    assert_eq!(
        receipt.source_machine_name(),
        "ProgramLocalProducer::handoff"
    );
    assert_eq!(receipt.terminal_entry(), decoded.entry);
    assert_eq!(
        receipt.terminal_psi_identity(),
        produced.artifact().manifest().semantic()
    );

    let entry = decoded
        .machines
        .iter()
        .find(|machine| machine.id == receipt.terminal_entry())
        .expect("receipt names one retained Terminal entry");
    let [image, initial_storage] = entry.structural_parameters.as_slice() else {
        panic!("ProgramStorage handoff must retain two structural inputs")
    };
    assert_eq!((image.position, initial_storage.position), (0, 1));
    assert!(!image.is_self && !initial_storage.is_self);
    assert_ne!(image.place, initial_storage.place);
    assert_eq!(image.structural_type, initial_storage.structural_type);
    assert_eq!(image.multiplicity, StructuralMultiplicity::Linear);
    assert_eq!(initial_storage.multiplicity, StructuralMultiplicity::Linear);
    assert_eq!(image.access, StructuralAccess::Owned);
    assert_eq!(initial_storage.access, StructuralAccess::Owned);
    let [image_domain] = image.qualifications.as_slice() else {
        panic!("Image must retain exactly one qualification")
    };
    let [storage_domain] = initial_storage.qualifications.as_slice() else {
        panic!("InitialStorage must retain exactly one qualification")
    };
    assert_eq!(image_domain, storage_domain);
    let domain = decoded
        .structural_domains
        .iter()
        .find(|domain| domain.id == *image_domain)
        .expect("Granted domain declaration remains in the canonical artifact");
    assert_eq!(domain.identity, "Extent::Granted");
    assert_eq!(domain.carrier, image.structural_type);
    let carrier = decoded
        .structural_types
        .iter()
        .find(|declaration| declaration.id == image.structural_type)
        .expect("Extent carrier declaration remains in the canonical artifact");
    assert_eq!(carrier.identity, "named(name(Extent))");
    let StructuralTypeShape::Record { fields } = &carrier.shape else {
        panic!("Extent carrier must remain a record")
    };
    assert!(matches!(fields.as_slice(), [base, length]
        if base.identity == "base"
            && base.relevance == BindingRelevance::Relevant
            && matches!(base.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.is_address())
            && length.identity == "length"
            && length.relevance == BindingRelevance::Relevant
            && matches!(length.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.sign() == psi_core::IntegerSign::Unsigned && integer.bits() == 64)));

    assert!(
        matches!(entry.structural_places.as_slice(), [image_place, storage_place]
        if image_place.id == image.place
            && image_place.kind == StructuralPlaceKind::Parameter { position: 0, is_self: false }
            && storage_place.id == initial_storage.place
            && storage_place.kind == StructuralPlaceKind::Parameter { position: 1, is_self: false })
    );
    let [image_claim, storage_claim] = entry.entry_claims.as_slice() else {
        panic!("ProgramStorage handoff must retain two entry claims")
    };
    assert_eq!(image_claim.input, image.place);
    assert_eq!(storage_claim.input, initial_storage.place);
    assert!(image_claim.path.is_empty() && storage_claim.path.is_empty());
    let [block] = entry.blocks.as_slice() else {
        panic!("ProgramStorage handoff must remain straight-line")
    };
    let [call] = block.operations.as_slice() else {
        panic!("ProgramStorage handoff must retain one Unit call")
    };
    let OperationKind::BoundaryCall {
        boundary,
        structural_arguments,
        completion_receipts,
        ..
    } = &call.kind
    else {
        panic!("ProgramStorage handoff operation must remain BoundaryCall")
    };
    let boundary = decoded
        .boundary_machines
        .iter()
        .find(|candidate| candidate.id == *boundary)
        .expect("generic ProgramStorage requirement remains a bodyless boundary");
    assert_eq!(boundary.structural_parameters.len(), 2);
    assert_eq!(boundary.structural_parameters[0].position, 0);
    assert_eq!(boundary.structural_parameters[1].position, 1);
    assert!(
        matches!(structural_arguments.as_slice(), [image_argument, storage_argument]
        if image_argument.place == image.place
            && image_argument.access == StructuralAccess::Owned
            && image_argument.path.is_empty()
            && storage_argument.place == initial_storage.place
            && storage_argument.access == StructuralAccess::Owned
            && storage_argument.path.is_empty())
    );
    assert!(
        matches!(completion_receipts.as_slice(), [image_receipt, storage_receipt]
        if image_receipt.claim == image_claim.claim
            && image_receipt.argument_index == 0
            && storage_receipt.claim == storage_claim.claim
            && storage_receipt.argument_index == 1)
    );
    assert!(matches!(
        block.terminator,
        Terminator::ReturnUnit {
            ref trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ));
    produced
        .artifact()
        .validate()
        .expect("two-root receipt-coupled artifact replays");
}

#[test]
fn program_entry_receipt_rejects_a_scalar_result_machine() {
    let checked = checked_source(
        r#"
            data Helper {}
            machine Helper::touch() {}
            data Token { value: u64; }
            machine Token::drop(&mut self) { Helper::touch(); }
            data Main {}
            machine Main::launch(token: Token) -> u64 { 7u64 }
        "#,
    );
    let error = produce_program_entry_terminal_artifact(&checked, "Main::launch", [0x11; 32])
        .expect_err("ProgramEntry receipt requires a Unit result");
    assert!(
        matches!(
            &error,
            TerminalArtifactProductionError::EntryReceipt(
                ProgramEntryTerminalReceiptError::NonUnitEntry
            )
        ),
        "unexpected receipt rejection: {error:?}"
    );
}

fn checked_write_line_literal() -> psi_checked_trees::CheckedTrees {
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn lowers_exact_raw_bytes_into_borrowed_boundary_argument() {
    let checked = checked_write_line_literal();
    let lowered = lower_machine(&checked, "Root::enter").expect("lower write_line literal");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("one source machine")
    };
    let literal_place = machine
        .structural_places
        .iter()
        .find_map(|place| {
            matches!(
                place.kind,
                StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: 0,
                    ..
                }
            )
            .then_some(place.id)
        })
        .expect("canonical byte-sequence literal place");
    let [establish, call] = machine.blocks[0].operations.as_slice() else {
        panic!("literal establishment then boundary call")
    };
    assert!(matches!(
        &establish.kind,
        OperationKind::EstablishByteSequenceLiteral { destination, bytes }
            if *destination == literal_place && bytes == &[0x80, b'A']
    ));
    assert!(matches!(
        &call.kind,
        OperationKind::BoundaryCall { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.place == literal_place && argument.path.is_empty())
    ));
    let literal_type = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
            )
        })
        .expect("borrowed-view declaration");
    assert!(machine.structural_places.iter().any(|place| matches!(
        place.kind,
        StructuralPlaceKind::ByteSequenceLiteral { structural_type, .. }
            if structural_type == literal_type.id
    )));
}

#[test]
fn affine_i64_record_literal_crosses_source_codec_and_verification() {
    let checked = checked_source(
        r#"
        data Packet { value: i64; }

        data Sink {}
        machine Sink::accept(packet: Packet) {}

        data Root {}
        machine Root::enter() {
            let packet: Packet = Packet { value: 7 };
            Sink::accept(move packet);
        }
        "#,
    );
    let lowered = lower_machine(&checked, "Root::enter").expect("lower affine scalar record");
    let module = &lowered.semantic_module;
    let caller = module.machines.first().expect("caller machine");
    let [establish, call] = caller.blocks[0].operations.as_slice() else {
        panic!("record establishment followed by owned call")
    };
    let result = establish
        .result
        .structural()
        .expect("constructor establishes one structural result");
    assert_eq!(result.multiplicity, StructuralMultiplicity::Affine);
    assert!(result.qualifications.is_empty());
    assert!(result.projected_qualifications.is_empty());
    assert!(result.claims.is_empty());
    assert!(matches!(
        establish.kind,
        OperationKind::EstablishAffineScalarRecord {
            value: IntegerValue::Signed(7),
            ..
        }
    ));
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.place == result.place
                    && argument.path.is_empty()
                    && argument.access == StructuralAccess::Owned)
    ));

    let encoded = psi_terminal_codec::encode_module(module).expect("encode affine scalar record");
    let decoded = psi_terminal_codec::decode_module(&encoded).expect("decode affine scalar record");
    assert_eq!(&decoded, module);
    psi_terminal_verifier::validate_module(&decoded).expect("verify affine scalar record");

    let mut forged = decoded;
    let OperationKind::EstablishAffineScalarRecord { value, .. } =
        &mut forged.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(7);
    assert!(psi_terminal_verifier::validate_module(&forged).is_err());
}

#[test]
fn mutable_to_write_only_access_crosses_source_codec_and_verification() {
    let source = r#"
        data Sink {}
        machine Sink::fill(destination: &write [u8]) {}

        data Root {}
        machine Root::enter(bytes: &mut [u8]) {
            Sink::fill(&write bytes);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "Root::enter").expect("lower write-only forwarding");
    let module = &lowered.semantic_module;

    assert_eq!(
        module.machines[0].structural_parameters[0].access,
        StructuralAccess::MutableBorrow
    );
    assert_eq!(
        module.machines[1].structural_parameters[0].access,
        StructuralAccess::WriteOnlyBorrow
    );
    let [call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("root emits one forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.access == StructuralAccess::WriteOnlyBorrow)
    ));

    let encoded = psi_terminal_codec::encode_module(module).expect("encode access-bearing module");
    let decoded =
        psi_terminal_codec::decode_module(&encoded).expect("decode access-bearing module");
    assert_eq!(&decoded, module);
    psi_terminal_verifier::validate_module(&decoded).expect("verify write-only attenuation");
}

#[test]
fn direct_write_only_primitive_store_crosses_source_codec_and_verification() {
    let checked = checked_source(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write i32) {
                destination = 2;
            }

            data Root {}
            machine Root::enter(destination: &mut i32) {
                Sink::fill(&write destination);
            }
        "#,
    );
    let lowered = lower_machine(&checked, "Root::enter").expect("lower primitive store closure");
    let module = &lowered.semantic_module;
    let [caller, callee] = module.machines.as_slice() else {
        panic!("caller and write-only callee are retained")
    };
    let [caller_parameter] = caller.structural_parameters.as_slice() else {
        panic!("caller retains one primitive referent")
    };
    let [callee_parameter] = callee.structural_parameters.as_slice() else {
        panic!("callee retains one primitive referent")
    };
    assert_eq!(caller_parameter.access, StructuralAccess::MutableBorrow);
    assert_eq!(callee_parameter.access, StructuralAccess::WriteOnlyBorrow);
    assert!(matches!(
        module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == callee_parameter.structural_type)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(_)))
    ));

    let [constant, store] = callee.blocks[0].operations.as_slice() else {
        panic!("callee emits one constant followed by one store")
    };
    let stored_value = constant.result.expect_scalar().id;
    assert!(matches!(
        constant.kind,
        OperationKind::IntegerConstant { .. }
    ));
    assert!(matches!(
        store.kind,
        OperationKind::WriteOnlyPrimitiveStore { destination, value }
            if destination == callee_parameter.place && value == stored_value
    ));
    assert_eq!(store.result, OperationResult::Unit);

    let encoded = psi_terminal_codec::encode_module(module).expect("encode primitive store");
    let decoded = psi_terminal_codec::decode_module(&encoded).expect("decode primitive store");
    assert_eq!(&decoded, module);
    psi_terminal_verifier::validate_module(&decoded).expect("verify primitive store");

    let mut widened = decoded.clone();
    widened.machines[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    psi_terminal_verifier::validate_module(&widened)
        .expect_err("a primitive store requires exact write-only access");

    let mut undefined = decoded;
    let OperationKind::WriteOnlyPrimitiveStore { value, .. } =
        &mut undefined.machines[1].blocks[0].operations[1].kind
    else {
        panic!("store operation")
    };
    *value = ValueId::new(u64::MAX).expect("nonzero undefined value");
    psi_terminal_verifier::validate_module(&undefined)
        .expect_err("a primitive store value must be defined and dominating");
}

#[test]
fn direct_write_only_boolean_store_crosses_source_codec_and_verification() {
    let checked = checked_source(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write bool) {
                destination = true;
            }

            data Root {}
            machine Root::enter(destination: &mut bool) {
                Sink::fill(&write destination);
            }
        "#,
    );
    let lowered = lower_machine(&checked, "Root::enter").expect("lower Boolean store closure");
    let module = &lowered.semantic_module;
    let [_, callee] = module.machines.as_slice() else {
        panic!("caller and write-only callee are retained")
    };
    let [callee_parameter] = callee.structural_parameters.as_slice() else {
        panic!("callee retains one primitive referent")
    };
    assert!(matches!(
        module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == callee_parameter.structural_type)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean))
    ));
    let [constant, store] = callee.blocks[0].operations.as_slice() else {
        panic!("callee emits one Boolean constant followed by one store")
    };
    let stored_value = constant.result.expect_scalar().id;
    assert!(matches!(
        constant.kind,
        OperationKind::BooleanConstant { value: true }
    ));
    assert!(matches!(
        store.kind,
        OperationKind::WriteOnlyPrimitiveStore { destination, value }
            if destination == callee_parameter.place && value == stored_value
    ));

    let encoded = psi_terminal_codec::encode_module(module).expect("encode Boolean store");
    let decoded = psi_terminal_codec::decode_module(&encoded).expect("decode Boolean store");
    assert_eq!(&decoded, module);
    psi_terminal_verifier::validate_module(&decoded).expect("verify Boolean store");

    let mut wrong_type = decoded;
    let declaration = wrong_type
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == callee_parameter.structural_type)
        .expect("Boolean structural declaration");
    declaration.shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
    ));
    psi_terminal_verifier::validate_module(&wrong_type)
        .expect_err("Boolean store value must match its exact referent type");
}

#[test]
fn write_only_common_field_subloan_crosses_source_codec_and_verification() {
    let source = r#"
        data Leaf [copy] { value: u16; }
        data Inner [copy] { leaf: Leaf; sibling: u16; }
        data Outer [copy] { inner: Inner; other: Inner; }

        data Sink {}
        machine Sink::fill(destination: &write Leaf) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.inner.leaf);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "Root::forward").expect("lower projected forwarding");
    let module = &lowered.semantic_module;

    assert_eq!(
        module.machines[0].structural_parameters[0].access,
        StructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(
        module.machines[1].structural_parameters[0].access,
        StructuralAccess::WriteOnlyBorrow
    );
    let [call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("projected caller emits one forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.access == StructuralAccess::WriteOnlyBorrow
                    && argument.path.len() == 2
                    && argument.path.iter().all(|segment| matches!(
                        segment,
                        StructuralPathSegment::Field(_)
                    )))
    ));

    let encoded = psi_terminal_codec::encode_module(module).expect("encode projected module");
    let decoded = psi_terminal_codec::decode_module(&encoded).expect("decode projected module");
    assert_eq!(&decoded, module);
    psi_terminal_verifier::validate_module(&decoded).expect("verify projected write-only subloan");

    let mut path_drifted = decoded.clone();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut path_drifted.machines[0].blocks[0].operations[0].kind
    else {
        panic!("projected caller call")
    };
    structural_arguments[0].path[1] = structural_arguments[0].path[0].clone();
    psi_terminal_verifier::validate_module(&path_drifted)
        .expect_err("a redirected common-field identity must reject");

    let mut target_type_drifted = decoded.clone();
    target_type_drifted.machines[1].structural_parameters[0].structural_type =
        target_type_drifted.machines[0].structural_parameters[0].structural_type;
    psi_terminal_verifier::validate_module(&target_type_drifted)
        .expect_err("the projected leaf must match the callee's exact structural type");

    let mut target_access_drifted = decoded.clone();
    target_access_drifted.machines[1].structural_parameters[0].access =
        StructuralAccess::MutableBorrow;
    psi_terminal_verifier::validate_module(&target_access_drifted)
        .expect_err("the projected leaf must match the callee's exact access");

    let mut access_drifted = decoded;
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut access_drifted.machines[0].blocks[0].operations[0].kind
    else {
        panic!("projected caller call")
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    psi_terminal_verifier::validate_module(&access_drifted)
        .expect_err("a projected write-only argument cannot widen to shared access");
}

#[test]
fn direct_root_literal_indexed_write_only_subloan_crosses_codec_and_verification() {
    let source = r#"
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(values: &write [u16; 2]) {
            Sink::fill(&write values[1]);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered =
        lower_machine(&checked, "Root::forward").expect("lower direct indexed forwarding");
    let module = &lowered.semantic_module;

    let [call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("direct indexed caller emits one forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.access == StructuralAccess::WriteOnlyBorrow
                    && argument.path == [StructuralPathSegment::FixedIndex(1)])
    ));

    let encoded = psi_terminal_codec::encode_module(module).expect("encode indexed module");
    let decoded = psi_terminal_codec::decode_module(&encoded).expect("decode indexed module");
    assert_eq!(&decoded, module);
    psi_terminal_verifier::validate_module(&decoded)
        .expect("verify direct indexed write-only subloan");

    let mutate_path = |module: &mut TerminalModule,
                       mutation: &dyn Fn(&mut Vec<StructuralPathSegment>)| {
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            panic!("direct indexed caller call")
        };
        mutation(&mut structural_arguments[0].path);
    };

    let mut out_of_bounds = decoded.clone();
    mutate_path(&mut out_of_bounds, &|path| {
        path[0] = StructuralPathSegment::FixedIndex(2);
    });
    psi_terminal_verifier::validate_module(&out_of_bounds)
        .expect_err("an out-of-bounds direct subloan index must reject");

    let mut missing_index = decoded.clone();
    mutate_path(&mut missing_index, &|path| path.clear());
    psi_terminal_verifier::validate_module(&missing_index)
        .expect_err("omitting the direct subloan coordinate must reject");

    let mut duplicated_index = decoded.clone();
    mutate_path(&mut duplicated_index, &|path| {
        path.push(StructuralPathSegment::FixedIndex(0));
    });
    psi_terminal_verifier::validate_module(&duplicated_index)
        .expect_err("duplicating the direct subloan coordinate must reject");

    let mut source_access_drifted = decoded.clone();
    source_access_drifted.machines[0].structural_parameters[0].access = StructuralAccess::Owned;
    psi_terminal_verifier::validate_module(&source_access_drifted)
        .expect_err("a direct indexed subloan requires an exact write-only source");

    let mut target_access_drifted = decoded.clone();
    target_access_drifted.machines[1].structural_parameters[0].access =
        StructuralAccess::MutableBorrow;
    psi_terminal_verifier::validate_module(&target_access_drifted)
        .expect_err("a direct indexed subloan cannot widen its target access");

    let mut target_type_drifted = decoded.clone();
    target_type_drifted.machines[1].structural_parameters[0].structural_type =
        target_type_drifted.machines[0].structural_parameters[0].structural_type;
    psi_terminal_verifier::validate_module(&target_type_drifted)
        .expect_err("a direct indexed subloan must retain the exact element type");

    let mut target_multiplicity_drifted = decoded;
    target_multiplicity_drifted.machines[1].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Linear;
    psi_terminal_verifier::validate_module(&target_multiplicity_drifted)
        .expect_err("a direct indexed subloan cannot become linear");
}

#[test]
fn two_index_write_only_subloan_crosses_source_codec_and_verification() {
    let source = r#"
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(values: &write [[u16; 3]; 2]) {
            Sink::fill(&write values[1][2]);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "Root::forward").expect("lower two-index forwarding");
    let module = &lowered.semantic_module;

    let [call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("two-index caller emits one forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.access == StructuralAccess::WriteOnlyBorrow
                    && argument.path == [
                        StructuralPathSegment::FixedIndex(1),
                        StructuralPathSegment::FixedIndex(2),
                    ])
    ));

    let encoded = psi_terminal_codec::encode_module(module).expect("encode two-index module");
    let decoded = psi_terminal_codec::decode_module(&encoded).expect("decode two-index module");
    assert_eq!(&decoded, module);
    psi_terminal_verifier::validate_module(&decoded).expect("verify two-index write-only subloan");

    let mutate_path = |module: &mut TerminalModule,
                       mutation: &dyn Fn(&mut Vec<StructuralPathSegment>)| {
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            panic!("two-index caller call")
        };
        mutation(&mut structural_arguments[0].path);
    };

    let mut outer_out_of_bounds = decoded.clone();
    mutate_path(&mut outer_out_of_bounds, &|path| {
        path[0] = StructuralPathSegment::FixedIndex(2);
    });
    psi_terminal_verifier::validate_module(&outer_out_of_bounds)
        .expect_err("an out-of-bounds outer subloan index must reject");

    let mut inner_out_of_bounds = decoded.clone();
    mutate_path(&mut inner_out_of_bounds, &|path| {
        path[1] = StructuralPathSegment::FixedIndex(3);
    });
    psi_terminal_verifier::validate_module(&inner_out_of_bounds)
        .expect_err("an out-of-bounds inner subloan index must reject");

    let mut missing_inner = decoded.clone();
    mutate_path(&mut missing_inner, &|path| {
        path.pop();
    });
    psi_terminal_verifier::validate_module(&missing_inner)
        .expect_err("omitting the inner coordinate must reject exact target rejoin");

    let mut third_index = decoded.clone();
    mutate_path(&mut third_index, &|path| {
        path.push(StructuralPathSegment::FixedIndex(0));
    });
    psi_terminal_verifier::validate_module(&third_index)
        .expect_err("a third write-only subloan index must remain fenced");

    let mut source_access_drifted = decoded.clone();
    source_access_drifted.machines[0].structural_parameters[0].access = StructuralAccess::Owned;
    psi_terminal_verifier::validate_module(&source_access_drifted)
        .expect_err("a two-index subloan requires exact write-only source access");

    let mut target_multiplicity_drifted = decoded;
    target_multiplicity_drifted.machines[1].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Linear;
    psi_terminal_verifier::validate_module(&target_multiplicity_drifted)
        .expect_err("a two-index subloan cannot become linear");
}

#[test]
fn field_prefixed_two_index_write_only_subloan_crosses_terminal() {
    let source = r#"
        data Outer [copy] { values: [[u16; 3]; 2]; sibling: u16; }
        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.values[1][2]);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "Root::forward")
        .expect("lower field-prefixed two-index forwarding");
    let module = &lowered.semantic_module;

    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &module.machines[0].blocks[0].operations[0].kind
    else {
        panic!("field-prefixed two-index forwarding call")
    };
    assert!(matches!(
        structural_arguments[0].path.as_slice(),
        [
            StructuralPathSegment::Field(_),
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::FixedIndex(2),
        ]
    ));
    let encoded = psi_terminal_codec::encode_module(module).expect("encode field-prefixed module");
    let decoded =
        psi_terminal_codec::decode_module(&encoded).expect("decode field-prefixed module");
    assert_eq!(&decoded, module);
    psi_terminal_verifier::validate_module(&decoded)
        .expect("verify field-prefixed two-index write-only subloan");
}

#[test]
fn literal_indexed_write_only_subloan_crosses_source_codec_and_verification() {
    let source = r#"
        data Inner [copy] { values: [u16; 2]; sibling: u16; }
        data Outer [copy] { inner: Inner; other: Inner; }

        data Sink {}
        machine Sink::fill(destination: &write u16) {}

        data Root {}
        machine Root::forward(outer: &write Outer) {
            Sink::fill(&write outer.inner.values[1]);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered =
        lower_machine(&checked, "Root::forward").expect("lower literal-indexed forwarding");
    let module = &lowered.semantic_module;

    let [call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("literal-indexed caller emits one forwarding call")
    };
    assert!(matches!(
        &call.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if matches!(structural_arguments.as_slice(), [argument]
                if argument.access == StructuralAccess::WriteOnlyBorrow
                    && matches!(argument.path.as_slice(), [
                        StructuralPathSegment::Field(_),
                        StructuralPathSegment::Field(_),
                        StructuralPathSegment::FixedIndex(1),
                    ]))
    ));

    let encoded = psi_terminal_codec::encode_module(module).expect("encode indexed module");
    let decoded = psi_terminal_codec::decode_module(&encoded).expect("decode indexed module");
    assert_eq!(&decoded, module);
    psi_terminal_verifier::validate_module(&decoded).expect("verify indexed write-only subloan");

    let mutate_path = |module: &mut TerminalModule,
                       mutation: &dyn Fn(&mut Vec<StructuralPathSegment>)| {
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            panic!("literal-indexed caller call")
        };
        mutation(&mut structural_arguments[0].path);
    };

    let mut out_of_bounds = decoded.clone();
    mutate_path(&mut out_of_bounds, &|path| {
        *path.last_mut().expect("index") = StructuralPathSegment::FixedIndex(2);
    });
    psi_terminal_verifier::validate_module(&out_of_bounds)
        .expect_err("an out-of-bounds literal subloan index must reject");

    let mut missing_index = decoded.clone();
    mutate_path(&mut missing_index, &|path| {
        path.pop();
    });
    psi_terminal_verifier::validate_module(&missing_index)
        .expect_err("omitting the indexed subloan coordinate must reject");

    let mut duplicated_index = decoded.clone();
    mutate_path(&mut duplicated_index, &|path| {
        path.push(StructuralPathSegment::FixedIndex(1));
    });
    psi_terminal_verifier::validate_module(&duplicated_index)
        .expect_err("duplicating the indexed subloan coordinate must reject");

    let mut reordered_index = decoded.clone();
    mutate_path(&mut reordered_index, &|path| {
        path.rotate_right(1);
    });
    psi_terminal_verifier::validate_module(&reordered_index)
        .expect_err("moving the index before its field path must reject");

    let mut source_access_drifted = decoded.clone();
    source_access_drifted.machines[0].structural_parameters[0].access = StructuralAccess::Owned;
    psi_terminal_verifier::validate_module(&source_access_drifted)
        .expect_err("an indexed subloan requires exact write-only source access");

    let mut target_multiplicity_drifted = decoded.clone();
    target_multiplicity_drifted.machines[1].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Linear;
    psi_terminal_verifier::validate_module(&target_multiplicity_drifted)
        .expect_err("an indexed subloan cannot become a linear projected call");

    let mut field_drifted = decoded;
    mutate_path(&mut field_drifted, &|path| {
        path[1] = path[0].clone();
    });
    psi_terminal_verifier::validate_module(&field_drifted)
        .expect_err("a redirected indexed-subloan field identity must reject");
}

#[test]
fn rejects_tampered_owned_carrier_for_source_literal() {
    let mut checked = checked_write_line_literal();
    let literal_type = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter_mut()
        .find(|plan| {
            matches!(
                plan.shape,
                psi_checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(_)
            )
        })
        .expect("literal type");
    literal_type.shape = psi_checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
        psi_checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity: 2 },
    );
    let error = lower_machine(&checked, "Root::enter")
        .expect_err("an owned carrier must not establish a borrowed source literal");
    assert!(
        error.to_string().contains("requires a borrowed-view type"),
        "{error}"
    );
}

#[test]
fn bounded_installation_reach_lowers_source_free_terminal_dependency() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        boundary trait InterruptCompletion {
            machine complete()
            reaches <= MachineControl + PortIo;
        }

        machine pic_complete()
        satisfies InterruptCompletion::complete
        reaches PortIo
        { }

        machine invoke<machine Completion>()
        where machine Completion satisfies InterruptCompletion::complete;
        { Completion(); }

    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let root = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke")
        .expect("generic invocation machine");
    let service_ids = ["MachineControl", "PortIo"]
        .iter()
        .enumerate()
        .map(|(index, name)| {
            (
                checked
                    .facts
                    .service_reaches
                    .services
                    .id_for_name(name)
                    .expect("service exists"),
                service_id(u64::try_from(index).expect("service index") + 1),
            )
        })
        .collect::<Vec<_>>();
    let closure = lower_root_service_reach(&checked, root.symbol, &service_ids)
        .expect("lower root service reach");
    assert!(closure.concrete.is_empty());
    let [dependency] = closure.installation_dependencies.as_slice() else {
        panic!("terminal root must retain one installation reach dependency");
    };
    assert!(
        dependency
            .requirement_identity
            .contains("InterruptCompletion::complete")
    );
    let bound_names = dependency
        .upper_bound
        .iter()
        .map(|id| {
            service_ids
                .iter()
                .find(|(_, terminal)| terminal == id)
                .and_then(|(source, _)| checked.facts.service_reaches.services.definition(*source))
                .expect("bound service is declared")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(bound_names, ["MachineControl", "PortIo"]);
}

#[test]
fn top_level_bounded_reach_lowers_normalized_machine_identity() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        boundary machine InterruptAcknowledgement::complete()
        reaches <= MachineControl + PortIo;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let requirement = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("top-level completion requirement");
    let expected_identity = typed
        .normalized_machine_overload_identity(requirement)
        .expect("normalized top-level requirement")
        .identity();
    let requirement_symbol = requirement.symbol;
    let checked = lower_typed_trees(typed).expect("check");
    let service_ids = ["MachineControl", "PortIo"]
        .iter()
        .enumerate()
        .map(|(index, name)| {
            (
                checked
                    .facts
                    .service_reaches
                    .services
                    .id_for_name(name)
                    .expect("service exists"),
                service_id(u64::try_from(index).expect("service index") + 1),
            )
        })
        .collect::<Vec<_>>();
    let closure = lower_root_service_reach(&checked, requirement_symbol, &service_ids)
        .expect("lower top-level requirement reach");
    assert!(closure.concrete.is_empty());
    let [dependency] = closure.installation_dependencies.as_slice() else {
        panic!("top-level requirement must retain one installation dependency");
    };
    assert_eq!(dependency.requirement_identity, expected_identity);
    let bound_names = dependency
        .upper_bound
        .iter()
        .map(|id| {
            service_ids
                .iter()
                .find(|(_, terminal)| terminal == id)
                .and_then(|(source, _)| checked.facts.service_reaches.services.definition(*source))
                .expect("bound service is declared")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(bound_names, ["MachineControl", "PortIo"]);
}

#[test]
fn actual_float_meaning_calls_emit_deduplicated_source_free_module_rows() {
    let source = r#"
        machine prove_projection(value32: f32, value64: f64)
        requires
            Float::meaning32(value32) == Float::meaning32(value32);
            Float::meaning64(value64) == Float::meaning64(value64);
        { }

        machine terminal_root(value: bool) -> bool
        requires
            true == true;
        ensures
            true == true;
        { value }
    "#;
    let checked = checked_float_projection_source(source);
    let lowered = lower_machine(&checked, "terminal_root").expect("lower");
    let projections = &lowered.semantic_module.float_meaning_projections;
    assert_eq!(projections.len(), 2);
    assert_eq!(projections[0].result.id, psi_terminal::ProofValueId(0));
    assert_eq!(
        projections[0].source,
        psi_terminal::FloatMeaningSource::TransitionalInput(psi_terminal::FloatProjectionInput {
            id: psi_terminal::FloatProjectionInputId(0),
            format: psi_core::IeeeFloatFormat::Binary32,
        })
    );
    assert_eq!(
        projections[0].operation,
        psi_terminal::FloatMeaningProjectionOperation::Meaning32
    );
    assert_eq!(
        projections[1].operation,
        psi_terminal::FloatMeaningProjectionOperation::Meaning64
    );
    assert_eq!(
        projections[1].source,
        psi_terminal::FloatMeaningSource::TransitionalInput(psi_terminal::FloatProjectionInput {
            id: psi_terminal::FloatProjectionInputId(1),
            format: psi_core::IeeeFloatFormat::Binary64,
        })
    );
    assert_eq!(
        lowered.semantic_module.float_meaning_equalities,
        vec![
            psi_terminal::FloatMeaningEqualityProposition {
                id: psi_terminal::ProofPropositionId(0),
                left: psi_terminal::ProofValueId(0),
                right: psi_terminal::ProofValueId(0),
            },
            psi_terminal::FloatMeaningEqualityProposition {
                id: psi_terminal::ProofPropositionId(1),
                left: psi_terminal::ProofValueId(1),
                right: psi_terminal::ProofValueId(1),
            },
        ]
    );
    psi_terminal_verifier::validate_module(&lowered.semantic_module).expect("verify");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module)
    );
}

#[test]
fn emitted_direct_float_parameter_rejoins_terminal_owner_and_dense_scalar_parameter() {
    let source = r#"
        data Token { value: i32; }
        data Root {}

        machine Root::forward(token: Token, value: f32)
        requires
            Float::meaning32(value) == Float::meaning32(value);
        {
            transition { _ -> done(token, value) }
            state done(token: Token, value: f32) {}
        }
    "#;
    let checked = checked_float_projection_source(source);
    let lowered = lower_machine(&checked, "Root::forward").expect("lower direct float owner");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [parameter] = machine.parameters.as_slice() else {
        panic!("the structural source position is excluded from the scalar parameter table")
    };
    assert_eq!(
        parameter.scalar_type,
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)
    );
    assert_eq!(
        lowered.semantic_module.float_meaning_projections[0].source,
        psi_terminal::FloatMeaningSource::DirectMachineParameter(
            psi_terminal::DirectMachineFloatParameter {
                owner: machine.id,
                parameter: parameter.id,
                format: IeeeFloatFormat::Binary32,
            }
        )
    );
    psi_terminal_verifier::validate_module(&lowered.semantic_module).expect("verify direct source");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module)
    );
}

#[test]
fn emitted_direct_structural_float_leaf_rejoins_owner_root_and_member_path() {
    let source = r#"
        data Sample { value: f32; }
        data Root {}

        machine Root::structural_source(sample: Sample)
        requires
            Float::meaning32(sample.value) == Float::meaning32(sample.value);
        {
            transition { _ -> done(sample) }
            state done(sample: Sample) {}
        }
    "#;
    let checked = checked_float_projection_source(source);
    let lowered = lower_machine(&checked, "Root::structural_source")
        .expect("lower direct structural float owner");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [parameter] = machine.structural_parameters.as_slice() else {
        panic!("one structural parameter expected")
    };
    let [projection] = lowered.semantic_module.float_meaning_projections.as_slice() else {
        panic!("one structural FloatMeaning projection expected")
    };
    let psi_terminal::FloatMeaningSource::DirectStructuralLeaf(leaf) = &projection.source else {
        panic!("structural source should retain an exact Terminal leaf")
    };
    assert_eq!(leaf.owner, machine.id);
    assert_eq!(leaf.field.root(), parameter.place);
    assert_eq!(leaf.field.path().len(), 1);
    assert_eq!(leaf.format, IeeeFloatFormat::Binary32);
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("verify direct structural source");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module)
    );

    let mut path_drift = checked;
    let psi_checked_trees::CheckedFloatProjectionSource::DirectStructuralLeaf(leaf) =
        &mut path_drift.facts.proof.float_meaning_projections[0].source
    else {
        panic!("checked structural source expected")
    };
    leaf.field.path[0] =
        psi_checked_trees::CheckedStructuralPredicatePathSegment::Field("missing".to_owned());
    assert!(lower_machine(&path_drift, "Root::structural_source").is_err());
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
        psi_terminal::FloatMeaningSource::DirectMachineResult(
            psi_terminal::DirectMachineFloatResult {
                owner: machine.id,
                result: result.id,
                format,
            }
        )
    );
    assert_eq!(
        lowered.semantic_module.float_meaning_equalities,
        vec![psi_terminal::FloatMeaningEqualityProposition {
            id: psi_terminal::ProofPropositionId(0),
            left: projection.result.id,
            right: projection.result.id,
        }]
    );
    psi_terminal_verifier::validate_module(&lowered.semantic_module).expect("verify direct result");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module)
    );
}

#[test]
fn source_direct_float_results_rejoin_terminal_owner_and_scalar_result() {
    assert_source_direct_float_result("f32", "meaning32", IeeeFloatFormat::Binary32);
    assert_source_direct_float_result("f64", "meaning64", IeeeFloatFormat::Binary64);
}

#[test]
fn direct_float_result_proof_only_contract_rejects_additional_value_clauses() {
    let checked = checked_float_projection_source(
        r#"
            machine result(value: f32) -> f32
            requires
                Float::meaning32(value) == Float::meaning32(value);
            ensures
                Float::meaning32(result) == Float::meaning32(result);
            { value }
        "#,
    );
    assert!(matches!(
        lower_machine(&checked, "result"),
        Err(LoweringError::Unsupported(
            "machine must have exactly one requires and one ensures clause"
        ))
    ));
}

#[test]
fn direct_float_result_proof_only_contract_replays_expression_and_owner() {
    let source = r#"
        machine result(value: f32) -> f32
        ensures
            Float::meaning32(result) == Float::meaning32(result);
        { value }

        machine other(value: f32) -> f32 { value }
    "#;
    let checked = checked_float_projection_source(source);

    let mut expression_drift = checked.clone();
    expression_drift.facts.proof.float_meaning_equalities[0].source_expression =
        psi_typed_trees::expression::ExpressionHandle::invalid();
    assert!(lower_machine(&expression_drift, "result").is_err());

    let mut owner_drift = checked;
    let other = owner_drift
        .machines()
        .iter()
        .find(|machine| owner_drift.symbols.name(machine.symbol) == "other")
        .expect("other machine")
        .symbol;
    let psi_checked_trees::CheckedFloatProjectionSource::DirectMachineResult(result) =
        &mut owner_drift.facts.proof.float_meaning_projections[0].source
    else {
        panic!("direct result source expected")
    };
    result.owner_machine = other;
    assert!(lower_machine(&owner_drift, "result").is_err());
}

#[test]
fn exact_float_literals_cross_checked_terminal_codec_and_verifier_as_raw_bits() {
    let source = r#"
        machine prove_projection()
        requires
            Float::meaning32(0.0f32) == Float::meaning32(0.00f32);
            Float::meaning32(-0.0f32) == Float::meaning32(-0.00f32);
            Float::meaning64(0.1f64) == Float::meaning64(0.10f64);
        { }

        machine terminal_root(value: bool) -> bool
        requires
            true == true;
        ensures
            true == true;
        { value }
    "#;
    let checked = checked_float_projection_source(source);
    let lowered = lower_machine(&checked, "terminal_root").expect("lower");
    assert_eq!(
        lowered
            .semantic_module
            .float_meaning_projections
            .iter()
            .map(|projection| projection.source.clone())
            .collect::<Vec<_>>(),
        vec![
            psi_terminal::FloatMeaningSource::ExactBinary32Literal(0x0000_0000),
            psi_terminal::FloatMeaningSource::ExactBinary32Literal(0x8000_0000),
            psi_terminal::FloatMeaningSource::ExactBinary64Literal(0.1_f64.to_bits()),
        ]
    );
    assert_eq!(
        lowered.semantic_module.float_meaning_equalities,
        vec![
            psi_terminal::FloatMeaningEqualityProposition {
                id: psi_terminal::ProofPropositionId(0),
                left: psi_terminal::ProofValueId(0),
                right: psi_terminal::ProofValueId(0),
            },
            psi_terminal::FloatMeaningEqualityProposition {
                id: psi_terminal::ProofPropositionId(1),
                left: psi_terminal::ProofValueId(1),
                right: psi_terminal::ProofValueId(1),
            },
            psi_terminal::FloatMeaningEqualityProposition {
                id: psi_terminal::ProofPropositionId(2),
                left: psi_terminal::ProofValueId(2),
                right: psi_terminal::ProofValueId(2),
            },
        ]
    );
    psi_terminal_verifier::validate_module(&lowered.semantic_module).expect("verify");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module)
    );
}

#[test]
fn integer_operation_obligations_follow_the_shared_policy_catalog() {
    let operation = operation_id(10);
    let obligation_kinds = [
        LoweredIntegerBinaryKind::ExactShiftLeft,
        LoweredIntegerBinaryKind::ExactShiftRight,
        LoweredIntegerBinaryKind::ExactAdd,
        LoweredIntegerBinaryKind::ExactSubtract,
        LoweredIntegerBinaryKind::ExactMultiply,
        LoweredIntegerBinaryKind::ExactDivide,
        LoweredIntegerBinaryKind::ExactRemainder,
        LoweredIntegerBinaryKind::WrappingDivide,
        LoweredIntegerBinaryKind::WrappingRemainder,
        LoweredIntegerBinaryKind::SaturatingDivide,
        LoweredIntegerBinaryKind::SaturatingRemainder,
    ];
    for kind in obligation_kinds {
        assert!(kind.formation_obligation(operation).is_some(), "{kind:?}");
    }
    for kind in [
        LoweredIntegerBinaryKind::BitwiseAnd,
        LoweredIntegerBinaryKind::BitwiseOr,
        LoweredIntegerBinaryKind::BitwiseXor,
        LoweredIntegerBinaryKind::WrappingShiftLeft,
        LoweredIntegerBinaryKind::WrappingShiftRight,
        LoweredIntegerBinaryKind::WrappingAdd,
        LoweredIntegerBinaryKind::SaturatingAdd,
        LoweredIntegerBinaryKind::WrappingSubtract,
        LoweredIntegerBinaryKind::SaturatingSubtract,
        LoweredIntegerBinaryKind::WrappingMultiply,
        LoweredIntegerBinaryKind::SaturatingMultiply,
    ] {
        assert!(kind.formation_obligation(operation).is_none(), "{kind:?}");
    }
    assert_eq!(
        LoweredIntegerBinaryKind::ExactSubtract.integer_policy_binding(),
        Some((IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Exact,)),
    );
    assert_eq!(
        LoweredIntegerBinaryKind::SaturatingDivide.integer_policy_binding(),
        Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Saturating,)),
    );
    assert_eq!(
        LoweredIntegerBinaryKind::ExactRemainder.integer_policy_binding(),
        Some((IntegerPolicyPrimitive::Remainder, ArithmeticDomain::Exact,)),
    );
    assert_eq!(
        LoweredIntegerBinaryKind::WrappingRemainder.integer_policy_binding(),
        Some((
            IntegerPolicyPrimitive::Remainder,
            ArithmeticDomain::Wrapping,
        )),
    );
    assert_eq!(
        LoweredIntegerBinaryKind::SaturatingRemainder.integer_policy_binding(),
        Some((
            IntegerPolicyPrimitive::Remainder,
            ArithmeticDomain::Saturating,
        )),
    );
}

#[test]
fn shared_boolean_comparison_normalization_rejects_two_runtime_sides() {
    let comparison = LoweredBooleanReturnExpression::Equal {
        left: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
        right: Box::new(LoweredBooleanReturnExpression::Parameter { position: 1 }),
    };
    assert!(normalize_shared_boolean_comparison_leaves(&comparison).is_none());

    let local_comparison = LoweredBooleanReturnExpression::Equal {
        left: Box::new(LoweredBooleanReturnExpression::Local { position: 1 }),
        right: Box::new(LoweredBooleanReturnExpression::Constant { value: false }),
    };
    assert!(normalize_shared_boolean_comparison_leaves(&local_comparison).is_none());
}

#[test]
fn generic_conformance_application_crosses_terminal_scalar_closure() {
    let source = r#"
        trait Ranked<'rank, Context> {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}
        data Borrow<'scope, Element> { value: &'scope Element }

        FieldOrder<'scope, Element>:
            Element satisfies Ranked<'scope, Borrow<'scope, Element>>
        {
            machine before(&self, other: &Element) -> bool { true }
        }

        machine choose<
            'call,
            Element,
            Order: Element satisfies Ranked<Borrow<'call, Element>>
        >(
            left: &'call Element,
            right: &'call Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller<'view>(left: &'view Card, right: &'view Card) -> bool {
            choose<Card, FieldOrder<Card>>(left, right)
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let owner = checked
        .machine_specializations
        .iter()
        .find(|specialization| !specialization.conformance_applications.is_empty())
        .expect("conformance specialization")
        .instance;

    let terminal_source = r#"
        machine terminal_root(value: bool) -> bool
        requires true == true
        ensures true == true
        { value }
    "#;
    let tokens = Lexer::new(terminal_source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let terminal_checked = lower_typed_trees(typed).expect("check");
    let mut lowered = lower_machine(&terminal_checked, "terminal_root").expect("lower terminal");
    lower_closed_conformance_applications(&checked, &[owner], &mut lowered.semantic_module)
        .expect("lower closed application");
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("verify closed application");
    let [application] = lowered
        .semantic_module
        .closed_conformance_applications
        .as_slice()
    else {
        panic!("one closed application should cross terminal lowering")
    };
    assert!(application.telescope.iter().any(|binding| {
        binding.kind == psi_terminal::ClosedConformanceParameterKind::Type
            && binding.parameter == "Element"
            && binding.argument == "Card"
    }));
    assert_eq!(application.subject_identity.as_deref(), Some("Card"));
    assert_eq!(application.trait_lifetime_arguments, ["view"]);
    assert_eq!(application.trait_arguments, ["Borrow<'view,Card>"]);
    assert_eq!(application.rows.len(), 1);
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .any(|machine| machine.id == application.owner)
    );
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode closed application");
    let decoded = psi_terminal_codec::decode_module(&bytes).expect("decode closed application");
    assert_eq!(decoded, lowered.semantic_module);

    let mut redirected_lifetime = decoded.clone();
    redirected_lifetime.closed_conformance_applications[0].trait_lifetime_arguments[0]
        .push_str("::redirected");
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected_lifetime),
        Err(psi_terminal_verifier::ModuleError::ClosedConformanceFingerprintMismatch { .. })
    ));

    let mut redirected = decoded;
    redirected.closed_conformance_applications[0].rows[0]
        .realization_identity
        .push_str("::redirected");
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected),
        Err(psi_terminal_verifier::ModuleError::ClosedConformanceFingerprintMismatch { .. })
    ));
}

#[test]
fn scalar_crash_disjunction_lowers_to_canonical_terminal_propositions() {
    let values = vec![
        ValueDeclaration {
            id: value_id(2),
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            id: value_id(1),
            scalar_type: ScalarType::Boolean,
        },
    ];
    let proposition = checked_boolean_proposition(
        &CheckedBooleanExpression::Or {
            left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
            right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
        },
        &values,
    )
    .expect("scalar disjunction lowers");
    let Proposition::Disjunction(disjuncts) = &proposition else {
        panic!("scalar disjunction retains proposition structure")
    };
    assert_eq!(disjuncts.len(), 2);
    let keys = disjuncts
        .iter()
        .map(|disjunct| psi_terminal_codec::canonical_proposition_order_key(disjunct).unwrap())
        .collect::<Vec<_>>();
    assert!(keys[0] < keys[1]);
    PropositionContext::from_value_types(values.iter().map(|value| (value.id, value.scalar_type)))
        .unwrap()
        .validate(&proposition)
        .expect("scalar disjunction is well typed");
}

#[test]
fn payloadless_sum_equality_lowers_to_case_membership_equivalence() {
    let source = r#"
        data Mode {
            case Off;
            case On;
        }

        data Root {}
        machine Root::enter(left: Mode, right: Mode)
        crashes Abort
            left == right
        {}

        machine Root::different(left: Mode, right: Mode)
        crashes Abort
            left != right
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "Root::enter").expect("lower terminal");
    let cases = lowered
        .semantic_module
        .structural_types
        .iter()
        .find_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Sum { cases } => Some(cases),
            _ => None,
        })
        .expect("payload-less sum retains a sum shape");
    assert_eq!(
        cases
            .iter()
            .map(|case| case.identity.as_str())
            .collect::<Vec<_>>(),
        ["Off", "On"]
    );
    let [bucket] = lowered.semantic_module.machines[0]
        .contract
        .crash_routes
        .as_slice()
    else {
        panic!("one crash bucket")
    };
    let [psi_terminal::CrashRouteGuard::Predicate(predicate)] = bucket.alternatives.as_slice()
    else {
        panic!("one predicate")
    };
    let Proposition::Conjunction(equivalences) = predicate.proposition() else {
        panic!("sum equality is one canonical conjunction")
    };
    assert_eq!(equivalences.len(), 4);
    assert!(equivalences.iter().all(|equivalence| matches!(
        equivalence,
        Proposition::Implication { premise, conclusion }
            if matches!(premise.as_ref(), Proposition::StructuralCaseMembership { .. })
                && matches!(conclusion.as_ref(), Proposition::StructuralCaseMembership { .. })
    )));
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("case-membership equality validates");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("case-membership module encodes");
    assert_eq!(&bytes[8..10], &71_u16.to_le_bytes());
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module.clone())
    );
    let different = lower_machine(&checked, "Root::different").expect("lower inequality");
    let [psi_terminal::CrashRouteGuard::Predicate(predicate)] =
        different.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        panic!("one inequality predicate")
    };
    assert!(matches!(
        predicate.proposition(),
        Proposition::Implication { premise, conclusion }
            if matches!(premise.as_ref(), Proposition::Conjunction(_))
                && matches!(conclusion.as_ref(), Proposition::Falsehood)
    ));
}

#[test]
fn payload_bearing_sum_equality_uses_exact_case_payload_paths() {
    let source = r#"
        trait Equatable {
            machine equals(&self, rhs: &Self) -> bool;
        }

        data Message {
            case Empty;
            case Data(value: i32);
        }
        MessageEquatable: Message satisfies Equatable;

        data Root {}
        machine Root::enter(left: Message, right: Message)
        crashes Abort
            left == right
        {}

        machine Root::different(left: Message, right: Message)
        crashes Abort
            left != right
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("payload-bearing equality has exact case-payload paths");
    let cases = lowered
        .semantic_module
        .structural_types
        .iter()
        .find_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Sum { cases } => Some(cases),
            _ => None,
        })
        .expect("payload-bearing sum shape");
    assert_eq!(cases.len(), 2);
    assert!(cases[0].fields.is_empty());
    assert_eq!(cases[1].fields.len(), 1);
    let [psi_terminal::CrashRouteGuard::Predicate(predicate)] =
        lowered.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        panic!("one equality predicate")
    };
    let Proposition::Disjunction(arms) = predicate.proposition() else {
        panic!("payload-bearing equality is a per-case disjunction")
    };
    assert_eq!(arms.len(), 2);
    assert!(format!("{arms:?}").contains("Case(StructuralCaseId(2))"));
    assert!(format!("{arms:?}").contains("Field(StructuralFieldId(1))"));
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("exact case-payload paths validate");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("payload-bearing sum module encodes");
    assert_eq!(&bytes[8..10], &71_u16.to_le_bytes());
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module.clone())
    );
    let mut redirected = lowered.semantic_module.clone();
    let payload_field = redirected
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Sum { cases } => {
                cases.iter_mut().find_map(|case| case.fields.first_mut())
            }
            _ => None,
        })
        .expect("payload field");
    payload_field.id = psi_core::StructuralFieldId::new(99).expect("redirected field");
    assert!(matches!(
        psi_terminal_verifier::validate_module(&redirected),
        Err(psi_terminal_verifier::ModuleError::InvalidIntegerFieldTerm { .. })
    ));

    let different = lower_machine(&checked, "Root::different").expect("lower inequality");
    let [psi_terminal::CrashRouteGuard::Predicate(predicate)] =
        different.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        panic!("one inequality predicate")
    };
    assert!(matches!(
        predicate.proposition(),
        Proposition::Implication { premise, conclusion }
            if matches!(premise.as_ref(), Proposition::Disjunction(_))
                && matches!(conclusion.as_ref(), Proposition::Falsehood)
    ));
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

fn unit_claim(machine: SymbolHandle, state: SymbolHandle) -> PermissionClaimIdentity {
    unit_claim_at(machine, state, 0)
}

fn hard_root_checked_fixture() -> CheckedTrees {
    let root = SymbolHandle::from_arena_index(1);
    let helper = SymbolHandle::from_arena_index(2);
    let boundary = SymbolHandle::from_arena_index(3);
    let root_state = SymbolHandle::from_arena_index(11);
    let helper_state = SymbolHandle::from_arena_index(12);
    let boundary_state = SymbolHandle::from_arena_index(13);
    let port_service_symbol = SymbolHandle::from_arena_index(20);
    let domain = SemanticDomainId(9);

    let mut checked = CheckedTrees::default();
    let port_service = checked
        .facts
        .service_reaches
        .services
        .intern(port_service_symbol, "PortIo");
    let empty_reach = checked.facts.service_reaches.rows.intern(Vec::new());
    assert_eq!(
        empty_reach,
        psi_language_semantics::ServiceReachRowTable::EMPTY_ROW
    );
    let port_reach = checked
        .facts
        .service_reaches
        .rows
        .intern(vec![port_service]);
    let reach = ServiceReachSummary {
        direct: port_reach,
        transitive: port_reach,
    };
    let contract_reach = ServiceReachPlan {
        interface: ServiceReachInterface::PublishedCeiling(port_reach),
        checked_inferred: port_reach,
    };
    checked.facts.service_reaches.machines.append_to_span(
        &mut checked.facts.service_reaches.root_machines,
        psi_checked_trees::MachineServiceReachRows {
            machine: root,
            interface: ServiceReachInterface::PublishedCeiling(port_reach),
            published_ceiling: port_reach,
            inferred_direct: port_reach,
            inferred_transitive: port_reach,
            effective: port_reach,
            concrete_effective: port_reach,
            ..Default::default()
        },
    );
    checked.facts.flow.terminal_machines = psi_checked_trees::CheckedTerminalMachineSelections {
        machines: vec![
            CheckedTerminalMachineSelection {
                machine: root,
                name: "example::Root::enter".to_owned(),
                signature: CheckedTerminalSignatureEligibility::Attached,
            },
            CheckedTerminalMachineSelection {
                machine: helper,
                name: "example::Helper::run".to_owned(),
                signature: CheckedTerminalSignatureEligibility::Attached,
            },
            CheckedTerminalMachineSelection {
                machine: boundary,
                name: "example::Acknowledgement::settle".to_owned(),
                signature: CheckedTerminalSignatureEligibility::Attached,
            },
        ],
    };
    let structural_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
        position,
        is_self: false,
        type_identity: "example::Acknowledgement".to_owned(),
        multiplicity: Multiplicity::Linear,
        access: psi_checked_trees::CheckedStructuralAccess::Owned,
        qualifications: vec![domain],
        fused_service_erasure: None,
    };
    let entry_claim = |machine, state| psi_checked_trees::CheckedUnitEntryClaimPlan {
        claim_identity: unit_claim(machine, state),
        parameter_index: 0,
        path: Vec::new(),
        carry: CarryPolicy::STRICT,
    };
    checked.facts.flow.terminal_unit_effects = psi_checked_trees::CheckedUnitEffectPlans {
        structural_types: vec![
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Acknowledgement".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record {
                    fields: vec![
                        psi_checked_trees::CheckedUnitStructuralFieldPlan {
                            identity: "sequence".to_owned(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64),
                        },
                        psi_checked_trees::CheckedUnitStructuralFieldPlan {
                            identity: "proof".to_owned(),
                            relevance: psi_terminal::BindingRelevance::Erased,
                            field_type: CheckedUnitStructuralFieldType::Erased {
                                type_identity: "named(name(example::Evidence))".to_owned(),
                            },
                        },
                    ],
                },
            },
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Helper".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
            },
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Root".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: vec![psi_checked_trees::CheckedUnitStructuralDomainPlan {
            domain,
            identity: "example::Acknowledgement::Pending".to_owned(),
            carrier_type_identity: "example::Acknowledgement".to_owned(),
        }],
        boundary_machines: vec![CheckedBoundaryMachinePlan {
            machine: boundary,
            state: boundary_state,
            contract_owner: boundary,
            attachment_type_identity: Some("example::Acknowledgement".to_owned()),
            structural_parameters: vec![psi_checked_trees::CheckedUnitStructuralParameterPlan {
                is_self: true,
                ..structural_parameter(0)
            }],
            scalar_parameters: Vec::new(),
            result_type: None,
            domain_requirements: vec![
                psi_checked_trees::CheckedUnitStructuralDomainRequirementPlan {
                    argument_index: 0,
                    domain,
                },
            ],
            contract_report_fingerprint: 0x303,
            contract_commitment: psi_checked_trees::MachineContractCommitment::from_digest(
                [0x03; 32],
            ),
            contract_service_reach: contract_reach,
            service_reach: reach,
        }],
        dynamic_dispatch: psi_checked_trees::CheckedDynamicDispatchPlans::default(),
        composed_machines: Vec::new(),
        machines: vec![
            CheckedUnitEffectMachinePlan {
                machine: root,
                state: root_state,
                attachment_type_identity: Some("example::Root".to_owned()),
                structural_parameters: vec![structural_parameter(7)],
                scalar_parameters: Vec::new(),
                provider_attachment_requirements: Vec::new(),
                trivial_affine_locals: Vec::new(),
                entry_claims: vec![entry_claim(root, root_state)],
                body_qualifications: vec![domain],
                contract_report_fingerprint: 0x101,
                contract_commitment: psi_checked_trees::MachineContractCommitment::from_digest(
                    [0x01; 32],
                ),
                contract_service_reach: contract_reach,
                service_reach: reach,
                operations: vec![
                    CheckedUnitEffectOperationPlan::CallUnit {
                        coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                            statement_index: 0,
                            call_ordinal: 0,
                        },
                        target_machine: helper,
                        target_state: helper_state,
                        target_contract_report_fingerprint: 0x202,
                        service_reach: reach,
                        structural_arguments: vec![
                            psi_checked_trees::CheckedUnitStructuralArgumentPlan {
                                source: psi_checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                    parameter_index: 0,
                                },
                                type_identity: "example::Acknowledgement".to_owned(),
                                access: psi_checked_trees::CheckedStructuralAccess::Owned,
                                path: Vec::new(),
                            },
                        ],
                        claim_transfers: vec![psi_checked_trees::CheckedUnitClaimTransferPlan {
                            claim_identity: unit_claim(root, root_state),
                            argument_index: 0,
                        }],
                    },
                    CheckedUnitEffectOperationPlan::ReturnUnit {
                        statement_index: 1,
                        trivial_affine_local_discard_ordinals: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            },
            CheckedUnitEffectMachinePlan {
                machine: helper,
                state: helper_state,
                attachment_type_identity: Some("example::Helper".to_owned()),
                structural_parameters: vec![structural_parameter(3)],
                scalar_parameters: Vec::new(),
                provider_attachment_requirements: Vec::new(),
                trivial_affine_locals: Vec::new(),
                entry_claims: vec![entry_claim(helper, helper_state)],
                body_qualifications: vec![domain],
                contract_report_fingerprint: 0x202,
                contract_commitment: psi_checked_trees::MachineContractCommitment::from_digest(
                    [0x02; 32],
                ),
                contract_service_reach: contract_reach,
                service_reach: reach,
                operations: vec![
                    CheckedUnitEffectOperationPlan::PortWrite {
                        coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                            statement_index: 0,
                            call_ordinal: 0,
                        },
                        port: 0x3f8,
                        value: 0x5a,
                        service_reach: reach,
                    },
                    CheckedUnitEffectOperationPlan::BoundaryCall {
                        coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                            statement_index: 1,
                            call_ordinal: 0,
                        },
                        source_site: None,
                        target_machine: boundary,
                        target_state: boundary_state,
                        target_contract_report_fingerprint: 0x303,
                        service_reach: reach,
                        scalar_arguments: Vec::new(),
                        structural_arguments: vec![
                            psi_checked_trees::CheckedUnitStructuralArgumentPlan {
                                source: psi_checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                    parameter_index: 0,
                                },
                                type_identity: "example::Acknowledgement".to_owned(),
                                access: psi_checked_trees::CheckedStructuralAccess::Owned,
                                path: Vec::new(),
                            },
                        ],
                        completion_receipts: vec![
                            psi_checked_trees::CheckedUnitClaimTransferPlan {
                                claim_identity: unit_claim(helper, helper_state),
                                argument_index: 0,
                            },
                        ],
                    },
                    CheckedUnitEffectOperationPlan::ReturnUnit {
                        statement_index: 2,
                        trivial_affine_local_discard_ordinals: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            },
        ],
    };
    checked.facts.contract_plans.machines = vec![
        psi_checked_trees::MachineContractPlan {
            machine: root,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 0x101,
            commitment: psi_checked_trees::MachineContractCommitment::from_digest([0x01; 32]),
        },
        psi_checked_trees::MachineContractPlan {
            machine: helper,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 0x202,
            commitment: psi_checked_trees::MachineContractCommitment::from_digest([0x02; 32]),
        },
        psi_checked_trees::MachineContractPlan {
            machine: boundary,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 0x303,
            commitment: psi_checked_trees::MachineContractCommitment::from_digest([0x03; 32]),
        },
    ];
    checked
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
