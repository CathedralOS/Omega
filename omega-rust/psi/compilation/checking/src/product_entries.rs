//! Admit source entry declarations and executed root bindings.
//! Query issuance and final admission share exact source compatibility, while
//! the target supplies source-only slot requirements. Physical layout and ABI
//! realization consume these judgments outside Psi.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::product_operand::ProductEntryDeclaration;

mod root_bindings;
pub use root_bindings::collect_root_bindings;

/// An executed Build root choice with its exact lexical product declaration.
/// Spelling is diagnostic payload; the resolved symbol selects the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub slot: String,
    pub implementation: String,
    /// Exact product machine selected under the occurrence's lexical package scope.
    pub implementation_symbol: symbols::SymbolHandle,
}

/// Validate the static source compatibility of the selected `ProgramEntry`
/// schema. Hosted targets expose no arrival parameters; freestanding parameters
/// must match the exact canonical types of the semantic arrival requirement.
///
/// Both description issuance and final admission use this decision. It does
/// not require the provider or layout choices the build is still computing.
pub fn validate_source_compatibility(
    typed: &TypedTrees,
    declaration: ProductEntryDeclaration<'_>,
    slot: semantic_vocabulary::ProgramEntrySourceContract,
) -> Result<(), Vec<Diagnostic>> {
    // Rejoin the exact owner before classifying its complete input header.
    // A borrowed declaration view alone does not authenticate arena identity.
    let declaration =
        ProductEntryDeclaration::from_declared_owner(typed, declaration.machine_symbol())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "ProgramEntry source declaration has no exact complete product owner",
                )]
            })?;
    let machine_name = declaration.name;
    let Some(entry) = declaration.entry else {
        return Err(vec![Diagnostic::error(format!(
            "entry machine `{machine_name}` has no executable entry state"
        ))]);
    };

    let mut diagnostics = Vec::new();
    if declaration.erased_requires_inputs().next().is_some() {
        // ProgramEntry supplies its declared physical/semantic arrivals. It
        // has no source invocation or erased witness actual lane; an entry
        // cannot assume an unsupplied nominal proof input merely because its
        // body was checked under that declared premise.
        diagnostics.push(Diagnostic::error(format!(
            "ProgramEntry cannot supply erased requires evidence to entry machine `{machine_name}`"
        )));
    }
    if !declaration.type_parameters.is_empty() {
        diagnostics.push(Diagnostic::error(format!(
            "entry machine `{machine_name}` is generic; a root slot must bind one exact machine"
        )));
    }
    if entry.return_type.is_valid() {
        diagnostics.push(Diagnostic::error(format!(
            "entry machine `{machine_name}` returns a value, but `ProgramEntry` has no result"
        )));
    }

    let parameters = entry.parameters;
    // Entry selection grants no storage. Only schema-declared arrivals are
    // supplied; a mutable reference is an ordinary required argument.
    let visible = visible_entry_parameters(typed, parameters);
    match slot.visible_parameters {
        semantic_vocabulary::ProgramEntryVisibleParameters::None if !visible.is_empty() => {
            diagnostics.push(Diagnostic::error(format!(
                "hosted `ProgramEntry` exposes no arrival parameters, but `{machine_name}` declares `{}`",
                visible
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect::<Vec<_>>()
                    .join("`, `")
            )));
        }
        semantic_vocabulary::ProgramEntryVisibleParameters::ImageAndInitialStorage
            if visible.len() != 2 =>
        {
            diagnostics.push(Diagnostic::error(format!(
                "target schema `{:?}` exposes exactly image and initial-storage roots, but `{machine_name}` declares {} visible parameter{}",
                slot.schema,
                visible.len(),
                if visible.len() == 1 { "" } else { "s" },
            )));
        }
        _ => {}
    }

    if slot.visible_parameters
        == semantic_vocabulary::ProgramEntryVisibleParameters::ImageAndInitialStorage
        && visible.len() == 2
    {
        match core_arrival_requirement_contract(typed, slot.semantic_arrival_requirement) {
            Ok(required) if required.parameters.len() == visible.len() => {
                for (index, (actual, required)) in
                    visible.iter().zip(required.parameters.iter()).enumerate()
                {
                    if typed.normalized_type_identity(actual.type_reference) != required.identity
                        || actual.is_const != required.is_const
                        || actual.is_mutable != required.is_mutable
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "target root slot `{}::{}` requires visible parameter {index} ({}) to have exact type `{}`, but entry machine `{machine_name}` declares `{}`",
                            slot.owner,
                            slot.slot_name,
                            ["image", "initial storage"][index],
                            required.display,
                            typed.display_type_reference_with_constraints(actual.type_reference),
                        )));
                    }
                }
            }
            Ok(required) => diagnostics.push(Diagnostic::error(format!(
                "target root slot `{}::{}` selects arrival requirement `{}` with {} visible parameters, but its target schema declares {}",
                slot.owner,
                slot.slot_name,
                slot.semantic_arrival_requirement,
                required.parameters.len(),
                visible.len(),
            ))),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(())
}

