use crate::arithmetic_domains::{ValueEnv, check_value_narrowing, validate_arithmetic_domains};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataMember};
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

/// Enforce a constructed field's value against its declared type: (1) the
/// cross-CLASS check -- a `bool`/text value into a numeric field (or vice versa)
/// is a silent miscompile at construction, same as the assignment / call-arg
/// positions; and (2) the decision-17 / fact-catalog RANGE check -- a field with
/// a range refinement (`index: i32 [0..=15]`) must be CONSTRUCTED with a value
/// provably in that range, otherwise a destructure / field read that trusts the
/// range (S4 narrowing in `places::declared_place_type_raw`) would rest on an
/// unenforced bound. An integer literal is checked exactly; a non-literal value
/// is accepted when its PROVEN interval (type ranges + the flow facts visible
/// here) is within the field range -- e.g. copying a same-range field.
pub(super) fn enforce_construction_field_obligations(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    literal: &TableStructLiteral,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_name = literal.type_name.as_str();
    let Some(data_definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == type_name)
    else {
        return;
    };
    if data_definition.type_parameters.count() > 0 {
        return;
    }

    for field in program.expression_table.struct_fields(literal.fields) {
        let Some(field_type) = construction_field_type(
            program,
            data_definition,
            literal.case_name.as_ref().map(|name| name.as_str()),
            field.name.as_str(),
        ) else {
            continue;
        };
        // An array-literal field value (`Holder { arr: [300, ..] }`) is checked
        // element-wise against the field's `[T; N]` element type. The scalar guards
        // below no-op on a non-primitive (array) field, so this is the complement.
        validate_array_literal_elements(
            program,
            machine,
            state,
            field.value,
            field_type,
            diagnostics,
        );
        // Cross-class guard: a `bool`/text value stored into a numeric field (or
        // vice versa) at construction is a silent miscompile, exactly as at the
        // assignment / call-argument positions. Reject it before the range check
        // (which only applies to `[a..=b]`-constrained fields), so every primitive
        // field -- range-constrained or not -- is class-checked.
        let slot_context = format!(
            "construction of `{type_name}` field `{}`",
            field.name.as_str()
        );
        // Shape guard: `P { x: self.xs }` puts an array into a scalar field (or a
        // scalar into an array field). Runs for EVERY field -- the scalar class
        // guard and the data nominal guard below both no-op on an array value.
        crate::expression_types::report_array_scalar_shape_mismatch(
            program,
            machine,
            Some(state),
            field.value,
            field_type,
            &slot_context,
            "field",
            diagnostics,
        );
        // Scalar-vs-data shape guard: `Outer { inner: 5 }` puts a scalar into a
        // struct field (or a struct into a scalar field). Runs for EVERY field --
        // the cross-class branch below only sees primitive fields and the nominal
        // branch needs both sides to be data names, so this cross-shape case slips
        // between them.
        crate::expression_types::report_scalar_data_shape_mismatch(
            program,
            machine,
            Some(state),
            field.value,
            field_type,
            &slot_context,
            "field",
            diagnostics,
        );
        crate::domain_weakening::validate_implicit_domain_weakening(
            program,
            machine,
            Some(state),
            field.value,
            field_type,
            &slot_context,
            diagnostics,
        );
        if let Some(field_primitive) = program.primitive_type_reference(field_type) {
            if crate::expression_types::report_cross_class_store(
                program,
                Some(machine),
                Some(state),
                field.value,
                field_primitive,
                &slot_context,
                "field",
                diagnostics,
            ) {
                continue;
            }
        } else if crate::expression_types::report_data_type_conflict(
            // Nominal guard: `Cont { f: self.bar }` puts a `Bar` value into a `Foo`
            // field -- wrong data type. Only runs for non-primitive (data) fields.
            program,
            machine,
            Some(state),
            field.value,
            field_type,
            &slot_context,
            "field",
            diagnostics,
        ) {
            continue;
        }
        // Narrowing guard (any primitive field): a value that does not fit the
        // field's TYPE -- `Small { v: self.i64_field }` into an `i8 v` -- is a
        // silent truncation at construction, the same decision-17 narrowing store
        // obligation the assignment / call-arg positions carry. The field range
        // check below only covers `[a..=b]`-refined fields; this covers the plain
        // scalar width. Flow-insensitive (empty env, like the field-range check
        // below), so a wider place must be `as`-cast or constrained at construction.
        if let Some(field_primitive) = program.primitive_type_reference(field_type) {
            let owner = format!(
                "construction of `{type_name}` field `{}`",
                field.name.as_str()
            );
            check_value_narrowing(
                program,
                machine,
                Some(state),
                field.value,
                field_primitive,
                &ValueEnv::new(),
                &owner,
                diagnostics,
            );
        }
        let Some(range) = crate::arithmetic_domains::range_constraint_interval(program, field_type)
        else {
            continue;
        };
        let bounds = format!(
            "{}..={}",
            range.low().map(|low| low.to_string()).unwrap_or_default(),
            range
                .high()
                .map(|high| high.to_string())
                .unwrap_or_default(),
        );
        match construction_field_literal(program, field.value) {
            Some(value) => {
                let below = range.low().is_some_and(|low| value < low);
                let above = range.high().is_some_and(|high| value > high);
                if below || above {
                    diagnostics.push(Diagnostic::error(format!(
                        "construction of `{type_name}` field `{}`: value {value} is outside its declared range `{bounds}`",
                        field.name.as_str()
                    )));
                }
            }
            None => {
                // Non-literal value: accept when its PROVEN interval is within
                // the field range (a same-range field copy, a guarded value),
                // else reject. The value's own arithmetic obligation is reported
                // by the normal statement walk, so its diagnostics go to a
                // throwaway buffer here -- we only add the range-violation.
                let owner = format!(
                    "construction of `{type_name}` field `{}`",
                    field.name.as_str()
                );
                let mut throwaway = Vec::new();
                let interval = validate_arithmetic_domains(
                    program,
                    machine,
                    Some(state),
                    field.value,
                    &ValueEnv::new(),
                    None,
                    psi_numerics::arithmetic::ArithmeticDomain::Exact,
                    &owner,
                    &mut throwaway,
                );
                let provably_in_range = range
                    .low()
                    .is_none_or(|low| interval.low().is_some_and(|value_low| value_low >= low))
                    && range.high().is_none_or(|high| {
                        interval.high().is_some_and(|value_high| value_high <= high)
                    });
                if !provably_in_range {
                    diagnostics.push(Diagnostic::error(format!(
                        "construction of `{type_name}` field `{}` cannot be proven within its declared range `{bounds}`; constrain the value, construct with a literal in range, or widen the field",
                        field.name.as_str()
                    )));
                }
            }
        }
    }
}

