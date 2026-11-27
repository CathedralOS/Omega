//! Retain the restrictions the Build actually selected. The evaluated case
//! number names a case of the toolchain's enum at the original Build call.
use crate::declaration_selection::{
    BuildOperation, ExclusionVocabulary, ExecutedEnumExclusion, ExecutedProductSelection,
    ProductDeclarationRequest,
};
use build_model::behavior_exclusions::{AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind};
use diagnostics::Diagnostic;
use semantic_vocabulary::{CrashCause, TerminalAuthorityClass};
use source_provenance::DependencyScope;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::BuildSelectionRequest;
use typed_trees::product_operand::{ProductRequestAuthors, visit_build_selection_occurrences};

pub(super) fn admit(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
    enums: &[ExecutedEnumExclusion],
) -> Result<Vec<AuthoredBehaviorExclusion>, Vec<Diagnostic>> {
    if let Some(request) = declared
        .iter()
        .map(Arc::as_ref)
        .chain(executed.iter().map(|event| event.request.as_ref()))
        .find(|request| request.operation == BuildOperation::ServiceExclusion)
    {
        return Err(vec![
            Diagnostic::error(
                "Build service exclusion requires its authenticated source declaration",
            )
            .with_source_span(request.source_span),
        ]);
    }
    let Some(first) = enums.first() else {
        return Ok(Vec::new());
    };
    if activation.is_none_or(|machine| {
        typed.machine_by_symbol(machine).is_none()
            || !typed
                .symbols
                .symbol_is_in_dependency_scope(machine, DependencyScope::Build)
    }) {
        return Err(vec![
            Diagnostic::error("behavior exclusion has no authoritative Build activation")
                .with_source_span(first.request.source_span),
        ]);
    }
    let authors = ProductRequestAuthors::new(typed);
    let mut occurrences = Vec::new();
    let machines = typed.machines().into_iter().filter(|machine| {
        typed.symbols.symbol_is_in_dependency_scope(
            typed.symbols.authored_origin(machine.symbol),
            DependencyScope::Build,
        )
    });
    visit_build_selection_occurrences(typed, machines, |machine, _, _, request| {
        if let BuildSelectionRequest::Exclusion(request) = request {
            occurrences.push((
                typed.symbols.authored_origin(machine),
                typed.symbols.rejoin_product_occurrence(request.source_span),
                request.vocabulary,
            ));
        }
    });
    let mut exclusions = Vec::with_capacity(enums.len());
    for event in enums {
        let span = typed
            .symbols
            .rejoin_product_occurrence(event.request.source_span);
        let refuse = |reason: String| vec![Diagnostic::error(reason).with_source_span(span)];
        let author = authors.resolve(event.selecting_machine).map_err(refuse)?;
        if !typed
            .symbols
            .symbol_is_in_dependency_scope(author, DependencyScope::Build)
            || !occurrences.contains(&(author, span, event.request.vocabulary))
        {
            return Err(refuse(
                "executed behavior exclusion has no matching call in its Build author".into(),
            ));
        }
        let kind = evaluated_case(typed, event.request.vocabulary, event.code)
            .map_err(|reason| refuse(reason.to_owned()))?;
        let selection = AuthoredBehaviorExclusion {
            kind,
            selecting_machine: author,
            source_span: span,
        };
        if !exclusions.contains(&selection) {
            exclusions.push(selection);
        }
    }
    Ok(exclusions)
}

fn evaluated_case(
    typed: &TypedTrees,
    vocabulary: ExclusionVocabulary,
    code: u64,
) -> Result<AuthoredBehaviorExclusionKind, &'static str> {
    let index = usize::try_from(code).map_err(|_| "behavior exclusion case is out of range")?;
    let name = vocabulary
        .cases()
        .get(index)
        .ok_or("behavior exclusion case is out of range")?;
    let type_name = vocabulary.type_name();
    let definitions = typed.data_definitions();
    let mut enums = typed
        .data_definition_positions_for_name(type_name)
        .into_iter()
        .filter_map(|position| definitions.get(position as usize))
        .filter(|declaration| {
            typed.is_exact_toolchain_build_prelude_data(declaration.symbol, type_name)
        });
    let declaration = enums
        .next()
        .filter(|_| enums.next().is_none())
        .ok_or("behavior exclusion requires one exact toolchain enum declaration")?;
    let mut variants = typed
        .data_members(declaration)
        .iter()
        .filter_map(|member| member.as_variant())
        .filter(|variant| variant.name.as_str() == *name);
    let variant = variants
        .next()
        .filter(|_| variants.next().is_none())
        .ok_or("behavior exclusion requires one exact toolchain case")?;
    if typed.symbols.get(variant.symbol).parent != declaration.symbol
        || !typed.data_payload_fields(variant).is_empty()
    {
        return Err("behavior exclusion requires a payload-free case of the toolchain enum");
    }
    let case_symbol = variant.symbol;
    Ok(match vocabulary {
        ExclusionVocabulary::CrashCause => {
            let cause = match *name {
                "Trap" => CrashCause::Trap,
                "Abort" => CrashCause::Abort,
                _ => return Err("unsupported Build crash cause"),
            };
            AuthoredBehaviorExclusionKind::CrashCause { cause, case_symbol }
        }
        ExclusionVocabulary::PhysicalAuthorityClass => {
            let class = TerminalAuthorityClass::ALL
                .get(index)
                .copied()
                .ok_or("unsupported physical authority class")?;
            AuthoredBehaviorExclusionKind::PhysicalAuthorityClass { class, case_symbol }
        }
    })
}
