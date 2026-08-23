use crate::expression::ExpressionTable;
use crate::types::{TypeConstraintNode, TypeReferenceNode, TypeReferenceTable};

impl TypeReferenceNode {
    pub fn display_name(&self, table: &TypeReferenceTable) -> String {
        match self {
            TypeReferenceNode::Reference {
                referee,
                access,
                // Lifetime intentionally omitted from display: this string is the
                // structural type-equality oracle (`type_references_match`), and
                // `&'a T` / `&'b T` / `&T` are the SAME type — regions are checked
                // by the borrow analysis, not by type equality (decision 15).
                lifetime: _,
            } => {
                let qualifier = reference_qualifier(*access);
                format!("&{qualifier}{}", table.display_name(*referee))
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                format!(
                    "{}[{}]",
                    table.display_name(*base_type),
                    match constraints.count() {
                        1 => "1 constraint".to_owned(),
                        count => format!("{count} constraints"),
                    }
                )
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                format!("[{}; {}]", table.display_name(*element_type), length)
            }
            TypeReferenceNode::Slice { element_type } => {
                format!("[{}]", table.display_name(*element_type))
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                format!(
                    "{base_name}<{}>",
                    comma_join_display(table.type_reference_handles(*arguments), |argument| {
                        table.display_name(*argument)
                    })
                )
            }
            TypeReferenceNode::ConstExpression(_) => "const <expression>".to_owned(),
            TypeReferenceNode::DynamicTrait {
                name,
                conformance_carrier,
                conformance_name,
                ..
            } => dynamic_trait_label(
                name,
                conformance_carrier.as_ref(),
                conformance_name.as_ref(),
            ),
            TypeReferenceNode::Named { name, .. } => name.to_string(),
            TypeReferenceNode::Unit => "()".to_owned(),
        }
    }

    pub fn display_name_with_constraints(
        &self,
        table: &TypeReferenceTable,
        expressions: &ExpressionTable,
    ) -> String {
        match self {
            TypeReferenceNode::Reference {
                referee,
                access,
                // Omitted from display: see `display_name` above — lifetimes do
                // not participate in structural type equality.
                lifetime: _,
            } => {
                let qualifier = reference_qualifier(*access);
                format!(
                    "&{qualifier}{}",
                    table.display_name_with_constraints(*referee, expressions)
                )
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                format!(
                    "{}[{}]",
                    table.display_name_with_constraints(*base_type, expressions),
                    comma_join_display(table.constraints(*constraints), |constraint| {
                        constraint.display_name(expressions)
                    })
                )
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                format!(
                    "[{}; {}]",
                    table.display_name_with_constraints(*element_type, expressions),
                    length
                )
            }
            TypeReferenceNode::Slice { element_type } => {
                format!(
                    "[{}]",
                    table.display_name_with_constraints(*element_type, expressions)
                )
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                format!(
                    "{base_name}<{}>",
                    comma_join_display(table.type_reference_handles(*arguments), |argument| {
                        table.display_name_with_constraints(*argument, expressions)
                    })
                )
            }
            TypeReferenceNode::ConstExpression(expression) => {
                format!("const {}", expressions.display_name(*expression))
            }
            TypeReferenceNode::DynamicTrait {
                name,
                conformance_carrier,
                conformance_name,
                ..
            } => dynamic_trait_label(
                name,
                conformance_carrier.as_ref(),
                conformance_name.as_ref(),
            ),
            TypeReferenceNode::Named { name, .. } => name.to_string(),
            TypeReferenceNode::Unit => "()".to_owned(),
        }
    }
}

fn reference_qualifier(access: psi_language_core::ReferenceAccess) -> &'static str {
    match access {
        psi_language_core::ReferenceAccess::Shared => "",
        psi_language_core::ReferenceAccess::Mutable => "mut ",
        psi_language_core::ReferenceAccess::WriteOnly => "write ",
    }
}

fn dynamic_trait_label(
    trait_name: &crate::name::Identifier,
    conformance_carrier: Option<&crate::name::Identifier>,
    conformance_name: Option<&crate::name::Identifier>,
) -> String {
    match (conformance_carrier, conformance_name) {
        (Some(carrier), Some(conformance)) => format!("dyn {carrier}::{conformance}"),
        _ => format!("dyn {trait_name}"),
    }
}

impl TypeConstraintNode {
    pub fn display_name(&self, expressions: &ExpressionTable) -> String {
        match self {
            TypeConstraintNode::Named(name) => name.to_string(),
            TypeConstraintNode::Domain(name) => format!("in {name}"),
            TypeConstraintNode::Range { minimum, maximum } => {
                format!(
                    "{}..={}",
                    expressions.display_name(*minimum),
                    expressions.display_name(*maximum)
                )
            }
            TypeConstraintNode::ArithmeticDomain(domain) => format!("in {}", domain.name()),
        }
    }
}

fn comma_join_display<'item, I, T>(
    values: I,
    mut display_name: impl FnMut(&'item T) -> String,
) -> String
where
    I: IntoIterator<Item = &'item T>,
    T: 'item,
{
    let mut output = String::new();
    let mut first = true;

    for value in values {
        if first {
            first = false;
        } else {
            output.push_str(", ");
        }

        output.push_str(&display_name(value));
    }

    output
}
