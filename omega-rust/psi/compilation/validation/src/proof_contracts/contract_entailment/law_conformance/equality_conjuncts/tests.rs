use super::*;
use symbols::{
    BuiltinTypeAtom, FixedCoreDeclaration, SymbolKind, SymbolNameRef, SymbolTableBuilder,
    builtin_function_symbols, builtin_type_symbols, fixed_core_declaration_symbols,
};
use typed_trees::{
    TypedTrees,
    expression::{
        Expression, ExpressionNode, NamePath, StaticMachineArgument, TableBinaryExpression,
        TableCallExpression,
    },
    name::Identifier,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

fn matching_equality_conjuncts(
    program: &dyn crate::CheckingInput,
    required: EqualityConjunct<'_>,
    required_binders: &[(SymbolHandle, String)],
    provided: EqualityConjunct<'_>,
    provided_binders: &[(SymbolHandle, String)],
) -> bool {
    SourceCorrespondence::default().matching_conjuncts(
        program,
        required,
        required_binders,
        provided,
        provided_binders,
    )
}

struct Fixture {
    program: TypedTrees,
    owner: SymbolHandle,
    impostor: SymbolHandle,
    namespace: SymbolHandle,
    trait_symbol: SymbolHandle,
    slot: SymbolHandle,
    entry: SymbolHandle,
    other_entry: SymbolHandle,
    generic_carrier: TypeReferenceHandle,
    parameters: [StateParameter; 2],
    subjects: [ExpressionHandle; 2],
    other_carrier: TypeReferenceHandle,
    boolean_carrier: TypeReferenceHandle,
}

impl Fixture {
    fn new(carrier: BuiltinTypeAtom, other: BuiltinTypeAtom) -> Self {
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = symbols.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain(builtin_function_symbols())
                .chain(fixed_core_declaration_symbols())
                .chain([
                    (SymbolKind::Machine, SymbolNameRef::Static("law")),
                    (
                        SymbolKind::MathematicalDefinition,
                        SymbolNameRef::Static("Identity"),
                    ),
                    (SymbolKind::Conformance, SymbolNameRef::Static("Selected")),
                    (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                    (SymbolKind::Trait, SymbolNameRef::Static("Operations")),
                ]),
        );
        let owner = children.handle_at(u32::try_from(children.len() - 5).unwrap());
        let impostor = children.handle_at(u32::try_from(children.len() - 4).unwrap());
        let namespace = children.handle_at(u32::try_from(children.len() - 3).unwrap());
        let generic = children.handle_at(u32::try_from(children.len() - 2).unwrap());
        let trait_symbol = children.handle_at(u32::try_from(children.len() - 1).unwrap());
        let slot = symbols
            .insert_children(
                trait_symbol,
                [(SymbolKind::State, SymbolNameRef::Static("operation"))],
            )
            .handle_at(0);
        let params = symbols.insert_children(
            owner,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("left")),
                (SymbolKind::Parameter, SymbolNameRef::Static("right")),
                (SymbolKind::State, SymbolNameRef::Static("apply")),
                (SymbolKind::State, SymbolNameRef::Static("other")),
            ],
        );
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let mut type_reference = |atom: BuiltinTypeAtom| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: children.handle_at(atom.ordinal() as u32),
                    name: Identifier::from(atom.symbol_name()),
                })
        };
        let carrier = type_reference(carrier);
        let other_carrier = type_reference(other);
        let boolean_carrier = type_reference(BuiltinTypeAtom::Bool);
        let generic_carrier = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: generic,
                name: Identifier::from("T"),
            });
        let parameters = ["left", "right"].map(|name| StateParameter {
            name: Identifier::from(name),
            symbol: params.handle_at(u32::from(name == "right")),
            type_reference: carrier,
            ..Default::default()
        });
        let subjects = parameters.each_ref().map(|parameter| {
            program
                .expression_table
                .insert_tree(&Expression::Name(NamePath::resolved(
                    vec![parameter.name.clone()],
                    parameter.symbol,
                    parameter.symbol,
                )))
        });
        Self {
            program,
            owner,
            impostor,
            namespace,
            trait_symbol,
            slot,
            entry: params.handle_at(2),
            other_entry: params.handle_at(3),
            generic_carrier,
            parameters,
            subjects,
            other_carrier,
            boolean_carrier,
        }
    }

    fn install_machine(&mut self, result: TypeReferenceHandle, borrowed: bool) {
        let mut machine = typed_trees::machine::Machine {
            symbol: self.owner,
            name: Identifier::from("selected"),
            ..Default::default()
        };
        for (symbol, name) in [(self.entry, "apply"), (self.other_entry, "other")] {
            let mut entry = typed_trees::state::State {
                symbol,
                name: Identifier::from(name),
                return_type: result,
                ..Default::default()
            };
            for parameter in &self.parameters {
                let mut parameter = parameter.clone();
                if borrowed {
                    parameter.type_reference =
                        self.program
                            .type_reference_table
                            .insert(TypeReferenceNode::Reference {
                                referee: parameter.type_reference,
                                access: language_core::ReferenceAccess::Shared,
                                lifetime: None,
                            });
                }
                self.program.push_state_parameter(&mut entry, parameter);
            }
            self.program.push_machine_state(&mut machine, entry);
        }
        self.program.push_machine(machine);
    }

    fn binary(
        &mut self,
        operator: BinaryOperator,
        subjects: [ExpressionHandle; 2],
    ) -> ExpressionHandle {
        self.program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: subjects[0],
                operator,
                right: subjects[1],
            }))
    }

    fn identity(
        &mut self,
        carrier: TypeReferenceHandle,
        subjects: [ExpressionHandle; 2],
    ) -> ExpressionHandle {
        let target = self
            .program
            .symbols
            .fixed_core_declaration_symbol(FixedCoreDeclaration::Identity)
            .unwrap();
        let arguments = self
            .program
            .expression_table
            .insert_expression_handles(subjects);
        self.program
            .expression_table
            .insert(ExpressionNode::Call(TableCallExpression {
                target_symbol: target,
                target: Identifier::from("Identity"),
                machine_arguments: Box::new([StaticMachineArgument {
                    type_reference: carrier,
                    ..Default::default()
                }]),
                arguments,
                authored_call_selection: None,
                authored_call_binding: None,
                authored_machine_application: Default::default(),
                exact_coercion_target: Default::default(),
                requirement_application: Default::default(),
                build_selection: Default::default(),
                receiver: Default::default(),
                static_machine_parameter: Default::default(),
                static_requirement_dispatch: None,
                quotient_operation: None,
                private_layout_operation: None,
                layout_query: None,
                native_code_layout: None,
                evidence_arguments: Box::new([]),
                operational_acknowledgement: Default::default(),
            }))
    }

    fn read(&self, expression: ExpressionHandle) -> Option<EqualityConjunct<'_>> {
        equality_conjunct(&self.program, self.owner, &self.parameters, expression)
    }
}

