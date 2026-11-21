#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_psi::{PredicateOperator, PredicateStep, TemplateAccess, TemplateStep, TypeExpressionStep};
use psi_handoff_consumer::{TemplateValue, template_result};

#[test]
fn the_library_plan_constructor_lowers_as_an_ordinary_template() {
    let library = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared = omega_preparation::prepare_source_package(&library).unwrap();
    let package = omega_lowering::lower_package(&prepared).unwrap();
    let machine = package
        .machine_templates
        .iter()
        .find(|m| m.declaration.package == omega_toolchain::TOOLCHAIN_PACKAGE && m.declaration.path == r"language\core\access\new_plan")
        .expect("the defining library publishes the constructor");
    let [
        TemplateStep::CopyReferent { formal: 0, .. },
        TemplateStep::MoveFormal { formal: 1, ty, .. },
        TemplateStep::Construct(construct),
    ] = &machine.body[..]
    else {
        panic!("copy schema, transfer issuance, then construct")
    };
    assert_eq!(ty, &machine.signature.parameters[1]);
    assert_eq!(construct.declaration.package, omega_toolchain::TOOLCHAIN_PACKAGE);
    assert_eq!(construct.declaration.path, r"language\core\AccessPlan");
    let returned = template_result(
        &package,
        machine,
        &[
            TemplateValue::Borrowed(Box::new(TemplateValue::Argument(0))),
            TemplateValue::Argument(1),
        ],
    )
    .unwrap();
    let TemplateValue::Construct { fields, establishes: true, .. } = returned else {
        panic!("constrained construction")
    };
    let suffix = |path: &str| path.rsplit("::").next().unwrap().to_owned();
    assert_eq!(
        fields
            .iter()
            .map(|(field, _)| suffix(field))
            .collect::<Vec<_>>(),
        [
            "schema",
            "issuance",
            "fields",
            "field_count",
            "readers",
            "reader_count",
            "writers",
            "writer_count",
            "reach"
        ]
    );
    assert_eq!(fields[0].1, TemplateValue::Argument(0));
    assert_eq!(fields[1].1, TemplateValue::Argument(1));
    assert!(
        fields[2..]
            .iter()
            .all(|(_, v)| matches!(v, TemplateValue::Zero(_)))
    );
    let TemplateValue::Zero(reach) = fields.last().unwrap().1 else {
        unreachable!()
    };
    assert!(matches!(&reach.0[..], [TypeExpressionStep::Named { declaration, argument_count: 0 }]
        if declaration.package == omega_toolchain::TOOLCHAIN_PACKAGE && declaration.path == r"language\core\BoundaryReach"));
    let expected = [
        vec!["field_count"],
        vec!["reader_count"],
        vec!["writer_count"],
        vec!["reach", "service_count"],
    ];
    assert_eq!(machine.signature.ensures.len(), expected.len());
    for (clause, path) in machine.signature.ensures.iter().zip(expected) {
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

#[test]
fn a_policy_constructor_copies_the_selected_schema_through_its_reference() {
    let template = r"use omega\language\core\reflect;
        public machine read_schema<'t>(left:@TypeSchema<'t>,right:@TypeSchema<'t>)->TypeSchema<'t>
            reaches none crashes none terminates { $SELECTED }";
    for (formal, name) in ["left", "right"].into_iter().enumerate() {
        let source = template.replace("SELECTED", name);
        let package = sources::lower(&[("copy.omg", &source)]).unwrap();
        let [machine] = &package.machine_templates[..] else {
            panic!("one open machine")
        };
        let [TemplateStep::CopyReferent { formal: selected, ty, site }] = &machine.body[..] else {
            panic!("one ordered read")
        };
        assert_eq!(*selected, formal);
        assert_eq!(site.span.slice(source.as_bytes()), Some(format!("${name}").as_bytes()));
        assert_eq!(Some(ty), machine.signature.result.as_ref());
        let (last, referent) = machine.signature.parameters[*selected].0.split_last().unwrap();
        assert_eq!(last, &TypeExpressionStep::Reference(TemplateAccess::Shared));
        assert_eq!(referent, ty.0.as_slice());
        let arguments = [0, 1].map(|id| TemplateValue::Borrowed(Box::new(TemplateValue::Argument(id))));
        assert_eq!(template_result(&package, machine, &arguments).unwrap(), TemplateValue::Argument(formal));
    }
}
