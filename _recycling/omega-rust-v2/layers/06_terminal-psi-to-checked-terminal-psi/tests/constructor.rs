//! The toolchain publishes `new_plan` as one closed machine body. These tests
//! check that body as the toolchain lowers it, then change one fact of the
//! unit at a time and require the proof to fail.
use omega_checker::{FieldOrigin, check_record_constructor};
use omega_psi::{
    BlockTerminator, CleanupAction, DeclarationId, LoweredMachine, LoweredPackage, OperationKind, Parameter, PlaceId, PredicateOperator, PredicateStep,
    ReturnEdge, ScalarLiteral, TemplateAccess, TypeExpressionStep as Step, ValueOperand,
};

const CORE: &str = r"language\core\";

fn toolchain_unit() -> (LoweredPackage, DeclarationId) {
    let source = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared = omega_preparation::prepare_source_package(&source).expect("prepare the defining toolchain package");
    let unit = omega_lowering::lower_package(&prepared).expect("lower the defining toolchain package");
    let id = unit
        .machines
        .iter()
        .find(|machine| machine.declaration.package == omega_toolchain::TOOLCHAIN_PACKAGE && machine.declaration.path == r"language\core\access\new_plan")
        .expect("provided constructor body")
        .declaration
        .clone();
    (unit, id)
}

fn data<'a>(
    unit: &'a mut LoweredPackage,
    name: &str,
) -> &'a mut omega_psi::LoweredData {
    unit.data
        .iter_mut()
        .find(|data| data.declaration.path.strip_prefix(CORE) == Some(name))
        .unwrap_or_else(|| panic!("the toolchain publishes {name}"))
}

/// The place an entry formal arrives in.
fn formal(
    machine: &LoweredMachine,
    position: usize,
) -> PlaceId {
    let Parameter::Structural(place) = machine.body.parameters[position] else {
        panic!("structural formal")
    };
    place
}

/// The initializers of the returned record, which the last operation establishes.
fn returned(machine: &mut LoweredMachine) -> &mut Vec<omega_psi::FieldInitializer> {
    let Some(OperationKind::EstablishRecord { fields }) = machine
        .body
        .operations
        .last_mut()
        .map(|operation| &mut operation.kind)
    else {
        panic!("the body ends by establishing its record")
    };
    fields
}

fn first(
    machine: &mut LoweredMachine,
    wanted: fn(&OperationKind) -> bool,
) -> &mut OperationKind {
    machine
        .body
        .operations
        .iter_mut()
        .map(|operation| &mut operation.kind)
        .find(|kind| wanted(kind))
        .expect("the body has such an operation")
}

