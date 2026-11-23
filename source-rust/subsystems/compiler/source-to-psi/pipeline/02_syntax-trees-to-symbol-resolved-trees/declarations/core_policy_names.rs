/*
    `Wrapping`, `Saturating` and `Trapping` are core's arithmetic-policy
    domains, not reserved words (domains.md, "Domain composition and
    operators"; numeric_values.md, "Policy domains"). Parsing never consults
    name lookup, so `u8 in Wrapping` and `x as +Wrapping` arrive here as
    ordinary domain names. A package that declares its own `domain
    u8::Wrapping;` means that declaration wherever it is visible, so the
    spelling stays a domain name and typing selects it like any other domain.
    A spelling that reaches no declared domain means core's policy: this pass
    rewrites it to the policy constraint or the cast's policy before any other
    preparation reads the forest.

    Whether a visible declared `Wrapping` should instead compete with core's
    policy is open (`OWNER_QUESTIONS.md`, "core-policy-domain-names").
*/

use super::declaration_selection::ConstantSelection;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::identifier::Identifier;
use syntax_trees::types::TypeConstraintNode;

const INDEXED_POLICY: &str = "compiler arithmetic domains do not take index arguments";

pub(crate) fn normalize(
    syntax: &mut SyntaxTrees,
    selection: &ConstantSelection<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let constraints = syntax
        .type_references
        .iter_constraints()
        .filter_map(|(handle, constraint)| match constraint {
            TypeConstraintNode::Domain(domain) => {
                let policy = core_policy(syntax, selection, &domain.name)?;
                Some((handle, policy, domain.name.clone(), domain.arguments.is_empty()))
            }
            TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => None,
        })
        .collect::<Vec<_>>();
    for (handle, policy, name, plain) in constraints {
        if plain {
            syntax
                .type_references
                .replace_constraint(handle, TypeConstraintNode::ArithmeticDomain(policy));
        } else {
            diagnostics.push(Diagnostic::error(INDEXED_POLICY).with_source_span(name.source_span()));
        }
    }

    let casts = syntax
        .expressions
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            let [name] = syntax
                .expressions
                .identifier_path_members(cast.semantic_domain)
            else {
                return None;
            };
            let policy = core_policy(syntax, selection, name)?;
            Some((handle, *cast, policy, name.source_span()))
        })
        .collect::<Vec<_>>();
    for (handle, mut cast, policy, reference) in casts {
        if cast.form.is_recast() {
            diagnostics.push(
                Diagnostic::error("a recast re-views a place's bytes and takes no arithmetic domain")
                    .with_source_span(reference),
            );
        } else if !cast.semantic_domain_arguments.is_empty() {
            diagnostics.push(Diagnostic::error(INDEXED_POLICY).with_source_span(reference));
        } else {
            cast.domain = policy;
            cast.semantic_domain = arena::HandleSpan::empty();
            syntax
                .expressions
                .replace_expression(handle, ExpressionNode::Cast(cast));
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/*
    Core's policy for a single-segment policy spelling that reaches no
    declared domain from where it is written; `None` for every other name.
*/
fn core_policy(
    syntax: &SyntaxTrees,
    selection: &ConstantSelection<'_>,
    name: &Identifier,
) -> Option<ArithmeticDomain> {
    let authored = name.as_str(syntax);
    let policy = ArithmeticDomain::from_name(authored)?;
    (!selection.names_a_domain(authored, name.source_span())).then_some(policy)
}
