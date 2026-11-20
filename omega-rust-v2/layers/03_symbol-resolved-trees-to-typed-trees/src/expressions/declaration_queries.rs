use super::body_typer::BodyTyper;
use crate::type_error::SourceTypeError;
use crate::{TypeError, ValueType};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_toolchain::DeclarationQuery;

impl BodyTyper<'_, '_> {
    pub(super) fn declaration_query(
        &self,
        node_index: usize,
        callee: usize,
        args: usize,
        package: &str,
        path: &str,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let query = DeclarationQuery::identify(package, path).ok_or_else(|| TypeError::at(source, callee, "unknown declaration query"))?;
        if !source.reference_sections(callee).is_empty() {
            return Err(TypeError::at(source, callee, "a declaration query does not take type arguments"));
        }
        let [argument] = *source.nodes[args].children.as_slice() else {
            return Err(TypeError::at(source, node_index, "a declaration query requires one declaration path"));
        };
        let path = source.ungroup(argument);
        let selected = if source.nodes[path].kind == NodeKind::PathExpression {
            self.unit.symbol(path)
        } else {
            None
        };
        if query == DeclarationQuery::Callable && matches!(selected, Some(Symbol::Candidates(_))) {
            return Err(TypeError::at(source, argument, "declaration selector is ambiguous"));
        }
        let selected = match selected {
            Some(Symbol::Declaration(id)) => Some(id.kind),
            _ => None,
        };
        let reason = match query {
            DeclarationQuery::Field if !matches!(selected, Some(NodeKind::Field | NodeKind::CaseField)) => "expected a field declaration",
            DeclarationQuery::Callable if !matches!(selected, Some(NodeKind::Machine | NodeKind::AsmMachine)) => "expected a callable declaration",
            DeclarationQuery::Field => "field descriptor construction is not yet supported",
            DeclarationQuery::Callable => "callable descriptor construction is not yet supported",
        };
        Err(TypeError::at(source, argument, reason))
    }
}
