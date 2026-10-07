use crate::symbol_resolved_trees::display::comma_join_display;
use arena::Arena;

use crate::types::{TypeConstraint, TypeReference};

impl TypeReference {
    pub fn display_name(&self) -> String {
        match self {
            TypeReference::Reference(reference) => {
                let qualifier = reference_qualifier(reference.access);
                format!("&{qualifier}<type>")
            }
            TypeReference::Constrained(constrained) => {
                format!(
                    "<type>[{}]",
                    match constrained.constraints.count() {
                        1 => "1 constraint".to_owned(),
                        count => format!("{count} constraints"),
                    }
                )
            }
            TypeReference::FixedArray(fixed_array) => {
                format!("[<type>; {}]", fixed_array.length)
            }
            TypeReference::Slice(slice) => {
                let _ = slice;
                "[<type>]".to_owned()
            }
            TypeReference::Generic(generic) => {
                let arguments = match generic.arguments.count() {
                    1 => "1 argument".to_owned(),
                    count => format!("{count} arguments"),
                };
                format!("{}<{arguments}>", generic.base_name)
            }
            TypeReference::ConstExpression(_) => "const <expression>".to_owned(),
            TypeReference::DynamicTrait {
                name, arguments, ..
            } => match arguments.count() {
                0 => format!("dyn {name}"),
                1 => format!("dyn {name}<1 argument>"),
                count => format!("dyn {name}<{count} arguments>"),
            },
            TypeReference::Named { name, .. } => name.to_string(),
            TypeReference::Unit => "()".to_owned(),
        }
    }

    pub fn display_name_with_constraints(
        &self,
        type_constraints: &Arena<TypeConstraint>,
    ) -> String {
        match self {
            TypeReference::Reference(reference) => {
                let qualifier = reference_qualifier(reference.access);
                format!("&{qualifier}<type>")
            }
            TypeReference::Constrained(constrained) => {
                let constraints = type_constraints
                    .span(constrained.constraints)
                    .unwrap_or(&[]);
                format!(
                    "<type>[{}]",
                    comma_join_display(constraints.iter(), TypeConstraint::display_name)
                )
            }
            TypeReference::FixedArray(fixed_array) => {
                format!("[<type>; {}]", fixed_array.length)
            }
            TypeReference::Slice(slice) => {
                let _ = slice;
                "[<type>]".to_owned()
            }
            TypeReference::Generic(generic) => {
                let arguments = match generic.arguments.count() {
                    1 => "1 argument".to_owned(),
                    count => format!("{count} arguments"),
                };
                format!("{}<{arguments}>", generic.base_name)
            }
            TypeReference::ConstExpression(_) => "const <expression>".to_owned(),
            TypeReference::DynamicTrait {
                name, arguments, ..
            } => match arguments.count() {
                0 => format!("dyn {name}"),
                1 => format!("dyn {name}<1 argument>"),
                count => format!("dyn {name}<{count} arguments>"),
            },
            TypeReference::Named { name, .. } => name.to_string(),
            TypeReference::Unit => "()".to_owned(),
        }
    }
}

fn reference_qualifier(access: language_core::ReferenceAccess) -> &'static str {
    match access {
        language_core::ReferenceAccess::Shared => "",
        language_core::ReferenceAccess::Mutable => "mut ",
        language_core::ReferenceAccess::WriteOnly => "write ",
    }
}

impl TypeConstraint {
    pub fn display_name(&self) -> String {
        match self {
            TypeConstraint::Named(name) => name.to_string(),
            TypeConstraint::Domain(domain) => {
                let _ = domain.arguments;
                format!("in {}", domain.name)
            }
            TypeConstraint::Range {
                minimum,
                maximum,
                end_inclusive,
            } => {
                let _ = (minimum, maximum);
                if *end_inclusive {
                    "expression..=expression"
                } else {
                    "expression..expression"
                }
                .to_owned()
            }
            TypeConstraint::ArithmeticDomain(domain) => format!("in {}", domain.name()),
        }
    }
}
