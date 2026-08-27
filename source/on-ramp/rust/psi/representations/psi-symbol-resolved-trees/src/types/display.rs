use psi_arena::Arena;

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
                name,
                conformance_carrier,
                conformance_name,
                ..
            } => display_dynamic_trait(
                name,
                conformance_carrier.as_ref(),
                conformance_name.as_ref(),
            ),
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
                name,
                conformance_carrier,
                conformance_name,
                ..
            } => display_dynamic_trait(
                name,
                conformance_carrier.as_ref(),
                conformance_name.as_ref(),
            ),
            TypeReference::Named { name, .. } => name.to_string(),
            TypeReference::SelfType { .. } => "Self".to_owned(),
            TypeReference::Unit => "()".to_owned(),
        }
    }
}

fn display_dynamic_trait(
    trait_name: &crate::name::DiagnosticName,
    conformance_carrier: Option<&crate::name::DiagnosticName>,
    conformance_name: Option<&crate::name::DiagnosticName>,
) -> String {
    match (conformance_carrier, conformance_name) {
        (Some(carrier), Some(conformance)) => format!("dyn {carrier}::{conformance}"),
        _ => format!("dyn {trait_name}"),
    }
}

fn reference_qualifier(access: psi_language_core::ReferenceAccess) -> &'static str {
    match access {
        psi_language_core::ReferenceAccess::Shared => "",
        psi_language_core::ReferenceAccess::Mutable => "mut ",
        psi_language_core::ReferenceAccess::WriteOnly => "write ",
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
            TypeConstraint::Range { minimum, maximum } => {
                let _ = (minimum, maximum);
                "expression..=expression".to_owned()
            }
            TypeConstraint::ArithmeticDomain(domain) => format!("in {}", domain.name()),
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
