//! Classify bodyless machine headers and validate the exact declarations supplied by the compiler.

use crate::resolution::lowering::lowerer::Lowerer;
use diagnostics::Diagnostic;
use language_core::atomic::ATOMIC_FENCE_NAMESPACE;
use language_core::byte_predicates::ByteSequencePredicate;
use language_core::sealed_intrinsics::{CompilerPrimitive, SealedIntrinsic};
use numerics::float_projection::FloatProjectionOperation;
use syntax::types::TypeReferenceNode;
use syntax_trees::{self as syntax, SyntaxTrees};

/*
    Whether a `machine` item is a top-level token-bearing boundary signature
    (`boundary machine + Owner::name(...);`): bodyless, boundary and tokened.
*/
pub(crate) fn is_token_bearing_boundary_signature(machine: &syntax::item::Machine) -> bool {
    machine.boundary && machine.bodyless && machine.spelling.is_some()
}

/*
    Whether a `machine` item is a bare bodyless signature: no body, not
    boundary, no `satisfies`/`via`, with or without a fixed
    token. The item parser admits the shape;
    only an exact compiler-catalog primitive may own it, so a token-bearing
    declaration (`machine + Owner::name(...);`) rejects here with the same
    missing-body guidance rather than lowering as an unsupplied checked body.
*/
pub(crate) fn is_bare_bodyless_signature(machine: &syntax::item::Machine) -> bool {
    machine.bodyless && !machine.boundary && machine.satisfies.is_empty()
}

/*
    A bodyless, nonboundary machine needs a definition supplied by the compiler.
    Check the source's sealed catalog identity and the header required by that
    family. The caller lowers the ordinary machine header after this succeeds.
*/
pub(crate) fn require_catalog_signature(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Result<(), Diagnostic> {
    let name = machine.name.as_str(syntax_trees);
    let (namespace, leaf) = name.rsplit_once("::").unwrap_or(("", name));
    let custody = lowerer
        .sources
        .as_ref()
        .and_then(|sources| sources.get(machine.name.source_span().source_id))
        .and_then(|file| crate::source_symbols::sealed_intrinsic_at(file, name));
    let family = match custody {
        Some(SealedIntrinsic::CompilerPrimitive(
            primitive @ (CompilerPrimitive::Collection(_) | CompilerPrimitive::RunCustody(_)),
        )) => Some(CatalogFamily::Typed(primitive)),
        Some(SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::AtomicFence)) => {
            Some(CatalogFamily::AtomicFence)
        }
        Some(SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::ByteObservation(predicate))) => {
            Some(CatalogFamily::ByteObservation(predicate))
        }
        _ => CatalogFamily::naming(namespace, leaf),
    };
    let sealed_source = family.is_some_and(|family| {
        custody == Some(SealedIntrinsic::CompilerPrimitive(family.primitive()))
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
        // The exact bundled declaration proceeds through the complete
        // generic machine owner. Typed schema normalization retains every
        // binder, lifetime relation and contract before a use is served.
        CatalogFamily::Typed(_) => {}
        CatalogFamily::ByteObservation(_) => {
            require_exact_byte_observation_row(syntax_trees, machine, entry)?;
        }
        CatalogFamily::AtomicFence => {
            require_exact_atomic_fence_row(syntax_trees, machine, entry)?;
        }
        CatalogFamily::FloatSemantics => {
            require_exact_semantic_row(syntax_trees, machine, entry, namespace, leaf)?;
        }
        CatalogFamily::RankingView(declaration) => {
            require_exact_ranking_view_row(syntax_trees, machine, entry, declaration)?;
        }
    }
    Ok(())
}

/*
    The closed catalogs a bare bodyless signature may name, each with the one
    sealed toolchain source (relative to the core package root) whose
    declaration it supplies. A family is selected by exact path; the leaf
    spelling alone selects nothing.
*/
#[derive(Clone, Copy)]
enum CatalogFamily {
    /// These declarations pass their full headers to the typed schema checker.
    Typed(CompilerPrimitive),
    ByteObservation(ByteSequencePredicate),
    /* Place-free portable fence (`atomic.omg`). */
    AtomicFence,
    /*
        `FloatSemantics::<name>` rows (`float_operations.omg`), keyed on the
        complete signature.
    */
    FloatSemantics,
    /*
        A canonical ranking view with a declaration row (`Nat::Descending` in
        `nat.omg`).
    */
    RankingView(declaration_contracts::RankingViewDeclaration),
}

