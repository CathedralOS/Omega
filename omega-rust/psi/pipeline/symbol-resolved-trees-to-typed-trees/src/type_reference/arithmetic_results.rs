//! Retain the policy-only result vocabulary while typing still owns mutation.
//! Authored declarations keep their original references and predicates. Later
//! read-only result queries can name a builtin operation's carrier and policy
//! without depending on an incidental return annotation or exporting an input
//! range as a result fact. This retention does not select an operation.

use numerics::arithmetic::ArithmeticDomain;
use symbols::BuiltinTypeAtom;
use typed_trees::TypedTrees;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn retain_arithmetic_result_type(
    source_symbols: &symbols::SymbolTable,
    program: &mut TypedTrees,
    reference: TypeReferenceHandle,
    cast_domain: Option<ArithmeticDomain>,
) {
    let mut current = reference;
    let mut domain = None;
    let mut visited = Vec::new();
    let carrier = loop {
        if !program
            .type_reference_table
            .contains_type_reference(current)
            || visited.contains(&current)
        {
            return;
        }
        visited.push(current);
        match program.type_reference_table.type_reference(current) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let Some(constraints) = program.type_reference_table.constraint_span(*constraints)
                else {
                    return;
                };
                for constraint in constraints {
                    match constraint {
                        TypeConstraintNode::Range { .. } => {}
                        TypeConstraintNode::ArithmeticDomain(policy) if domain.is_none() => {
                            domain = Some(*policy);
                        }
                        // Conflicting/duplicate policies and arbitrary domains
                        // are validation obligations, not arithmetic aliases.
                        _ => return,
                    }
                }
                current = *base_type;
            }
            TypeReferenceNode::Named { symbol, .. } => break *symbol,
            // In particular, a reference's referent is not its value carrier.
            _ => return,
        }
    };
    if !matches!(
        // The typed symbol table is installed only at Lowerer::finish.
        // These retained handles already belong to the resolved source table.
        source_symbols.builtin_type_atom(carrier),
        Some(
            BuiltinTypeAtom::I8
                | BuiltinTypeAtom::I16
                | BuiltinTypeAtom::I32
                | BuiltinTypeAtom::I64
                | BuiltinTypeAtom::U8
                | BuiltinTypeAtom::U16
                | BuiltinTypeAtom::U32
                | BuiltinTypeAtom::U64
                | BuiltinTypeAtom::F32
                | BuiltinTypeAtom::F64
        )
    ) {
        return;
    }
    let domain = cast_domain.unwrap_or(domain.unwrap_or(ArithmeticDomain::Exact));
    if program
        .type_reference_table
        .find_arithmetic_result_type_reference(carrier, domain)
        .is_some()
    {
        return;
    }
    // `current` is the exact already-retained bare builtin reference. Never
    // replace the declaration or its selection ledger with this projection.
    if domain != ArithmeticDomain::Exact {
        let constraints = program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::ArithmeticDomain(domain)]);
        program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: current,
                constraints,
            });
    }
}
