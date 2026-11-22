//! The result of executing the two closed bodies covered by a policy proof.
//! A caller can inspect the receipt. Only this execution path creates one.
use crate::{Argument, Ceilings, CheckedBody, Usage, Value, admit};
use omega_checker::CheckedClosedPolicyCall;
use omega_psi::{LoweredMachine, LoweredPackage, PolicySchemaValue, TypeShape};
use std::sync::Arc;

pub struct PolicyReceipt<'unit> {
    unit: &'unit LoweredPackage,
    entry: &'unit LoweredMachine,
    schema: Arc<[u8]>,
    result: Value,
    ceilings: Ceilings,
    usage: Usage,
}

impl<'unit> PolicyReceipt<'unit> {
    pub fn unit(&self) -> &'unit LoweredPackage {
        self.unit
    }
    pub fn entry(&self) -> &'unit LoweredMachine {
        self.entry
    }
    pub fn schema(&self) -> &Arc<[u8]> {
        &self.schema
    }
    pub fn ceilings(&self) -> Ceilings {
        self.ceilings
    }
    pub fn usage(&self) -> &Usage {
        &self.usage
    }
    pub fn into_result(self) -> Value {
        self.result
    }
}

pub fn execute_policy<'unit>(
    checked: &CheckedClosedPolicyCall<'unit>,
    schema: &PolicySchemaValue,
    ceilings: Ceilings,
) -> Result<PolicyReceipt<'unit>, String> {
    let unit = checked.unit();
    let entry = checked.plan();
    let [ty] = entry.parameter_types.as_slice() else {
        return Err("selected policy needs one semantic schema argument".into());
    };
    let Some(TypeShape::Nominal(id)) = unit.types.get(*ty) else {
        return Err("selected policy argument has no schema record".into());
    };
    let row = unit
        .data
        .get(id.get() - 1)
        .filter(|row| row.id == *id && row.declaration == schema.record);
    if !row.is_some_and(|row| matches!(row.fields.as_slice(), [field] if unit.types.get(field.carrier) == Some(&TypeShape::SharedByteView))) {
        return Err("selected policy argument differs from the frozen semantic schema".into());
    }
    let closure = [
        CheckedBody::new(unit, entry),
        CheckedBody::new(unit, checked.constructor()),
    ];
    let admitted = admit(unit, &entry.declaration, &closure).map_err(|error| error.to_string())?;
    let argument = Value::Record {
        data: *id,
        fields: vec![Value::Bytes(schema.storage.clone())],
    };
    let completed = admitted
        .execute(vec![Argument::Owned(argument)], ceilings)
        .map_err(|error| error.to_string())?;
    if completed.usage.work_used != admitted.certified_work() {
        return Err("policy execution charge disagrees with its closed straight-line schedule".into());
    }
    let result = completed
        .result
        .ok_or("selected policy returned no access plan")?;
    Ok(PolicyReceipt {
        unit,
        entry,
        schema: schema.storage.clone(),
        result,
        ceilings,
        usage: completed.usage,
    })
}
