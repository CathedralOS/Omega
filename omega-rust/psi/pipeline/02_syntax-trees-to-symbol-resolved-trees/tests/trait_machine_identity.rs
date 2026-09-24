use source_files_to_tokens::Lexer;
use symbol_resolved_trees::data::{MachineParameterContract, TypeParameterKind};
use symbol_resolved_trees::types::TypeReference;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn resolves_trait_requirement_argument_as_exact_state_identity() {
    let tokens = Lexer::new(
        r#"
        trait PrivateCallbackSlot<machine Requirement> {}
        boundary trait WindowProcedure {
            machine call(value: i32) -> i32;
        }
        data WndClassLayout {}
        WindowProcedureSlot: WndClassLayout satisfies PrivateCallbackSlot<WindowProcedure::call> {}
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = resolve(ResolutionRequest::new(&syntax)).expect("resolve");

    let slot_trait = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "PrivateCallbackSlot")
        .expect("slot trait");
    let [parameter] = program.trait_type_parameters(slot_trait) else {
        panic!("expected one slot parameter");
    };
    assert!(matches!(
        parameter.kind,
        TypeParameterKind::Machine {
            contract: MachineParameterContract::RequirementIdentity
        }
    ));

    let callback_trait = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "WindowProcedure")
        .expect("callback trait");
    let [callback_requirement] = program.trait_machine_signatures(callback_trait.machines) else {
        panic!("expected one callback requirement");
    };
    let conformance = program
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "WindowProcedureSlot")
        })
        .expect("private slot conformance");
    let [argument] = program.child_type_references(conformance.arguments) else {
        panic!("expected one trait argument");
    };
    let TypeReference::Named { symbol, .. } = argument else {
        panic!("expected direct declaration identity");
    };
    assert_eq!(*symbol, callback_requirement.symbol);
}

fn shadowed_method_program() -> symbol_resolved_trees::SymbolResolvedTrees {
    let tokens = Lexer::new(
        "domain<Value> u64::Tagged<Value>;\n\
         boundary trait Device<T, Owner> {\n\
             machine consume<T>(carrier: &mut T, retained: &mut Owner, value: u64) -> T\n\
             requires value in u64::Tagged<T>\n\
             ensures value in u64::Tagged<Owner>;\n\
         }",
    )
    .tokenize()
    .expect("tokenize shadowed method");
    let syntax = parse_syntax_trees(&tokens).expect("parse shadowed method");
    resolve(ResolutionRequest::new(&syntax)).expect("resolve shadowed method")
}

#[test]
fn method_type_binder_shadows_owner_in_parameters_and_result() {
    let program = shadowed_method_program();
    let definition = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Device")
        .unwrap();
    let [owner, retained] = program.trait_type_parameters(definition) else {
        panic!("owner binders")
    };
    let [signature] = program.trait_machine_signatures(definition.machines) else {
        panic!("one method")
    };
    let [method] = program.data_type_parameters(signature.type_parameters) else {
        panic!("method binder")
    };
    assert!(method.symbol.is_valid());
    assert_ne!(method.symbol, owner.symbol);
    let parameters = program.state_parameters(signature.parameters);
    for (parameter, expected) in parameters[..2].iter().zip([method.symbol, retained.symbol]) {
        let TypeReference::Reference(reference) = &parameter.type_reference else {
            panic!("reference parameter")
        };
        let TypeReference::Named { symbol, .. } =
            program.child_type_reference(reference.storage.referee)
        else {
            panic!("named referee")
        };
        assert_eq!(*symbol, expected, "nearest binder owns the formal type");
    }
    let Some(TypeReference::Named { symbol, .. }) = &signature.return_type else {
        panic!("named result")
    };
    assert_eq!(*symbol, method.symbol);
}

#[test]
fn method_type_binder_shadows_owner_in_membership_arguments() {
    let program = shadowed_method_program();
    let definition = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Device")
        .unwrap();
    let [owner, retained] = program.trait_type_parameters(definition) else {
        panic!("owner binders")
    };
    let [signature] = program.trait_machine_signatures(definition.machines) else {
        panic!("one method")
    };
    let [method] = program.data_type_parameters(signature.type_parameters) else {
        panic!("method binder")
    };
    assert_ne!(method.symbol, owner.symbol);
    let contracts = program.signature_contracts(signature.contracts);
    assert_eq!(contracts.len(), 2);
    for (contract, expected) in contracts.iter().zip([method.symbol, retained.symbol]) {
        let [symbol_resolved_trees::domain::ProofFact::Membership(membership)] =
            program.proof_facts(contract.facts)
        else {
            panic!("membership contract")
        };
        let [TypeReference::Named { symbol, .. }] =
            program.child_type_references(membership.domain_arguments)
        else {
            panic!("one named index argument")
        };
        assert_eq!(
            *symbol, expected,
            "membership indices share the signature scope"
        );
    }
}
