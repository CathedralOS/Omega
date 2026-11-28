use typed_trees::source_state::{self, FrozenTypedTrees};

/// One constant the declarations of an accepted module name.
///
/// `declaration` is where the constant is declared, kept as source so a
/// frontier typed later can find the same declaration; `None` when the
/// constant has no authored source to rejoin. `value` is what the judged
/// source recorded for it, `None` when no stage had evaluated it. `name` is
/// for messages only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantRead {
    pub name: String,
    pub declaration: Option<source_admission::RetainedDeclaration>,
    pub value: Option<numeric_literals::EvaluatedConstant>,
}

impl ConstantRead {
    pub fn from_preparation(prepared: &crate::PreparedTypedTrees) -> Vec<Self> {
        let source = prepared.trees();
        prepared
            .constants()
            .filter_map(|constant| {
                let declaration = source_symbol_table::source_symbols::retain_declaration(
                    &source.symbols,
                    constant.symbol,
                )
                .ok();
                if declaration.as_ref().is_some_and(|declaration| {
                    is_immutable_toolchain_build_file(declaration.source())
                        && is_immutable_toolchain_literal(source, constant)
                }) {
                    return None;
                }
                Some(Self {
                    name: source.symbols.name(constant.symbol).to_owned(),
                    declaration,
                    value: constant.evaluated_value.clone(),
                })
            })
            .collect()
    }
}

/// Pure literal or constructor constants in compiler-owned Build toolchain
/// sources (`<build-prelude>`, `<default-build>`, and `core`) cannot change on
/// rejoin, so Build rechecks do not owe constant settlement for them alone.
fn is_immutable_toolchain_literal(
    source: &FrozenTypedTrees,
    constant: &source_state::constant::ConstDeclaration,
) -> bool {
    if constant.evaluated_value.is_none()
        || constant.pending_target_observation
        || constant.evaluation_entry.is_valid()
        || source.tables.machines.is_valid(constant.evaluation_entry)
        || constant.evaluation_expression.is_valid()
        || !constant.authored_initializer.is_valid()
        || !source
            .expression_table
            .expression_is_valid(constant.authored_initializer)
    {
        return false;
    }
    is_pure_literal_initializer(source, constant.authored_initializer)
}

fn is_immutable_toolchain_build_file(file: &source_admission::SourceFile) -> bool {
    file.origin == source_admission::SourceOrigin::Toolchain
        && file.package_identity.is_none()
        && file.dependency_scope == source_provenance::DependencyScope::Build
        && file.resolution_stratum == source_admission::SourceResolutionStratum::Base
        && (source_admission::source_identity::is_build_prelude_path(&file.path)
            || file.path
                == std::path::Path::new(source_admission::source_identity::DEFAULT_BUILD_PATH)
            || file.package_root.file_name().and_then(|name| name.to_str()) == Some("core"))
}

fn is_immutable_toolchain_build_symbol(
    source: &FrozenTypedTrees,
    symbol: symbols::SymbolHandle,
) -> bool {
    source
        .symbols
        .symbol_provenance_source_span(symbol)
        .and_then(|span| source.symbols.source_file(span))
        .is_some_and(is_immutable_toolchain_build_file)
}

fn is_pure_literal_initializer(
    source: &FrozenTypedTrees,
    handle: source_state::expression::ExpressionHandle,
) -> bool {
    use source_state::expression::{ExpressionNode, UnaryOperator};
    if !handle.is_valid() || !source.expression_table.expression_is_valid(handle) {
        return false;
    }
    match source.expression_table.expression(handle) {
        ExpressionNode::Boolean(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::String(_) => true,
        ExpressionNode::Unary(unary)
            if matches!(
                unary.operator,
                UnaryOperator::BitwiseNot | UnaryOperator::LogicalNot
            ) =>
        {
            is_pure_literal_initializer(source, unary.operand)
        }
        ExpressionNode::Binary(binary) if binary.negation => {
            is_pure_literal_initializer(source, binary.left)
                && is_pure_literal_initializer(source, binary.right)
        }
        ExpressionNode::ArrayLiteral(elements) => source
            .expression_table
            .expression_handles(*elements)
            .iter()
            .all(|&element| is_pure_literal_initializer(source, element)),
        ExpressionNode::StructLiteral(literal) => {
            !literal.zero_omitted_fields
                && literal.type_symbol.is_valid()
                && is_immutable_toolchain_build_symbol(source, literal.type_symbol)
                && literal
                    .case_symbol
                    .is_none_or(|case| is_immutable_toolchain_build_symbol(source, case))
                && source
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .all(|field| is_pure_literal_initializer(source, field.value))
        }
        ExpressionNode::Name(path)
            if !path.static_application.is_valid() && !path.reserved_result_of.is_valid() =>
        {
            path.resolved_symbol().is_some_and(|symbol| {
                source.symbols.symbols().nodes().is_valid(symbol)
                    && source.symbols.get(symbol).kind == symbols::SymbolKind::Variant
                    && is_immutable_toolchain_build_symbol(source, symbol)
            })
        }
        _ => false,
    }
}
