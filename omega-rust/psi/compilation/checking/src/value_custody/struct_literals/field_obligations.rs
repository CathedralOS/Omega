mod scalar_values;
mod stored_integers;

use super::observations::ConstructionObservations;
use crate::proof_contracts::arithmetic_domains::ValueEnvironment;
use diagnostics::Diagnostic;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

/// Whether every declared member of `data_definition` is concrete at
/// construction. `Value` binders lower to implicit leading fields of their
/// carrier type, so an all-value-parametered data type constructs and reads
/// like a plain record; type, `const`, and machine parameters defer member
/// types to instantiation.
pub(crate) fn construction_members_are_concrete(
    program: &dyn crate::CheckingInput,
    data_definition: &DataDefinition,
) -> bool {
    program
        .data_type_parameters(data_definition)
        .iter()
        .all(|parameter| {
            matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Value { .. }
            )
        })
}

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
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    observations: &ConstructionObservations<'_>,
    expression: ExpressionHandle,
    literal: &TableStructLiteral,
    diagnostics: &mut Vec<Diagnostic>,
    stored_integer_writes: &mut Vec<checked_trees::CheckedStoredIntegerWrite>,
    statement_index: usize,
) {
    let type_name = literal.type_name.as_str();
    let Some(data_definition) =
        crate::declarations::data_queries::data_definition_by_symbol(program, literal.type_symbol)
    else {
        return;
    };
    if !construction_members_are_concrete(program, data_definition) {
        return;
    }

    for field in program.expression_table.struct_fields(literal.fields) {
        let Some(field_type) =
            selected_construction_field_type(program, literal, field.field_symbol)
        else {
            continue;
        };
        // A structural shell cannot turn shared image bytes or a shared
        // stored view into exclusive access. Use the same source permission
        // judgment as calls before the independent domain/shape checks; byte
        // compatibility and destination mutability establish no authority.
        let slot_context = format!(
            "construction of `{type_name}` field `{}`",
            field.name.as_str()
        );
        if crate::value_custody::reference_access::is_reference_destination(program, field_type) {
            use crate::value_custody::reference_access::ReferenceAccessRefusal;
            let prior_statements =
                &program.statement_table.statements(state.statement_nodes)[..statement_index];
            if let Some(refusal) =
                crate::value_custody::reference_access::receiving_reference_access_refusal(
                    program,
                    machine,
                    Some(state),
                    prior_statements,
                    field.value,
                    field_type,
                )
            {
                let reason = match refusal {
                    ReferenceAccessRefusal::ExplicitWriteOnlyRequired => {
                        "requires explicit write-only attenuation; pass `&write ...`"
                    }
                    ReferenceAccessRefusal::WriteOnlyMayNotRead => {
                        "may read, but the source supplies only write-only authority"
                    }
                    ReferenceAccessRefusal::ExclusiveBorrowSourceRequired { access, .. } => {
                        if access == language_semantics::ReferenceAccess::WriteOnly {
                            "requires exclusive source access; shared or immutable storage cannot supply it"
                        } else {
                            "requires mutable source access; shared or immutable storage cannot supply it"
                        }
                    }
                    ReferenceAccessRefusal::MutableSourceRequired => {
                        "requires mutable source access; shared or immutable storage cannot supply it"
                    }
                    ReferenceAccessRefusal::NamedPlaceRequired { .. } => {
                        "requires a named source place; a temporary cannot lend exclusive access"
                    }
                    ReferenceAccessRefusal::BareRangeFormsSharedView => {
                        "receives a bare range that forms a shared view; `bare-range-reference-access` spells the mutable subrange `&mut collection[start..end]`"
                    }
                };
                diagnostics.push(Diagnostic::error(format!("{slot_context} {reason}")));
                continue;
            }
            if !crate::value_custody::expression_types::argument_matches_carrier(
                program,
                machine.symbol,
                field.value,
                field_type,
                &[],
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "{slot_context} expects reference `{}`, but the source does not supply its carrier and access",
                    program.display_type_reference_with_constraints(field_type),
                )));
                continue;
            }
            if crate::value_custody::expression_types::report_view_element_argument_mismatch(
                program,
                machine,
                Some(state),
                field.value,
                field_type,
                &slot_context,
                diagnostics,
            ) {
                continue;
            }
        }
        // An array-literal field value (`Holder { arr: [300, ..] }`) is checked
        // element-wise against the field's `[T; N]` element type. The scalar guards
        // below no-op on a non-primitive (array) field, so this is the complement.
        check_array_literal_elements(
            program,
            machine,
            state,
            observations,
            field.value,
            field_type,
            diagnostics,
        );
        // Cross-class guard: a `bool`/text value stored into a numeric field (or
        // vice versa) at construction is a silent miscompile, exactly as at the
        // assignment / call-argument positions. Reject it before the range check
        // (which only applies to `[a..=b]`-constrained fields), so every primitive
        // field -- range-constrained or not -- is class-checked.
        // Shape guard: `P { x: self.xs }` puts an array into a scalar field (or a
        // scalar into an array field). Runs for EVERY field -- the scalar class
        // guard and the data nominal guard below both no-op on an array value.
        crate::value_custody::expression_types::report_array_scalar_shape_mismatch(
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
        crate::value_custody::expression_types::report_scalar_data_shape_mismatch(
            program,
            machine,
            Some(state),
            field.value,
            field_type,
            &slot_context,
            "field",
            diagnostics,
        );
        crate::proof_contracts::domain_weakening::validate_implicit_domain_weakening(
            program,
            machine,
            Some(state),
            field.value,
            field_type,
            &slot_context,
            diagnostics,
        );
        if let Some(field_primitive) = program.primitive_type_reference(field_type) {
            if crate::value_custody::expression_types::report_cross_class_store(
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
        } else if crate::value_custody::expression_types::report_data_type_conflict(
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
        let scalar = scalar_values::validate(
            program,
            machine,
            state,
            observations,
            field.value,
            field_type,
            &slot_context,
            diagnostics,
        );
        if let Some(scalar) = scalar.as_ref() {
            stored_integer_writes.extend(stored_integers::validate_construction(
                program,
                machine,
                state,
                expression,
                literal,
                field,
                scalar,
                &slot_context,
                diagnostics,
                statement_index,
            ));
        }
        let Some(range) = crate::proof_contracts::arithmetic_domains::range_constraint_interval(
            program, field_type,
        ) else {
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
                // Arithmetic and semantic landing were checked once above.
                // Refined fields and physical encodings consume the same value
                // observed before later sibling effects.
                let Some(scalar) = scalar.as_ref() else {
                    continue;
                };
                let interval = scalar.interval;
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

/// Standalone array checking owns one observation batch; nested construction
/// checks borrow the state-wide batch prepared by their caller.
pub(crate) fn validate_array_literal_elements(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    value: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let observations = ConstructionObservations::new(program, machine, state, value);
    check_array_literal_elements(
        program,
        machine,
        state,
        &observations,
        value,
        expected_type,
        diagnostics,
    );
}

pub(crate) fn validate_array_literal_elements_for_shape(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    value: ExpressionHandle,
    element_type: TypeReferenceHandle,
    expected_len: Option<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let observations = ConstructionObservations::new(program, machine, state, value);
    check_array_literal_elements_for_shape(
        program,
        machine,
        state,
        &observations,
        value,
        element_type,
        expected_len,
        diagnostics,
    );
}

/// Enforce each ELEMENT of an array literal against the array's declared element
/// type -- the same cross-class + narrowing obligations a scalar store carries.
/// `[300, ..]` into a `[i8; N]` truncates silently; `[true, ..]` into `[i8; N]`
/// stores garbage. Selected arms retain the destination; explicit borrows
/// retain the referee destination. Scalar elements use their exact occurrence
/// environments, shared with the enclosing construction checks. Reused across
/// binding sites that know the array's expected type (assignment target, etc.).
fn check_array_literal_elements(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    observations: &ConstructionObservations<'_>,
    value: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(dispatch) = program.expression_table.match_node(value) {
        for arm in program.expression_table.match_arms(dispatch.arms) {
            check_array_literal_elements(
                program,
                machine,
                state,
                observations,
                arm.value,
                expected_type,
                diagnostics,
            );
        }
        return;
    }
    // Qualifications constrain the value, not the array's storage shape.
    // Element formation still owes its exact carrier checks independently.
    let array_shape =
        crate::value_custody::places::peel_declared_constraints(program, expected_type);
    if let Some(borrow) = program.expression_table.borrow_expression(value)
        && let TypeReferenceNode::Reference { referee, .. } =
            program.type_reference_table.type_reference(array_shape)
    {
        // This checks formation only. The independent reference gate checks
        // access and the loan checker retains the actual backing permission.
        check_array_literal_elements(
            program,
            machine,
            state,
            observations,
            borrow.target,
            *referee,
            diagnostics,
        );
        return;
    }
    let TypeReferenceNode::FixedArray { length, .. } =
        program.type_reference_table.type_reference(array_shape)
    else {
        return;
    };
    let Some(element_type) =
        crate::value_custody::places::collection_element_type_reference(program, expected_type)
    else {
        return;
    };
    let expected_len = match length {
        FixedArrayLength::Literal(length) => Some(*length),
        FixedArrayLength::ConstParameter { .. }
        | FixedArrayLength::Expression(_)
        | FixedArrayLength::ConstCall { .. } => None,
    };
    check_array_literal_elements_for_shape(
        program,
        machine,
        state,
        observations,
        value,
        element_type,
        expected_len,
        diagnostics,
    );
}

/// Validate an array literal against an element type and an optional concrete
/// width supplied by a projection rather than by a named fixed-array type.
/// This is used by fixed write-only windows, whose destination width is
/// `end - start` rather than the width of the containing array.
fn check_array_literal_elements_for_shape(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    observations: &ConstructionObservations<'_>,
    value: ExpressionHandle,
    element_type: TypeReferenceHandle,
    expected_len: Option<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(elements) = program.expression_table.array_literal_node(value) else {
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
        crate::proof_contracts::domain_weakening::validate_implicit_domain_weakening(
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
                if crate::value_custody::expression_types::report_cross_class_store(
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
                // Range inclusion is not a representation conversion. Typed
                // integer leaves must already have the destination carrier;
                // anonymous expressions receive their contextual landing here.
                // Statement arithmetic validation owns the query's diagnostics;
                // this check consumes only its selected result carrier.
                let mut arithmetic_diagnostics = Vec::new();
                let (_, source_primitive, _) =
                    crate::proof_contracts::arithmetic_domains::validate_value_range(
                        program,
                        machine,
                        Some(state),
                        *element,
                        &ValueEnvironment::new(),
                        Some(element_primitive),
                        numerics::arithmetic::ArithmeticDomain::Exact,
                        &owner,
                        &mut arithmetic_diagnostics,
                    );
                if let Some(source_primitive) = source_primitive
                    && source_primitive != element_primitive
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "array literal element has type `{}`, expected `{}`; use an explicit conversion",
                        source_primitive.name(),
                        element_primitive.name(),
                    )));
                    continue;
                }
                scalar_values::validate(
                    program,
                    machine,
                    state,
                    observations,
                    *element,
                    element_type,
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
            program.type_reference_table.type_reference(
                crate::value_custody::places::peel_declared_constraints(program, element_type),
            ),
            TypeReferenceNode::FixedArray { .. }
        ) =>
        {
            for element in element_handles {
                check_array_literal_elements(
                    program,
                    machine,
                    state,
                    observations,
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
                crate::value_custody::expression_types::report_data_type_conflict(
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
                crate::value_custody::expression_types::report_scalar_data_shape_mismatch(
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
/// literal-width gate in crate::value_custody::literals.)
pub(crate) fn construction_field_type(
    program: &dyn crate::CheckingInput,
    data_definition: &DataDefinition,
    case_name: Option<&str>,
    field_name: &str,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    crate::value_custody::field_domains::construction_field_type(
        program,
        data_definition,
        case_name,
        field_name,
    )
}

/// Literal children already carry selected field identities. Rejoin their
/// exact constructor and case before choosing a destination; a same-spelled
/// declaration in the consumer's source cannot change numeric landing or
/// nominal field obligations. Projection-name lookup remains a separate caller.
/// Calls `each` with `(value, declared field type)` for every authored field
/// of `literal` whose construction field type resolves — the peel the literal
/// walk-downs share.
pub(crate) fn each_selected_literal_field(
    program: &dyn crate::CheckingInput,
    literal: &TableStructLiteral,
    mut each: impl FnMut(ExpressionHandle, TypeReferenceHandle),
) {
    for field in program.expression_table.struct_fields(literal.fields) {
        let Some(field_type) =
            selected_construction_field_type(program, literal, field.field_symbol)
        else {
            continue;
        };
        each(field.value, field_type);
    }
}

pub fn selected_construction_field_type(
    program: &dyn crate::CheckingInput,
    literal: &TableStructLiteral,
    field: symbols::SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !literal.type_symbol.is_valid() || !field.is_valid() {
        return None;
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == literal.type_symbol);
    let definition = definitions.next()?;
    if definitions.next().is_some() || !construction_members_are_concrete(program, definition) {
        return None;
    }
    let case = if let Some(case_symbol) = literal.case_symbol.filter(|symbol| symbol.is_valid()) {
        let mut cases = program
            .data_members(definition)
            .iter()
            .filter_map(|member| match member {
                DataMember::Variant(case) if case.symbol == case_symbol => Some(case),
                _ => None,
            });
        let case = cases.next()?;
        if cases.next().is_some() {
            return None;
        }
        Some(case)
    } else {
        if literal.case_name.is_some() {
            return None;
        }
        None
    };
    let mut fields = program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(candidate) => Some(candidate),
            _ => None,
        })
        .chain(
            case.into_iter()
                .flat_map(|case| program.data_payload_fields(case)),
        )
        .filter(|candidate| candidate.symbol == field);
    let reference = fields.next()?.type_reference;
    if fields.next().is_some() {
        return None;
    }
    program
        .type_reference_table
        .contains_type_reference(reference)
        .then_some(reference)
}

/// Read an integer-literal construction value (the parser folds a negative
/// `-5` into `Integer(-5)`, so the bare `Integer` case suffices). `None` for any
/// non-literal value -- those are conservatively rejected by the caller.
fn construction_field_literal(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
) -> Option<i64> {
    match program.expression_table.expression(value) {
        ExpressionNode::Integer(literal) => literal.value_i64(),
        _ => None,
    }
}
