//! Prepare exact source calling applications once, before target selection.
//!
//! This owner follows checked inheritance, closes named policy captures and
//! retains their exact signature correspondence. Target choice and evaluation
//! remain Omega work. Every refusal stays on its original request/instance so
//! an earlier target-body or signature refusal still wins at consumption.

use super::BoundarySignatureDeclaration;
use crate::TypedTrees;
use crate::boundary_values::{BoundaryTypeBinding, substitute_boundary_type_reference};
use crate::provider_declarations::ProviderPlanningContext;
use crate::types::TypeReferenceHandle;
use arena::{Arena, Handle, HandleSpan};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub struct PreparedCallingRequest {
    pub boundary: SymbolHandle,
    pub name: String,
    pub span: source::SourceSpan,
    instances: Result<HandleSpan<PreparedCallingInstance>, Vec<Diagnostic>>,
}

impl Default for PreparedCallingRequest {
    fn default() -> Self {
        Self {
            boundary: SymbolHandle::invalid(),
            name: String::new(),
            span: source::SourceSpan::default(),
            instances: Ok(HandleSpan::default()),
        }
    }
}

pub struct PreparedCallingInstance {
    pub argument_owner: SymbolHandle,
    pub policy: Result<SymbolHandle, String>,
    arguments: HandleSpan<TypeReferenceHandle>,
    signatures: HandleSpan<PreparedAppliedBoundarySignature>,
}

impl Default for PreparedCallingInstance {
    fn default() -> Self {
        Self {
            argument_owner: SymbolHandle::invalid(),
            policy: Err("calling application is absent".into()),
            arguments: HandleSpan::default(),
            signatures: HandleSpan::default(),
        }
    }
}

#[derive(Default)]
pub struct PreparedAppliedBoundarySignature {
    declaration: Handle<BoundarySignatureDeclaration>,
    bindings: HandleSpan<BoundaryTypeBinding>,
}

#[derive(Default)]
pub struct PreparedCallingApplications {
    requests: Arena<PreparedCallingRequest>,
    instances: Arena<PreparedCallingInstance>,
    signatures: Arena<PreparedAppliedBoundarySignature>,
    arguments: Arena<TypeReferenceHandle>,
    bindings: Arena<BoundaryTypeBinding>,
}

