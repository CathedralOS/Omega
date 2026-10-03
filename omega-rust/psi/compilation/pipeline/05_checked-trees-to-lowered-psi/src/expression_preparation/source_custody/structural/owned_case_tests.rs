//! Whole affine sum operands retain their source transfer when wrapped in a case.

use super::{CheckedStructuralValueKind, CheckedUnitEffectOperationPlan, validate};
use language_semantics::{Multiplicity, PermissionEventKind, PermissionEventSource};

#[test]
fn affine_case_operand_replay_requires_its_exact_statement_transfer() {
    let original = crate::front_end::checked_program(
        "data Kind { case Missing; case Denied; }
         data Payload { case Error(kind: Kind); case Empty; }
         machine wrap() -> Payload {
             let kind: Kind = Kind::Missing;
             Payload::Error { kind: kind }
         }",
    );
    let mut roots = original
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| root.statement_ordinal == 1);
    let root = roots
        .next()
        .expect("the payload constructor has a retained root");
    assert!(roots.next().is_none());
    let operation = CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result: checked_trees::CheckedUnitStructuralResultBindingPlan {
            statement_index: root.statement_ordinal,
            binding_ordinal: root.statement_ordinal,
            type_identity: original
                .normalized_type_identity(root.type_reference)
                .into_string(),
            multiplicity: original.type_multiplicity(root.type_reference),
        },
        value: root.root,
        calls: Vec::new(),
        operand_source: None,
        discard_result_on_return: false,
    };
    validate(&original, root.machine, root.state, &operation)
        .expect("wrapping a whole affine enum preserves its case and custody");

    let mut sources = original
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .filter_map(|(_, node)| match &node.kind {
            CheckedStructuralValueKind::Place(argument) => match argument.source {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                } => Some(symbol),
                _ => None,
            },
            _ => None,
        });
    let source = sources.next().expect("the payload consumes its enum local");
    assert!(sources.next().is_none());
    let mut transfers = original
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == root.machine
                && event.state_symbol == root.state
                && event.root == facts::PlaceRoot::Symbol(source)
                && event.kind == PermissionEventKind::Transfer
                && event.source
                    == PermissionEventSource::Statement {
                        statement_index: root.statement_ordinal as usize,
                    }
        });
    let (transfer, event) = transfers
        .next()
        .expect("the constructor consumes the affine local");
    assert!(transfers.next().is_none());
    assert_eq!(event.multiplicity, Multiplicity::Affine);

    for remove_transfer in [false, true] {
        let mut forged = original.clone();
        if remove_transfer {
            assert!(forged.facts.flow.ownership.permissions.free(transfer));
        } else {
            forged
                .facts
                .flow
                .ownership
                .permissions
                .get_mut(transfer)
                .kind = PermissionEventKind::AffineDrop;
        }
        let error = validate(&forged, root.machine, root.state, &operation)
            .expect_err("disposal or missing evidence cannot authorize constructor consumption");
        assert!(error.to_string().contains("lost its statement transfer"));
    }
}

#[test]
fn projected_constructor_operand_replays_exact_statement_custody() {
    let original = crate::front_end::checked_program(
        "data Message { value: u8; }
         data Remainder { identity: u64; bytes: &[u8]; }
         data Relayed<T> { value: T; remainder: Remainder; }
         machine update(relayed: Relayed<Message>) -> Relayed<Message> {
             let updated: Relayed<Message> = Relayed {
                 value: Message { value: 42 }, remainder: relayed.remainder
             };
             updated
         }",
    );
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "update")
        .unwrap();
    let mut roots = original
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| root.machine == machine.symbol && root.statement_ordinal == 0);
    let root = roots.next().expect("ordinary generic constructor root");
    assert!(roots.next().is_none());
    let operation = CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result: checked_trees::CheckedUnitStructuralResultBindingPlan {
            statement_index: root.statement_ordinal,
            binding_ordinal: root.statement_ordinal,
            type_identity: original
                .normalized_type_identity(root.type_reference)
                .into_string(),
            multiplicity: original.type_multiplicity(root.type_reference),
        },
        value: root.root,
        calls: Vec::new(),
        operand_source: None,
        discard_result_on_return: false,
    };
    validate(&original, root.machine, root.state, &operation)
        .expect("projected remainder keeps its shared descriptor backing");
    assert!(
        original
            .facts
            .flow
            .ownership
            .owned_selection_at(root.state, 0)
            .is_none(),
        "an ordinary constructor does not invent a match receipt"
    );
    let mut events = original
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == root.machine
                && event.state_symbol == root.state
                && event.kind == PermissionEventKind::Transfer
                && event.source == PermissionEventSource::Statement { statement_index: 0 }
                && original
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span(event.segments)
                    .is_some_and(|path| !path.is_empty())
        });
    let (event, _) = events.next().expect("exact projected statement transfer");
    assert!(events.next().is_none());
    for corruption in 0..3 {
        let mut forged = original.clone();
        match corruption {
            0 => {
                assert!(forged.facts.flow.ownership.permissions.free(event));
            }
            1 => {
                forged
                    .facts
                    .flow
                    .ownership
                    .permissions
                    .get_mut(event)
                    .source = PermissionEventSource::Statement { statement_index: 1 }
            }
            _ => {
                forged.facts.flow.ownership.permissions.get_mut(event).kind =
                    PermissionEventKind::AffineDrop
            }
        }
        validate(&forged, root.machine, root.state, &operation)
            .expect_err("missing, displaced or disposal evidence cannot authorize a field move");
    }
}
