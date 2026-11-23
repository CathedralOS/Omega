//! The value `access\new_plan` returns for one frozen policy invocation.
//!
//! Preparation executes no policy, so a test that must decide which value the
//! validator sees builds that value here. It follows the constructor's body in
//! `toolchain/core/access.omg`: each written field keeps its frozen input and
//! each omitted field is the zero of its declared type. A test of the whole
//! package path should run the package driver and use its executed result.
use omega_prepared::{PolicyFieldInput, PreparedTrees, PreparedValue};
use omega_resolved::DeclarationId;
use omega_typed::{TypedTrees, ValueType};

fn zero(
    typed: &TypedTrees<'_>,
    ty: &ValueType,
) -> PreparedValue {
    let core = omega_toolchain::core_interface();
    match ty {
        ValueType::Integer { .. } => PreparedValue::Integer(0.into()),
        ValueType::Array(element, count) => PreparedValue::Array((0..*count).map(|_| zero(typed, element)).collect()),
        _ => {
            let (owner, _) = ty
                .nominal_parts()
                .expect("an omitted plan field is an integer, an array or core data");
            // A slot's zero is its payload-free first case.
            for (data, empty) in [
                ("AccessFieldSlot", "AccessFieldSlot::Empty"),
                ("AccessRuleSlot", "AccessRuleSlot::Empty"),
            ] {
                if owner == core.id(data) {
                    return PreparedValue::Sum {
                        owner: owner.clone(),
                        case: core.id(empty).clone(),
                        fields: [].into(),
                    };
                }
            }
            assert_eq!(owner, core.id("BoundaryReach"));
            let fields = &typed.library.expect("the toolchain forest").record_fields[owner];
            PreparedValue::Record {
                owner: owner.clone(),
                fields: fields.iter().map(|field| zero(typed, &field.ty)).collect(),
            }
        }
    }
}

/// The plan record for `domain`'s frozen construction, in declared field order.
pub fn new_plan_return(
    prepared: &PreparedTrees<'_>,
    domain: &DeclarationId,
) -> PreparedValue {
    let construction = prepared
        .policy_invocation(domain)
        .expect("the domain has a frozen policy invocation")
        .construction;
    let (owner, _) = construction
        .carrier
        .nominal_parts()
        .expect("the constructor returns core data");
    PreparedValue::Record {
        owner: owner.clone(),
        fields: construction
            .fields
            .iter()
            .map(|(_, input)| match input {
                PolicyFieldInput::Written(value) => value.clone(),
                PolicyFieldInput::Zero(ty) => zero(prepared.typed(), ty),
            })
            .collect(),
    }
}

fn position(
    typed: &TypedTrees<'_>,
    owner: &DeclarationId,
    name: &str,
) -> usize {
    typed.library.expect("the toolchain forest").record_fields[owner]
        .iter()
        .position(|field| &field.declaration == omega_toolchain::core_interface().id(name))
        .expect("declared field")
}

/// Replace the field named `name` in a plan or reach record.
pub fn with_field(
    typed: &TypedTrees<'_>,
    record: &PreparedValue,
    name: &str,
    replacement: PreparedValue,
) -> PreparedValue {
    let PreparedValue::Record { owner, fields } = record else {
        panic!("expected a record")
    };
    let mut fields = fields.to_vec();
    fields[position(typed, owner, name)] = replacement;
    PreparedValue::Record {
        owner: owner.clone(),
        fields: fields.into(),
    }
}

pub fn field<'a>(
    typed: &TypedTrees<'_>,
    record: &'a PreparedValue,
    name: &str,
) -> &'a PreparedValue {
    let PreparedValue::Record { owner, fields } = record else {
        panic!("expected a record")
    };
    &fields[position(typed, owner, name)]
}