impl PreparedCallingApplications {
    pub(super) fn prepare(
        context: ProviderPlanningContext<'_>,
        declarations: &Arena<BoundarySignatureDeclaration>,
    ) -> Self {
        let typed = context.typed;
        let vocabularies = crate::calling_vocabulary::standard_calling_vocabularies(typed);
        let mut prepared = Self::default();
        if vocabularies.is_empty() {
            return prepared;
        }
        // Explicit Calling evaluation has always covered the whole checked
        // input, including Build copies. Product default selection is separate.
        for boundary in typed
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
        {
            let relationships = typed
                .trait_requirements(boundary)
                .iter()
                .filter(|requirement| {
                    vocabularies
                        .iter()
                        .any(|(calling, _)| requirement.symbol == calling.symbol)
                })
                .collect::<Vec<_>>();
            let Some(relationship) = relationships.first() else {
                continue;
            };
            let span = relationship.source_span;
            let instances = (|| {
                if relationships.len() != 1 {
                    return Err(vec![Diagnostic::error(format!(
                        "boundary trait `{}` selects {} Calling<Policy> policies; exactly one concrete policy may define a boundary contract",
                        boundary.name, relationships.len(),
                    )).with_source_span(span)]);
                }
                let arguments = typed
                    .type_reference_table
                    .type_reference_handles(relationship.arguments);
                if arguments.len() != 1 {
                    return Err(vec![Diagnostic::error(format!(
                        "boundary trait `{}` has a Calling relationship with {} policy arguments; expected one",
                        boundary.name, arguments.len(),
                    )).with_source_span(span)]);
                }
                let Some((calling, policy)) = vocabularies
                    .iter()
                    .find(|(calling, _)| calling.symbol == relationship.symbol)
                    .copied()
                else {
                    return Err(vec![
                        Diagnostic::error("calling relationship lost its exact vocabulary")
                            .with_source_span(span),
                    ]);
                };
                let instances = boundary_policy_instances(typed, boundary).map_err(|reason| {
                    vec![
                        Diagnostic::error(format!(
                            "boundary trait `{}` cannot evaluate Calling<Policy>: {reason}",
                            boundary.name
                        ))
                        .with_source_span(span),
                    ]
                })?;
                let mut retained = Vec::with_capacity(instances.len());
                for instance in instances {
                    let policy = prepared_policy_machine(
                        typed,
                        boundary,
                        &instance.arguments,
                        instance.argument_owner,
                        calling,
                        policy,
                    )
                    .map(|machine| machine.symbol);
                    let mut signatures = Vec::new();
                    collect_boundary_signatures(
                        typed,
                        boundary,
                        &instance.arguments,
                        &mut Vec::new(),
                        &mut signatures,
                    );
                    let signatures = signatures
                        .into_iter()
                        .map(|signature| {
                            let declaration = declarations
                                .iter()
                                .find(|(_, declaration)| {
                                    declaration.symbol == signature.signature.symbol
                                        && declaration.requirement_identity
                                            == signature.requirement_identity
                                })
                                .map(|(handle, _)| handle)
                                .unwrap_or_default();
                            let bindings = prepared.bindings.insert_many(signature.bindings);
                            PreparedAppliedBoundarySignature {
                                declaration,
                                bindings,
                            }
                        })
                        .collect::<Vec<_>>();
                    let signatures = prepared.signatures.insert_many(signatures);
                    let arguments = prepared.arguments.insert_many(instance.arguments);
                    retained.push(PreparedCallingInstance {
                        argument_owner: instance.argument_owner,
                        policy,
                        arguments,
                        signatures,
                    });
                }
                Ok(prepared.instances.insert_many(retained))
            })();
            prepared.requests.insert(PreparedCallingRequest {
                boundary: boundary.symbol,
                name: boundary.name.as_str().to_owned(),
                span,
                instances,
            });
        }
        prepared
    }

    pub fn requests(&self) -> impl Iterator<Item = &PreparedCallingRequest> {
        self.requests.iter().map(|(_, request)| request)
    }

    pub fn instances(
        &self,
        request: &PreparedCallingRequest,
    ) -> Result<&[PreparedCallingInstance], Vec<Diagnostic>> {
        let instances = request.instances.as_ref().map_err(Clone::clone)?;
        Ok(self.instances.span_or_empty(*instances))
    }

    pub fn arguments(&self, instance: &PreparedCallingInstance) -> &[TypeReferenceHandle] {
        self.arguments.span_or_empty(instance.arguments)
    }

    pub fn signatures(
        &self,
        instance: &PreparedCallingInstance,
    ) -> &[PreparedAppliedBoundarySignature] {
        self.signatures.span_or_empty(instance.signatures)
    }

    pub fn bindings(&self, signature: &PreparedAppliedBoundarySignature) -> &[BoundaryTypeBinding] {
        self.bindings.span_or_empty(signature.bindings)
    }

    pub(super) fn declaration(
        &self,
        signature: &PreparedAppliedBoundarySignature,
    ) -> Handle<BoundarySignatureDeclaration> {
        signature.declaration
    }
}

struct BoundarySignatureInstance<'input> {
    signature: &'input crate::signature::StateSignature,
    requirement_identity: String,
    bindings: Vec<BoundaryTypeBinding>,
}

struct BoundaryPolicyInstance {
    pub arguments: Vec<TypeReferenceHandle>,
    pub argument_owner: symbols::SymbolHandle,
}

