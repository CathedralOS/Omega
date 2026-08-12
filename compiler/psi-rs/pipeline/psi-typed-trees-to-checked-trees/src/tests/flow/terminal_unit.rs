use super::*;
use psi_checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType};
use psi_language_core::BindingRelevance;
use psi_language_semantics::Multiplicity;
use psi_typed_trees::types::PrimitiveType;

fn checked(source: &str) -> psi_checked_trees::CheckedTrees {
    let source = format!("boundary trait PortIo {{}}\n{source}");
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn machine_named(
    checked: &psi_checked_trees::CheckedTrees,
    name: &str,
) -> psi_symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == name || machine.name.as_str().ends_with(&format!("::{name}"))
        })
        .unwrap_or_else(|| panic!("missing machine `{name}`"))
        .symbol
}

#[test]
fn retains_static_attached_root_helper_port_and_boundary_settlement() {
    let checked = checked(
        r#"
        data Acknowledgement [linear] {
            root: u64;
            provider_execution: u64;
            invocation: u64;
            policy: u64;
            acknowledgement: u64;
        }

        domain Acknowledgement::Pending;

        boundary machine Acknowledgement::settle(self)
        reaches PortIo
        requires
            self in Acknowledgement::Pending
        ensures true;

        data Helper {}

        machine Helper::run(acknowledgement: Acknowledgement in Pending)
        reaches PortIo
        {
            asm { out 32, 7 }
            acknowledgement.settle();
        }

        data Root {}

        machine Root::enter(acknowledgement: Acknowledgement in Pending)
        reaches PortIo
        {
            Helper::run(acknowledgement);
        }
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    let root_symbol = machine_named(&checked, "enter");
    let helper_symbol = machine_named(&checked, "run");
    let settle_symbol = machine_named(&checked, "settle");
    let root = plans
        .for_machine(root_symbol)
        .expect("static attached root plan");
    let helper = plans
        .for_machine(helper_symbol)
        .expect("static attached helper plan");
    let settle = plans
        .boundary_for_machine(settle_symbol)
        .expect("boundary settlement plan");

    assert!(root.attachment_type_identity.contains("Root"));
    assert!(helper.attachment_type_identity.contains("Helper"));
    assert_eq!(root.structural_parameters.len(), 1);
    assert_eq!(helper.structural_parameters.len(), 1);
    assert_eq!(
        root.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(root.structural_parameters[0].qualifications.len(), 1);
    assert_eq!(root.entry_claims.len(), 1);
    assert!(root.entry_claims[0].field_path.is_empty());
    assert_eq!(helper.entry_claims.len(), 1);
    assert_eq!(settle.structural_parameters.len(), 1);
    assert_eq!(
        settle.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(settle.domain_requirements.len(), 1);
    assert_eq!(settle.domain_requirements[0].argument_index, 0);
    assert!(root.service_reach.transitive.is_valid());
    assert!(helper.service_reach.direct.is_valid());
    assert!(helper.service_reach.transitive.is_valid());
    assert!(settle.service_reach.direct.is_valid());
    assert_ne!(root.contract_fingerprint, 0);
    assert_ne!(helper.contract_fingerprint, 0);
    assert_ne!(settle.contract_fingerprint, 0);

    let port_values = checked
        .facts
        .values
        .values
        .iter()
        .filter_map(|(_, value)| {
            matches!(
                value.origin,
                psi_checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index: 0,
                    role: psi_checked_trees::CheckedValueStatementRole::CallArgument,
                } if machine_symbol == helper_symbol && state_symbol == helper.state
            )
            .then_some(value)
        })
        .collect::<Vec<_>>();
    assert_eq!(port_values.len(), 2);
    assert_eq!(port_values[0].primitive_type, Some(PrimitiveType::U16));
    assert_eq!(port_values[1].primitive_type, Some(PrimitiveType::U8));
    assert_eq!(
        port_values[0]
            .integer_range
            .as_ref()
            .and_then(|range| range.minimum.to_u64()),
        Some(32)
    );
    assert_eq!(
        port_values[1]
            .integer_range
            .as_ref()
            .and_then(|range| range.minimum.to_u64()),
        Some(7)
    );

    let acknowledgement = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity.contains("Acknowledgement"))
        .expect("acknowledgement shape");
    assert_eq!(acknowledgement.fields.len(), 5);
    assert_eq!(
        acknowledgement
            .fields
            .iter()
            .map(|field| field.identity.as_str())
            .collect::<Vec<_>>(),
        [
            "root",
            "provider_execution",
            "invocation",
            "policy",
            "acknowledgement",
        ]
    );
    assert!(acknowledgement.fields.iter().all(|field| {
        field.field_type == CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64)
    }));

    assert_eq!(root.operations.len(), 2);
    match &root.operations[0] {
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            structural_arguments,
            claim_transfers,
            ..
        } => {
            assert_eq!(coordinate.statement_index, 0);
            assert_eq!(coordinate.call_ordinal, 0);
            assert_eq!(*target_machine, helper_symbol);
            assert_eq!(structural_arguments.len(), 1);
            assert_eq!(structural_arguments[0].source_parameter_index, 0);
            assert_eq!(claim_transfers.len(), 1);
            assert_eq!(claim_transfers[0].argument_index, 0);
            assert_eq!(
                claim_transfers[0].claim_identity,
                root.entry_claims[0].claim_identity
            );
        }
        operation => panic!("unexpected root operation: {operation:?}"),
    }
    assert!(matches!(
        root.operations[1],
        CheckedUnitEffectOperationPlan::ReturnUnit { statement_index: 1 }
    ));

    assert_eq!(helper.operations.len(), 3);
    assert!(matches!(
        helper.operations[0],
        CheckedUnitEffectOperationPlan::PortWrite {
            coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                statement_index: 0,
                call_ordinal: 0,
            },
            port: 32,
            value: 7,
            ..
        }
    ));
    match &helper.operations[1] {
        CheckedUnitEffectOperationPlan::BoundaryCallUnit {
            coordinate,
            target_machine,
            structural_arguments,
            claim_settlements,
            ..
        } => {
            assert_eq!(coordinate.statement_index, 1);
            assert_eq!(coordinate.call_ordinal, 0);
            assert_eq!(*target_machine, settle_symbol);
            assert_eq!(structural_arguments.len(), 1);
            assert_eq!(claim_settlements.len(), 1);
            assert_eq!(claim_settlements[0].argument_index, 0);
            assert_eq!(
                claim_settlements[0].claim_identity,
                helper.entry_claims[0].claim_identity
            );
        }
        operation => panic!("unexpected helper operation: {operation:?}"),
    }
    assert!(matches!(
        helper.operations[2],
        CheckedUnitEffectOperationPlan::ReturnUnit { statement_index: 2 }
    ));
}

