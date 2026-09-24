//! Operator definitions. Domain homes are selected later by
//! `selection::domain_operator_homes`.

use crate::lowering::data::lower_type_parameters;
use crate::lowering::state::{lower_signature_contracts, lower_state_parameters};
use crate::lowering::type_reference::lower_type_reference_handle;
use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_operator_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    operator: &syntax::item::OperatorDefinition,
) -> Result<symbol_resolved_trees::operator::OperatorDefinition, Diagnostic> {
    Ok(symbol_resolved_trees::operator::OperatorDefinition {
        is_public: operator.is_public,
        is_boundary: operator.is_boundary,
        symbol: Default::default(),
        name: lower_operator_name(lowerer, syntax_trees, operator.name),
        lifetime_parameters: operator
            .lifetime_parameters
            .iter()
            .map(crate::lowering::name::lower_name)
            .collect(),
        type_parameters: lower_type_parameters(lowerer, syntax_trees, operator.type_parameters)?,
        parameters: lower_state_parameters(lowerer, syntax_trees, operator.parameters)?,
        return_type: operator
            .return_type
            .is_valid()
            .then(|| lower_type_reference_handle(lowerer, syntax_trees, operator.return_type))
            .transpose()?,
        contracts: lower_signature_contracts(lowerer, syntax_trees, operator.contracts)?,
        spelling: operator.spelling,
        token_count: operator.token_count,
    })
}

fn lower_operator_name(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    name: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<symbol_resolved_trees::name::DiagnosticName> {
    let mut span = HandleSpan::empty();

    for member in syntax_trees.items.identifier_path_members(name) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .operator_path_members
            .append_to_span(&mut span, crate::lowering::name::lower_name(member));
    }

    span
}

/// Whether a `machine` item is a top-level token-bearing boundary signature
/// (`boundary machine + Owner::name(...);`): bodyless, boundary, tokened, and
/// not the explicit `boundary requirement` form.
pub(crate) fn is_token_bearing_boundary_signature(machine: &syntax::item::Machine) -> bool {
    machine.boundary
        && machine.bodyless
        && machine.spelling.is_some()
        && !machine.is_top_level_boundary_requirement
}

