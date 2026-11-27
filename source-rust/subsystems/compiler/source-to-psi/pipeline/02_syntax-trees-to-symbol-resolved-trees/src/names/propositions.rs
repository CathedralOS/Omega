use crate::source_resolved::data::{TypeParameter, TypeParameterKind};
use crate::source_resolved::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, MatchPattern,
};
use crate::source_resolved::proposition::PropositionBody;
use crate::source_resolved::types::TypeConstraint;
use crate::source_symbols::SymbolTable;
use arena::{Arena, HandleSpan};
use symbols::{SymbolHandle, SymbolKind};

use super::expressions::{NameRole, assign_struct_literal_symbols};
use super::lookup::child_symbol_by_kinds;
use super::paths::{
    resolve_state_scoped_table_path, resolve_state_scoped_table_path_member_symbols,
};
use super::targets::{
    assign_static_argument_symbols, normalize_expression_module_call,
    resolve_free_call_target_symbol,
};

pub(super) fn assign_proposition_expression_symbols(
    program: &mut crate::source_resolved::SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let proposition_binders = &program.tables.declarations.proposition_binders;
    let child_type_references = &mut program.tables.declarations.child_type_references;
    let type_constraints = &program.tables.types.constraints;
    let expressions = &mut program.tables.bodies.expressions;
    let propositions = &program.roots.propositions;
    for proposition in propositions {
        let PropositionBody::Transparent { proposition: body } = proposition.body else {
            continue;
        };
        let local_type_parameters = proposition_binders
            .span_or_empty(proposition.binders)
            .iter()
            .map(|binder| TypeParameter {
                symbol: binder.symbol,
                name: binder.name.clone(),
                kind: TypeParameterKind::Type,
                bounds: binder.bounds,
            })
            .collect::<Vec<_>>();
        assign_expression_symbols(
            expressions,
            child_type_references,
            type_constraints,
            symbols,
            proposition.symbol,
            &local_type_parameters,
            body,
        );
    }
}

fn assign_expression_span_symbols(
    expressions: &mut ExpressionTable,
    child_type_references: &mut crate::source_resolved::types::TypeReferenceArena,
    type_constraints: &Arena<TypeConstraint>,
    symbols: &SymbolTable,
    owner_symbol: SymbolHandle,
    local_type_parameters: &[TypeParameter],
    span: HandleSpan<ExpressionHandle>,
) {
    let children = expressions.expression_handles(span).to_vec();
    for child in children {
        assign_expression_symbols(
            expressions,
            child_type_references,
            type_constraints,
            symbols,
            owner_symbol,
            local_type_parameters,
            child,
        );
    }
}

/*
    Resolve the authored names inside one proof-surface expression.
    `owner_symbol` is the declaration whose binder/parameter children are in
    scope; callable declarations use the shared source target resolver.
    Exact lexical binders precede authored declarations, which precede the
    outermost compiler builtins. Ambiguous declarations stay unresolved.
    Shared by proposition formulas and mathematical `let`/`boundary let`
    terms (PROOF-CONTRACT-MIGRATION).
*/
pub(super) fn assign_expression_symbols(
    expressions: &mut ExpressionTable,
    child_type_references: &mut crate::source_resolved::types::TypeReferenceArena,
    type_constraints: &Arena<TypeConstraint>,
    symbols: &SymbolTable,
    owner_symbol: SymbolHandle,
    local_type_parameters: &[TypeParameter],
    expression: ExpressionHandle,
) {
    assign_expression_symbols_in_role(
        expressions,
        child_type_references,
        type_constraints,
        symbols,
        owner_symbol,
        local_type_parameters,
        (expression, NameRole::Value),
    );
}

/*
    A foundation declaration applied in a formula, `|proof|::Box::<S>`, is
    named through the import alias like any type, so an applied member or
    argument the binder rules left unresolved is read as the fixed core
    declaration its spelling selects, when it selects one.
*/
fn fixed_core_static_symbols(
    symbols: &SymbolTable,
    argument: &mut crate::source_resolved::expression::StaticMachineArgument,
) {
    if !argument.symbol.is_valid()
        && !argument.type_reference.is_valid()
        && let [name] = argument.path.as_ref()
        && let Some(symbol) = symbols
            .lookup_source_type(name.as_str(), name.source_span())
            .unique()
        && symbols.fixed_core_declaration_for_symbol(symbol).is_some()
    {
        argument.symbol = symbol;
    }
    if let Some(application) = &mut argument.application {
        for nested in &mut application.arguments {
            fixed_core_static_symbols(symbols, nested);
        }
    }
}

