//! A zero predicate is proved over raw field values, before its record exists
//! as an established value. Only declaration-owned projections may stand for
//! those bytes; neither the record invariant nor a reference's pointee follows.

use language_semantics::const_value::{
    CanonicalConstIdentity, CanonicalConstValue, DecodedCanonicalConstValue,
};
use numerics::literals::FloatFormat;
use semantic_vocabulary::MachineId;
use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};
use terminal_check_tables::Checked;
use typed_trees::{
    TypedTrees,
    data::DataMember,
    expression::{ExpressionHandle, ExpressionNode, SubtreeEdges},
    types::{FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode},
};

use crate::SourceExpressionProjectionBinding;

/// Canonical zero for a scalar carrier with no unproved domain admission.
/// The resolved builtin atom supplies identity; authored names never do.
pub fn source_zero_scalar(
    typed: &TypedTrees,
    mut carrier: TypeReferenceHandle,
) -> Option<CanonicalConstIdentity> {
    loop {
        if !typed.contains_type_reference(carrier) {
            return None;
        }
        match typed.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                if !typed
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .all(|constraint| matches!(constraint, TypeConstraintNode::ArithmeticDomain(_)))
                {
                    return None;
                }
                carrier = *base_type;
            }
            TypeReferenceNode::Named { symbol, .. } => {
                let value = match typed.symbols.builtin_type_atom(*symbol)? {
                    BuiltinTypeAtom::Bool => DecodedCanonicalConstValue::Boolean(false),
                    BuiltinTypeAtom::F32 => DecodedCanonicalConstValue::Float {
                        format: FloatFormat::F32,
                        bits: 0,
                    },
                    BuiltinTypeAtom::F64 => DecodedCanonicalConstValue::Float {
                        format: FloatFormat::F64,
                        bits: 0,
                    },
                    atom => {
                        let name = match atom {
                            BuiltinTypeAtom::I8 => "i8",
                            BuiltinTypeAtom::I16 => "i16",
                            BuiltinTypeAtom::I32 => "i32",
                            BuiltinTypeAtom::I64 => "i64",
                            BuiltinTypeAtom::U8 => "u8",
                            BuiltinTypeAtom::U16 => "u16",
                            BuiltinTypeAtom::U32 => "u32",
                            BuiltinTypeAtom::U64 => "u64",
                            BuiltinTypeAtom::Address => "addr",
                            _ => return None,
                        };
                        DecodedCanonicalConstValue::Integer {
                            type_name: name.to_owned(),
                            value: 0,
                        }
                    }
                };
                return match value {
                    DecodedCanonicalConstValue::Integer { type_name, value } => {
                        Some(CanonicalConstIdentity::integer(type_name, value))
                    }
                    DecodedCanonicalConstValue::Boolean(value) => {
                        Some(CanonicalConstValue::boolean(value).identity())
                    }
                    _ => CanonicalConstIdentity::from_declaration_value(&value),
                };
            }
            _ => return None,
        }
    }
}

/// Collect only scalar paths of the declaration's raw subject. Other source
/// expressions remain ordinary expressions and receive ordinary obligations.
pub fn source_zero_projection_bindings(
    typed: &TypedTrees,
    owner: SymbolHandle,
    expression: ExpressionHandle,
) -> Checked<Vec<SourceExpressionProjectionBinding>> {
    let machine = MachineId::new(1).expect("one is nonzero");
    let (subject, carrier) = subject(typed, owner, machine)?;
    let mut bindings = Vec::new();
    let mut failure = None;
    typed.expression_table.subtree_for_each(
        expression,
        SubtreeEdges::STRUCTURAL,
        |expression, node| {
            if failure.is_some()
                || !matches!(node, ExpressionNode::Member(_) | ExpressionNode::Indexed(_))
            {
                return;
            }
            match projection(typed, owner, subject, carrier, expression, machine) {
                Ok(Some(carrier)) => {
                    if let Some(value) = source_zero_scalar(typed, carrier) {
                        bindings.push(SourceExpressionProjectionBinding {
                            expression,
                            carrier,
                            value,
                        });
                    }
                }
                Ok(None) => {}
                Err(reason) => failure = Some(reason),
            }
        },
    );
    if let Some(reason) = failure {
        return Err(reason);
    }
    Ok(bindings)
}

pub(super) fn validate_source_zero_projections(
    typed: &TypedTrees,
    owner: SymbolHandle,
    bindings: &[SourceExpressionProjectionBinding],
    machine: MachineId,
) -> Checked<()> {
    if bindings.is_empty() {
        return Ok(());
    }
    let (subject, root) = subject(typed, owner, machine)?;
    for binding in bindings {
        let Some(carrier) = projection(typed, owner, subject, root, binding.expression, machine)?
        else {
            return Err(super::services::unavailable(
                machine,
                "zero substitution is not a projection of this declaration's subject",
            ));
        };
        if !typed.contains_type_reference(binding.carrier) {
            return Err(super::services::unavailable(
                machine,
                "zero substitution has a stale carrier",
            ));
        }
        let actual = typed.type_identity(
            typed_trees::type_identity::TypeIdentityRequest::ordinary(carrier),
        );
        let supplied = typed.type_identity(
            typed_trees::type_identity::TypeIdentityRequest::ordinary(binding.carrier),
        );
        if !actual.is_valid() || actual != supplied {
            return Err(super::services::unavailable(
                machine,
                "zero substitution changes the declared projection carrier",
            ));
        }
        let expected = source_zero_scalar(typed, carrier)
            .and_then(|value| value.decode_declaration_encoding());
        if expected.is_none() || expected != binding.value.decode_declaration_encoding() {
            return Err(super::services::unavailable(
                machine,
                "zero substitution is not canonical scalar zero for its declared carrier",
            ));
        }
    }
    Ok(())
}