/// Enforce each ELEMENT of an array literal against the array's declared element
/// type -- the same cross-class + narrowing obligations a scalar store carries.
/// `[300, ..]` into a `[i8; N]` truncates silently; `[true, ..]` into `[i8; N]`
/// stores garbage. Does nothing unless `value` is an array literal and
/// `expected_type` is a fixed array of a scalar primitive. Flow-insensitive (empty
/// env), matching the construction field-obligation checks. Reused across the
/// binding sites that know the array's expected type (assignment target, etc.).
pub(crate) fn validate_array_literal_elements(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    value: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
    } = program.type_reference_table.type_reference(expected_type)
    else {
        return;
    };
    let expected_len = match length {
        FixedArrayLength::Literal(length) => Some(*length),
        FixedArrayLength::ConstParameter { .. } | FixedArrayLength::ConstCall { .. } => None,
    };
    validate_array_literal_elements_for_shape(
        program,
        machine,
        state,
        value,
        *element_type,
        expected_len,
        diagnostics,
    );
}

/// Validate an array literal against an element type and an optional concrete
/// width supplied by a projection rather than by a named fixed-array type.
/// This is used by fixed write-only windows, whose destination width is
/// `end - start` rather than the width of the containing array.
pub(crate) fn validate_array_literal_elements_for_shape(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    value: ExpressionHandle,
    element_type: TypeReferenceHandle,
    expected_len: Option<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(value) else {
        return;
    };
    let element_handles = program.expression_table.expression_handles(*elements);
    // LENGTH check: a `[T; N]` literal must supply exactly N elements. Too few
    // leaves trailing slots reading uninitialized; too many overflows the storage
    // -- a write PAST the array's bounds into adjacent fields (memory corruption),
    // both silent before this. Only a resolved `Literal` length is checked; a
    // generic `ConstParameter` length is unknown until instantiation (a `ConstCall`
    // is const-eval'd to `Literal` upstream, so it never reaches here unresolved).
    if let Some(expected_len) = expected_len
        && element_handles.len() != expected_len
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` assigns an array literal with {} element(s) to a \
                 `[_; {expected_len}]` place; a fixed-array literal must supply exactly \
                 {expected_len} element(s)",
            machine.name.as_str(),
            state.name.as_str(),
            element_handles.len(),
        )));
        // A mis-sized literal is reported once; skip the per-element checks so
        // the count error is not buried under class/narrowing noise.
        return;
    }
    for element in element_handles {
        crate::domain_weakening::validate_implicit_domain_weakening(
            program,
            machine,
            Some(state),
            *element,
            element_type,
            "array literal element",
            diagnostics,
        );
    }
    match program.primitive_type_reference(element_type) {
        // SCALAR element type: cross-class + narrowing per element.
        Some(element_primitive) => {
            let owner = format!(
                "array literal element of type `{}`",
                element_primitive.name()
            );
            for element in element_handles {
                // Class check first; a cross-class element is not also narrowing-checked.
                if crate::expression_types::report_cross_class_store(
                    program,
                    Some(machine),
                    Some(state),
                    *element,
                    element_primitive,
                    "array literal element",
                    "element",
                    diagnostics,
                ) {
                    continue;
                }
                // Narrowing check: the element must fit the element type's width.
                check_value_narrowing(
                    program,
                    machine,
                    Some(state),
                    *element,
                    element_primitive,
                    &ValueEnv::new(),
                    &owner,
                    diagnostics,
                );
            }
        }
        // NESTED array element type (`[[i32; 2]; 2] = [[1, 2], [3, 4, 5]]`): each
        // element is itself an array literal, so recurse to check its length +
        // elements against the inner element type. Without this the inner
        // over-length was silently accepted (the extra element truncated away). The
        // recursion terminates: the element type is strictly smaller each level.
        None if matches!(
            program.type_reference_table.type_reference(element_type),
            TypeReferenceNode::FixedArray { .. }
        ) =>
        {
            for element in element_handles {
                validate_array_literal_elements(
                    program,
                    machine,
                    state,
                    *element,
                    element_type,
                    diagnostics,
                );
            }
        }
        // DATA (non-primitive) element type: a wrong-data-type element
        // (`[Foo; N] = [self.bar, ..]`) is otherwise silently accepted. Nominal guard.
        None => {
            for element in element_handles {
                crate::expression_types::report_data_type_conflict(
                    program,
                    machine,
                    Some(state),
                    *element,
                    element_type,
                    "array literal element",
                    "element",
                    diagnostics,
                );
                // A scalar element into a DATA-typed array (`[Inner; 3] = [5, ..]`)
                // slips the nominal guard above (a scalar has no data name).
                crate::expression_types::report_scalar_data_shape_mismatch(
                    program,
                    machine,
                    Some(state),
                    *element,
                    element_type,
                    "array literal element",
                    "element",
                    diagnostics,
                );
            }
        }
    }
}

/// The declared type of a constructed field: a case literal's PAYLOAD field (for
/// the named variant) or a record/common struct field. (Also consumed by the
/// literal-width gate in crate::literals.)
pub(crate) fn construction_field_type(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    case_name: Option<&str>,
    field_name: &str,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    if let Some(case_name) = case_name
        && let Some(variant) = program
            .data_members(data_definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.name.as_str() == case_name => Some(variant),
                _ => None,
            })
    {
        for payload_field in program.data_payload_fields(variant) {
            if payload_field.name.as_str() == field_name {
                return payload_field
                    .type_reference
                    .is_valid()
                    .then_some(payload_field.type_reference);
            }
        }
    }
    program
        .data_members(data_definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == field_name => field
                .type_reference
                .is_valid()
                .then_some(field.type_reference),
            _ => None,
        })
}

/// Read an integer-literal construction value (the parser folds a negative
/// `-5` into `Integer(-5)`, so the bare `Integer` case suffices). `None` for any
/// non-literal value -- those are conservatively rejected by the caller.
fn construction_field_literal(program: &TypedTrees, value: ExpressionHandle) -> Option<i64> {
    match program.expression_table.expression(value) {
        ExpressionNode::Integer(literal) => literal.value_i64(),
        _ => None,
    }
}
