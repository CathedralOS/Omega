//! A package's concrete witnesses may change from value to value. Its stored
//! fields and the calling conventions of its operations must stay uniform.

use crate::source_state::TypedTrees;
use crate::source_state::expression::ExpressionHandle;
use crate::source_state::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use diagnostics::Diagnostic;
use language_core::Multiplicity;
use language_core::core_vocabulary::{CoreVocabulary, ExtentDomain};
use std::collections::HashMap;
use symbols::SymbolHandle;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Storage {
    Independent,
    Uniform,
    Owned,
    Inline,
    Unsized,
    Invalid,
}

type Bindings = [(SymbolHandle, Storage)];

#[derive(Default)]
struct Traversal {
    active: Vec<SymbolHandle>,
    recursive_edges: usize,
    completed: HashMap<(SymbolHandle, Vec<Storage>), Storage>,
}

pub(crate) fn validate_in_cohort(program: &TypedTrees, first: usize) -> Result<(), Diagnostic> {
    for data in program.data_definitions().iter().skip(first) {
        if data.package_parameters.is_empty() {
            continue;
        }
        let bound: Vec<_> = program
            .data_type_parameters
            .span_or_empty(data.package_parameters)
            .iter()
            .map(|p| (p.symbol, Storage::Inline))
            .collect();
        let mut visiting = Traversal::default();
        for field in program.stored_data_fields(data) {
            let reason = match storage(program, field.type_reference, &bound, &mut visiting) {
                Storage::Inline => "has a witness-dependent inline layout",
                Storage::Unsized => "has a witness-dependent unsized element type",
                Storage::Invalid => "requires a witness-dependent layout or calling convention",
                Storage::Owned if data.properties.multiplicity != Multiplicity::Linear => {
                    "owns the hidden state and requires a [linear] package"
                }
                _ => continue,
            };
            let mut error = Diagnostic::error(format!(
                "package field '{}' in '{}' {reason}",
                field.name, data.name
            ));
            if let Some(span) = program.symbols.symbol_source_span(field.symbol) {
                error = error.with_source_span(span);
            }
            return Err(error);
        }
    }
    Ok(())
}

fn binding(symbol: SymbolHandle, bound: &Bindings) -> Storage {
    bound
        .iter()
        .find(|(key, _)| *key == symbol)
        .map_or(Storage::Independent, |(_, usage)| *usage)
}

fn descriptor(arg: Storage) -> Storage {
    match arg {
        Storage::Independent => Storage::Independent,
        Storage::Uniform | Storage::Owned => Storage::Uniform,
        _ => Storage::Invalid,
    }
}