#[test]
fn typed_identity_conjunct_retains_carrier_subjects_and_sealed_declaration() {
    let mut f = Fixture::new(BuiltinTypeAtom::U64, BuiltinTypeAtom::U16);
    let carrier = f.parameters[0].type_reference;
    let expression = f.identity(carrier, f.subjects);
    let value = f.read(expression).unwrap().identity().unwrap();
    assert_eq!(value.carrier, carrier);
    assert_eq!(value.subjects, f.subjects);
    // The reader returns the original obligation, without an entailment receipt.
    assert_eq!(f.read(expression).unwrap().expression(), expression);
    let reversed = f.identity(carrier, [f.subjects[1], f.subjects[0]]);
    assert!(matching_equality_conjuncts(
        &f.program,
        f.read(expression).unwrap(),
        &[],
        f.read(reversed).unwrap(),
        &[]
    ));
    let other = f.identity(f.other_carrier, f.subjects);
    assert!(!matching_equality_conjuncts(
        &f.program,
        f.read(expression).unwrap(),
        &[],
        f.read(other).unwrap(),
        &[]
    ));
    let ExpressionNode::Call(call) = f.program.expression_table.expression_mut(expression) else {
        unreachable!()
    };
    call.target_symbol = f.impostor;
    assert!(f.read(expression).is_none());
}

