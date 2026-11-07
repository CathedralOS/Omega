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
    types::TypeReferenceNode,
};

struct Fixture {
    program: TypedTrees,
    owner: SymbolHandle,
    impostor: SymbolHandle,
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
                ]),
        );
        let owner = children.handle_at(u32::try_from(children.len() - 2).unwrap());
        let impostor = children.handle_at(u32::try_from(children.len() - 1).unwrap());
        let params = symbols.insert_children(
            owner,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("left")),
                (SymbolKind::Parameter, SymbolNameRef::Static("right")),
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
                    name: Identifier::from(atom.name()),
                })
        };
        let carrier = type_reference(carrier);
        let other_carrier = type_reference(other);
        let boolean_carrier = type_reference(BuiltinTypeAtom::Bool);
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
            parameters,
            subjects,
            other_carrier,
            boolean_carrier,
        }
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