fn boundary_policy_instances(
    typed: &TypedTrees,
    boundary: &crate::trait_definition::TraitDefinition,
) -> Result<Vec<BoundaryPolicyInstance>, String> {
    let parameters = typed.trait_type_parameters(boundary);
    if parameters.is_empty() {
        return Ok(vec![BoundaryPolicyInstance {
            arguments: Vec::new(),
            argument_owner: boundary.symbol,
        }]);
    }

    let mut instances = Vec::new();
    for conformance in typed
        .conformances()
        .iter()
        .filter(|conformance| conformance.trait_symbol == boundary.symbol)
    {
        // Every named implementation of the generic boundary trait is one
        // concrete application; its trait arguments carry every data type.
        let arguments = typed
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .to_vec();
        if arguments.len() != parameters.len() {
            return Err(format!(
                "conformance `{} satisfies {}` supplies {} argument(s), expected {}",
                typed.symbols.display_path(conformance.symbol, "::"),
                conformance.trait_name,
                arguments.len(),
                parameters.len()
            ));
        }
        instances.push(BoundaryPolicyInstance {
            arguments,
            argument_owner: conformance.symbol,
        });
    }

    // A generic declaration is not itself a callable ABI. With no concrete
    // conformance there is no requirement identity to publish yet.
    Ok(instances)
}

/// Close the selected implementation before selecting its exact requirement
/// row. A same-named attached machine is never an implementation selection.
fn prepared_policy_machine<'program>(
    typed: &'program TypedTrees,
    boundary: &crate::trait_definition::TraitDefinition,
    boundary_arguments: &[TypeReferenceHandle],
    argument_owner: symbols::SymbolHandle,
    calling_trait: &crate::trait_definition::TraitDefinition,
    calling_policy: &crate::trait_definition::TraitDefinition,
) -> Result<&'program crate::machine::Machine, String> {
    use crate::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
    let boundary_scope = AppliedTraitScope::new(
        typed,
        boundary,
        TraitApplicationArguments {
            lexical_owner: argument_owner,
            enclosing_data_applications: &[],
            arguments: boundary_arguments,
            lifetimes: &[],
        },
    )
    .ok_or("calling policy has no complete boundary application")?;
    let calling_scope = boundary_scope
        .declaring_scope(typed, calling_trait.symbol)
        .ok_or("calling policy has no unique exact Calling application")?;
    let selections = typed
        .trait_implementation_applications
        .iter()
        .filter(|(_, selected)| {
            selected.source_scope.scope(typed).is_some_and(|scope| {
                scope.argument_owner == argument_owner
                    && scope.same_application(typed, &calling_scope)
            })
        })
        .map(|(_, selected)| selected)
        .collect::<Vec<_>>();
    let [selected] = selections.as_slice() else {
        return Err("calling-policy implementation application has no uniquely prepared closed source selection; unresolved static binders cannot execute".to_owned());
    };
    // Settlement runs on the admitted program: the conformance application's
    // arity, kind and carrier obligations already passed checking, so the
    // realization join needs only the conformance's exact trait and rows.
    let Some(conformance) = typed.conformance_by_symbol(selected.application.declaration) else {
        return Err(format!(
            "static argument `{}` does not name a package-scoped conformance",
            typed
                .symbols
                .display_path(selected.application.declaration, "::")
        ));
    };
    let trait_definition = typed
        .trait_definition_by_symbol(conformance.trait_symbol)
        .ok_or_else(|| {
            format!(
                "conformance `{}` names unresolved trait `{}`",
                conformance
                    .alias
                    .as_ref()
                    .map_or("<unnamed-conformance>", |name| name.as_str()),
                conformance.trait_name
            )
        })?;
    if trait_definition.symbol != calling_policy.symbol || !conformance.arguments.is_empty() {
        return Err(
            "calling policy must explicitly select a complete CallingPolicy implementation"
                .to_owned(),
        );
    }
    let requirements = typed.trait_machine_signatures(calling_policy);
    let [requirement] = requirements else {
        return Err("CallingPolicy must declare exactly one canonical plan requirement".to_owned());
    };
    if requirement.name.as_str() != "plan" {
        return Err("CallingPolicy has no canonical plan requirement".to_owned());
    }
    let rows = conformance
        .rows
        .iter()
        .filter(|row| {
            row.declaring_trait == calling_policy.symbol && row.requirement == requirement.symbol
        })
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        return Err(
            "calling policy implementation must retain exactly one CallingPolicy::plan row"
                .to_owned(),
        );
    };
    let machine = typed
        .machine(row.realization_machine)
        .ok_or_else(|| "calling policy row has no exact realization machine".to_owned())?;
    if typed
        .machine_states(machine)
        .first()
        .is_none_or(|state| state.symbol != row.realization_state)
    {
        return Err(
            "calling policy row must select its realization machine's entry state".to_owned(),
        );
    }
    let machine = if typed.declaration_scope_parent(machine.symbol) == conformance.symbol
        && (!typed.conformance_type_parameters(conformance).is_empty()
            || !conformance.lifetime_parameters.is_empty())
    {
        let captures = typed
            .captured_machine_applications
            .iter()
            .filter(|(_, capture)| {
                capture.template == machine.symbol
                    && capture.conformance_application == selected.application
                    && typed
                        .expression_table
                        .static_machine_application(capture.supplying_application)
                        .lexical_owner
                        == argument_owner
            })
            .map(|(_, capture)| capture)
            .collect::<Vec<_>>();
        let [capture] = captures.as_slice() else {
            return Err(
                "calling policy has no unique checked realization of its exact map capture"
                    .to_owned(),
            );
        };
        let entry = typed
            .captured_machine_states
            .span_or_empty(capture.states)
            .iter()
            .filter(|state| state.template == row.realization_state)
            .collect::<Vec<_>>();
        if entry.len() != 1 {
            return Err("calling policy capture lost its exact entry correspondence".to_owned());
        }
        let realized = typed
            .machine(capture.instance)
            .ok_or("calling policy capture has no executable body")?;
        if typed
            .machine_states(realized)
            .first()
            .is_none_or(|state| state.symbol != entry[0].instance)
        {
            return Err(
                "calling policy capture does not select its checked entry state".to_owned(),
            );
        }
        realized
    } else {
        machine
    };
    if !typed.machine_type_parameters(machine).is_empty() || !machine.conformance_bounds.is_empty()
    {
        return Err("generic calling-policy realization requires an explicitly specialized machine before policy evaluation".to_owned());
    }
    Ok(machine)
}