#[test]
fn scalar_identity_bridge_requires_same_complete_non_ieee_carrier() {
    let mut f = Fixture::new(BuiltinTypeAtom::U64, BuiltinTypeAtom::U16);
    let comparison = f.binary(BinaryOperator::Equal, f.subjects);
    assert!(f.read(comparison).unwrap().identity().is_some());
    let identity = f.identity(f.parameters[0].type_reference, f.subjects);
    assert!(matching_equality_conjuncts(
        &f.program,
        f.read(identity).unwrap(),
        &[],
        f.read(comparison).unwrap(),
        &[]
    ));
    f.parameters[1].type_reference = f.other_carrier;
    assert!(f.read(comparison).unwrap().identity().is_none());

    let mut ieee = Fixture::new(BuiltinTypeAtom::F64, BuiltinTypeAtom::U64);
    let comparison = ieee.binary(BinaryOperator::Equal, ieee.subjects);
    assert!(ieee.read(comparison).unwrap().identity().is_none());
    let identity = ieee.identity(ieee.parameters[0].type_reference, ieee.subjects);
    assert!(!matching_equality_conjuncts(
        &ieee.program,
        ieee.read(identity).unwrap(),
        &[],
        ieee.read(comparison).unwrap(),
        &[]
    ));
}

#[test]
fn conjunction_receipts_require_every_leaf_and_keep_type_equations_separate() {
    let mut f = Fixture::new(BuiltinTypeAtom::U64, BuiltinTypeAtom::U16);
    let identity = f.identity(f.parameters[0].type_reference, f.subjects);
    let unsupported = f
        .program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let conjunction = f.binary(BinaryOperator::And, [identity, unsupported]);
    let mut conjuncts = Vec::new();
    assert!(!collect_equality_conjuncts(
        &f.program,
        f.owner,
        &f.parameters,
        conjunction,
        &mut conjuncts
    ));
    assert_eq!(conjuncts.len(), 1);
    assert_eq!(conjuncts[0].expression(), identity);
    let types = [f.parameters[0].type_reference, f.other_carrier].map(|reference| {
        f.program
            .expression_table
            .insert(ExpressionNode::TypeExpression(reference))
    });
    let equation = f.binary(BinaryOperator::Equal, types);
    let equation = f.read(equation).unwrap();
    assert!(matches!(equation.meaning, EqualityMeaning::TypeEquality));
    assert!(equation.identity().is_none());
    assert!(matching_equality_conjuncts(
        &f.program,
        equation,
        &[],
        equation,
        &[]
    ));
}

#[test]
fn selected_comparison_calls_remain_comparisons_and_retain_static_actuals() {
    let mut f = Fixture::new(BuiltinTypeAtom::U64, BuiltinTypeAtom::U16);
    f.program
        .push_operator(typed_trees::operator::OperatorDefinition {
            symbol: f.impostor,
            spelling: Some(language_core::OperatorSpelling::Equal),
            return_type: f.boolean_carrier,
            ..Default::default()
        });
    let first = f.identity(f.parameters[0].type_reference, f.subjects);
    let second = f.identity(f.other_carrier, f.subjects);
    for expression in [first, second] {
        let ExpressionNode::Call(call) = f.program.expression_table.expression_mut(expression)
        else {
            unreachable!()
        };
        call.target_symbol = f.impostor;
    }
    let first = f.read(first).unwrap();
    let second = f.read(second).unwrap();
    assert!(first.identity().is_none());
    assert!(second.identity().is_none());
    assert!(matching_equality_conjuncts(
        &f.program,
        first,
        &[],
        first,
        &[]
    ));
    assert!(!matching_equality_conjuncts(
        &f.program,
        first,
        &[],
        second,
        &[]
    ));
}

