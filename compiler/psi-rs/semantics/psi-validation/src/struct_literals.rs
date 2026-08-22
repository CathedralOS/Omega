//! Struct-literal field validation: every named field of a brace construction
//! must be a declared member of the constructed shape. Covers current-shape
//! record literals (`Counter { count: 0 }`), historical-shape literals
//! (for example, an ordinary historical shape `CounterV1 { counter: 3 }`),
//! version's root-level shape definition), and case-payload literals
//! (`Command::Say { text: ... }`). Literals whose head type is not a data
//! definition in this program (or is generic, where member types depend on
//! instantiation) are left to later layers.

use crate::arithmetic_domains::{ValueEnv, check_value_narrowing, validate_arithmetic_domains};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataMember};
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use psi_typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};

mod construction_bounds;

use construction_bounds::{Bounds, Truth, bounds_fold, value_bounds};

pub(crate) fn validate_struct_literal_fields(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::AssemblyFact(fact) => {
                        scan_expression(program, machine, state, fact.expression, diagnostics);
                    }
                    StatementNode::Assignment(assignment) => {
                        scan_expression(program, machine, state, assignment.target, diagnostics);
                        scan_expression(program, machine, state, assignment.value, diagnostics);
                    }
                    StatementNode::Call(call) => {
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            scan_expression(program, machine, state, *argument, diagnostics);
                        }
                    }
                    StatementNode::Expression(expression) => {
                        scan_expression(program, machine, state, *expression, diagnostics);
                    }
                    StatementNode::LocalData(local_data) => {
                        scan_expression(
                            program,
                            machine,
                            state,
                            local_data.initial_value,
                            diagnostics,
                        );
                    }
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = &transition.guard {
                            scan_expression(program, machine, state, *guard, diagnostics);
                        }
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            scan_transition_target(
                                program,
                                machine,
                                state,
                                program.statement_table.transition_target(target),
                                diagnostics,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn scan_transition_target(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: &TransitionTargetNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match target {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                scan_expression(program, machine, state, *argument, diagnostics);
            }
        }
        TransitionTargetNode::Value(expression) => {
            scan_expression(program, machine, state, *expression, diagnostics);
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn scan_expression(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            scan_expression(program, machine, state, atomic.value, diagnostics)
        }
        ExpressionNode::StructLiteral(literal) => {
            validate_literal_field_names(program, machine, state, &literal, diagnostics);
            enforce_construction_field_obligations(program, machine, state, &literal, diagnostics);
            for field in program.expression_table.struct_fields(literal.fields) {
                scan_expression(program, machine, state, field.value, diagnostics);
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                scan_expression(program, machine, state, *element, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            scan_expression(program, machine, state, binary.left, diagnostics);
            scan_expression(program, machine, state, binary.right, diagnostics);
        }
        ExpressionNode::Cast(cast) => {
            scan_expression(program, machine, state, cast.value, diagnostics)
        }
        ExpressionNode::Call(call) => {
            scan_expression(program, machine, state, call.receiver, diagnostics);
            for argument in program.expression_table.expression_handles(call.arguments) {
                scan_expression(program, machine, state, *argument, diagnostics);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            scan_expression(program, machine, state, indexed.collection, diagnostics);
            scan_expression(program, machine, state, indexed.index, diagnostics);
        }
        ExpressionNode::Member(member) => {
            scan_expression(program, machine, state, member.receiver, diagnostics)
        }
        ExpressionNode::Mutable(inner) => {
            scan_expression(program, machine, state, *inner, diagnostics)
        }
        ExpressionNode::Range(range) => {
            scan_expression(program, machine, state, range.start, diagnostics);
            scan_expression(program, machine, state, range.end, diagnostics);
        }
        ExpressionNode::Unary(unary) => {
            scan_expression(program, machine, state, unary.operand, diagnostics)
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// Check one literal's named fields against the constructed shape's declared
/// members: record literals (current or historical shape) construct FIELD
/// members; case literals construct the named variant's PAYLOAD fields.
fn validate_literal_field_names(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    literal: &TableStructLiteral,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_name = literal.type_name.as_str();

    // A field named more than once in one literal is ambiguous: only the FIRST
    // value is stored and the rest are silently dropped (verified: `Point { x: 1,
    // x: 2, .. }` keeps x == 1). Reject it, mirroring the duplicate-member
    // rejection on the data DECLARATION. Independent of type resolution, so it runs
    // before the definition lookup (and thus for generic/unresolved shapes too);
    // field counts are tiny, so a linear scan is fine.
    let mut seen: Vec<&str> = Vec::new();
    for field in program.expression_table.struct_fields(literal.fields) {
        let name = field.name.as_str();
        if seen.contains(&name) {
            diagnostics.push(Diagnostic::error(format!(
                "data `{type_name}` literal has duplicate field `{name}`"
            )));
        } else {
            seen.push(name);
        }
    }

    let Some(data_definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == type_name)
    else {
        // The literal names a type that is not a data definition -- a primitive
        // (`i32 { a: 1 }`) or an undefined name (`Nonexistent { a: 1 }`). Neither is
        // constructible with `{ ... }`; both used to compile silently, binding a ZII
        // value. Generic data definitions ARE found here (handled just below), so
        // this fires only on genuinely non-constructible names.
        if PrimitiveType::from_name(type_name).is_some() {
            diagnostics.push(Diagnostic::error(format!(
                "cannot construct primitive type `{type_name}` with a struct literal"
            )));
        } else {
            diagnostics.push(Diagnostic::error(format!(
                "struct literal names unknown data type `{type_name}`"
            )));
        }
        return;
    };
    if data_definition.quotient.is_some() {
        diagnostics.push(Diagnostic::error(format!(
            "cannot construct quotient `{type_name}` with a struct or case literal: retained representatives are opaque and quotient values may be minted only from the exact carrier with `as {type_name}`"
        )));
        return;
    }
    if data_definition.type_parameters.count() > 0 {
        return;
    }
    if data_definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque {
        diagnostics.push(Diagnostic::error(format!(
            "opaque boundary data `{type_name}` has no public constructor; a boundary provider must establish its values"
        )));
        return;
    }

    // R2 rung 2b + slice 9 (ch12 "Construction is the gate"): a literal of
    // a domain-carrying type must PROVE the default domain -- every `where`
    // fact folds over the field-value INTERVALS (integer literals as
    // points; ranged places by their DECLARED intervals; omitted fields
    // read 0). Definitely-false refuses as a violation; unprovable refuses
    // with direction.
    validate_literal_default_domain(
        program,
        machine,
        state,
        literal,
        data_definition,
        diagnostics,
    );
    validate_omitted_gated_fields(program, literal, data_definition, diagnostics);

    match &literal.case_name {
        None => {
            // A case-bearing type (sum or mixed) has no record-form literal:
            // construction always names the case, which pins the tag. The
            // zero case with named common fields is `Type::ZeroCase { ... }`.
            if program
                .data_members(data_definition)
                .iter()
                .any(|member| matches!(member, DataMember::Variant(_)))
            {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{type_name}` is case-bearing; construct a case (`{type_name}::Case {{ ... }}`) instead of a record literal"
                )));
                return;
            }
            for field in program.expression_table.struct_fields(literal.fields) {
                if !data_declares_field(program, data_definition, field.name.as_str()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{type_name}` has no field `{}`",
                        field.name.as_str()
                    )));
                }
            }
        }
        Some(case_name) => {
            let Some(variant) = program
                .data_members(data_definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name.as_str() => {
                        Some(variant)
                    }
                    _ => None,
                })
            else {
                return;
            };
            // A case literal names the case's PAYLOAD fields, and -- for mixed
            // shapes -- may name COMMON fields alongside them (unnamed common
            // fields zero-initialize; frozen decision 7's construction rule).
            for field in program.expression_table.struct_fields(literal.fields) {
                let declared = program
                    .data_payload_fields(variant)
                    .iter()
                    .any(|payload_field| payload_field.name.as_str() == field.name.as_str())
                    || data_declares_field(program, data_definition, field.name.as_str());
                if !declared {
                    diagnostics.push(Diagnostic::error(format!(
                        "case `{type_name}::{}` has no payload field `{}`",
                        case_name.as_str(),
                        field.name.as_str()
                    )));
                }
            }
        }
    }
}

/// Omitted fields are physically zeroed. That is construction sugar only when
/// zero is already an established value of the field type; a zero-excluding
/// range or nested gated record makes the field mandatory. Sum construction
/// checks common fields plus the selected case payload only, so a payload-free
/// zero case honestly absorbs gates carried by later cases.
fn validate_omitted_gated_fields(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    data_definition: &DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let authored = program.expression_table.struct_fields(literal.fields);
    let field_was_authored = |name: &str| authored.iter().any(|field| field.name.as_str() == name);
    let mut candidates: Vec<&psi_typed_trees::data::DataField> = program
        .data_members(data_definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .collect();
    if let Some(case_name) = literal.case_name.as_ref()
        && let Some(variant) = program
            .data_members(data_definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.name.as_str() == case_name.as_str() => {
                    Some(variant)
                }
                _ => None,
            })
    {
        candidates.extend(program.data_payload_fields(variant));
    }

    for field in candidates {
        if !field_was_authored(field.name.as_str())
            && crate::data::type_requires_establishment(program, field.type_reference)
        {
            diagnostics.push(Diagnostic::error(format!(
                "construction of `{}` omits gated field `{}`: its zero-filled representation is not an established value -- initialize it explicitly",
                literal.type_name.as_str(),
                field.name.as_str(),
            )));
        }
    }
}

pub(crate) fn data_declares_field(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    field_name: &str,
) -> bool {
    program.data_members(data_definition).iter().any(
        |member| matches!(member, DataMember::Field(field) if field.name.as_str() == field_name),
    )
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
fn enforce_construction_field_obligations(
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
    let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(value) else {
        return;
    };
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
    } = program.type_reference_table.type_reference(expected_type)
    else {
        return;
    };
    let element_type = *element_type;
    let element_handles = program.expression_table.expression_handles(*elements);
    // LENGTH check: a `[T; N]` literal must supply exactly N elements. Too few
    // leaves trailing slots reading uninitialized; too many overflows the storage
    // -- a write PAST the array's bounds into adjacent fields (memory corruption),
    // both silent before this. Only a resolved `Literal` length is checked; a
    // generic `ConstParameter` length is unknown until instantiation (a `ConstCall`
    // is const-eval'd to `Literal` upstream, so it never reaches here unresolved).
    if let FixedArrayLength::Literal(expected_len) = length {
        let expected_len = *expected_len;
        if element_handles.len() != expected_len {
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

/// R2 rung 2b: fold every default-domain fact at the LITERAL's field
/// valuation. Field names read the literal's integer value (omitted -> 0);
/// literals, `+ - *`, comparisons, and `&&`/`||` fold. A fact that fails
/// refuses naming it; a fact that cannot fold (a runtime-valued field)
/// refuses as unverifiable.
fn validate_literal_default_domain(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    literal: &TableStructLiteral,
    data_definition: &DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if data_definition.where_facts.is_empty() {
        return;
    }
    let type_name = literal.type_name.as_str();
    // Slice 9: each field's value resolves to an INTERVAL -- an integer
    // literal is a point; a place with a declared `[a..=b]` range (a ranged
    // parameter, a range-refined field) contributes its DECLARED interval
    // (declared ranges always hold); anything else is unknown.
    let mut valuation: Vec<(&str, Bounds)> = Vec::new();
    for field in program.expression_table.struct_fields(literal.fields) {
        let value = value_bounds(program, machine, state, field.value);
        valuation.push((field.name.as_str(), value));
    }
    for fact in program
        .proof_facts
        .span_or_empty(data_definition.where_facts)
    {
        match fact {
            psi_typed_trees::domain::ProofFact::Expression(expression) => {
                match bounds_fold(program, &valuation, *expression) {
                    Truth::True => {}
                    Truth::False => diagnostics.push(Diagnostic::error(format!(
                        "data `{type_name}` literal violates the default domain: a `where` \
                         fact evaluates FALSE at this construction (ch12: construction is \
                         the gate)"
                    ))),
                    Truth::Unknown => diagnostics.push(Diagnostic::error(format!(
                        "data `{type_name}` literal cannot PROVE the default domain: a \
                         `where`-mentioned field's value is neither a literal nor a \
                         declared-range place whose interval decides the fact -- spell a \
                         literal, or constrain the value's declared range"
                    ))),
                }
            }
            psi_typed_trees::domain::ProofFact::Membership(membership) => {
                let field_name = membership_field_name(program, membership.value);
                let authored_value = field_name.and_then(|wanted| {
                    program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .find(|field| field.name.as_str() == wanted)
                        .map(|field| field.value)
                });
                let proven = authored_value.map_or_else(
                    || {
                        crate::proof_facts::domain_admits_empty_bytes(
                            program,
                            membership.domain_symbol,
                        )
                    },
                    |value| {
                        crate::proof_facts::string_literal_grants_domain(
                            program,
                            value,
                            membership.domain_symbol,
                        )
                    },
                );
                if !proven {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{type_name}` literal cannot prove default-domain fact: \
                         field `{}` is not known to satisfy domain `{}` at construction",
                        field_name.unwrap_or("<unknown>"),
                        membership_domain_label(program, membership.domain),
                    )));
                }
            }
            psi_typed_trees::domain::ProofFact::Proposition(application) => {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{type_name}` literal cannot prove default-domain proposition `{}` at construction",
                    application.name.as_str(),
                )));
            }
        }
    }
}

fn membership_field_name(program: &TypedTrees, value: ExpressionHandle) -> Option<&str> {
    let ExpressionNode::Name(path) = program.expression_table.expression(value) else {
        return None;
    };
    program
        .expression_table
        .name_path_members(path.members)
        .last()
        .map(|member| member.as_str())
}

fn membership_domain_label(
    program: &TypedTrees,
    domain: psi_arena::HandleSpan<psi_typed_trees::name::Identifier>,
) -> String {
    program
        .domain_path_members(domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}
