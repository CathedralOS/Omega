//! Tags of the callable and trait-application texts built with the identity
//! writers. No grammar reads these: they are compared whole, or embedded as the
//! opaque coordinate of a declaration, so they need no vocabulary shared with a
//! reader. Naming them here keeps the writers free of string tags.

use language_semantics::type_identity::vocabulary::Spelled;

#[derive(Clone, Copy)]
pub(super) enum CoordinateTag {
    NamedCallable,
    NestedMachine,
    ClosedRequirement,
    LocalTraitApplication,
    TraitApplication,
    Parameter,
    Path,
    Parameters,
    ResultDispatch,
    Owner,
    Name,
    Template,
    Declaration,
    Arguments,
    LocalArguments,
    Lifetimes,
    Mutable,
    Const,
}

impl Spelled for CoordinateTag {
    fn spelling(self) -> &'static str {
        match self {
            Self::NamedCallable => "named-callable",
            Self::NestedMachine => "nested-machine",
            Self::ClosedRequirement => "closed-requirement",
            Self::LocalTraitApplication => "local-trait-application",
            Self::TraitApplication => "trait-application",
            Self::Parameter => "parameter",
            Self::Path => "path",
            Self::Parameters => "parameters",
            Self::ResultDispatch => "result-dispatch",
            Self::Owner => "owner",
            Self::Name => "name",
            Self::Template => "template",
            Self::Declaration => "declaration",
            Self::Arguments => "arguments",
            Self::LocalArguments => "local-arguments",
            Self::Lifetimes => "lifetimes",
            Self::Mutable => "mutable",
            Self::Const => "const",
        }
    }
}