#[test]
fn identity_law_matching_preserves_distinct_parameter_roles() {
    let mut f = Fixture::new(BuiltinTypeAtom::U64, BuiltinTypeAtom::U16);
    let carrier = f.parameters[0].type_reference;
    let sum = f.binary(BinaryOperator::Add, f.subjects);
    let required = f.identity(carrier, [sum, f.subjects[0]]);
    let reversed_sum = f.binary(BinaryOperator::Add, [f.subjects[1], f.subjects[0]]);
    let provided = f.identity(carrier, [reversed_sum, f.subjects[1]]);
    let required_binders = [
        (f.parameters[0].symbol, "$V0".to_owned()),
        (f.parameters[1].symbol, "$V1".to_owned()),
    ];
    let provided_binders = [
        (f.parameters[0].symbol, "$V1".to_owned()),
        (f.parameters[1].symbol, "$V0".to_owned()),
    ];
    assert!(matching_equality_conjuncts(
        &f.program,
        f.read(required).unwrap(),
        &required_binders,
        f.read(provided).unwrap(),
        &provided_binders
    ));
    assert!(!matching_equality_conjuncts(
        &f.program,
        f.read(required).unwrap(),
        &required_binders,
        f.read(provided).unwrap(),
        &required_binders
    ));
    let repeated_sum = f.binary(BinaryOperator::Add, [f.subjects[0]; 2]);
    let repeated = f.identity(carrier, [repeated_sum, f.subjects[0]]);
    assert!(!matching_equality_conjuncts(
        &f.program,
        f.read(required).unwrap(),
        &required_binders,
        f.read(repeated).unwrap(),
        &required_binders
    ));
}

#[test]
fn comparison_namespace_keeps_both_explicit_borrowed_operands() {
    let mut f = Fixture::new(BuiltinTypeAtom::U64, BuiltinTypeAtom::U16);
    f.install_machine(f.boolean_carrier, true);
    let receiver = f
        .program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::from("Selected")],
            f.namespace,
            f.namespace,
        )));
    let borrowed = f.subjects.map(|target| {
        f.program.expression_table.insert(ExpressionNode::Borrow(
            typed_trees::expression::TableBorrowExpression {
                target,
                access: language_core::ReferenceAccess::Shared,
            },
        ))
    });
    let arguments = f
        .program
        .expression_table
        .insert_expression_handles(borrowed);
    let expression = f.identity(f.parameters[0].type_reference, f.subjects);
    let ExpressionNode::Call(call) = f.program.expression_table.expression_mut(expression) else {
        unreachable!()
    };
    call.receiver = receiver;
    call.target_symbol = f.entry;
    call.target = Identifier::from("apply");
    call.machine_arguments = Box::new([]);
    call.arguments = arguments;
    let comparison = f.read(expression).unwrap();
    assert!(comparison.identity().is_none());
    assert_eq!(comparison.expression(), expression);
    let original = f.program.expression_table.call_node(expression).unwrap();
    assert_eq!(original.receiver, receiver);
    assert_eq!(
        f.program
            .expression_table
            .expression_handles(original.arguments),
        borrowed
    );
    let mut other_call = original.clone();
    other_call.target_symbol = f.other_entry;
    other_call.target = Identifier::from("other");
    let other = f
        .program
        .expression_table
        .insert(ExpressionNode::Call(other_call));
    assert!(f.read(other).unwrap().identity().is_none());
    assert!(!matching_equality_conjuncts(
        &f.program,
        f.read(expression).unwrap(),
        &[],
        f.read(other).unwrap(),
        &[]
    ));
}

