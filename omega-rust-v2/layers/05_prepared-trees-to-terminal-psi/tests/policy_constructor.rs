#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_psi::{
    AggregateRegionSlot, BlockTerminator, MachineRegion, OperationKind, OperationResult, Parameter, PlaceProjection, PredicateOperator, PredicateStep,
    ReferenceAccess, ReturnEdge, ScalarLiteral, TypeShape, ValueOperand,
};

/// `new_plan<'t>(schema: @TypeSchema<'t>, issuance: Issuance<'t>) -> AccessPlan<'t>`
/// names `'t` only inside records the toolchain lowers, so the toolchain
/// publishes one executable body for it and no open template.
#[test]
fn the_library_plan_constructor_lowers_as_one_closed_body() {
    let library = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared = omega_preparation::prepare_source_package(&library).unwrap();
    let package = omega_lowering::lower_package(&prepared).unwrap();
    let core = |path: &str| format!(r"language\core\{path}");
    let found: Vec<_> = package
        .machines
        .iter()
        .filter(|m| m.declaration.package == omega_toolchain::TOOLCHAIN_PACKAGE && m.declaration.path == core(r"access\new_plan"))
        .collect();
    let [machine] = found.as_slice() else {
        panic!("the defining library publishes one constructor body")
    };
    assert!(package.machine_templates.iter().all(|template| template.declaration != machine.declaration));
    assert!(package.data_templates.is_empty());
    let data = |path: &str| {
        package
            .data
            .iter()
            .find(|data| data.declaration.path == core(path))
            .unwrap_or_else(|| panic!("the library publishes {path}"))
    };
    let (schema, issuance, plan, reach) = (data(r"reflect\TypeSchema"), data("Issuance"), data("AccessPlan"), data("BoundaryReach"));

    // One region. It fills the lifetime binder of the schema the first formal
    // refers to, of the issuance the second formal holds, and of the result.
    assert_eq!(
        machine.regions,
        [MachineRegion {
            aggregate_parameters: vec![
                AggregateRegionSlot { parameter: 0, binder: 0 },
                AggregateRegionSlot { parameter: 1, binder: 0 }
            ],
            aggregate_result: vec![0],
            ..Default::default()
        }]
    );
    assert!(machine.arguments.is_empty());
    assert_eq!(
        package.types.get(machine.parameter_types[0]),
        Some(&TypeShape::NominalReference {
            access: ReferenceAccess::Shared,
            referent: schema.id
        })
    );
    assert_eq!((machine.parameter_types[1], machine.result_type), (issuance.carrier, Some(plan.carrier)));

    // The plan passes its lifetime to each record it stores, and an issuance
    // passes it to the callables of its owned route sequence.
    let suffix = |path: &str| path.rsplit("::").next().unwrap().to_owned();
    let stored: Vec<_> = plan
        .fields
        .iter()
        .map(|field| (suffix(&field.declaration.path), field.aggregate_lifetimes.clone()))
        .collect();
    let lifetime = |name: &str, passed: &[usize]| (name.to_owned(), passed.to_vec());
    assert_eq!(
        stored,
        [
            lifetime("schema", &[0]),
            lifetime("issuance", &[0]),
            lifetime("fields", &[0]),
            lifetime("field_count", &[]),
            lifetime("readers", &[0]),
            lifetime("reader_count", &[]),
            lifetime("writers", &[0]),
            lifetime("writer_count", &[]),
            lifetime("reach", &[]),
        ]
    );
    let routes = &issuance.cases[0].fields[0];
    assert_eq!(routes.aggregate_lifetimes, [0]);
    assert_eq!(
        package.types.get(routes.carrier),
        Some(&TypeShape::ElementSequence {
            element: data(r"reflect\CallableInfo").carrier
        })
    );

    // The body copies the schema out of its reference and moves the issuance.
    let body = &machine.body;
    let [
        Parameter::Structural(schema_formal),
        Parameter::Structural(issuance_formal),
    ] = body.parameters.as_slice()
    else {
        panic!("two structural formals")
    };
    let result = |index: usize| {
        let OperationResult::Structural(place) = body.operations[index].result else {
            panic!("{:?}", body.operations[index])
        };
        place
    };
    let OperationKind::CopyStructural { source } = &body.operations[0].kind else {
        panic!("{:?}", body.operations[0])
    };
    assert_eq!((source.root, source.path.as_slice()), (*schema_formal, &[PlaceProjection::Referent][..]));
    assert_eq!(body.places[result(0).index()].ty, schema.carrier);
    let OperationKind::MoveStructural { source } = &body.operations[1].kind else {
        panic!("{:?}", body.operations[1])
    };
    assert_eq!((source.root, source.path.as_slice()), (*issuance_formal, &[][..]));

    // Every other field is its zero, formed by the body: 32 empty field slots,
    // 32 empty reader and 32 empty writer slots, three zero counts and a zeroed reach.
    let empty = |sum: &str| {
        body.operations
            .iter()
            .filter(|operation| matches!(&operation.kind, OperationKind::EstablishCase { case, fields } if case.path == core(sum) && fields.is_empty()))
            .count()
    };
    assert_eq!((empty("AccessFieldSlot::Empty"), empty("AccessRuleSlot::Empty")), (32, 64));
    let count = |wanted: fn(&OperationKind) -> bool| {
        body.operations
            .iter()
            .filter(|operation| wanted(&operation.kind))
            .count()
    };
    assert_eq!(
        count(|kind| matches!(kind, OperationKind::EstablishStructuralArray { elements } if elements.len() == 32)),
        3
    );
    assert_eq!(
        count(|kind| matches!(kind, OperationKind::Constant(ScalarLiteral::Integer(n)) if n == &0.into())),
        3
    );
    let zeroed: Vec<_> = body
        .operations
        .iter()
        .filter(|operation| operation.kind == OperationKind::EstablishZeroedStructural)
        .collect();
    let [zeroed] = zeroed.as_slice() else { panic!("one zeroed record") };
    let OperationResult::Structural(zeroed) = zeroed.result else {
        panic!("{zeroed:?}")
    };
    assert_eq!(body.places[zeroed.index()].ty, reach.carrier);
    assert_eq!(body.operations.len(), 2 + 96 + 3 + 3 + 1 + 1);

    // The last operation establishes the plan from those inputs, and the body returns it.
    let last = body.operations.len() - 1;
    let OperationKind::EstablishRecord { fields } = &body.operations[last].kind else {
        panic!("{:?}", body.operations[last])
    };
    assert_eq!(
        fields.iter().map(|field| &field.field).collect::<Vec<_>>(),
        plan.fields
            .iter()
            .map(|field| &field.declaration)
            .collect::<Vec<_>>()
    );
    assert_eq!(fields[0].operand, ValueOperand::CopyWhole(result(0)));
    assert_eq!(fields[1].operand, ValueOperand::MoveWhole(result(1)));
    assert_eq!(fields[8].operand, ValueOperand::MoveWhole(zeroed));
    for (field, declared) in fields.iter().zip(&plan.fields).skip(2) {
        match field.operand {
            ValueOperand::Scalar(value) => assert_eq!(body.values[value.index()].ty, declared.carrier),
            ValueOperand::MoveWhole(place) => assert_eq!(body.places[place.index()].ty, declared.carrier),
            ref other => panic!("{other:?}"),
        }
    }
    let [block] = body.blocks.as_slice() else { panic!("one block") };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, result: returned },
        cleanup,
        ..
    } = &block.terminator
    else {
        panic!("{:?}", block.terminator)
    };
    assert_eq!((*source, body.places[returned.index()].ty), (result(last), plan.carrier));
    assert!(cleanup.is_empty());
    let site = &body.diagnostics.operations[last];
    assert_eq!(
        (site.package.as_str(), site.file.as_str()),
        (omega_toolchain::TOOLCHAIN_PACKAGE, "language/core/access/access.omg")
    );

    assert_eq!(plan.requires.len(), 3);
    let expected = [
        vec!["field_count"],
        vec!["reader_count"],
        vec!["writer_count"],
        vec!["reach", "service_count"],
    ];
    assert_eq!(machine.contract.ensures.len(), expected.len());
    for (clause, path) in machine.contract.ensures.iter().zip(expected) {
        let [
            PredicateStep::Result,
            steps @ ..,
            PredicateStep::Integer(zero),
            PredicateStep::Apply(PredicateOperator::Equal),
        ] = &clause.predicate[..]
        else {
            panic!("result field must equal zero: {:?}", clause.predicate)
        };
        assert_eq!(zero, &0.into());
        let fields = steps
            .iter()
            .map(|step| {
                let PredicateStep::Field(id) = step else { panic!("result field") };
                assert_eq!(id.package, omega_toolchain::TOOLCHAIN_PACKAGE);
                suffix(&id.path)
            })
            .collect::<Vec<_>>();
        assert_eq!(fields, path);
    }
}