#[test]
fn constructor_proof_binds_its_input_transfers_zeroes_and_nested_guarantees() {
    let (unit, id) = toolchain_unit();
    // The toolchain publishes one body for the constructor and no open form of it.
    assert!(
        unit.machine_templates
            .iter()
            .all(|template| template.declaration != id)
    );
    let checked = check_record_constructor(&unit, &id).unwrap();
    assert!(std::ptr::eq(checked.unit(), &unit));
    let machine = checked.machine();
    assert!(std::ptr::eq(machine, unit.machines.iter().find(|m| m.declaration == id).unwrap()));

    // The declared interface is read from the body's rows and its one region.
    let region = Step::GenericParameter { owner: id.clone(), index: 0 };
    let named = |ty: &omega_psi::TypeExpression| -> Vec<String> {
        ty.0.iter()
            .map(|step| match step {
                Step::Named { declaration, argument_count } => format!("{}/{argument_count}", declaration.path.strip_prefix(CORE).unwrap()),
                Step::Reference(TemplateAccess::Shared) => "@".to_owned(),
                step if step == &region => "'t".to_owned(),
                other => panic!("{other:?}"),
            })
            .collect()
    };
    let [schema, issuance] = checked.parameters() else { panic!("two formals") };
    assert_eq!(named(schema), ["'t", r"reflect\TypeSchema/1", "@"]);
    assert_eq!(named(issuance), ["'t", "Issuance/1"]);
    assert_eq!(named(&checked.result().ty), ["'t", "AccessPlan/1"]);
    assert_eq!(checked.result().declaration.path, r"language\core\AccessPlan");
    assert!(checked.result().establishes);
    // The establishing operation is the authored construction in the toolchain's file.
    let site = checked.result().site;
    assert_eq!(
        (site.package.as_str(), site.file.as_str()),
        (omega_toolchain::TOOLCHAIN_PACKAGE, "language/core/access/access.omg")
    );
    assert!(std::ptr::eq(site, machine.body.diagnostics.operations.last().unwrap()));

    let fields = checked.fields();
    let names: Vec<_> = fields
        .iter()
        .map(|(field, _)| field.path.rsplit("::").next().unwrap())
        .collect();
    assert_eq!(
        names,
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
    let FieldOrigin::CopySharedFormal { formal: 0, ty: copied } = &fields[0].1 else {
        panic!("{:?}", fields[0].1)
    };
    assert_eq!(named(copied), ["'t", r"reflect\TypeSchema/1"]);
    assert_eq!(
        fields[1].1,
        FieldOrigin::MoveFormal {
            formal: 1,
            ty: issuance.clone()
        }
    );
    assert!(
        fields[2..]
            .iter()
            .all(|(_, field)| matches!(field, FieldOrigin::Zero(_)))
    );
    let FieldOrigin::Zero(slots) = &fields[2].1 else { unreachable!() };
    assert_eq!(slots.0.last(), Some(&Step::Array(32)));
    assert_eq!(named(&omega_psi::TypeExpression(slots.0[..2].to_vec())), ["'t", "AccessFieldSlot/1"]);
    assert_eq!(machine.contract.ensures.len(), 4);
}

#[test]
fn a_source_body_and_catalog_operation_cannot_claim_the_same_machine() {
    let (mut unit, id) = toolchain_unit();
    check_record_constructor(&unit, &id).unwrap();
    unit.catalog_declarations.push(id.clone());
    assert!(check_record_constructor(&unit, &id).is_err());
}

#[test]
fn requirement_identity_cannot_also_identify_the_constructor() {
    let (mut unit, id) = toolchain_unit();
    check_record_constructor(&unit, &id).unwrap();
    let requirement = unit
        .traits
        .iter_mut()
        .flat_map(|owner| &mut owner.dependent_requirements)
        .next()
        .unwrap();
    requirement.declaration = id.clone();
    assert!(check_record_constructor(&unit, &id).is_err());
}

#[test]
fn changing_one_fact_of_the_closed_body_invalidates_its_proof() {
    type Mutation = fn(&mut LoweredMachine);
    let changes: &[(&str, Mutation)] = &[
        ("read the issuance as a shared schema", |m| {
            let issuance = formal(m, 1);
            let OperationKind::CopyStructural { source } = first(m, |kind| matches!(kind, OperationKind::CopyStructural { .. })) else {
                unreachable!()
            };
            source.root = issuance;
        }),
        ("copy the reference instead of its referent", |m| {
            let OperationKind::CopyStructural { source } = first(m, |kind| matches!(kind, OperationKind::CopyStructural { .. })) else {
                unreachable!()
            };
            source.path.clear();
        }),
        ("move the schema reference as if it were owned", |m| {
            let schema = formal(m, 0);
            let OperationKind::MoveStructural { source } = first(m, |kind| matches!(kind, OperationKind::MoveStructural { .. })) else {
                unreachable!()
            };
            source.root = schema;
        }),
        ("use a spent copy for both fields", |m| {
            let fields = returned(m);
            fields[1].operand = fields[0].operand.clone();
        }),
        ("copy the owned issuance into the plan", |m| {
            let fields = returned(m);
            let ValueOperand::MoveWhole(issuance) = fields[1].operand else {
                unreachable!()
            };
            fields[1].operand = ValueOperand::CopyWhole(issuance);
        }),
        ("admit an undeclared synchronous callback", |m| {
            m.contract.invokes.push(omega_psi::SynchronousEntry::RuntimeParameter(0));
        }),
        ("omit a required field", |m| {
            returned(m).pop();
        }),
        ("write two fields in each other's slot", |m| returned(m).swap(3, 5)),
        ("name a field twice", |m| {
            let fields = returned(m);
            fields[5].field = fields[3].field.clone();
        }),
        ("use the record as its own input", |m| {
            let omega_psi::OperationResult::Structural(record) = m.body.operations.last().unwrap().result else {
                unreachable!()
            };
            returned(m)[1].operand = ValueOperand::MoveWhole(record);
        }),
        ("write a count that is not zero", |m| {
            *first(m, |kind| matches!(kind, OperationKind::Constant(_))) = OperationKind::Constant(ScalarLiteral::Integer(1.into()));
        }),
        ("fill a slot with its second case", |m| {
            let OperationKind::EstablishCase { case, .. } = first(m, |kind| matches!(kind, OperationKind::EstablishCase { .. })) else {
                unreachable!()
            };
            std::sync::Arc::make_mut(&mut case.name).path = r"language\core\AccessFieldSlot::Entry".into();
        }),
        ("leave a slot of the array unfilled", |m| {
            let OperationKind::EstablishStructuralArray { elements } = first(m, |kind| matches!(kind, OperationKind::EstablishStructuralArray { .. })) else {
                unreachable!()
            };
            elements.pop();
        }),
        ("fill two slots from one zero", |m| {
            let OperationKind::EstablishStructuralArray { elements } = first(m, |kind| matches!(kind, OperationKind::EstablishStructuralArray { .. })) else {
                unreachable!()
            };
            elements[1] = elements[0];
        }),
        ("return a slot instead of the plan", |m| {
            let OperationKind::EstablishStructuralArray { elements } = first(m, |kind| matches!(kind, OperationKind::EstablishStructuralArray { .. })) else {
                unreachable!()
            };
            let slot = elements[0];
            let BlockTerminator::Return {
                result: ReturnEdge::Structural { source, .. },
                ..
            } = &mut m.body.blocks[0].terminator
            else {
                unreachable!()
            };
            *source = slot;
        }),
        ("leave cleanup for the return", |m| {
            let schema = formal(m, 0);
            let BlockTerminator::Return { cleanup, .. } = &mut m.body.blocks[0].terminator else {
                unreachable!()
            };
            cleanup.push(CleanupAction::DiscardRoot(schema));
        }),
        ("compute after the record is established", |m| {
            let extra = m.body.operations[m.body.operations.len() - 2].clone();
            m.body.operations.push(extra);
            m.body.blocks[0].operations.end += 1;
            let site = m.body.diagnostics.operations.last().unwrap().clone();
            m.body.diagnostics.operations.push(site);
        }),
        ("apply the region to the schema twice and never to the issuance", |m| {
            m.regions[0].aggregate_parameters[1].parameter = 0;
        }),
        ("leave the result outside the region", |m| m.regions[0].aggregate_result.clear()),
        ("add a second region", |m| m.regions.push(Default::default())),
        ("bind a returned borrow to the region", |m| {
            let schema = formal(m, 0);
            m.body.lifetime_bindings.push((schema, 0));
        }),
        ("specialize the body", |m| {
            let row = m.parameter_types[1];
            m.arguments.push(omega_psi::StaticArgument::Type(row));
        }),
        ("leave a reference formal unaccounted for", |m| {
            let row = m.parameter_types[0];
            m.parameter_types.push(row);
        }),
        ("swap the formals' rows", |m| m.parameter_types.swap(0, 1)),
        ("assert that the new plan contains a reader", |m| {
            let n = m.contract.ensures[1]
                .predicate
                .iter_mut()
                .find_map(|s| if let PredicateStep::Integer(n) = s { Some(n) } else { None })
                .unwrap();
            *n = 1.into();
        }),
        ("guarantee a count of the moved issuance", |m| {
            let issuance = returned(m)[1].field.clone();
            let PredicateStep::Field(subject) = &mut m.contract.ensures[0].predicate[1] else {
                unreachable!()
            };
            *subject = issuance;
        }),
        ("introduce blocking", |m| m.contract.realized_pause.block = true),
        ("declare an unhandled crash route", |m| m.contract.has_empty_crash_ceiling = false),
        ("withdraw the termination promise", |m| m.contract.promises_termination = false),
        ("require something of the caller", |m| {
            let clause = m.contract.ensures[0].clone();
            m.contract.requires.push(clause);
        }),
        ("claim the body for another package's source", |m| {
            m.body.diagnostics.operations.last_mut().unwrap().package = "impostor".into();
        }),
    ];
    for (reason, change) in changes {
        let (mut unit, id) = toolchain_unit();
        check_record_constructor(&unit, &id).unwrap();
        change(unit.machines.iter_mut().find(|m| m.declaration == id).unwrap());
        assert!(check_record_constructor(&unit, &id).is_err(), "{reason}");
    }
}

#[test]
fn the_established_plan_owes_its_own_data_predicates() {
    // `AccessPlan requires self.field_count <= 32`. The body writes zero there,
    // which a bound that excludes zero refuses.
    let (mut unit, id) = toolchain_unit();
    let clause = &mut data(&mut unit, "AccessPlan").requires[0].predicate;
    let [.., PredicateStep::Apply(comparison)] = &mut clause[..] else {
        panic!("field count bound")
    };
    *comparison = PredicateOperator::GreaterThan;
    assert_eq!(check_record_constructor(&unit, &id).unwrap_err(), "zero fails the active data predicate");
}

#[test]
fn a_zeroed_nested_record_must_meet_its_own_data_predicate() {
    let (mut unit, id) = toolchain_unit();
    check_record_constructor(&unit, &id).unwrap();
    let clause = &mut data(&mut unit, "BoundaryReach").requires[0].predicate;
    let [
        ..,
        PredicateStep::Integer(limit),
        PredicateStep::Apply(comparison),
    ] = &mut clause[..]
    else {
        panic!("reach count bound")
    };
    *limit = 0.into();
    *comparison = PredicateOperator::LessThan;
    assert!(check_record_constructor(&unit, &id).is_err(), "zero services cannot satisfy service_count < 0");
}

#[test]
fn whole_input_transfer_requires_the_declared_data_and_its_binders() {
    type Mutation = fn(&mut LoweredPackage);
    let changes: &[(&str, Mutation)] = &[
        ("the issuance loses its lifetime binder", |unit| data(unit, "Issuance").lifetime_count = 0),
        ("the issuance gains a binder no region fills", |unit| data(unit, "Issuance").lifetime_count = 2),
        ("the issuance row names another record", |unit| {
            let other = data(unit, "BoundaryReach").carrier;
            data(unit, "Issuance").carrier = other;
        }),
        ("the plan stores its schema at a binder it does not declare", |unit| {
            data(unit, "AccessPlan").fields[0].aggregate_lifetimes = vec![1];
        }),
        ("the plan stores its issuance without the lifetime", |unit| {
            data(unit, "AccessPlan").fields[1].aggregate_lifetimes.clear();
        }),
        ("the plan is linear", |unit| {
            data(unit, "AccessPlan").multiplicity = omega_psi::Multiplicity::Linear
        }),
        ("the schema is not copyable", |unit| {
            unit.data
                .iter_mut()
                .find(|data| data.declaration.path == r"language\core\reflect\TypeSchema")
                .unwrap()
                .multiplicity = omega_psi::Multiplicity::Affine;
        }),
    ];
    for (reason, change) in changes {
        let (mut unit, id) = toolchain_unit();
        check_record_constructor(&unit, &id).unwrap();
        change(&mut unit);
        assert!(check_record_constructor(&unit, &id).is_err(), "{reason}");
    }
}

#[test]
fn zero_uses_the_first_cases_payload_and_cannot_create_a_reference() {
    let (mut unit, id) = toolchain_unit();
    check_record_constructor(&unit, &id).unwrap();
    // The same fields, types and case identities are present in both packages.
    // Selecting Entry makes FieldInfo's borrowed storage part of the active zero.
    data(&mut unit, "AccessFieldSlot").cases.swap(0, 1);
    assert!(check_record_constructor(&unit, &id).is_err());
}

#[test]
fn a_nominal_origin_and_an_alias_cannot_share_one_declaration() {
    let (mut unit, id) = toolchain_unit();
    let issuance = data(&mut unit, "Issuance");
    let alias = omega_psi::LoweredAlias {
        declaration: issuance.declaration.clone(),
        target: omega_psi::AliasTarget::Builtin("bool".into()),
        asserted_multiplicity: None,
        public: true,
        site: issuance.site.clone(),
    };
    unit.aliases.push(alias);
    assert!(check_record_constructor(&unit, &id).is_err());
}

#[test]
fn a_sum_zero_requires_a_unique_first_case_of_its_own_schema() {
    let (mut unit, id) = toolchain_unit();
    let slots = data(&mut unit, "AccessFieldSlot");
    slots.cases[1].declaration = slots.cases[0].declaration.clone();
    assert!(check_record_constructor(&unit, &id).is_err());
}