#[test]
fn selected_slot_rewrite_replaces_target_and_result_carrier_before_matching() {
    use super::super::super::StructuralTerm;
    use super::super::slot_bindings::{
        TraitOperationSlots, diagnostic_shape_match, rewrite_slot_applications,
    };
    let mut f = Fixture::new(BuiltinTypeAtom::U64, BuiltinTypeAtom::U16);
    let carrier = f.parameters[0].type_reference;
    f.install_machine(carrier, false);
    let arguments = f
        .parameters
        .iter()
        .map(|parameter| StructuralTerm::Variable(parameter.name.as_str().to_owned()))
        .collect::<Vec<_>>();
    let mut declaration = typed_trees::trait_definition::TraitDefinition {
        symbol: f.trait_symbol,
        name: Identifier::from("Operations"),
        ..Default::default()
    };
    f.program.push_trait_machine_signature(
        &mut declaration,
        typed_trees::signature::StateSignature {
            symbol: f.slot,
            name: Identifier::from("operation"),
            return_type: f.generic_carrier,
            ..Default::default()
        },
    );
    f.program.push_trait_definition(declaration);
    let declaration = f
        .program
        .trait_definition_by_symbol(f.trait_symbol)
        .unwrap();
    let slots = TraitOperationSlots::new(&f.program, declaration, None);
    let slot_key = slots.resolve(&f.program, f.slot).unwrap().0;
    let abstract_call = StructuralTerm::Application {
        target: f.slot,
        result_type: f.generic_carrier,
        selections: Vec::new(),
        machine: "operation".to_owned(),
        arguments: arguments.clone(),
    };
    let concrete_call = StructuralTerm::Application {
        target: f.entry,
        result_type: carrier,
        selections: Vec::new(),
        machine: "selected".to_owned(),
        arguments,
    };
    let mut missing = Vec::new();
    let rewritten = rewrite_slot_applications(
        &f.program,
        &abstract_call,
        &slots,
        &[(slot_key, ("selected".to_owned(), f.entry))],
        &mut missing,
    );
    assert!(missing.is_empty());
    let matches =
        |provided| diagnostic_shape_match(&f.program, &rewritten, provided, &[], &mut Vec::new());
    assert!(!diagnostic_shape_match(
        &f.program,
        &abstract_call,
        &concrete_call,
        &[],
        &mut Vec::new()
    ));
    assert!(matches(&concrete_call));
    let mut unrelated = abstract_call.clone();
    let StructuralTerm::Application { target, .. } = &mut unrelated else {
        unreachable!()
    };
    *target = f.other_entry;
    let unchanged = rewrite_slot_applications(
        &f.program,
        &unrelated,
        &slots,
        &[(slot_key, ("selected".to_owned(), f.entry))],
        &mut Vec::new(),
    );
    assert_eq!(unchanged, unrelated);
    assert!(!slots.covers_term(&f.program, &unrelated));
    assert!(slots.covers_term(&f.program, &abstract_call));
    let mut wrong_target = concrete_call.clone();
    let StructuralTerm::Application { target, .. } = &mut wrong_target else {
        unreachable!()
    };
    *target = f.other_entry;
    assert!(!matches(&wrong_target));
    let mut wrong_selection = concrete_call.clone();
    let StructuralTerm::Application { selections, .. } = &mut wrong_selection else {
        unreachable!()
    };
    selections.push(StaticMachineArgument {
        type_reference: f.other_carrier,
        ..Default::default()
    });
    assert!(!matches(&wrong_selection));
}