fn storage(
    program: &TypedTrees,
    ty: TypeReferenceHandle,
    bound: &Bindings,
    visiting: &mut Traversal,
) -> Storage {
    use Storage::*;
    let table = &program.type_reference_table;
    if !table.contains_type_reference(ty) {
        return Independent;
    }
    match table.type_reference(ty) {
        TypeReferenceNode::Unit => Independent,
        TypeReferenceNode::Named { symbol, .. } => binding(*symbol, bound),
        TypeReferenceNode::Reference { referee, .. } => {
            match storage(program, *referee, bound, visiting) {
                Independent => Independent,
                Unsized | Invalid => Invalid,
                _ => Uniform,
            }
        }
        TypeReferenceNode::Slice { element_type } => {
            match storage(program, *element_type, bound, visiting) {
                Independent => Independent,
                _ => Unsized,
            }
        }
        TypeReferenceNode::Callable { contract } => {
            let sig = &program.runtime_callable_contracts.get(*contract).signature;
            program
                .state_signature_parameters(sig)
                .iter()
                .map(|p| p.type_reference)
                .chain(std::iter::once(sig.return_type))
                .map(|ty| descriptor(storage(program, ty, bound, visiting)))
                .max()
                .unwrap_or(Independent)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let mut usage = storage(program, *base_type, bound, visiting);
            for constraint in table.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    for (index, arg) in domain.arguments.iter().enumerate() {
                        if storage(program, *arg, bound, visiting) != Independent {
                            // Resident<P,T> owns one T. Its policy P and ordinary
                            // proof indices do not convey ownership of their types.
                            let resident = index == 1
                                && program.core_vocabulary(domain.symbol)
                                    == Some(CoreVocabulary::ExtentDomain(ExtentDomain::Resident));
                            usage = usage.max(if resident { Owned } else { Uniform });
                        }
                    }
                }
            }
            usage
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let count = match length {
                FixedArrayLength::Expression(expr) => storage(program, *expr, bound, visiting),
                FixedArrayLength::ConstParameter { symbol, .. } => binding(*symbol, bound),
                _ => Independent,
            };
            if count != Independent {
                Inline
            } else {
                storage(program, *element_type, bound, visiting)
            }
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let args: Vec<_> = table
                .type_reference_handles(*arguments)
                .iter()
                .map(|arg| storage(program, *arg, bound, visiting))
                .collect();
            application_storage(program, *base_symbol, args, visiting)
        }
        TypeReferenceNode::ConstExpression(expr) => {
            constant_storage(program, *expr, bound, visiting)
        }
    }
}

fn application_storage(
    program: &TypedTrees,
    base_symbol: SymbolHandle,
    args: Vec<Storage>,
    visiting: &mut Traversal,
) -> Storage {
    use Storage::*;
    if args.iter().all(|arg| *arg == Independent) {
        return Independent;
    }
    // A recursive inline occurrence has no finite layout. A surrounding
    // reference still forms a uniform address to a sized nominal type.
    let key = (base_symbol, args);
    if let Some(usage) = visiting.completed.get(&key) {
        return *usage;
    }
    if visiting.active.contains(&base_symbol) {
        visiting.recursive_edges += 1;
        return Inline;
    }
    let Some(data) = program.unique_data_definition_by_symbol(base_symbol) else {
        return Invalid;
    };
    let formals = program.data_type_parameters(data);
    if formals.len() != key.1.len() {
        return Invalid;
    }
    let actuals: Vec<_> = formals
        .iter()
        .zip(&key.1)
        .map(|(p, use_)| (p.symbol, *use_))
        .collect();
    let recursive_edges = visiting.recursive_edges;
    visiting.active.push(base_symbol);
    let usage = program
        .stored_data_fields(data)
        .map(|field| storage(program, field.type_reference, &actuals, visiting))
        .max()
        .unwrap_or(Independent)
        .max(Uniform);
    visiting.active.pop();
    // Results reached through an unfinished outer declaration depend on
    // which recursive edge was cut. Only complete subtrees are reusable.
    if visiting.recursive_edges == recursive_edges {
        visiting.completed.insert(key, usage);
    }
    usage
}

fn constant_storage(
    program: &TypedTrees,
    expr: ExpressionHandle,
    bound: &Bindings,
    visiting: &mut Traversal,
) -> Storage {
    use crate::source_state::{SubtreeEdges, SubtreeVisit, expression::ExpressionNode};
    use Storage::*;
    let found = program.subtree_any(expr, SubtreeEdges::STRUCTURAL, |_, node| {
        let uses_hidden = match node {
            ExpressionNode::Name(name) => name
                .resolved_symbol()
                .is_some_and(|s| binding(s, bound) != Independent),
            ExpressionNode::TypeExpression(ty) | ExpressionNode::ZeroValue(ty) => {
                storage(program, *ty, bound, visiting) != Independent
            }
            ExpressionNode::Call(call) => {
                let depends =
                    |argument: &crate::source_state::expression::StaticMachineArgument,
                     visiting: &mut Traversal| {
                        binding(argument.symbol, bound).max(storage(
                            program,
                            argument.type_reference,
                            bound,
                            visiting,
                        ))
                    };
                if let Some(operand) = call
                    .layout_query
                    .as_ref()
                    .and_then(|query| query.type_operand())
                {
                    depends(operand, visiting) >= Inline
                } else {
                    let authored = call.authored_machine_application;
                    call.machine_arguments
                        .iter()
                        .any(|arg| depends(arg, visiting) != Independent)
                        || (authored.is_valid()
                            && program
                                .expression_table
                                .static_machine_application_arguments(authored)
                                .iter()
                                .any(|arg| depends(arg, visiting) != Independent))
                }
            }
            _ => false,
        };
        if uses_hidden {
            SubtreeVisit::Found
        } else {
            SubtreeVisit::Descend
        }
    });
    if found { Inline } else { Independent }
}

