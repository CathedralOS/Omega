//! Restore nominal identities at the admitted, typed build-time argument seam.
//! The detached value format has no source handles. Its receiving parameter's
//! exact field telescope supplies those identities; names never select a type.

use crate::interpreter::evaluator::{
    EvalResult, Evaluator, TypeReferenceHandle, TypeReferenceNode, Value, trap,
    type_reference_table,
};

impl Evaluator<'_> {
    pub(super) fn bind_build_time_argument(
        &self,
        value: &mut Value,
        reference: TypeReferenceHandle,
    ) -> EvalResult<()> {
        let reference = type_reference_table(self.program).unconstrained_type_reference(reference);
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                // Detached arguments carry the referent snapshot. The entry
                // invocation creates its ordinary reference wrapper afterward.
                return self.bind_build_time_argument(value, *referee);
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                if let Value::Array(elements) = value {
                    for element in elements {
                        self.bind_build_time_argument(&mut element.borrow_mut(), *element_type)?;
                    }
                }
                return Ok(());
            }
            _ => {}
        }
        let Some(data) = self.program.closed_data_field_definition(reference) else {
            return Ok(());
        };
        // Opaque host and issuer values retain their issuing protocol. A
        // receiving spelling is never authority to construct their internals.
        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
            return Ok(());
        }
        match value {
            Value::Struct {
                type_symbol,
                fields,
                ..
            } => {
                for (name, cell) in fields {
                    let Some(field) = self
                        .program
                        .live_data_fields(self.program.data_members(data))
                        .find(|field| field.name.as_str() == name.as_ref())
                    else {
                        return trap("build-time record argument has an undeclared field");
                    };
                    self.bind_build_time_argument(&mut cell.borrow_mut(), field.type_reference)?;
                }
                *type_symbol = data.symbol;
            }
            Value::Enum {
                type_symbol,
                variant_name,
                payload,
            } => {
                // The detached protocol permits a bare case or an owner-qualified
                // export. The parameter already selected the nominal carrier;
                // an unrelated qualifier cannot nominate another declaration.
                let selected_name = variant_name
                    .strip_prefix(data.name.as_str())
                    .and_then(|tail| tail.strip_prefix("::"))
                    .unwrap_or(variant_name.as_ref());
                let Some(case) = self.program.data_members(data).iter().find_map(|member| {
                    let typed_trees::data::DataMember::Variant(case) = member else {
                        return None;
                    };
                    (case.name.as_str() == selected_name).then_some(case)
                }) else {
                    return trap(format!(
                        "build-time case argument `{}` has no case in its exact receiving application `{}`",
                        variant_name,
                        self.program.display_type_reference(reference)
                    ));
                };
                for (name, cell) in payload {
                    let Some(field) = self
                        .program
                        .data_payload_fields(case)
                        .iter()
                        .chain(
                            self.program
                                .live_data_fields(self.program.data_members(data)),
                        )
                        .find(|field| field.name.as_str() == name.as_ref())
                    else {
                        return trap("build-time case argument has an undeclared payload field");
                    };
                    self.bind_build_time_argument(&mut cell.borrow_mut(), field.type_reference)?;
                }
                *type_symbol = data.symbol;
                *variant_name = case.name.as_str().into();
            }
            _ => {}
        }
        Ok(())
    }
}
