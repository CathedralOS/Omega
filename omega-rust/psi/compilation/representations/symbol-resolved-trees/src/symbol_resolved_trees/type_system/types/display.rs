use crate::types::{ConstrainedTypeReference, TypeConstraint, TypeReference};

impl TypeReference {
    /// The arm both spellings share; only the `Constrained` summary differs.
    fn display_name_with(
        &self,
        constrained_name: impl Fn(&ConstrainedTypeReference) -> String,
    ) -> String {
        match self {
            TypeReference::Reference(reference) => {
                format!("{}<type>", reference.access.borrow_prefix())
            }
            TypeReference::Constrained(constrained) => constrained_name(constrained),
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

    pub fn display_name(&self) -> String {
        self.display_name_with(|constrained| {
            format!(
                "<type>[{}]",
                match constrained.constraints.count() {
                    1 => "1 constraint".to_owned(),
                    count => format!("{count} constraints"),
                }
            )
        })
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