pub struct ArrivalRequirementParameterType {
    pub identity: typed_trees::type_identity::NormalizedTypeIdentity,
    pub display: String,
    pub is_const: bool,
    pub is_mutable: bool,
}

pub struct ArrivalRequirementContract {
    pub signature: symbols::SymbolHandle,
    pub requirement_identity: String,
    pub parameters: Vec<ArrivalRequirementParameterType>,
    pub result_type_identity: Option<typed_trees::type_identity::NormalizedTypeIdentity>,
}

/// Source-shape queries run before package calling-plan selection. Their
/// visible storage parameters belong to core's semantic arrival, independently
/// of the eventual physical contract package and any local same-named trait.
pub fn core_arrival_requirement_contract(
    typed: &TypedTrees,
    requirement: &str,
) -> Result<ArrivalRequirementContract, Diagnostic> {
    let Some((owner, _)) = requirement.split_once("::") else {
        return Err(Diagnostic::error(
            "target semantic arrival has no requirement owner",
        ));
    };
    let mut declarations = typed
        .symbols
        .source_files()
        .filter(|file| {
            file.origin == source::SourceOrigin::Toolchain
                && file.dependency_scope == source::DependencyScope::Product
                && file.path.strip_prefix(&file.package_root).ok()
                    == Some(std::path::Path::new("extent.omg"))
        })
        .flat_map(|file| {
            typed.traits().iter().filter(move |definition| {
                typed
                    .symbols
                    .symbol_source_span(definition.symbol)
                    .is_some_and(|span| span.source_id == file.source_id)
                    && definition.name.as_str() == owner
            })
        });
    let Some(definition) = declarations.next() else {
        return Err(Diagnostic::error(format!(
            "target semantic arrival `{requirement}` has no exact core declaration"
        )));
    };
    if declarations.next().is_some() {
        return Err(Diagnostic::error(format!(
            "target semantic arrival `{requirement}` has multiple core declaration instances"
        )));
    }
    arrival_requirement_contract(typed, definition, requirement)
}

/// Resolve the target declaration through the selected schema's composition.
/// The result is deliberately taken from Psi's normalized identities rather
/// than reconstructed from display strings in the Omega orchestrator.
pub fn arrival_requirement_contract(
    typed: &TypedTrees,
    schema: &typed_trees::trait_definition::TraitDefinition,
    requirement: &str,
) -> Result<ArrivalRequirementContract, Diagnostic> {
    let Some((owner, method)) = requirement.split_once("::") else {
        return Err(Diagnostic::error(format!(
            "target entry arrival requirement `{requirement}` is not an exact `Trait::machine` identity"
        )));
    };
    let mut inherited = vec![schema];
    let mut position = 0;
    while let Some(definition) = inherited.get(position).copied() {
        for parent in typed.trait_requirements(definition) {
            if inherited
                .iter()
                .any(|candidate| candidate.symbol == parent.symbol)
            {
                continue;
            }
            if let Some(parent) = typed.trait_definition_by_symbol(parent.symbol) {
                inherited.push(parent);
            }
        }
        position += 1;
    }
    let definitions = inherited
        .into_iter()
        .filter(|definition| definition.is_boundary && definition.name.as_str() == owner)
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        return Err(Diagnostic::error(format!(
            "target entry arrival requirement `{requirement}` resolves to {} boundary trait declarations in the selected schema instead of exactly one",
            definitions.len()
        )));
    };
    let signatures = typed
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| signature.name.as_str() == method)
        .collect::<Vec<_>>();
    let [signature] = signatures.as_slice() else {
        return Err(Diagnostic::error(format!(
            "target entry arrival requirement `{requirement}` resolves to {} machine declarations instead of exactly one",
            signatures.len()
        )));
    };
    Ok(ArrivalRequirementContract {
        signature: signature.symbol,
        requirement_identity: typed.trait_requirement_callable_identity_text(definition, signature),
        parameters: typed
            .state_signature_parameters(signature)
            .iter()
            .map(|parameter| ArrivalRequirementParameterType {
                identity: typed.normalized_type_identity(parameter.type_reference),
                display: typed.display_type_reference_with_constraints(parameter.type_reference),
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
            })
            .collect(),
        result_type_identity: signature
            .return_type
            .is_valid()
            .then(|| typed.normalized_type_identity(signature.return_type)),
    })
}

/// Schema arrival parameters exclude separately installed placed-view loans.
/// An ordinary reference remains visible and must be supplied by the schema.
pub fn visible_entry_parameters<'program>(
    typed: &TypedTrees,
    parameters: &'program [typed_trees::signature::StateParameter],
) -> Vec<&'program typed_trees::signature::StateParameter> {
    parameters
        .iter()
        .filter(|parameter| {
            typed
                .placed_view_plan_for_input_type_reference(parameter.type_reference)
                .is_none()
        })
        .collect::<Vec<_>>()
}
