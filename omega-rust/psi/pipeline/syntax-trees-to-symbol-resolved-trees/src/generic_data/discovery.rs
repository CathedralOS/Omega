//! Private discovered templates and pending rewrites; not a program representation.

use super::*;

pub(super) struct GenericData {
    pub(super) declaration: syntax_trees::item::ItemHandle,
    pub(super) name: String,
    pub(super) origin_name: Identifier,
    pub(super) is_public: bool,
    pub(super) lifetime_parameters: Vec<Identifier>,
    pub(super) parameter_names: Vec<String>,
    pub(super) const_parameter_types: Vec<Option<TypeReferenceHandle>>,
    pub(super) where_facts: HandleSpan<ProofFact>,
    pub(super) members: HandleSpan<DataMember>,
    pub(super) properties: syntax_trees::item::DataProperties,
    pub(super) supply_mode: language_semantics::DataSupplyMode,
}

pub(super) struct PendingRewrite {
    pub(super) type_reference: TypeReferenceHandle,
    pub(super) synthetic_name: String,
    pub(super) lifetime_arguments: Vec<Identifier>,
}

/// One discovered instantiation: the base generic definition and the argument
/// type references spelled for it, plus the plain name of the record to
/// synthesize.
#[derive(Clone)]
pub(super) struct Instantiation {
    pub(super) synthetic_name: String,
    pub(super) base_name: String,
    pub(super) template: syntax_trees::item::ItemHandle,
    pub(super) argument_handles: Vec<TypeReferenceHandle>,
    pub(super) argument_identity: Vec<ClosedArgumentIdentity>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum GenericDataShape {
    Record,
    PureSum,
    MixedSum,
}

/// Rejoin the selected declaration to this syntax owner. Header symbols never
/// escape, and generated declarations retain distinct arena identities even if
/// they share an absent or authored derivation span.
pub(super) fn selected_data_item(
    syntax: &SyntaxTrees,
    selection: Option<&constant_selection::ConstantSelection>,
    name: &Identifier,
) -> Option<syntax_trees::item::ItemHandle> {
    if let Some(selection) = selection {
        let declaration = selection.data(syntax, name).ok()?;
        return syntax.root_item_handles().iter().copied().find(|handle| {
            matches!(syntax.root_item(*handle), Item::Data(data) if std::ptr::eq(data, declaration))
        });
    }
    let mut candidates = syntax.root_item_handles().iter().copied().filter(|handle| {
        matches!(syntax.root_item(*handle), Item::Data(data) if data.name.as_str() == name.as_str())
    });
    let first = candidates.next()?;
    candidates.next().is_none().then_some(first)
}

pub(super) fn selected_generic_data<'a>(
    syntax: &SyntaxTrees,
    templates: &'a HashMap<syntax_trees::item::ItemHandle, GenericData>,
    selection: Option<&constant_selection::ConstantSelection>,
    name: &Identifier,
) -> Option<&'a GenericData> {
    templates.get(&selected_data_item(syntax, selection, name)?)
}

/// Equality for the already admitted closed argument shapes. Rendered names are
/// only lookup/diagnostic metadata; nominal equality names a live declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ClosedArgumentIdentity {
    Builtin(symbols::BuiltinTypeAtom),
    Nominal(syntax_trees::item::ItemHandle),
    RetainedNominal(symbols::SymbolHandle),
    Instance(syntax_trees::item::ItemHandle, Vec<ClosedArgumentIdentity>),
    Constant(String),
    Array(Box<ClosedArgumentIdentity>, usize),
    Constrained(Box<ClosedArgumentIdentity>, Vec<ClosedConstraintIdentity>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ClosedConstraintIdentity {
    Arithmetic(numerics::arithmetic::ArithmeticDomain),
    Declaration(syntax_trees::item::ItemHandle),
    RetainedDeclaration(symbols::SymbolHandle),
}
