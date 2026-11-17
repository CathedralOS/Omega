//! Bodyless headers retain the qualifications generated for their exact formals.

use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionNode;

#[test]
fn a_bodyless_boundary_keeps_its_qualified_formal_in_the_complete_header() {
    let program = super::qualified_operator_home_tests::checked(&[(
        "header.omg",
        "domain u32::Tagged; boundary machine transfer(input: u32 in Tagged) -> u32 in Tagged;",
    )])
    .expect("an ordinary qualified boundary header types");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "transfer")
        .unwrap();
    assert!(!machine.body_is_present);
    let entries = program.machine_states(machine);
    let [entry] = entries.as_array().expect("one actual bodyless entry");
    assert!(entry.contracts.is_empty());
    let (owner, signature) = program
        .bodyless_declaration(machine.symbol)
        .unwrap()
        .unwrap();
    assert_eq!(owner, machine.symbol);
    assert_eq!(signature.symbol, machine.symbol);
    assert_eq!(signature.contracts, machine.contracts);
    let parameters = program.state_parameters.span(signature.parameters).unwrap();
    let [parameter] = parameters.as_array().expect("one actual source formal");
    let memberships = program
        .signature_contracts
        .span(signature.contracts)
        .unwrap()
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| match fact {
            ProofFact::Membership(membership) => Some(membership),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(memberships.len(), 1);
    let ExpressionNode::Name(subject) = program.expression_table.expression(memberships[0].value)
    else {
        panic!("the original qualified input remains the membership subject");
    };
    assert_eq!(subject.symbol, parameter.symbol);
}

#[test]
fn a_bodyful_machine_keeps_its_qualified_formal_requirement_with_its_state() {
    let program = super::qualified_operator_home_tests::checked(&[(
        "body.omg",
        "domain u32::Tagged; machine value(input: u32 in Tagged) -> u32 in Tagged { input }",
    )])
    .expect("an ordinary qualified machine types");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    assert!(machine.body_is_present);
    assert!(
        program
            .machine_states(machine)
            .iter()
            .flat_map(|state| program.signature_contracts.span_or_empty(state.contracts))
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .any(|fact| matches!(fact, ProofFact::Membership(_)))
    );
    assert!(
        !program
            .signature_contracts
            .span_or_empty(machine.contracts)
            .iter()
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .any(|fact| matches!(fact, ProofFact::Membership(_)))
    );
}

