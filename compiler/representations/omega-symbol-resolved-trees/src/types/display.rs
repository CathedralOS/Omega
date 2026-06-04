use omega_core::arena::Arena;

use crate::types::{TypeConstraint, TypeReference};

impl TypeReference {
    pub fn display_name(&self) -> String {
        match self {
            TypeReference::Reference(reference) => {
                let qualifier = reference_qualifier(reference.is_mutable, reference.is_relaxed);
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
            TypeReference::DynamicTrait { name, .. } => format!("dyn {name}"),
            TypeReference::Named { name, .. } => name.to_string(),
            TypeReference::SelfType { .. } => "Self".to_owned(),
            TypeReference::Unit => "()".to_owned(),
        }
    }

    pub fn display_name_with_constraints(
        &self,
        type_constraints: &Arena<TypeConstraint>,
    ) -> String {
        match self {
            TypeReference::Reference(reference) => {
                let qualifier = reference_qualifier(reference.is_mutable, reference.is_relaxed);
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
            TypeReference::DynamicTrait { name, .. } => format!("dyn {name}"),
            TypeReference::Named { name, .. } => name.to_string(),
            TypeReference::SelfType { .. } => "Self".to_owned(),
            TypeReference::Unit => "()".to_owned(),
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

impl TypeConstraint {
    pub fn display_name(&self) -> String {
        match self {
            TypeConstraint::Named(name) => name.to_string(),
            TypeConstraint::Range { minimum, maximum } => {
                let _ = (minimum, maximum);
                "range<expression, expression>".to_owned()
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
