//! A package literal chooses concrete witnesses once, before callable selection.
//! The concrete input field types belong to this construction alone.

use super::{Diagnostic, ExpressionHandle, ExpressionNode, TypedTrees, infer_operand_bindings};
use crate::source_state::expression::{PackageConstruction, PackageFieldType};
use crate::source_state::types::TypeReferenceNode;
use crate::types::typing_queries::exact_declared_field;

pub(super) fn construct(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
) -> Result<(), Diagnostic> {
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        unreachable!("package construction is called only for record literals")
    };
    let data = program
        .unique_data_definition_by_symbol(literal.type_symbol)
        .ok_or_else(|| Diagnostic::error("package constructor lost its data declaration"))?;
    let declaration = data.symbol;
    let hidden = program
        .data_type_parameters
        .span_or_empty(data.package_parameters)
        .to_vec();
    let public = program.data_type_parameters(data);
    let application = literal.application_type;
    let mut substitutions = Vec::new();
    if !public.is_empty() {
        let Some(TypeReferenceNode::Generic { arguments, .. }) = program
            .type_reference_table
            .contains_type_reference(application)
            .then(|| program.type_reference_table.type_reference(application))
        else {
            return Err(Diagnostic::error(
                "package construction requires its public type arguments",
            ));
        };
        let args = program
            .type_reference_table
            .type_reference_handles(*arguments);
        if args.len() != public.len() {
            return Err(Diagnostic::error(
                "package construction has the wrong public type arguments",
            ));
        }
        substitutions.extend(
            public
                .iter()
                .zip(args)
                .map(|(binder, ty)| (binder.symbol, *ty)),
        );
    }
    let case = literal.case_symbol;
    let fields = program
        .expression_table
        .struct_fields(literal.fields)
        .to_vec();
    let mut inferred = vec![None; hidden.len()];
    let mut field_types = Vec::with_capacity(fields.len());
    for field in fields {
        let declared = exact_declared_field(program, field.field_symbol)
            .ok_or_else(|| Diagnostic::error("package constructor field has no declaration"))?;
        let parent = program.symbols.get(declared.symbol).parent;
        if parent != declaration
            && !(case == Some(parent) && program.symbols.get(parent).parent == declaration)
        {
            return Err(Diagnostic::error(
                "package initializer belongs to a different declaration",
            ));
        }
        let ty = declared.type_reference;
        let span = program.expression_table.source_span(field.value);
        infer_operand_bindings(
            program,
            ty,
            field.value,
            &hidden,
            &mut inferred,
            &mut Vec::new(),
        )
        .map_err(|diagnostic| diagnostic.with_source_span(span))?;
        for (formal, actual) in callable_operands(program, ty, field.value) {
            super::unify(program, formal, actual, &hidden, &mut inferred)
                .map_err(|diagnostic| diagnostic.with_source_span(span))?;
        }
        field_types.push(PackageFieldType {
            symbol: field.field_symbol,
            ty,
        });
    }
    let mut witnesses = Vec::with_capacity(hidden.len());
    for (binder, inferred) in hidden.iter().zip(inferred) {
        let actual = inferred.ok_or_else(|| {
            Diagnostic::error(format!(
                "package construction does not determine hidden type `{}`",
                binder.name
            ))
            .with_source_span(program.expression_table.source_span(expression))
        })?;
        if program.package_witness_is_unsized(actual) {
            return Err(Diagnostic::error(format!(
                "package witness for hidden type `{}` must be sized",
                binder.name
            ))
            .with_source_span(program.expression_table.source_span(expression)));
        }
        substitutions.push((binder.symbol, actual));
        witnesses.push(actual);
    }
    for field in &mut field_types {
        field.ty = program
            .substitute_type_parameters(field.ty, declaration, &substitutions)
            .map_err(Diagnostic::error)?;
    }
    let ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression_mut(expression)
    else {
        unreachable!("typing preserves the package literal")
    };
    literal.package = Some(Box::new(PackageConstruction {
        witnesses,
        fields: field_types,
    }));
    Ok(())
}

