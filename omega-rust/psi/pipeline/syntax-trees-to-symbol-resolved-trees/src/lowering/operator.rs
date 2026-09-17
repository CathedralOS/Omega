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

/// Whether a `machine` item is a bare bodyless tokenless signature: no body,
/// no token, not boundary, no `satisfies`/`via`, and not the explicit
/// `boundary requirement` form. The item parser admits the shape; only an
/// exact compiler-catalog primitive may own it.
pub(crate) fn is_bare_bodyless_signature(machine: &syntax::item::Machine) -> bool {
    machine.bodyless
        && !machine.boundary
        && machine.spelling.is_none()
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
/// the path must be one the catalog publishes (`Float::meaning32`,
/// `Float::meaning64`). The admitted declaration lowers to the same resolved
/// operator declaration the retiring `operator` introducer produced, so the
/// sealed projection route (hermetic toolchain symbol identity, exact
/// signature shape, catalog contract identity) validates it unchanged; a
/// same-spelled declaration in any other source is refused here.
pub(crate) fn lower_bare_bodyless_signature(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Result<symbol_resolved_trees::operator::OperatorDefinition, Diagnostic> {
    let name = machine.name.as_str();
    let catalog_path = name
        .rsplit_once("::")
        .and_then(|(namespace, leaf)| {
            numerics::float_projection::FloatProjectionOperation::from_source_identity(
                namespace, leaf,
            )
        })
        .is_some();
    let sealed_source = lowerer
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
                        relative
                            == std::path::Path::new(
                                numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE,
                            )
                    })
        });
    if !catalog_path || !sealed_source {
        let guidance = if catalog_path {
            format!(
                "`{name}` names a compiler primitive, but only the sealed toolchain declaration \
                 supplies it; merely naming a declaration `{name}` grants no primitive \
                 implementation or proof authority"
            )
        } else {
            format!(
                "`{name}` has no body and is neither a boundary signature nor a compiler-catalog \
                 primitive; a nonboundary direct machine must own a checked body \
                 (`machine {name}(...) {{ ... }}`), a boundary contract is spelled `boundary \
                 machine ...;`, and an external leaf `satisfies Requirement via <Binding>;`"
            )
        };
        return Err(Diagnostic::error(guidance).with_source_span(machine.name.source_span()));
    }
    let Some(entry) = syntax_trees.items.state_handles(machine.states).first() else {
        return Err(Diagnostic::error(format!(
            "`{name}` is a compiler-catalog signature without an entry signature"
        ))
        .with_source_span(machine.name.source_span()));
    };
    let entry = syntax_trees.items.state(*entry);
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
        spelling: None,
        token_count: 0,
    })
}