#[test]
fn source_correspondence_reuses_scope_without_accepting_cycles_or_stale_handles() {
    let mut f = Fixture::new(BuiltinTypeAtom::U64, BuiltinTypeAtom::U16);
    let value = f
        .program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let conjunction = f.binary(BinaryOperator::And, [value; 2]);
    let mut correspondence = SourceCorrespondence::default();
    assert!(
        correspondence
            .canonical_source_value(&f.program, (f.owner, &f.parameters), conjunction, &[])
            .is_some()
    );
    assert!(
        correspondence
            .canonical_source_value(&f.program, (f.owner, &f.parameters), conjunction, &[])
            .is_some()
    );
    let stale =
        ExpressionHandle::from_parts(conjunction.arena_index(), conjunction.generation() + 1);
    assert!(
        correspondence
            .canonical_source_value(&f.program, (f.owner, &f.parameters), stale, &[])
            .is_none()
    );
    let ExpressionNode::Binary(binary) = f.program.expression_table.expression_mut(conjunction)
    else {
        unreachable!()
    };
    binary.right = conjunction;
    assert!(
        correspondence
            .canonical_source_value(&f.program, (f.owner, &f.parameters), conjunction, &[])
            .is_none()
    );
    let mut shared = value;
    for _ in 0..20 {
        shared = f.binary(BinaryOperator::And, [shared; 2]);
    }
    let scope = (f.owner, f.parameters.as_slice());
    assert!(
        correspondence
            .canonical_source_value(&f.program, scope, shared, &[])
            .is_none()
    );
    assert!(!correspondence.matching_operation_payloads(
        &f.program,
        scope,
        shared,
        &[],
        scope,
        shared,
        &[]
    ));
    let mut remaining_work = MAX_SOURCE_MATCH_WORK;
    assert!(!neutral_operation_payloads(
        &f.program,
        shared,
        &mut remaining_work
    ));
}

#[test]
fn source_call_evidence_cannot_hide_behind_an_equal_index_key() {
    let mut f = Fixture::new(BuiltinTypeAtom::U64, BuiltinTypeAtom::U16);
    f.install_machine(f.boolean_carrier, false);
    let expression = f.identity(f.parameters[0].type_reference, f.subjects);
    let ExpressionNode::Call(call) = f.program.expression_table.expression_mut(expression) else {
        unreachable!()
    };
    call.target_symbol = f.entry;
    call.target = Identifier::from("apply");
    call.machine_arguments = Box::new([]);
    let mut with_evidence = f
        .program
        .expression_table
        .call_node(expression)
        .unwrap()
        .clone();
    with_evidence.evidence_arguments = Box::new([Identifier::from("unresolved_evidence")]);
    let with_evidence = f
        .program
        .expression_table
        .insert(ExpressionNode::Call(with_evidence));
    let scope = (f.owner, f.parameters.as_slice());
    let mut correspondence = SourceCorrespondence::default();
    assert_eq!(
        correspondence.canonical_source_value(&f.program, scope, expression, &[]),
        correspondence.canonical_source_value(&f.program, scope, with_evidence, &[])
    );
    assert!(!correspondence.matching_values(
        &f.program,
        scope,
        expression,
        &[],
        scope,
        with_evidence,
        &[]
    ));
}

#[test]
fn cast_domain_payload_cannot_hide_behind_an_equal_index_key() {
    let mut f = Fixture::new(BuiltinTypeAtom::U64, BuiltinTypeAtom::U16);
    let cast = f
        .program
        .expression_table
        .insert_tree(&Expression::Cast(Box::new(
            typed_trees::expression::CastExpression {
                value: Expression::Name(NamePath::resolved(
                    vec![f.parameters[0].name.clone()],
                    f.parameters[0].symbol,
                    f.parameters[0].symbol,
                )),
                target_type: f.parameters[0].type_reference,
                target_label: NamePath::unresolved(vec![Identifier::from("u64")]),
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
                form: language_core::cast_form::CastForm::Value,
            },
        )));
    let mut other = *f.program.expression_table.cast_node(cast).unwrap();
    other.semantic_domain_symbol = f.trait_symbol;
    let other = f
        .program
        .expression_table
        .insert(ExpressionNode::Cast(other));
    let scope = (f.owner, f.parameters.as_slice());
    let mut correspondence = SourceCorrespondence::default();
    assert_eq!(
        correspondence.canonical_source_value(&f.program, scope, cast, &[]),
        correspondence.canonical_source_value(&f.program, scope, other, &[])
    );
    assert!(correspondence.matching_values(&f.program, scope, cast, &[], scope, cast, &[]));
    assert!(!correspondence.matching_values(&f.program, scope, cast, &[], scope, other, &[]));
}