/// A named monomorphic machine supplies the types in its signature even
/// before the receiving callable has been selected. Contract refinement
/// and the right to call that machine are checked by ordinary call typing.
fn callable_operands(
    program: &TypedTrees,
    field: super::TypeReferenceHandle,
    value: ExpressionHandle,
) -> Vec<(super::TypeReferenceHandle, super::TypeReferenceHandle)> {
    let field = program
        .type_reference_table
        .unconstrained_type_reference(field);
    let TypeReferenceNode::Callable { contract } =
        program.type_reference_table.type_reference(field)
    else {
        return Vec::new();
    };
    let ExpressionNode::Name(name) = program.expression_table.expression(value) else {
        return Vec::new();
    };
    let Some(machine) = name
        .resolved_symbol()
        .and_then(|symbol| program.machine_by_own_or_state_symbol(symbol))
    else {
        return Vec::new();
    };
    if !machine.type_parameters.is_empty() {
        return Vec::new();
    }
    let Some(entry) = program.machine_states(machine).first() else {
        return Vec::new();
    };
    let required = &program.runtime_callable_contracts.get(*contract).signature;
    let inputs = program.state_signature_parameters(required);
    let parameters = program.state_parameters(entry);
    if inputs.len() != parameters.len() {
        return Vec::new();
    }
    let mut pairs: Vec<_> = inputs
        .iter()
        .zip(parameters.iter())
        .map(|(formal, actual)| (formal.type_reference, actual.type_reference))
        .collect();
    if required.return_type.is_valid() && entry.return_type.is_valid() {
        pairs.push((required.return_type, entry.return_type));
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qualified_operator_home_tests::checked;

    #[test]
    fn one_public_type_retains_different_witnesses_on_its_values() {
        let program = checked(&[(
            "main.omg",
            r#"
            data Pair<'a, some S> { first: @'a S; second: @'a S; }
            machine byte(a: @u8) { let one: Pair = Pair { first: a, second: a }; }
            machine word(a: @u32) { let two: Pair = Pair { first: a, second: a }; }
        "#,
        )])
        .unwrap();
        let packages: Vec<_> = program
            .expression_table
            .expression_entries()
            .filter_map(|(_, node)| match node {
                ExpressionNode::StructLiteral(literal) => literal.package.as_ref().map(|package| {
                    assert_eq!(package.fields.len(), 2);
                    assert_eq!(package.witnesses.len(), 1);
                    (
                        literal.type_symbol,
                        program.normalized_type_identity(package.witnesses[0]),
                    )
                }),
                _ => None,
            })
            .collect();
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].0, packages[1].0);
        assert_ne!(packages[0].1, packages[1].1);
    }

    #[test]
    fn packing_distinguishes_unsized_storage_from_a_sized_view_descriptor() {
        let with_argument = |ty: &str| {
            format!(
                r#"
            data Inline<T> {{ component: T; }}
            data Indirect<T> {{ component: @T; }}
            data Package<'a, some S> {{ state: @'a S; }}
            machine pack(input: @{ty}) {{ let packed: Package = Package {{ state: input }}; }}
        "#
            )
        };
        for ty in ["[u8]", "[[u8]; 2]", "Inline<[u8]>"] {
            let error = checked(&[("main.omg", &with_argument(ty))]).unwrap_err();
            assert!(
                error.contains("package witness for hidden type `S` must be sized"),
                "{ty}: {error}"
            );
        }
        for ty in ["[u8; 2]", "@[u8]", "Indirect<[u8]>", "Inline<@[u8]>"] {
            checked(&[("main.omg", &with_argument(ty))])
                .unwrap_or_else(|error| panic!("{ty}: {error}"));
        }
        let helper = r#"
            data Package<'a, some S> { state: @'a S; }
            machine pack<T>(input: @T) { let packed: Package = Package { state: input }; }
        "#;
        checked(&[("main.omg", helper)]).unwrap();

        let recursive = "data Node<'a> { next: Node<'a>; }
            data Package<'a, some S> { state: @'a S; }
            machine pack<'a>(input: @'a Node<'a>) { let value: Package = Package { state: input }; }";
        let error = checked(&[("main.omg", recursive)]).unwrap_err();
        assert!(
            error.contains("package witness for hidden type `S` must be sized"),
            "{error}"
        );
        let linked = recursive.replace("next: Node", "next: @'a Node");
        checked(&[("main.omg", &linked)]).unwrap();
    }

    #[test]
    fn callback_and_borrow_supply_one_witness() {
        let source = r#"
            data File { byte: u8; }
            data Socket { unused: u8; byte: u8; }
            data Read<S> = machine(state: @mutable S) -> u8 reaches none crashes none;
            data Reader<'a, some S> { state: @'a mutable S; read: Read<S>; }
            machine file(state: @mutable File) -> u8 reaches none crashes none { state.byte }
            machine socket(state: @mutable Socket) -> u8 reaches none crashes none { state.byte }
            machine pack(state: @mutable File) {
                let reader: Reader = Reader { state: state, read: file };
            }
        "#;
        let program = checked(&[("main.omg", source)]).unwrap();
        let package = program
            .expression_table
            .expression_entries()
            .find_map(|(_, node)| {
                if let ExpressionNode::StructLiteral(literal) = node {
                    literal.package.as_ref()
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(package.witnesses.len(), 1);
        let wrong = source.replace("read: file }", "read: socket }");
        let refusal = checked(&[("main.omg", &wrong)]).unwrap_err();
        assert!(
            refusal.contains("conflicting generic arguments"),
            "{refusal}"
        );
    }

    #[test]
    fn fields_of_one_package_must_determine_the_same_witness() {
        let error = checked(&[(
            "main.omg",
            r#"
            data Pair<'a, some S> { first: @'a S; second: @'a S; }
            machine bad(a: @u8, b: @u32) { let pair: Pair = Pair { first: a, second: b }; }
        "#,
        )])
        .unwrap_err();
        assert!(error.contains("conflicting generic arguments"), "{error}");
    }
}