#[test]
fn omits_nonconstant_port_and_unsupported_nested_shape_without_placeholder() {
    let checked = checked(
        r#"
        data DynamicPort { port: u16; }
        machine DynamicPort::write(&mut self)
        reaches PortIo
        {
            asm { out self.port, 7 }
        }

        data Unsupported {
            case Empty;
        }
        data NestedRoot { value: Unsupported; }
        machine NestedRoot::run() {}
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans
            .for_machine(machine_named(&checked, "write"))
            .is_none()
    );
    assert!(plans.for_machine(machine_named(&checked, "run")).is_none());
    assert!(
        plans
            .structural_types
            .iter()
            .all(|shape| !shape.identity.contains("NestedRoot")),
        "unsupported nested construction must not leave an accepted empty placeholder"
    );
}

#[test]
fn retains_opaque_erased_field_identity_in_unit_structural_shape() {
    let checked = checked(
        r#"
        data Evidence { case Only; }
        data Certified {
            value: u64;
            proof [erased]: Evidence;
        }
        machine Certified::run(&self) {}
        "#,
    );

    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(plans.for_machine(machine_named(&checked, "run")).is_some());
    let certified = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity.contains("Certified"))
        .expect("certified structural shape");
    assert_eq!(certified.fields.len(), 2);
    assert_eq!(certified.fields[0].relevance, BindingRelevance::Relevant);
    assert_eq!(certified.fields[1].relevance, BindingRelevance::Erased);
    assert!(matches!(
        &certified.fields[1].field_type,
        CheckedUnitStructuralFieldType::Erased { type_identity }
            if type_identity.contains("Evidence")
    ));
    assert!(
        plans
            .structural_types
            .iter()
            .all(|shape| !shape.identity.contains("Evidence")),
        "opaque erased field carriers do not enter the executable structural graph"
    );
}
