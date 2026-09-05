use super::semantic_contexts_prove_contract_fact;
use psi_facts::{Fact, FactContextHandle, FactPayload, FactPlace, FactPlan, ProgramPoint};
use psi_language_semantics::CarryPermission;

fn fixture() -> (psi_typed_trees::TypedTrees, FactPlan, [FactPlace; 2]) {
    let tokens = psi_source_files_to_tokens::Lexer::new(
        "machine inspect(first: &mut u64, second: &mut u64) {}",
    )
    .tokenize()
    .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let program = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type");
    let state = &program.machine_states(&program.machines()[0])[0];
    let parameters = program.state_parameters(state);
    let mut semantic = FactPlan::default();
    let places = [0, 1]
        .map(|index| FactPlace::Place(semantic.append_symbol_place(parameters[index].symbol)));
    (program, semantic, places)
}

fn context(semantic: &mut FactPlan, place: FactPlace, payload: FactPayload) -> FactContextHandle {
    let fact = semantic.append_fact(Fact {
        place,
        payload,
        ..Fact::default()
    });
    let mut references = psi_arena::HandleSpan::empty();
    semantic.append_ref(&mut references, fact);
    semantic.append_context(ProgramPoint::Global, references)
}

#[test]
fn raw_carry_permission_requires_live_matching_place_and_permission() {
    let (program, mut semantic, [required_place, other_place]) = fixture();
    let payload = FactPayload::CarryPermission {
        value: Default::default(),
        permission: CarryPermission::AnyCpu,
    };
    let required = Fact {
        place: required_place,
        payload,
        ..Fact::default()
    };
    let matching = context(&mut semantic, required_place, payload);
    let contract = context(
        &mut semantic,
        required_place,
        FactPayload::ContractCarryPermission {
            kind: Default::default(),
            fact: Default::default(),
            value: Default::default(),
            permission: CarryPermission::AnyCpu,
        },
    );
    let wrong_place = context(&mut semantic, other_place, payload);
    let missing_place = context(&mut semantic, FactPlace::Unknown, payload);
    let wrong_permission = context(
        &mut semantic,
        required_place,
        FactPayload::CarryPermission {
            value: Default::default(),
            permission: CarryPermission::AnyThread,
        },
    );
    // Arena presence is not liveness: the matching fact exists in every case,
    // but only the explicitly supplied contexts may establish the obligation.
    for (contexts, accepted) in [
        (vec![], false),
        (vec![matching], true),
        (vec![contract], true),
        (vec![wrong_place], false),
        (vec![missing_place], false),
        (vec![wrong_permission], false),
    ] {
        assert_eq!(
            semantic_contexts_prove_contract_fact(&program, &semantic, &contexts, &required),
            accepted,
            "contexts: {contexts:?}"
        );
    }
    assert!(!semantic_contexts_prove_contract_fact(
        &program,
        &semantic,
        &[matching],
        &Fact {
            place: FactPlace::Unknown,
            ..required
        },
    ));
}

#[test]
fn raw_carry_origin_requires_live_matching_place() {
    let (program, mut semantic, [required_place, other_place]) = fixture();
    let payload = FactPayload::CarryOrigin {
        value: Default::default(),
    };
    let required = Fact {
        place: required_place,
        payload,
        ..Fact::default()
    };
    let matching = context(&mut semantic, required_place, payload);
    let wrong_place = context(&mut semantic, other_place, payload);
    let missing_place = context(&mut semantic, FactPlace::Unknown, payload);
    let permission_only = context(
        &mut semantic,
        required_place,
        FactPayload::CarryPermission {
            value: Default::default(),
            permission: CarryPermission::AnyCpu,
        },
    );
    for (contexts, accepted) in [
        (vec![], false),
        (vec![matching], true),
        (vec![wrong_place], false),
        (vec![missing_place], false),
        (vec![permission_only], false),
    ] {
        assert_eq!(
            semantic_contexts_prove_contract_fact(&program, &semantic, &contexts, &required),
            accepted,
            "contexts: {contexts:?}"
        );
    }
    assert!(!semantic_contexts_prove_contract_fact(
        &program,
        &semantic,
        &[matching],
        &Fact {
            place: FactPlace::Unknown,
            ..required
        },
    ));
}

#[test]
fn evidence_and_deferred_payloads_are_not_proved_contracts() {
    let (program, mut semantic, [place, _]) = fixture();
    for payload in [
        FactPayload::AssignedValue {
            value: Default::default(),
        },
        FactPayload::BooleanValue {
            expression: Default::default(),
            value: true,
        },
        FactPayload::TypeConstraint {
            constraint: Default::default(),
        },
        FactPayload::ProofObligation {
            kind: Default::default(),
        },
        FactPayload::Contract {
            kind: Default::default(),
            fact: Default::default(),
        },
    ] {
        let required = Fact {
            place,
            payload,
            ..Fact::default()
        };
        let identical = context(&mut semantic, place, payload);
        for contexts in [vec![], vec![identical]] {
            assert!(
                !semantic_contexts_prove_contract_fact(&program, &semantic, &contexts, &required,),
                "metadata must not grant proof: {payload:?}"
            );
        }
    }
}