fn collect_boundary_signatures<'a>(
    typed: &'a TypedTrees,
    definition: &'a crate::trait_definition::TraitDefinition,
    arguments: &[TypeReferenceHandle],
    visited: &mut Vec<symbols::SymbolHandle>,
    signatures: &mut Vec<BoundarySignatureInstance<'a>>,
) {
    if visited.contains(&definition.symbol) {
        return;
    }
    visited.push(definition.symbol);
    let bindings = trait_type_bindings(typed, definition, arguments);
    for requirement in typed.trait_requirements(definition) {
        if let Some(parent) = typed
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == requirement.symbol && candidate.is_boundary)
        {
            let parent_arguments = typed
                .type_reference_table
                .type_reference_handles(requirement.arguments)
                .iter()
                .map(|argument| substitute_boundary_type_reference(typed, *argument, &bindings))
                .collect::<Vec<_>>();
            collect_boundary_signatures(typed, parent, &parent_arguments, visited, signatures);
        }
    }
    for signature in typed.trait_machine_signatures(definition) {
        let requirement_identity =
            typed.trait_requirement_callable_identity_text(definition, signature);
        if !signatures
            .iter()
            .any(|candidate| candidate.requirement_identity == requirement_identity)
        {
            signatures.push(BoundarySignatureInstance {
                signature,
                requirement_identity,
                bindings: bindings.clone(),
            });
        }
    }
    visited.pop();
}

fn trait_type_bindings(
    typed: &TypedTrees,
    definition: &crate::trait_definition::TraitDefinition,
    arguments: &[TypeReferenceHandle],
) -> Vec<BoundaryTypeBinding> {
    typed
        .trait_type_parameters(definition)
        .iter()
        .zip(arguments.iter().copied())
        .map(|(parameter, actual)| BoundaryTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.as_str().to_owned(),
            actual,
        })
        .collect()
}