/// The constructor's first step, on a record of the same shape as the
/// toolchain's schema that is declared in the package reading it.
#[test]
fn a_policy_constructor_copies_the_selected_schema_through_its_reference() {
    let template = r"public data Schema<'t> [copy] { storage:@'t [u8]; }
        public machine read_schema<'t>(left:@Schema<'t>,right:@Schema<'t>)->Schema<'t>
            reaches none crashes none terminates { $SELECTED }";
    for (formal, name) in ["left", "right"].into_iter().enumerate() {
        let source = template.replace("SELECTED", name);
        let package = sources::lower(&[("copy.omg", &source)]).unwrap();
        assert!(package.machine_templates.is_empty());
        let [machine] = &package.machines[..] else { panic!("one closed machine") };
        let [schema] = &package.data[..] else { panic!("the schema record") };
        assert_eq!(
            (schema.declaration.path.as_str(), schema.lifetime_count, schema.fields[0].lifetime),
            ("Schema", 1, Some(0))
        );
        // Both formals refer to a schema at `'t`, and the result is one.
        assert_eq!(
            machine.regions,
            [MachineRegion {
                aggregate_parameters: vec![
                    AggregateRegionSlot { parameter: 0, binder: 0 },
                    AggregateRegionSlot { parameter: 1, binder: 0 }
                ],
                aggregate_result: vec![0],
                ..Default::default()
            }]
        );
        let reference = TypeShape::NominalReference {
            access: ReferenceAccess::Shared,
            referent: schema.id,
        };
        assert_eq!(
            machine
                .parameter_types
                .iter()
                .map(|row| package.types.get(*row))
                .collect::<Vec<_>>(),
            [Some(&reference); 2]
        );
        assert_eq!(machine.result_type, Some(schema.carrier));

        // The one operation reads the referent of the formal the source selects.
        let body = &machine.body;
        let [read] = &body.operations[..] else { panic!("one ordered read") };
        let OperationKind::CopyStructural { source: selected } = &read.kind else {
            panic!("{:?}", read.kind)
        };
        assert_eq!(Parameter::Structural(selected.root), body.parameters[formal]);
        assert_eq!(selected.path, [PlaceProjection::Referent]);
        assert_eq!(
            body.diagnostics.operations[0].span.slice(source.as_bytes()),
            Some(format!("${name}").as_bytes())
        );
        let OperationResult::Structural(copy) = read.result else {
            panic!("{:?}", read.result)
        };
        assert_eq!(body.places[copy.index()].ty, schema.carrier);
        let [block] = body.blocks.as_slice() else { panic!("one block") };
        assert!(
            matches!(&block.terminator, BlockTerminator::Return { result: ReturnEdge::Structural { source, .. }, cleanup, .. } if *source == copy && cleanup.is_empty())
        );
    }
}