impl CatalogFamily {
    fn naming(namespace: &str, leaf: &str) -> Option<Self> {
        if namespace == ATOMIC_FENCE_NAMESPACE && leaf == language_core::atomic::ATOMIC_FENCE_NAME {
            return Some(Self::AtomicFence);
        }
        if FloatProjectionOperation::from_source_identity(namespace, leaf).is_some() {
            return Some(Self::Typed(CompilerPrimitive::FloatProjection));
        }
        if numerics::float_semantics_catalog::FloatSemanticOperation::names_a_row(namespace, leaf) {
            return Some(Self::FloatSemantics);
        }
        declaration_contracts::RankingViewId::from_catalog_declaration(namespace, leaf)
            .map(Self::RankingView)
    }

    fn primitive(self) -> CompilerPrimitive {
        match self {
            Self::Typed(primitive) => primitive,
            Self::ByteObservation(predicate) => CompilerPrimitive::ByteObservation(predicate),
            Self::AtomicFence => CompilerPrimitive::AtomicFence,
            Self::FloatSemantics => CompilerPrimitive::FloatSemantics,
            Self::RankingView(_) => CompilerPrimitive::NaturalRanking,
        }
    }
}

/*
    A sealed ranking-view signature must match its declaration row exactly:
    plain parameters of the row's carrier and the row's result, no type or
    lifetime parameters; a drifted declaration rejects instead of lowering as
    an ordinary (unsupplied) declaration.
*/
fn require_exact_ranking_view_row(
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
    entry: &syntax::item::State,
    declaration: declaration_contracts::RankingViewDeclaration,
) -> Result<(), Diagnostic> {
    let name = machine.name.as_str(syntax_trees);
    let drift = |detail: String| {
        let parameters = vec![declaration.parameter; declaration.subject_count].join(", ");
        Diagnostic::error(format!(
            "`{name}` names the compiler ranking-view catalog, but {detail}; the sealed \
             declaration must have the signature `{}({parameters}) -> {}`",
            declaration.path(),
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
        TypeReferenceNode::Named(identifier) => Some(identifier.as_str(syntax_trees)),
        _ => None,
    };
    let parameters = syntax_trees.items.state_parameters(entry.parameters);
    if parameters.len() != declaration.subject_count {
        return Err(drift(format!(
            "the row ranks exactly {} subjects",
            declaration.subject_count
        )));
    }
    for parameter in parameters {
        let parameter = syntax_trees.items.state_parameter(*parameter);
        if parameter.is_const || parameter.is_mutable {
            return Err(drift(format!(
                "parameter `{}` carries a binding mode the row does not declare",
                parameter.name.as_str(syntax_trees)
            )));
        }
        if named_spelling(parameter.type_reference) != Some(declaration.parameter) {
            return Err(drift(format!(
                "parameter `{}` is not the row's `{}` subject",
                parameter.name.as_str(syntax_trees),
                declaration.parameter
            )));
        }
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

/*
    A sealed `FloatSemantics::<leaf>` signature must select one exact catalog
    row by its complete normalized signature; a drifted declaration rejects
    here rather than lowering as an ordinary (unsupplied) declaration.
*/
fn require_exact_semantic_row(
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
    entry: &syntax::item::State,
    namespace: &str,
    leaf: &str,
) -> Result<numerics::float_semantics_catalog::FloatSemanticOperation, Diagnostic> {
    use numerics::float_semantics_catalog::{FloatSemanticOperation, FloatSemanticValueKind};
    let name = machine.name.as_str(syntax_trees);
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
        TypeReferenceNode::Named(identifier) => FloatSemanticValueKind::from_spelling(
            identifier.as_str(syntax_trees),
        )
        .ok_or_else(|| {
            drift(format!(
                "{position} type `{}` is not a catalog value kind",
                identifier.as_str(syntax_trees)
            ))
        }),
        _ => Err(drift(format!(
            "{position} type is not a named catalog value kind"
        ))),
    };
    let mut parameters = Vec::new();
    for handle in syntax_trees.items.state_parameters(entry.parameters) {
        let parameter = syntax_trees.items.state_parameter(*handle);
        if parameter.is_const || parameter.is_mutable {
            return Err(drift(format!(
                "parameter `{}` carries a binding mode no catalog row declares",
                parameter.name.as_str(syntax_trees)
            )));
        }
        parameters.push(kind_of(
            parameter.type_reference,
            &format!("parameter `{}`", parameter.name.as_str(syntax_trees)),
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
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                result
            ))
        },
    )
}

// Catalog custody selects the operation. Its one ordinary machine header
// still has to be the complete pure shared-byte observation declaration.
fn require_exact_byte_observation_row(
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
    entry: &syntax::item::State,
) -> Result<(), Diagnostic> {
    let drift = || {
        Diagnostic::error(
        "the sealed byte-observation declaration requires one explicit input lifetime, one relevant shared byte-view input, a bool result, reaches none, crashes none and unconditional terminates, with no additional header or body",
    ).with_source_span(machine.name.source_span())
    };
    if !machine.is_public
        || !machine.bodyless
        || machine.boundary
        || !machine
            .generic_data_template
            .as_str(syntax_trees)
            .is_empty()
        || !machine.default_body_trait.as_str(syntax_trees).is_empty()
        || machine.attached_data.is_some()
        || machine.spelling.is_some()
        || machine.target_family_interface
        || machine.target.is_some()
        || machine.authored_target.is_some()
        || machine.type_parameters != arena::HandleSpan::empty()
        || machine.satisfies != arena::HandleSpan::empty()
        || !machine.conformance_bounds.is_empty()
        || !machine.terminates_guarantee
        || machine.ranking_subjects != arena::HandleSpan::empty()
        || machine.ranking_view != arena::HandleSpan::empty()
        || machine.ranking_view_arguments != arena::HandleSpan::empty()
        || machine.ranking_range.is_valid()
        || machine.service_reach_keyword_source_spans.len() != 1
        || machine.service_reaches != arena::HandleSpan::empty()
        || machine.service_reach_is_installation_bound
        || machine.invokes != arena::HandleSpan::empty()
        || machine.suspends
        || machine.blocks
        || !machine.suspends_keyword_source_spans.is_empty()
        || !machine.blocks_keyword_source_spans.is_empty()
        || machine.admitted_same_stack_bytes != 0
        || !machine.declares_no_crash_route
        || machine.contracts != arena::HandleSpan::empty()
        || machine.states.len() != 1
        || syntax_trees.items.state_handles(machine.states).len() != 1
        || machine.nested_machines != arena::HandleSpan::empty()
        || entry.contracts != arena::HandleSpan::empty()
        || entry.statements != arena::HandleSpan::empty()
        || entry.parameters.len() != 1
    {
        return Err(drift());
    }
    let [lifetime] = machine.lifetime_parameters.as_slice() else {
        return Err(drift());
    };
    if lifetime.as_str(syntax_trees).is_empty() {
        return Err(drift());
    }
    let [parameter] = syntax_trees.items.state_parameters(entry.parameters) else {
        return Err(drift());
    };
    let parameter = syntax_trees.items.state_parameter(*parameter);
    if parameter.is_const
        || parameter.is_mutable
        || parameter.relevance != language_core::BindingRelevance::Relevant
    {
        return Err(drift());
    }
    let types = &syntax_trees.tables.type_references;
    let TypeReferenceNode::Reference {
        referee,
        access: language_core::ReferenceAccess::Shared,
        lifetime: Some(input_lifetime),
    } = types.type_reference(parameter.type_reference)
    else {
        return Err(drift());
    };
    if input_lifetime.as_str(syntax_trees) != lifetime.as_str(syntax_trees) {
        return Err(drift());
    }
    let TypeReferenceNode::Slice { element_type } = types.type_reference(*referee) else {
        return Err(drift());
    };
    if !matches!(types.type_reference(*element_type), TypeReferenceNode::Named(name)
        if name.as_str(syntax_trees) == "u8")
        || !entry.return_type.is_valid()
        || !matches!(types.type_reference(entry.return_type), TypeReferenceNode::Named(name)
            if name.as_str(syntax_trees) == "bool")
    {
        return Err(drift());
    }
    Ok(())
}

fn require_exact_atomic_fence_row(
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
    entry: &syntax::item::State,
) -> Result<(), Diagnostic> {
    let valid_parameter = match syntax_trees.items.state_parameters(entry.parameters) {
        [parameter] => {
            let parameter = syntax_trees.items.state_parameter(*parameter);
            !parameter.is_const
                && !parameter.is_mutable
                && !parameter.relevance.is_erased()
                && matches!(syntax_trees.tables.type_references.type_reference(parameter.type_reference),
                TypeReferenceNode::Named(name) if name.as_str(syntax_trees) == "FenceOrdering")
        }
        _ => false,
    };
    if !machine.is_public
        || machine.spelling.is_some()
        || !machine.type_parameters.is_empty()
        || !machine.lifetime_parameters.is_empty()
        || !machine.contracts.is_empty()
        || entry.return_type.is_valid()
        || !valid_parameter
    {
        return Err(Diagnostic::error("the sealed atomic fence declaration must be exactly `public machine fence(ordering: FenceOrdering);` in `module Atomic`").with_source_span(machine.name.source_span()));
    }
    Ok(())
}

#[cfg(test)]
mod compiler_machine_tests {
    use crate::resolution::{ResolutionRequest, resolve};
    use crate::source_inputs::SourceMap;
    use declaration_contracts::source_identity::DependencyScope;
    use source_admission::{SourceOrigin, SourceResolutionStratum};
    use std::path::PathBuf;
    use std::sync::Arc;

    const ATOMIC: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/atomic.omg"
    ));

    const ORDERING: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/fence_ordering.omg"
    ));

    fn parse_into(
        sources: &mut SourceMap,
        text: &str,
        path: &str,
        origin: SourceOrigin,
    ) -> syntax_trees::SyntaxTrees {
        let source_id = sources
            .add_checked_instance(
                PathBuf::from("toolchain").join(path),
                text.to_owned(),
                PathBuf::from("toolchain"),
                None,
                origin,
                SourceResolutionStratum::Base,
                DependencyScope::Product,
            )
            .source_id;
        let tokens = crate::frontend_fixture::Lexer::new(text)
            .tokenize()
            .expect("the test source must tokenize");
        crate::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens)
            .expect("the test source must parse")
    }

    fn parsed(
        text: &str,
        path: &str,
        origin: SourceOrigin,
    ) -> (syntax_trees::SyntaxTrees, Arc<SourceMap>) {
        let mut sources = SourceMap::default();
        let syntax = parse_into(&mut sources, text, path, origin);
        (syntax, Arc::new(sources))
    }

    fn resolved(
        text: &str,
        path: &str,
        origin: SourceOrigin,
    ) -> Result<crate::source_resolved::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
        let mut sources = SourceMap::default();
        let mut syntax = parse_into(&mut sources, text, path, origin);
        syntax.extend_from(&parse_into(
            &mut sources,
            ORDERING,
            "fence_ordering.omg",
            SourceOrigin::Toolchain,
        ));
        let mut request = ResolutionRequest::new(syntax);
        request.sources = Some(Arc::new(sources));
        resolve(request)
    }

    #[test]
    fn the_installer_selects_its_slot_before_reading_its_type_arguments() {
        use crate::source_resolved::expression::ExpressionNode;
        use symbols::SymbolKind;
        let core = std::str::from_utf8(language_core::static_interrupts::CORE_DECLARATIONS)
            .expect("bundled declaration is UTF-8");
        let caller = "trait SlotKind {}
                TimerSlot satisfies SlotKind {}
                data Table {}
                machine owner() { let selected: bool = X86StaticInterruptScope::run<owner, TimerSlot, Table>(); }";
        for origin in [SourceOrigin::Toolchain, SourceOrigin::User] {
            let mut sources = SourceMap::default();
            let declaration = if origin == SourceOrigin::Toolchain {
                core
            } else {
                "data X86StaticInterruptScope {} machine X86StaticInterruptScope::run<Table, Context>() -> bool { true }"
            };
            let mut syntax = parse_into(&mut sources, declaration, "static_interrupt.omg", origin);
            if origin == SourceOrigin::Toolchain {
                let assembly = include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../../../source/library/core/assembly.omg"
                ));
                syntax.extend_from(&parse_into(
                    &mut sources,
                    assembly,
                    "assembly.omg",
                    SourceOrigin::Toolchain,
                ));
                let interrupts = include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../../../source/library/core/interrupt.omg"
                ));
                syntax.extend_from(&parse_into(
                    &mut sources,
                    interrupts,
                    "interrupt.omg",
                    SourceOrigin::Toolchain,
                ));
                let representation = include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../../../source/library/core/representation.omg"
                ));
                let mut representation = parse_into(
                    &mut sources,
                    representation,
                    "representation.omg",
                    SourceOrigin::Toolchain,
                );
                let tables = &representation.tables;
                representation.roots.items.retain(|item| {
                    matches!(tables.items.item(*item), syntax_trees::item::Item::Trait(trait_)
                        if trait_.name.as_str(&tables.text) == "OpaqueRepresentation")
                });
                syntax.extend_from(&representation);
            }
            syntax.extend_from(&parse_into(
                &mut sources,
                caller,
                "main.omg",
                SourceOrigin::User,
            ));
            let mut request = ResolutionRequest::new(syntax);
            request.sources = Some(Arc::new(sources));
            let resolved = resolve(request).expect("resolve the installer and its caller");
            let call = resolved
                .tables
                .bodies
                .expressions
                .iter_expressions()
                .find_map(|(_, node)| match node {
                    ExpressionNode::Call(call)
                        if call.machine_arguments.len() == 3
                            && call.machine_arguments[1]
                                .path
                                .last()
                                .is_some_and(|name| name.as_str() == "TimerSlot") =>
                    {
                        Some(call)
                    }
                    _ => None,
                })
                .expect("the source installation call");
            let slot = call.machine_arguments[1].symbol;
            if origin == SourceOrigin::Toolchain {
                assert_eq!(resolved.symbols.get(slot).kind, SymbolKind::Conformance);
                assert_eq!(
                    resolved.symbols.get(call.machine_arguments[2].symbol).kind,
                    SymbolKind::Data
                );
            } else {
                assert!(
                    !slot.is_valid(),
                    "a user's second type parameter cannot select a conformance"
                );
            }
        }
    }
    #[test]
    fn sealed_bodyless_declaration_retains_its_machine_owner() {
        let trees = resolved(ATOMIC, "atomic.omg", SourceOrigin::Toolchain)
            .expect("the bundled fence declaration must resolve");
        assert_eq!(trees.machines.len(), 1, "one authoritative machine owner");
        let machine = &trees.machines[0];
        assert_eq!(machine.name.as_str(), "fence");
        assert_eq!(
            trees.symbols.display_path(machine.symbol, "::"),
            "Atomic::fence"
        );
        assert_eq!(
            machine.supply_mode,
            declaration_contracts::MachineSupplyMode::CompilerProvided
        );
        assert!(!machine.body_is_present);
        assert!(machine.symbol.is_valid());
        let [entry] = trees.machine_state_handles(machine.states) else {
            panic!("the complete machine must retain its entry signature");
        };
        let entry = trees.machine_state(*entry);
        assert_eq!(trees.state_parameters(entry.parameters).len(), 1);
        assert!(
            trees.operators.is_empty(),
            "no partial operator header survives"
        );
    }

    const BYTE_OBSERVATION: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/byte_observation.omg"
    ));

    #[test]
    fn bundled_byte_observations_retain_one_complete_machine_header() {
        let trees = resolved(
            BYTE_OBSERVATION,
            "byte_observation.omg",
            SourceOrigin::Toolchain,
        )
        .expect("the exact bundled observations must resolve");
        assert_eq!(trees.machines.len(), 4);
        for machine in &trees.machines {
            assert_eq!(
                machine.supply_mode,
                declaration_contracts::MachineSupplyMode::CompilerProvided
            );
            assert!(!machine.body_is_present);
            assert_eq!(machine.lifetime_parameters.len(), 1);
            assert!(matches!(machine.termination_plan.interface,
                declaration_contracts::TerminationInterface::Published(
                    declaration_contracts::TerminationGuarantee::Terminates { ref premises }
                ) if premises.is_empty()));
        }
        assert!(trees.operators.is_empty());
    }

    #[test]
    fn byte_observation_shape_does_not_replace_catalog_custody() {
        let edited = format!("{BYTE_OBSERVATION}\n// local edit\n");
        for (text, path, origin) in [
            (BYTE_OBSERVATION, "byte_observation.omg", SourceOrigin::User),
            (
                BYTE_OBSERVATION,
                "copied/byte_observation.omg",
                SourceOrigin::Toolchain,
            ),
            (
                edited.as_str(),
                "byte_observation.omg",
                SourceOrigin::Toolchain,
            ),
        ] {
            let errors = resolved(text, path, origin).expect_err("no primitive supply");
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("has no body")),
                "{errors:?}"
            );
        }
    }

    #[test]
    fn byte_observation_header_checker_refuses_each_changed_admission_axis() {
        const HEADER: &str = "public machine valid_utf8<'input>(bytes: &'input [u8]) -> bool reaches none crashes none terminates;";
        let check = |text: &str| {
            // Directly exercise shape checking; these edited bytes receive
            // no catalog custody, as the separate source-origin test pins.
            let (syntax, _) = parsed(text, "byte_observation.omg", SourceOrigin::Toolchain);
            let machine = syntax
                .root_items()
                .find_map(|item| match item {
                    syntax_trees::item::Item::Machine(machine) => Some(machine),
                    _ => None,
                })
                .unwrap();
            let [entry] = syntax.items.state_handles(machine.states) else {
                panic!("one entry");
            };
            super::require_exact_byte_observation_row(&syntax, machine, syntax.items.state(*entry))
        };
        check(HEADER).expect("the exact pure header");
        for changed in [
            HEADER.replace("public machine", "machine"),
            HEADER.replace("&'input [u8]", "&'input mut [u8]"),
            HEADER.replace("&'input [u8]", "&[u8]"),
            HEADER.replace("[u8]", "[u16]"),
            HEADER.replace("-> bool", "-> u64"),
            HEADER.replace("<'input>", "<'input, 'other>"),
            HEADER.replace("reaches none ", ""),
            HEADER.replace("crashes none ", ""),
            HEADER.replace("terminates;", ";"),
            HEADER.replace("terminates;", "terminates requires true;"),
            HEADER.replace("bytes: &'input [u8]", "bytes: &'input [u8], extra: u64"),
        ] {
            assert!(
                check(&changed).is_err(),
                "header unexpectedly admitted: {changed}"
            );
        }
    }

    #[test]
    fn copied_or_edited_catalog_source_cannot_acquire_compiler_supply() {
        let edited = format!("{ATOMIC}\n// local edit\n");
        for (text, path, origin) in [
            (ATOMIC, "atomic.omg", SourceOrigin::User),
            (ATOMIC, "other.omg", SourceOrigin::Toolchain),
            (edited.as_str(), "atomic.omg", SourceOrigin::Toolchain),
        ] {
            let errors = resolved(text, path, origin)
                .expect_err("source custody must not be inferred from the machine shape");
            assert!(
                errors.iter().any(|error| error
                    .message
                    .contains("neither a boundary signature nor a compiler-catalog primitive")),
                "unexpected errors: {errors:?}"
            );
        }
    }

    #[test]
    fn ordinary_bodyless_machine_still_requires_a_supplier() {
        let errors = resolved(
            "machine unsupported(value: u64) -> u64;",
            "user.omg",
            SourceOrigin::User,
        )
        .expect_err("a bodyless user declaration has no compiler supply");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("has no body"))
        );
    }

    #[test]
    fn existing_catalog_families_use_complete_machine_lowering() {
        const FLOAT: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../source/library/core/float_operations.omg"
        ));
        const NAT: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../source/library/core/nat.omg"
        ));
        for (text, path, name, arity) in [
            (FLOAT, "float_operations.omg", "Float::meaning32", 1),
            (FLOAT, "float_operations.omg", "FloatSemantics::add", 3),
            (NAT, "nat.omg", "Nat::Descending", 1),
            (NAT, "nat.omg", "Nat::BoundedDistance", 2),
        ] {
            let (syntax, sources) = parsed(text, path, SourceOrigin::Toolchain);
            let authored = syntax
                .root_items()
                .find_map(|item| match item {
                    syntax_trees::item::Item::Machine(machine)
                        if machine.name.as_str(&syntax) == name =>
                    {
                        Some(machine)
                    }
                    _ => None,
                })
                .expect("the bundled declaration must exist");
            let mut lowerer =
                crate::resolution::lowering::lowerer::Lowerer::new(Some(sources), vec![]);
            crate::resolution::lowering::machine::lower_machine_into(
                &mut lowerer,
                &syntax,
                authored,
            )
            .expect("existing catalog rows must use ordinary machine lowering");
            let trees = &lowerer.symbol_resolved_trees;
            assert_eq!(trees.machines.len(), 1);
            let machine = &trees.machines[0];
            assert_eq!(machine.name.as_str(), name);
            assert_eq!(
                machine.supply_mode,
                declaration_contracts::MachineSupplyMode::CompilerProvided
            );
            assert_eq!(machine.is_public, authored.is_public);
            assert!(!machine.body_is_present);
            let [entry] = trees.machine_state_handles(machine.states) else {
                panic!("the catalog machine must retain its entry");
            };
            let entry = trees.machine_state(*entry);
            assert_eq!(trees.state_parameters(entry.parameters).len(), arity);
            assert!(entry.return_type.is_some());
            assert!(trees.operators.is_empty());
        }
    }

    #[test]
    fn run_primitives_keep_generic_headers_and_refuse_copied_sources() {
        const RUN: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../source/library/core/run.omg"
        ));
        let edited = format!("{RUN}\n// local edit\n");
        for (text, origin, admitted) in [
            (RUN, SourceOrigin::Toolchain, true),
            (RUN, SourceOrigin::User, false),
            (edited.as_str(), SourceOrigin::Toolchain, false),
        ] {
            for (name, arity) in [
                ("Run::retire_empty", 1),
                ("Run::split", 2),
                ("Run::merge", 2),
            ] {
                let (syntax, sources) = parsed(text, "run.omg", origin);
                let authored = syntax
                    .root_items()
                    .find_map(|item| match item {
                        syntax_trees::item::Item::Machine(machine)
                            if machine.name.as_str(&syntax) == name =>
                        {
                            Some(machine)
                        }
                        _ => None,
                    })
                    .expect("the bundled Run declaration exists");
                let mut lowerer =
                    crate::resolution::lowering::lowerer::Lowerer::new(Some(sources), vec![]);
                let result = crate::resolution::lowering::machine::lower_machine_into(
                    &mut lowerer,
                    &syntax,
                    authored,
                );
                if !admitted {
                    let error = result.expect_err("copied source cannot supply Run primitives");
                    assert!(
                        error.message.contains(
                            "neither a boundary signature nor a compiler-catalog primitive"
                        ),
                        "{error:?}"
                    );
                    continue;
                }
                result.expect("the exact bundled Run declaration must lower");
                let trees = &lowerer.symbol_resolved_trees;
                assert_eq!(
                    trees.machines.len(),
                    1,
                    "one complete Run machine must remain"
                );
                let machine = &trees.machines[0];
                assert_eq!(
                    machine.supply_mode,
                    declaration_contracts::MachineSupplyMode::CompilerProvided
                );
                assert_eq!(machine.is_public, authored.is_public);
                assert!(!machine.body_is_present);
                let [policy, element] = trees.machine_type_parameters(machine) else {
                    panic!("Run needs its exact policy and element binders");
                };
                use crate::source_resolved::data::TypeParameterKind;
                assert!(matches!(policy.kind, TypeParameterKind::Const { .. }));
                assert!(matches!(element.kind, TypeParameterKind::Type));
                let [entry] = trees.machine_state_handles(machine.states) else {
                    panic!("the Run machine must retain its entry signature");
                };
                let entry = trees.machine_state(*entry);
                assert_eq!(trees.state_parameters(entry.parameters).len(), arity);
                assert!(entry.return_type.is_some());
            }
        }
    }

    #[test]
    fn sealed_integer_conversion_overloads_keep_distinct_machine_symbols() {
        const FLOAT: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../source/library/core/float_operations.omg"
        ));
        let trees = resolved(FLOAT, "float_operations.omg", SourceOrigin::Toolchain)
            .expect("the real floating-point declaration family must resolve");
        let family = trees
            .machines
            .iter()
            .filter(|machine| machine.name.as_str() == "FloatSemantics::from_integer")
            .collect::<Vec<_>>();
        assert_eq!(family.len(), 8, "retain every signed/unsigned integer row");
        let mut symbols = family
            .iter()
            .map(|machine| machine.symbol)
            .collect::<Vec<_>>();
        assert!(symbols.iter().all(|symbol| symbol.is_valid()));
        symbols.sort_unstable_by_key(|symbol| symbol.arena_index());
        symbols.dedup();
        assert_eq!(
            symbols.len(),
            8,
            "same path must not collapse declaration owners"
        );
        let carriers = family
            .iter()
            .map(|machine| {
                let [entry] = trees.machine_state_handles(machine.states) else {
                    panic!("each overload must retain its entry");
                };
                let entry = trees.machine_state(*entry);
                let [_, value] = trees.state_parameters(entry.parameters) else {
                    panic!("each integer conversion needs its format and value");
                };
                let crate::source_resolved::types::TypeReference::Named { name, .. } =
                    &value.type_reference
                else {
                    panic!("the integer catalog row must retain its named carrier");
                };
                name.as_str()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            carriers,
            ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"]
        );
    }
}