#[cfg(test)]
mod tests {
    use crate::qualified_operator_home_tests::checked;

    #[test]
    fn unused_package_rejects_witness_sized_fields_and_callable_abi() {
        for (prefix, field) in [
            ("", "S"),
            ("", "[S; 2]"),
            ("", "[u8; size_of::<S>()]"),
            ("", "@[S]"),
            (
                "data Maybe<T> { case None; case Some(val: T); }",
                "Maybe<S>",
            ),
            (
                "data Inner<T> { value: T; } data Outer<Q> { inner: Inner<Q>; }",
                "Outer<S>",
            ),
            (
                "data Op<T> = machine(value: T) -> u8 reaches none crashes none;",
                "Op<S>",
            ),
            (
                "data Op<T> = machine() -> T reaches none crashes none;",
                "Op<S>",
            ),
        ] {
            let text = format!("{prefix} data Hidden<some S> {{ state: {field}; }}");
            let error = checked(&[("main.omg", &text)]).unwrap_err();
            assert!(
                error.contains("package field 'state'") && error.contains("witness-dependent"),
                "{field}: {error}"
            );
        }
    }

    #[test]
    fn nested_uniform_fields_and_uniform_by_value_callables_are_admitted() {
        let source = r#"
            data Inline<T> { value: T; }
            data Wrap<T> { state: @T; }
            data Use<T> = machine(value: Wrap<T>) -> Wrap<T> reaches none crashes none;
            data Hidden<'a, P, some S> {
                public_field: P; address: @'a Inline<S>; wrap: Wrap<S>;
                borrowed: [@'a S; 2]; width: [u8; size_of::<@S>()]; op: Use<S>;
            }
        "#;
        checked(&[("main.omg", source)]).unwrap();
    }
    #[test]
    fn resident_custody_through_a_provider_record_requires_a_linear_package() {
        use crate::qualified_operator_home_tests::{bundled_core_sources, packages};

        let files = bundled_core_sources();
        let owner = r#"
            use {omega}/language/core/extent.omg as range;
            data Allocation<P, T> [linear] {
                storage: |range|::Extent in |range|::Extent::Granted & |range|::Extent::Resident<P,T>;
                provider_handle: u64;
            }
            data Finish<P, T> = machine(owner: Allocation<P,T>) reaches none crashes none;
            data Job<P, some S> [linear] { owner: Allocation<P,S>; finish: Finish<P,S>; }
            data Operations<P, some S> { finish: Finish<P,S>; }
        "#;
        let check = |text: &str| {
            let mut units: Vec<_> = files
                .iter()
                .map(|(path, text)| (0, path.as_str(), text.as_str()))
                .collect();
            units.push((31, "client.omg", text));
            packages(&units, &[])
        };
        check(owner).expect("owned fields and by-value consuming operations have uniform layout");
        let affine = owner.replace("Job<P, some S> [linear]", "Job<P, some S>");
        let error = check(&affine)
            .err()
            .expect("the owner field requires linearity");
        assert!(
            error.contains("package field 'owner'")
                && error.contains("requires a [linear] package"),
            "{error}"
        );
    }
}