fn assign_expression_symbols_in_role(
    expressions: &mut ExpressionTable,
    child_type_references: &mut crate::source_resolved::types::TypeReferenceArena,
    type_constraints: &Arena<TypeConstraint>,
    symbols: &SymbolTable,
    owner_symbol: SymbolHandle,
    local_type_parameters: &[TypeParameter],
    (expression, role): (ExpressionHandle, NameRole),
) {
    macro_rules! recurse {
        ($expression:expr) => {
            assign_expression_symbols(
                expressions,
                child_type_references,
                type_constraints,
                symbols,
                owner_symbol,
                local_type_parameters,
                $expression,
            )
        };
    }
    match expressions.expression(expression).clone() {
        // Typing refuses a block value in a proposition.
        ExpressionNode::Block(_) => {}
        ExpressionNode::Match(dispatch) => {
            recurse!(dispatch.subject);
            for arm in expressions.match_arms(dispatch.arms).to_vec() {
                let head = match arm.pattern {
                    MatchPattern::Value(head) => head,
                    MatchPattern::Destructure(pattern) => pattern.head,
                    MatchPattern::Guard(_) | MatchPattern::Wildcard => ExpressionHandle::invalid(),
                };
                for pattern in expressions
                    .match_pattern_dependencies(arm.pattern)
                    .collect::<Vec<_>>()
                {
                    assign_expression_symbols_in_role(
                        expressions,
                        child_type_references,
                        type_constraints,
                        symbols,
                        owner_symbol,
                        local_type_parameters,
                        (
                            pattern,
                            if pattern == head {
                                NameRole::PatternHead
                            } else {
                                NameRole::Value
                            },
                        ),
                    );
                }
                recurse!(arm.value);
            }
        }
        ExpressionNode::SavedSubject(_) => {}
        ExpressionNode::ArrayLiteral(values) => assign_expression_span_symbols(
            expressions,
            child_type_references,
            type_constraints,
            symbols,
            owner_symbol,
            local_type_parameters,
            values,
        ),
        ExpressionNode::Atomic(atomic) => {
            recurse!(atomic.value);
            if atomic.result.is_valid() {
                recurse!(atomic.result);
            }
        }
        ExpressionNode::Binary(binary) => {
            recurse!(binary.left);
            recurse!(binary.right);
        }
        ExpressionNode::Cast(cast) => {
            recurse!(cast.value);
            crate::resolution::names::type_references::reassign_type_reference_symbols(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                cast.target_type,
            );
            for handle in cast.semantic_domain_arguments.handles() {
                crate::resolution::names::type_references::reassign_type_reference_symbols(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    handle,
                );
            }
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                recurse!(call.receiver);
            }
            assign_expression_span_symbols(
                expressions,
                child_type_references,
                type_constraints,
                symbols,
                owner_symbol,
                local_type_parameters,
                call.arguments,
            );
            let normalized = normalize_expression_module_call(symbols, expressions, expression);
            let target_symbol = if let Some(target) = normalized {
                target
            } else if call.receiver.is_valid() {
                SymbolHandle::invalid()
            } else {
                let parameter = child_symbol_by_kinds(
                    symbols,
                    owner_symbol,
                    &[
                        SymbolKind::Parameter,
                        SymbolKind::PropositionParameter,
                        SymbolKind::MachineParameter,
                    ],
                    call.target.as_str(),
                );
                if parameter.is_valid() {
                    parameter
                } else {
                    resolve_free_call_target_symbol(symbols, &call.target)
                }
            };
            if let ExpressionNode::Call(call) = expressions.expression_mut(expression) {
                call.target_symbol = target_symbol;
                for argument in &mut call.machine_arguments {
                    assign_static_argument_symbols(
                        symbols, owner_symbol, local_type_parameters, argument, true,
                    );
                }
            }
            if !target_symbol.is_valid() && !call.receiver.is_valid() {
                let outcome = super::paths::unresolved_scoped_call_name(
                    symbols,
                    owner_symbol,
                    &[],
                    &call.target,
                );
                expressions.set_unresolved_name(expression, outcome);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            recurse!(indexed.collection);
            recurse!(indexed.index);
        }
        ExpressionNode::Membership(membership) => {
            recurse!(membership.value);
            for handle in membership.domain_arguments.handles() {
                crate::resolution::names::type_references::reassign_type_reference_symbols(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    handle,
                );
            }
        }
        ExpressionNode::Member(member) => recurse!(member.receiver),
        ExpressionNode::Borrow(inner) => recurse!(inner.target),
        // A foundation type applied in a formula, `|proof|::Box::<S>` or
        // `|proof|::Squash::<W>`, names the type its static arguments form;
        // a proposition written as one of those arguments is resolved here
        // like the formula around it.
        ExpressionNode::Name(path)
            if path.static_application.is_valid()
                && expressions.static_member(path.static_application).is_some() =>
        {
            let mut member = expressions
                .static_member(path.static_application)
                .cloned()
                .expect("the guard found the applied member");
            assign_static_argument_symbols(
                symbols,
                owner_symbol,
                local_type_parameters,
                &mut member,
                true,
            );
            fixed_core_static_symbols(symbols, &mut member);
            let mut written = Vec::new();
            let mut pending = vec![&member];
            while let Some(argument) = pending.pop() {
                if argument.type_reference.is_valid()
                    && let crate::source_resolved::types::TypeReference::ConstExpression(constant) =
                        child_type_references.get(argument.type_reference)
                {
                    written.push(constant.expression);
                }
                if let Some(application) = &argument.application {
                    pending.extend(application.arguments.iter());
                }
            }
            for argument in written {
                recurse!(argument);
            }
            let symbol = member.symbol;
            *expressions
                .static_member_mut(path.static_application)
                .expect("the applied member belongs to this table") = member;
            if let ExpressionNode::Name(path) = expressions.expression_mut(expression) {
                path.head_symbol = symbol;
                path.symbol = symbol;
            }
        }
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            let (head_symbol, symbol) = if let [name] = members {
                let symbol = child_symbol_by_kinds(
                    symbols,
                    owner_symbol,
                    &[
                        SymbolKind::Parameter,
                        SymbolKind::TypeParameter,
                        SymbolKind::PropositionMachineParameter,
                    ],
                    name.as_str(),
                );
                (symbol, symbol)
            } else {
                resolve_state_scoped_table_path(
                    symbols,
                    owner_symbol,
                    SymbolHandle::invalid(),
                    expressions,
                    &path,
                    None,
                )
            };
            let member_count = members.len();
            // Mathematical bodies and proof formulas construct the same case
            // values as machine expressions. Resolve the lexical head first so
            // a value binding cannot be reinterpreted as a constructor namespace.
            if role == NameRole::Value
                && (!head_symbol.is_valid()
                    || matches!(
                        symbols.get(head_symbol).kind,
                        SymbolKind::Data | SymbolKind::Module
                    ))
                && super::expressions::normalize_qualified_case_value(
                    symbols,
                    expressions,
                    &path,
                    expression,
                )
            {
                return;
            }
            let member_symbols = if member_count == 1 && head_symbol.is_valid() {
                vec![head_symbol]
            } else if head_symbol.is_valid() {
                resolve_state_scoped_table_path_member_symbols(
                    symbols,
                    owner_symbol,
                    SymbolHandle::invalid(),
                    expressions,
                    &path,
                )
            } else {
                Vec::new()
            };
            for (offset, member_symbol) in (0u32..).zip(member_symbols) {
                expressions.set_name_path_member_symbol_at_offset(
                    path.member_symbols,
                    offset,
                    member_symbol,
                );
            }
            if let ExpressionNode::Name(path) = expressions.expression_mut(expression) {
                path.head_symbol = head_symbol;
                path.symbol = symbol;
            }
            if !symbol.is_valid() && !path.reserved_result_of.is_valid() {
                let outcome = super::paths::unresolved_scoped_name(
                    symbols,
                    owner_symbol,
                    SymbolHandle::invalid(),
                    expressions.name_path_members(path.members),
                );
                expressions.set_unresolved_name(expression, outcome);
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                recurse!(range.start);
            }
            if range.end.is_valid() {
                recurse!(range.end);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            let fields = expressions.struct_fields(struct_literal.fields).to_vec();
            for field in fields {
                recurse!(field.value);
            }
            assign_struct_literal_symbols(symbols, expressions, expression);
        }
        ExpressionNode::Unary(unary) => recurse!(unary.operand),
        ExpressionNode::TypeExpression(type_reference)
        | ExpressionNode::ZeroValue(type_reference) => {
            crate::resolution::names::type_references::reassign_type_reference_symbols(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                type_reference,
            );
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolution::frontend_fixture::Lexer;
    use crate::resolution::{ResolutionRequest, resolve};
    use crate::source_resolved::mathematical::MathematicalBody;
    use symbols::FixedCoreDeclaration;

    #[test]
    fn mathematical_match_keeps_case_selector_separate_from_constructed_value() {
        let text = r#"
                data Choice { case First; case Other; }
                let select(value: Choice): Choice = match value {
                    Choice::First {} -> Choice::First,
                    _ -> Choice::Other
                };
            "#;
        let mut sources = crate::source_inputs::SourceMap::default();
        let source_id = sources
            .add("mathematical-match.omg".into(), text.into())
            .source_id;
        let tokens = Lexer::new(text).tokenize().unwrap();
        let syntax =
            crate::resolution::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens)
                .unwrap();
        let program = resolve(ResolutionRequest {
            syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        let MathematicalBody::Definition(body) = program.mathematical_definitions[0].body else {
            panic!("the mathematical match must retain its definition");
        };
        let expressions = &program.tables.bodies.expressions;
        let ExpressionNode::Match(dispatch) = expressions.expression(body) else {
            panic!("the mathematical body must retain its match");
        };
        let arm = &expressions.match_arms(dispatch.arms)[0];
        let MatchPattern::Destructure(pattern) = arm.pattern else {
            panic!("the case pattern must retain its destructure selector");
        };
        let ExpressionNode::Name(selector) = expressions.expression(pattern.head) else {
            panic!("a pattern selector must retain declaration identity");
        };
        assert_eq!(
            program.symbols.get(selector.symbol).kind,
            SymbolKind::Variant
        );
        let ExpressionNode::StructLiteral(value) = expressions.expression(arm.value) else {
            panic!("the arm result must construct its case value");
        };
        assert_eq!(value.case_symbol, Some(selector.symbol));
    }

    #[test]
    fn a_contract_call_selects_the_fixed_core_identity_like_a_body_call() {
        let text = "use {omega}/language/core/foundation.omg as logic;\n\
                    use foundation.omg as user;\n\
                    machine user_witness(value: |user|::Identity) {}\n\
                    machine same(value: f64) -> f64\n\
                    ensures |logic|::Identity::<f64>(result, value)\n\
                    { transition |logic|::Identity::<f64>(value, value) { _ -> (value) } }\n\
                    machine unbound(value: f64) -> f64\n\
                    ensures core::Identity::<f64>(result, value)\n\
                    { transition core::Identity::<f64>(value, value) { _ -> (value) } }\n";
        let mut sources = crate::source_inputs::SourceMap::default();
        let source_id = sources
            .add("app/identity.omg".into(), text.into())
            .source_id;
        sources.add_with_metadata(
            "core/foundation.omg".into(),
            std::str::from_utf8(bundled_declarations::bytes(
                language_core::bundled_files::BundledFile::CoreFoundation,
            ))
            .unwrap()
            .into(),
            "core".into(),
            None,
            source_admission::SourceOrigin::Toolchain,
        );
        let user_text = "public data Identity {}";
        let user_id = sources
            .add("app/foundation.omg".into(), user_text.into())
            .source_id;
        let tokens = Lexer::new(text).tokenize().unwrap();
        let mut syntax =
            crate::resolution::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens)
                .unwrap();
        crate::resolution::frontend_fixture::parse(
            &mut syntax,
            user_id,
            &Lexer::new(user_text).tokenize().unwrap(),
        )
        .unwrap();
        let program = resolve(ResolutionRequest {
            syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        let identity = program
            .symbols
            .fixed_core_declaration_symbol(FixedCoreDeclaration::Identity)
            .expect("the fixed core vocabulary is installed");
        let calls = program
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .filter_map(|(_, node)| match node {
                ExpressionNode::Call(call) => Some(call.target_symbol),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 4);
        assert_eq!(
            calls.iter().filter(|&&symbol| symbol == identity).count(),
            2
        );
        assert_eq!(calls.iter().filter(|symbol| !symbol.is_valid()).count(), 2);

        let reference = source_text::SourceSpan::new(source_id, source_text::Span::new(0, 3));
        let lookup = |path: &str| program.symbols.lookup_source_type(path, reference).unique();
        for (member, fixed) in [
            ("Level", FixedCoreDeclaration::Level),
            ("Type", FixedCoreDeclaration::Type),
            ("Strict", FixedCoreDeclaration::Strict),
            ("Squash", FixedCoreDeclaration::Squash),
            ("Identity", FixedCoreDeclaration::Identity),
        ] {
            assert_eq!(
                lookup(&format!("|logic|::{member}")),
                program.symbols.fixed_core_declaration_symbol(fixed)
            );
            assert_eq!(lookup(&format!("|absent|::{member}")), None);
            assert_eq!(lookup(member), None);
            assert_eq!(lookup(fixed.name()), None);
        }
        let user_identity = lookup("|user|::Identity").unwrap();
        assert_eq!(program.symbols.get(user_identity).kind, SymbolKind::Data);
        assert_eq!(
            program
                .symbols
                .fixed_core_declaration_for_symbol(user_identity),
            None
        );
    }
}