fn subject(
    typed: &TypedTrees,
    owner: SymbolHandle,
    machine: MachineId,
) -> Checked<(SymbolHandle, TypeReferenceHandle)> {
    let pair = if let Some(data) = typed.data_definition_by_symbol(owner) {
        (data.predicate_subject_symbol, data.predicate_subject_type)
    } else if let Some(domain) = typed.domain_definition(owner) {
        (domain.subject_symbol, domain.target_type)
    } else {
        return Err(super::services::unavailable(
            machine,
            "zero projection owner is not a data or domain declaration",
        ));
    };
    if !typed.symbols.symbols().nodes().is_valid(pair.0) || !typed.contains_type_reference(pair.1) {
        return Err(super::services::unavailable(
            machine,
            "zero projection declaration has no live subject and carrier",
        ));
    }
    Ok(pair)
}

fn projection(
    typed: &TypedTrees,
    owner: SymbolHandle,
    subject: SymbolHandle,
    root: TypeReferenceHandle,
    expression: ExpressionHandle,
    machine: MachineId,
) -> Checked<Option<TypeReferenceHandle>> {
    if !typed.expression_table.expression_is_valid(expression) {
        return Err(super::services::unavailable(
            machine,
            "zero projection expression is stale",
        ));
    }
    match typed.expression_table.expression(expression) {
        ExpressionNode::Name(path) => Ok((path.symbol == subject
            && typed.expression_table.name_path_members(path.members).len() == 1)
            .then_some(root)),
        ExpressionNode::Member(member) => {
            let Some(receiver) = projection(typed, owner, subject, root, member.receiver, machine)?
            else {
                return Ok(None);
            };
            let base = typed
                .type_reference_table
                .unconstrained_type_reference(receiver);
            let head = match typed.type_reference_table.type_reference(base) {
                TypeReferenceNode::Named { symbol, .. } => *symbol,
                TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                _ => {
                    return Err(super::services::unavailable(
                        machine,
                        "raw zero field projection requires an owned record, without reference indirection",
                    ));
                }
            };
            if !typed
                .symbols
                .symbols()
                .nodes()
                .is_valid(member.member_symbol)
            {
                return Err(super::services::unavailable(
                    machine,
                    "raw zero field symbol is stale",
                ));
            }
            let field = typed
                .data_constructor_field(base, head, member.member_symbol)
                .ok_or_else(|| {
                    super::services::unavailable(
                        machine,
                        "raw zero field does not belong to the selected record instance",
                    )
                })?;
            if field.relevance.is_erased() {
                return Err(super::services::unavailable(
                    machine,
                    "raw zero projection cannot materialize an erased field",
                ));
            }
            let parent = typed.symbols.get(member.member_symbol).parent;
            if typed.symbols.get(parent).kind == SymbolKind::Variant {
                let definition =
                    typed
                        .data_constructor_definition(base, head)
                        .ok_or_else(|| {
                            super::services::unavailable(
                                machine,
                                "raw zero case has no selected record declaration",
                            )
                        })?;
                let first = typed
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(case) => Some(case),
                        _ => None,
                    });
                let selected = typed.data_constructor_case(base, head, parent);
                if first.map(|case| case.symbol) != selected.map(|case| case.symbol)
                    || first.is_none()
                {
                    return Err(super::services::unavailable(
                        machine,
                        "raw zero payload belongs to an inactive case",
                    ));
                }
            }
            Ok(Some(field.type_reference))
        }
        ExpressionNode::Indexed(indexed) => {
            let Some(collection) =
                projection(typed, owner, subject, root, indexed.collection, machine)?
            else {
                return Ok(None);
            };
            let base = typed
                .type_reference_table
                .unconstrained_type_reference(collection);
            let TypeReferenceNode::FixedArray {
                element_type,
                length,
            } = typed.type_reference_table.type_reference(base)
            else {
                return Err(super::services::unavailable(
                    machine,
                    "raw zero index requires a fixed array",
                ));
            };
            let length = match length {
                FixedArrayLength::Literal(length) => u64::try_from(*length).ok(),
                FixedArrayLength::Expression(reference) => {
                    match typed.type_reference_table.type_reference(*reference) {
                        TypeReferenceNode::ConstExpression(expression) => typed
                            .closed_integer_expression_value_in(*expression, owner)
                            .and_then(|value| value.to_u64()),
                        _ => None,
                    }
                }
                _ => None,
            }
            .ok_or_else(|| {
                super::services::unavailable(
                    machine,
                    "raw zero array length is not a closed integer",
                )
            })?;
            let index = typed
                .closed_integer_expression_value_in(indexed.index, owner)
                .and_then(|value| value.to_u64())
                .ok_or_else(|| {
                    super::services::unavailable(
                        machine,
                        "raw zero array index is not a closed nonnegative integer",
                    )
                })?;
            if index >= length
                || !typed_trees::operator::builtin_index_read_is_spelled(
                    typed,
                    owner,
                    expression,
                    &[Some(collection), None],
                )
            {
                return Err(super::services::unavailable(
                    machine,
                    "raw zero array projection is out of bounds or uses a selected index operation",
                ));
            }
            Ok(Some(*element_type))
        }
        _ => Ok(None),
    }
}
