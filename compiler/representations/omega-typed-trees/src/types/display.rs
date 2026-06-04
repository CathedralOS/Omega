use crate::expression::ExpressionTable;
use crate::types::{TypeConstraintNode, TypeReferenceNode, TypeReferenceTable};

impl TypeReferenceNode {
    pub fn display_name(&self, table: &TypeReferenceTable) -> String {
        match self {
            TypeReferenceNode::Reference {
                referee,
                is_mutable,
                is_relaxed,
            } => {
                let qualifier = reference_qualifier(*is_mutable, *is_relaxed);
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
            TypeReferenceNode::DynamicTrait { name, .. } => format!("dyn {name}"),
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
                is_mutable,
                is_relaxed,
            } => {
                let qualifier = reference_qualifier(*is_mutable, *is_relaxed);
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
            TypeReferenceNode::DynamicTrait { name, .. } => format!("dyn {name}"),
            TypeReferenceNode::Named { name, .. } => name.to_string(),
            TypeReferenceNode::Unit => "()".to_owned(),
        }
    }
}

fn reference_qualifier(is_mutable: bool, is_relaxed: bool) -> &'static str {
    match (is_mutable, is_relaxed) {
        (true, true) => "mut relaxed ",
        (true, false) => "mut ",
        (false, true) => "relaxed ",
        (false, false) => "",
    }
}

impl TypeConstraintNode {
    pub fn display_name(&self, expressions: &ExpressionTable) -> String {
        match self {
            TypeConstraintNode::Named(name) => name.to_string(),
            TypeConstraintNode::Range { minimum, maximum } => {
                format!(
                    "range<{}, {}>",
                    expressions.display_name(*minimum),
                    expressions.display_name(*maximum)
                )
            }
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