/// Lower a top-level token-bearing boundary signature.
///
/// The executable-supply contract says the token binding identifies the
/// required operator slot and a realizing boundary machine reaches it through
/// the ordinary exact `satisfies` relationship without redeclaring the token
/// ([expressions: executable supply](../../../../../../wiki/spec/language/expressions.md#executable-supply)).
/// Provider selection, build-time provider bodies, review evidence, and
/// Terminal lowering all key that slot on the resolved `OperatorDefinition`
/// the `operator` introducer produced, so this form lowers to that same
/// declaration: one owner for every route, no second identity, and the
/// `operator` spelling keeps working until its introducer is retired. The
/// slot carries only its signature and contracts; the machine-only clauses
/// (`reaches`, `invokes`, `suspends`, `blocks`, termination witnesses,
/// conformance bounds) belong to a named `boundary requirement` and reject
/// here rather than being dropped.
pub(crate) fn lower_token_bearing_boundary_signature(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Result<symbol_resolved_trees::operator::OperatorDefinition, Diagnostic> {
    let rejected = |clause: &str| {
        Diagnostic::error(format!(
            "`{}` is a token-bearing boundary signature and declares only its signature and \
             contracts; `{clause}` belongs to a named `boundary requirement`",
            machine.name
        ))
        .with_source_span(machine.name.source_span())
    };
    if !machine.service_reaches.is_empty() || !machine.service_reach_keyword_source_spans.is_empty()
    {
        return Err(rejected("reaches"));
    }
    if !machine.invokes.is_empty() {
        return Err(rejected("invokes"));
    }
    if machine.suspends || machine.blocks {
        return Err(rejected("suspends`/`blocks"));
    }
    if machine.terminates_guarantee || !machine.ranking_subjects.is_empty() {
        return Err(rejected("terminates"));
    }
    if !machine.conformance_bounds.is_empty() {
        return Err(rejected("a conformance bound"));
    }
    let Some(entry) = syntax_trees.items.state_handles(machine.states).first() else {
        return Err(Diagnostic::error(format!(
            "`{}` is a token-bearing boundary signature without an entry signature",
            machine.name
        ))
        .with_source_span(machine.name.source_span()));
    };
    let entry = syntax_trees.items.state(*entry);
    let name_span = machine.name.source_span();
    let mut name = HandleSpan::empty();
    for member in machine.name.as_str().split("::") {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .operator_path_members
            .append_to_span(
                &mut name,
                crate::lowering::name::lower_name(&syntax::identifier::Identifier::new(
                    member, name_span,
                )),
            );
    }
    Ok(symbol_resolved_trees::operator::OperatorDefinition {
        is_public: machine.is_public,
        is_boundary: true,
        symbol: Default::default(),
        name,
        lifetime_parameters: machine
            .lifetime_parameters
            .iter()
            .map(crate::lowering::name::lower_name)
            .collect(),
        type_parameters: lower_type_parameters(lowerer, syntax_trees, machine.type_parameters)?,
        parameters: lower_state_parameters(lowerer, syntax_trees, entry.parameters)?,
        return_type: entry
            .return_type
            .is_valid()
            .then(|| lower_type_reference_handle(lowerer, syntax_trees, entry.return_type))
            .transpose()?,
        contracts: lower_signature_contracts(lowerer, syntax_trees, machine.contracts)?,
        spelling: machine.spelling,
        // The `operator` head counted its own tokens for the declaration
        // fingerprint; the machine head has no such count. The signature and
        // path already distinguish the slot.
        token_count: 0,
    })
}

/// Whether a `machine` item is a bare bodyless signature: no body, not
/// boundary, no `satisfies`/`via`, and not the explicit `boundary requirement`
/// form, with or without a fixed token. The item parser admits the shape;
/// only an exact compiler-catalog primitive may own it, so a token-bearing
/// declaration (`machine + Owner::name(...);`) rejects here with the same
/// missing-body guidance rather than lowering as an unsupplied checked body.
pub(crate) fn is_bare_bodyless_signature(machine: &syntax::item::Machine) -> bool {
    machine.bodyless
        && !machine.boundary
        && machine.satisfies.is_empty()
        && !machine.is_top_level_boundary_requirement
}

/// The compiler catalog's exact identity for a bare bodyless signature, or
/// the rejection every other one earns.
///
/// The executable-supply contract's catalog row supplies "exact compiler-owned
/// bodyless machine" declarations from the closed catalog, and "merely naming
/// a declaration `Float::meaning32` grants no primitive implementation". The
/// key is therefore exact declaration custody, never the leaf spelling: the
/// declaring source must be the sealed toolchain float-operations file, and
/// the path must be one a catalog publishes -- the projection rows
/// (`Float::meaning32`, `Float::meaning64`) or a semantic-definition row
/// (`FloatSemantics::<name>` with its complete normalized signature, so the
/// `from_integer` carriers select distinct rows). The admitted declaration
/// lowers to the same resolved operator declaration the retiring `operator`
/// introducer produced, so the sealed routes (hermetic toolchain symbol
/// identity, exact signature shape, catalog contract identity) validate it
/// unchanged; a same-spelled declaration in any other source is refused here,
/// and a sealed declaration whose signature matches no row rejects instead of
/// lowering as an ordinary declaration.
pub(crate) fn lower_bare_bodyless_signature(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Result<symbol_resolved_trees::operator::OperatorDefinition, Diagnostic> {
    let name = machine.name.as_str();
    let (namespace, leaf) = name.rsplit_once("::").unwrap_or(("", name));
    let family = CatalogFamily::naming(namespace, leaf);
    let sealed_source = family.is_some_and(|family| {
        lowerer
            .sources
            .as_ref()
            .and_then(|sources| sources.get(machine.name.source_span().source_id))
            .is_some_and(|file| {
                file.origin == source::SourceOrigin::Toolchain
                    && file
                        .path
                        .strip_prefix(&file.package_root)
                        .ok()
                        .is_some_and(|relative| {
                            relative == std::path::Path::new(family.sealed_source())
                        })
            })
    });
    let Some(family) = family.filter(|_| sealed_source) else {
        let token = machine
            .spelling
            .map(|spelling| format!("{} ", spelling.symbol()))
            .unwrap_or_default();
        let guidance = if family.is_some() {
            format!(
                "`{name}` names a compiler primitive, but only the sealed toolchain declaration \
                 supplies it; merely naming a declaration `{name}` grants no primitive \
                 implementation or proof authority"
            )
        } else {
            format!(
                "`{name}` has no body and is neither a boundary signature nor a compiler-catalog \
                 primitive; a nonboundary direct machine must own a checked body \
                 (`machine {token}{name}(...) {{ ... }}`), a boundary contract is spelled \
                 `boundary machine {token}...;`, and an external leaf `satisfies Requirement \
                 via <Binding>;`"
            )
        };
        return Err(Diagnostic::error(guidance).with_source_span(machine.name.source_span()));
    };
    let Some(entry) = syntax_trees.items.state_handles(machine.states).first() else {
        return Err(Diagnostic::error(format!(
            "`{name}` is a compiler-catalog signature without an entry signature"
        ))
        .with_source_span(machine.name.source_span()));
    };
    let entry = syntax_trees.items.state(*entry);
    match family {
        CatalogFamily::FloatProjection => {}
        CatalogFamily::FloatSemantics => {
            require_exact_semantic_row(syntax_trees, machine, entry, namespace, leaf)?;
        }
        CatalogFamily::RankingView(declaration) => {
            require_exact_ranking_view_row(syntax_trees, machine, entry, declaration)?;
        }
    }
    let name_span = machine.name.source_span();
    let mut path = HandleSpan::empty();
    for member in name.split("::") {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .operator_path_members
            .append_to_span(
                &mut path,
                crate::lowering::name::lower_name(&syntax::identifier::Identifier::new(
                    member, name_span,
                )),
            );
    }
    Ok(symbol_resolved_trees::operator::OperatorDefinition {
        is_public: machine.is_public,
        is_boundary: false,
        symbol: Default::default(),
        name: path,
        lifetime_parameters: machine
            .lifetime_parameters
            .iter()
            .map(crate::lowering::name::lower_name)
            .collect(),
        type_parameters: lower_type_parameters(lowerer, syntax_trees, machine.type_parameters)?,
        parameters: lower_state_parameters(lowerer, syntax_trees, entry.parameters)?,
        return_type: entry
            .return_type
            .is_valid()
            .then(|| lower_type_reference_handle(lowerer, syntax_trees, entry.return_type))
            .transpose()?,
        contracts: lower_signature_contracts(lowerer, syntax_trees, machine.contracts)?,
        // The catalog admits a sealed bodyless declaration with or without a
        // fixed token; whatever token it declared is the slot's own spelling.
        spelling: machine.spelling,
        token_count: 0,
    })
}

/// The closed catalogs a bare bodyless signature may name, each with the one
/// sealed toolchain source (relative to the core package root) whose
/// declaration it supplies. A family is selected by exact path; the leaf
/// spelling alone selects nothing.
#[derive(Clone, Copy)]
enum CatalogFamily {
    /// `Float::meaning32` / `Float::meaning64` (`float_operations.omg`).
    FloatProjection,
    /// `FloatSemantics::<name>` rows (`float_operations.omg`), keyed on the
    /// complete signature.
    FloatSemantics,
    /// A canonical ranking view with a declaration row (`Nat::Descending` in
    /// `nat.omg`).
    RankingView(language_semantics::RankingViewDeclaration),
}

impl CatalogFamily {
    fn naming(namespace: &str, leaf: &str) -> Option<Self> {
        if numerics::float_projection::FloatProjectionOperation::from_source_identity(
            namespace, leaf,
        )
        .is_some()
        {
            return Some(Self::FloatProjection);
        }
        if numerics::float_semantics_catalog::FloatSemanticOperation::names_a_row(namespace, leaf) {
            return Some(Self::FloatSemantics);
        }
        language_semantics::RankingViewId::from_catalog_declaration(namespace, leaf)
            .map(Self::RankingView)
    }

    fn sealed_source(self) -> &'static str {
        match self {
            Self::FloatProjection | Self::FloatSemantics => {
                numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE
            }
            Self::RankingView(declaration) => declaration.source,
        }
    }
}

/// A sealed ranking-view signature must match its declaration row exactly:
/// one plain parameter of the row's carrier and the row's result, no type or
/// lifetime parameters; a drifted declaration rejects instead of lowering as
/// an ordinary (unsupplied) declaration.
fn require_exact_ranking_view_row(
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
    entry: &syntax::item::StateNode,
    declaration: language_semantics::RankingViewDeclaration,
) -> Result<(), Diagnostic> {
    let name = machine.name.as_str();
    let drift = |detail: String| {
        Diagnostic::error(format!(
            "`{name}` names the compiler ranking-view catalog, but {detail}; the sealed \
             declaration must be exactly `machine {}({}) -> {};`",
            declaration.path(),
            declaration.parameter,
            declaration.result
        ))
        .with_source_span(machine.name.source_span())
    };
    if !machine.type_parameters.is_empty() || !machine.lifetime_parameters.is_empty() {
        return Err(drift(
            "catalog rows declare no type or lifetime parameters".to_owned(),
        ));
    }
    let named_spelling = |handle: syntax::types::TypeReferenceHandle| match syntax_trees
        .tables
        .type_references
        .type_reference(handle)
    {
        syntax::types::TypeReferenceNode::Named(identifier) => Some(identifier.as_str()),
        _ => None,
    };
    let [parameter] = syntax_trees.items.state_parameters(entry.parameters) else {
        return Err(drift("the row ranks exactly one subject".to_owned()));
    };
    let parameter = syntax_trees.items.state_parameter(*parameter);
    if parameter.is_const || parameter.is_mutable || parameter.is_self {
        return Err(drift(format!(
            "parameter `{}` carries a binding mode the row does not declare",
            parameter.name.as_str()
        )));
    }
    if named_spelling(parameter.type_reference) != Some(declaration.parameter) {
        return Err(drift(format!(
            "parameter `{}` is not the row's `{}` subject",
            parameter.name.as_str(),
            declaration.parameter
        )));
    }
    if !entry.return_type.is_valid()
        || named_spelling(entry.return_type) != Some(declaration.result)
    {
        return Err(drift(format!(
            "its result is not the row's `{}` rank",
            declaration.result
        )));
    }
    Ok(())
}

/// A sealed `FloatSemantics::<leaf>` signature must select one exact catalog
/// row by its complete normalized signature; a drifted declaration rejects
/// here rather than lowering as an ordinary (unsupplied) declaration.
fn require_exact_semantic_row(
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
    entry: &syntax::item::StateNode,
    namespace: &str,
    leaf: &str,
) -> Result<&'static numerics::float_semantics_catalog::FloatSemanticOperation, Diagnostic> {
    use numerics::float_semantics_catalog::{FloatSemanticOperation, FloatSemanticValueKind};
    let name = machine.name.as_str();
    let drift = |detail: String| {
        Diagnostic::error(format!(
            "`{name}` names the compiler float-semantics catalog, but {detail}; the sealed \
             declaration must match one catalog row exactly"
        ))
        .with_source_span(machine.name.source_span())
    };
    if !machine.type_parameters.is_empty() || !machine.lifetime_parameters.is_empty() {
        return Err(drift(
            "catalog rows declare no type or lifetime parameters".to_owned(),
        ));
    }
    let kind_of = |handle: syntax::types::TypeReferenceHandle, position: &str| match syntax_trees
        .tables
        .type_references
        .type_reference(handle)
    {
        syntax::types::TypeReferenceNode::Named(identifier) => {
            FloatSemanticValueKind::from_spelling(identifier.as_str()).ok_or_else(|| {
                drift(format!(
                    "{position} type `{}` is not a catalog value kind",
                    identifier.as_str()
                ))
            })
        }
        _ => Err(drift(format!(
            "{position} type is not a named catalog value kind"
        ))),
    };
    let mut parameters = Vec::new();
    for handle in syntax_trees.items.state_parameters(entry.parameters) {
        let parameter = syntax_trees.items.state_parameter(*handle);
        if parameter.is_const || parameter.is_mutable || parameter.is_self {
            return Err(drift(format!(
                "parameter `{}` carries a binding mode no catalog row declares",
                parameter.name.as_str()
            )));
        }
        parameters.push(kind_of(
            parameter.type_reference,
            &format!("parameter `{}`", parameter.name.as_str()),
        )?);
    }
    if !entry.return_type.is_valid() {
        return Err(drift("it declares no result type".to_owned()));
    }
    let result = kind_of(entry.return_type, "result")?;
    FloatSemanticOperation::from_source_identity(namespace, leaf, &parameters, result).ok_or_else(
        || {
            drift(format!(
                "its signature ({}) -> {} matches no catalog row",
                parameters
                    .iter()
                    .map(|kind| kind.spelling())
                    .collect::<Vec<_>>()
                    .join(", "),
                result.spelling()
            ))
        },
    )
}