fn retained_policy_calls() -> typed_trees::TypedTrees {
    const SOURCE: &str = r#"
use omega::language::core::AccessPolicy;
use omega::language::core::AccessPlan;
use omega::language::core::Issuance;
use omega::language::core::reflect::TypeSchema;
use omega::language::core::access;
First satisfies AccessPolicy {
    machine plan(schema: TypeSchema) -> AccessPlan reaches none crashes none terminates {
        access::new_plan(&schema, Issuance::Unrestricted)
    }
}
Second satisfies AccessPolicy {
    machine plan(schema: TypeSchema) -> AccessPlan reaches none crashes none terminates {
        access::new_plan(&schema, Issuance::Unrestricted)
    }
}
trait ForeignPolicy { machine identity(value: u32) -> u32; }
Different satisfies ForeignPolicy { machine identity(value: u32) -> u32 { value } }
boundary machine retain<const P: AccessPolicy, T>(value: T) -> T
reaches none crashes none terminates requires true ensures true;
machine first(value: u32) -> u32 { retain<First, u32>(value) }
machine second(value: bool) -> bool { retain<Second, bool>(value) }
boundary machine foreign_retain<const P: ForeignPolicy, T>(value: T) -> T
reaches none crashes none terminates requires true ensures true;
machine different(value: u32) -> u32 { foreign_retain<Different, u32>(value) }
boundary machine unsealed<T>(value: T) -> T;
"#;
    // Match the real source-loading path: the bundled file is not cut down to
    // an invented AccessPolicy or a synthetic compiler declaration.
    let core =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../../source/library/core");
    let mut files = std::fs::read_dir(core)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "omg"))
        .filter(|path| path.file_name().is_some_and(|name| name != "equality.omg"))
        .collect::<Vec<_>>();
    files.sort_unstable();
    let files = files
        .into_iter()
        .map(|path| {
            (
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                std::fs::read_to_string(path).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let product = files.len();
    let mut units = files
        .iter()
        .map(|(name, text)| (0, name.as_str(), text.as_str()))
        .collect::<Vec<_>>();
    units.push((31, "retained-policy.omg", SOURCE));
    let file = |name: &str| {
        files
            .iter()
            .position(|(candidate, _)| candidate == name)
            .unwrap()
    };
    let imports = [
        (
            product,
            file("layout.omg"),
            "omega::language::core::AccessPolicy",
            3,
        ),
        (
            product,
            file("layout.omg"),
            "omega::language::core::AccessPlan",
            3,
        ),
        (
            product,
            file("layout.omg"),
            "omega::language::core::Issuance",
            3,
        ),
        (
            product,
            file("reflect.omg"),
            "omega::language::core::reflect::TypeSchema",
            3,
        ),
        (
            product,
            file("access.omg"),
            "omega::language::core::access",
            3,
        ),
    ];
    super::qualified_operator_home_tests::packages_with_equality(&units, &[], &imports)
        .expect("the real policy source retains implementation and type operands")
}

fn retained_policy_call(
    program: &typed_trees::TypedTrees,
    owner: &str,
) -> (
    typed_trees::static_applications::StaticMachineApplicationHandle,
    symbols::SymbolHandle,
) {
    program
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| {
            let ExpressionNode::Call(call) = expression else {
                return None;
            };
            let application = call.authored_machine_application;
            if !program
                .expression_table
                .static_machine_application_is_valid(application)
            {
                return None;
            }
            let owner_symbol = program
                .expression_table
                .static_machine_application(application)
                .lexical_owner;
            program
                .machine_by_own_or_state_symbol(owner_symbol)
                .is_some_and(|machine| machine.name.as_str() == owner)
                .then_some((application, call.target_symbol))
        })
        .expect("the original source call has its exact lexical owner")
}

#[test]
fn bodyless_policy_applications_keep_actual_bindings_and_refuse_changed_occurrences() {
    use typed_trees::static_applications::{
        StaticMachineApplicationHandle, StaticMachineBindingValue,
    };

    let mut program = retained_policy_calls();
    let sources =
        source::source_identity::toolchain_source_identities(program.symbols.source_files())
            .expect("the source identity registry is prepared once");
    let (first, target) = retained_policy_call(&program, "first");
    let (second, second_target) = retained_policy_call(&program, "second");
    let (foreign, foreign_target) = retained_policy_call(&program, "different");
    assert!(
        program
            .bodyless_callable_signature(first, target, &sources)
            .unwrap()
            .is_none()
    );
    assert!(
        program
            .prepare_bodyless_callable_application(foreign, foreign_target, &sources)
            .unwrap()
    );
    {
        let mut wrong_type = program.clone();
        let mut arguments = program
            .expression_table
            .static_machine_application_arguments(first)
            .to_vec();
        arguments[0] = arguments[1].clone();
        wrong_type
            .expression_table
            .set_static_machine_application_arguments(first, &arguments);
        assert_eq!(
            wrong_type
                .prepare_bodyless_callable_application(first, target, &sources)
                .unwrap_err(),
            "bodyless call needs its complete implementation value schema"
        );
    }
    {
        let mut wrong_schema = program.clone();
        let mut arguments = program
            .expression_table
            .static_machine_application_arguments(first)
            .to_vec();
        let StaticMachineBindingValue::Conformance(foreign_argument) = &program
            .expression_table
            .static_machine_application_bindings(foreign)[0]
            .value
        else {
            panic!("the valid foreign call retained its implementation operand");
        };
        arguments[0] = foreign_argument.clone();
        wrong_schema
            .expression_table
            .set_static_machine_application_arguments(first, &arguments);
        assert_eq!(
            wrong_schema
                .prepare_bodyless_callable_application(first, target, &sources)
                .unwrap_err(),
            "bodyless call implementation has another declared schema"
        );
    }
    for (application, target) in [(first, target), (second, second_target)] {
        assert!(
            program
                .prepare_bodyless_callable_application(application, target, &sources)
                .unwrap()
        );
    }
    let source = program.bodyless_declaration(target).unwrap().unwrap().1;
    let signature = program
        .bodyless_callable_signature(first, target, &sources)
        .unwrap()
        .unwrap();
    let other = program
        .bodyless_callable_signature(second, second_target, &sources)
        .unwrap()
        .unwrap();
    assert_ne!(
        program.normalized_type_identity(signature.return_type),
        program.normalized_type_identity(other.return_type)
    );
    assert!(signature.type_parameters.is_empty());
    assert_eq!(
        signature.conformance_bounds.len(),
        source.conformance_bounds.len()
    );
    assert_eq!(
        signature.termination_guarantee,
        source.termination_guarantee
    );
    let clause_kinds = |signature: &typed_trees::signature::StateSignature| {
        program
            .state_signature_contracts(signature)
            .iter()
            .map(|clause| clause.kind.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(clause_kinds(signature), clause_kinds(&source));
    let bindings = program
        .expression_table
        .static_machine_application_bindings(first)
        .to_vec();
    assert!(matches!(
        bindings[0].value,
        StaticMachineBindingValue::Conformance(_)
    ));
    assert!(matches!(
        bindings[1].value,
        StaticMachineBindingValue::Type(_)
    ));
    {
        let mut changed = program.clone();
        let mut forged = bindings;
        forged[1].value = program
            .expression_table
            .static_machine_application_bindings(second)[1]
            .value
            .clone();
        changed
            .expression_table
            .set_static_machine_application_bindings(first, &forged);
        assert_eq!(
            changed
                .bodyless_callable_signature(first, target, &sources)
                .unwrap_err(),
            "bodyless call source or bindings changed after contract preparation"
        );
    }
    {
        let mut changed = program.clone();
        let mut arguments = program
            .expression_table
            .static_machine_application_arguments(first)
            .to_vec();
        arguments[0] = program
            .expression_table
            .static_machine_application_arguments(second)[0]
            .clone();
        changed
            .expression_table
            .set_static_machine_application_arguments(first, &arguments);
        assert_eq!(
            changed
                .bodyless_callable_signature(first, target, &sources)
                .unwrap_err(),
            "bodyless call source or bindings changed after contract preparation"
        );
    }
    let stale =
        StaticMachineApplicationHandle::from_parts(first.arena_index(), first.generation() + 1);
    assert_eq!(
        program
            .bodyless_callable_signature(stale, target, &sources)
            .unwrap_err(),
        "bodyless call has no live source application"
    );
    {
        let mut changed = program.clone();
        changed
            .expression_table
            .static_machine_application_mut(first)
            .lexical_owner = program
            .expression_table
            .static_machine_application(second)
            .lexical_owner;
        assert!(
            changed
                .bodyless_callable_signature(first, target, &sources)
                .unwrap()
                .is_none(),
            "another caller has no prepared header for this occurrence"
        );
    }
    let mut changed_sources = sources.clone();
    for (_, digest) in &mut changed_sources {
        digest[0] ^= 1;
    }
    assert_eq!(
        program
            .bodyless_callable_signature(first, target, &changed_sources)
            .unwrap_err(),
        "bodyless call source or bindings changed after contract preparation"
    );
}

#[test]
fn an_unsealed_bodyless_machine_does_not_gain_compiler_invocation_eligibility() {
    let mut program = retained_policy_calls();
    let (application, _) = retained_policy_call(&program, "first");
    let sources =
        source::source_identity::toolchain_source_identities(program.symbols.source_files())
            .unwrap();
    let position = program
        .machines()
        .iter()
        .position(|machine| machine.name.as_str() == "unsealed")
        .unwrap();
    let target = program.machines()[position].symbol;
    assert!(
        program
            .bodyless_machine_signature(target)
            .unwrap()
            .is_some()
    );
    program.machine_mut(position).supply_mode =
        language_semantics::MachineSupplyMode::CompilerProvided;
    assert!(program.bodyless_declaration(target).unwrap().is_none());
    assert!(
        program
            .retained_bodyless_declaration(target)
            .unwrap()
            .is_none()
    );
    assert!(
        program
            .compiler_primitive_callable_signature(application, target, &sources)
            .unwrap()
            .is_none()
    );
}
