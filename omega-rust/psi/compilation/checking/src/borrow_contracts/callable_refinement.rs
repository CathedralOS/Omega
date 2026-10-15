//! Compare callable regions in original declaration and operand scopes.
//! Type substitution drops the callee's fresh telescope. Equal lifetime
//! spellings in a caller and a callback never imply equal region identities.
//! Opaque types remain under the preceding complete formation proof.

use super::carried_lifetimes::{
    CarriedLifetime, LifetimeFrontier, RegionIdentity, scoped_carried_lifetimes,
};
use super::{ViewReturnSource, structural_frontier_return_source};
use checked_trees::BorrowLoanOwnerSegment;
use symbols::SymbolHandle;
use typed_trees::name::Identifier;
use typed_trees::signature::StateParameter;
use typed_trees::static_applications::StaticMachineLifetimeBinding;
use typed_trees::type_identity::AppliedTypeIdentityScope;
use typed_trees::types::TypeReferenceHandle;

pub struct CallableBorrowContract<'contract> {
    pub lexical_owner: SymbolHandle,
    pub lifetime_parameters: &'contract [Identifier],
    pub parameters: &'contract [StateParameter],
    pub return_type: TypeReferenceHandle,
    pub substitutions: &'contract [(SymbolHandle, TypeReferenceHandle)],
    pub substitution_scope_starts: &'contract [usize],
    pub application_scopes: &'contract [AppliedTypeIdentityScope<'contract>],
    pub binders: &'contract [(SymbolHandle, String)],
    pub lifetime_bindings: &'contract [StaticMachineLifetimeBinding],
    pub argument_owner: SymbolHandle,
    pub supplied_telescope: bool,
}

struct ContractFrontier {
    regions: super::callable_regions::CallableRegions,
    inputs: Vec<Vec<CarriedLifetime>>,
    outputs: Vec<CarriedLifetime>,
    sources: ViewReturnSource,
}

/// A requirement's invocation binders are fresh with respect to its captured
/// Type operands. Their unknown leaves cannot supply a fresh returned region,
/// but they are not view-free: concrete captured leaves and every non-fresh
/// result still require the ordinary complete source relation. Requirement-local
/// Type parameters are deliberately absent from this captured roster.
pub(super) fn resolve_invocation_view_return_source(
    program: &dyn crate::CheckingInput,
    contract: CallableBorrowContract<'_>,
    scope_starts: &[usize],
) -> ViewReturnSource {
    let captured_types = captured_type_parameters(program, contract.lexical_owner);
    let incomplete = || {
        ViewReturnSource::Ambiguous(super::ViewReturnAmbiguity::IncompleteStructure {
            subject: "invocation contract".to_owned(),
        })
    };
    // Never publish a partial output frontier. Only the *inputs* captured
    // before this fresh invocation may remain abstract.
    if !super::substituted_result_frontier_is_complete(
        program,
        contract.return_type,
        contract.substitutions,
        scope_starts,
    ) {
        return incomplete();
    }
    let Ok(frontier) = contract_frontier(program, &contract, &captured_types, scope_starts) else {
        return incomplete();
    };
    if frontier
        .outputs
        .iter()
        .all(|leaf| matches!(leaf.lifetime, Some(RegionIdentity::Fresh(_))))
    {
        frontier.sources
    } else {
        super::resolve_substituted_view_return_source(
            program,
            contract.parameters,
            contract.return_type,
            contract.substitutions,
            scope_starts,
        )
    }
}

fn captured_type_parameters(
    program: &dyn crate::CheckingInput,
    signature: SymbolHandle,
) -> Vec<SymbolHandle> {
    let mut captured = Vec::new();
    // Start outside the queried telescope. A requirement-local Type is
    // selected together with this invocation, not a captured environment.
    let mut owner = program.declaration_scope_parent(signature);
    while owner.is_valid() {
        let parameters = if let Some(contract) = program.retained_static_machine_contract(owner) {
            program.state_signature_type_parameters(contract.signature())
        } else if let Some(machine) = program.machine_by_symbol(owner) {
            program.machine_type_parameters(machine)
        } else if let Some(definition) = program.trait_definition_by_symbol(owner) {
            program.trait_type_parameters(definition)
        } else if let Some(conformance) = program.conformance_by_symbol(owner) {
            program.conformance_type_parameters(conformance)
        } else if let Some(definition) = program.data_definition_by_symbol(owner) {
            program.data_type_parameters(definition)
        } else if let Some((_, signature)) = program.trait_machine_signature(owner) {
            program.state_signature_type_parameters(signature)
        } else {
            &[]
        };
        captured.extend(parameters.iter().filter_map(|parameter| {
            matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                .then_some(parameter.symbol)
        }));
        owner = program.declaration_scope_parent(owner);
    }
    captured
}

pub fn validate_callable_borrow_refinement(
    program: &dyn crate::CheckingInput,
    required: CallableBorrowContract<'_>,
    actual: CallableBorrowContract<'_>,
) -> Result<(), &'static str> {
    let symbolic_parameters = program
        .data_type_parameters
        .iter()
        .filter_map(|(_, parameter)| {
            matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                .then_some(parameter.symbol)
        })
        .collect::<Vec<_>>();
    let required_frontier = contract_frontier(
        program,
        &required,
        &symbolic_parameters,
        required.substitution_scope_starts,
    )?;
    let mut actual_frontier = contract_frontier(
        program,
        &actual,
        &symbolic_parameters,
        actual.substitution_scope_starts,
    )?;
    let application = super::region_correspondence::infer_inputs(
        program,
        &required,
        &actual,
        &required_frontier.regions,
        &actual_frontier.regions,
        &actual_frontier.inputs,
    )?;
    for (_, region) in &mut actual_frontier.regions.names {
        application.apply(region);
    }
    for (ordinal, leaves) in actual_frontier.inputs.iter_mut().enumerate() {
        for leaf in leaves {
            if let Some(region) = &mut leaf.lifetime {
                application.apply(region);
            } else if let Some((_, _, region)) =
                application.elided.iter().find(|(parameter, reference, _)| {
                    *parameter == ordinal && *reference == leaf.reference
                })
            {
                leaf.lifetime = Some(region.clone());
            }
        }
    }
    for leaf in &mut actual_frontier.outputs {
        if let Some(region) = &mut leaf.lifetime {
            application.apply(region);
        }
    }
    // Input instantiation can merge previously distinct regions. Recompute
    // the result relation from the complete mapped frontier rather than retain
    // a pre-instantiation source that could conceal newly ambiguous storage.
    actual_frontier.sources = frontier_sources(
        &actual_frontier.inputs,
        &actual_frontier.outputs,
        actual.parameters,
    );
    for (required_leaves, actual_leaves) in
        required_frontier.inputs.iter().zip(&actual_frontier.inputs)
    {
        for required_leaf in required_leaves {
            let Some(actual_leaf) = actual_leaves.iter().find(|leaf| {
                required_leaf.source == leaf.source
                    && contract_paths_correspond(
                        program,
                        &required_leaf.owner_path,
                        &leaf.owner_path,
                    )
            }) else {
                return Err("callable input has a different carried region frontier");
            };
            if let Some(required_region) = &required_leaf.lifetime
                && actual_leaf.lifetime.as_ref() != Some(required_region)
            {
                return Err(match required_region {
                    RegionIdentity::Fresh(_) => {
                        "fresh invocation region is replaced by a captured or different region"
                    }
                    _ => "callable input does not preserve its captured region identity",
                });
            }
        }
    }
    // Elision is meaningful only after the independent returned-source
    // relation agrees. One input leaf can supply its region; a nominal tag
    // or a matching spelling cannot manufacture that authority. Retain the
    // enumerated output Type handles: a reference's referee and phantom Type
    // operands do not acquire the physical leaf's elision.
    let refined = sources_refine(
        program,
        &required_frontier.sources,
        &actual_frontier.sources,
    );
    let mut input_leaves = actual_frontier.inputs.iter().flatten();
    let input_region = input_leaves.next().and_then(|input| {
        input_leaves
            .next()
            .is_none()
            .then_some(input.lifetime.as_ref())
            .flatten()
    });
    let output_elisions = actual_frontier
        .outputs
        .iter()
        .filter(|output| output.lifetime.is_none())
        .filter_map(|output| input_region.map(|region| (output.reference, region.clone())))
        .collect::<Vec<_>>();
    if let Err(error) = refined {
        // An actual whose elided result could come from several input
        // regions is unresolved, not refined. When the required callable
        // names one relation, a definite region-identity mismatch is the
        // refusal the program can act on; report it before the gap.
        if matches!(actual_frontier.sources, ViewReturnSource::Ambiguous(_))
            && !matches!(required_frontier.sources, ViewReturnSource::Ambiguous(_))
        {
            super::region_correspondence::validate(
                program,
                &required,
                &actual,
                &required_frontier.regions,
                &actual_frontier.regions,
                &application.elided,
                &output_elisions,
            )?;
        }
        return Err(error);
    }
    super::region_correspondence::validate(
        program,
        &required,
        &actual,
        &required_frontier.regions,
        &actual_frontier.regions,
        &application.elided,
        &output_elisions,
    )
}

fn contract_frontier(
    program: &dyn crate::CheckingInput,
    contract: &CallableBorrowContract<'_>,
    symbolic_parameters: &[SymbolHandle],
    scope_starts: &[usize],
) -> Result<ContractFrontier, &'static str> {
    let regions = super::callable_regions::resolve(program, contract)?;
    let collect = |reference, frontier| {
        let leaves = scoped_carried_lifetimes(
            program,
            reference,
            contract.substitutions,
            scope_starts,
            symbolic_parameters,
            &regions.names,
            &regions.arguments,
            &regions.applications,
            contract.lexical_owner,
            true,
            frontier,
        )
        .ok_or("unimplemented: callable input region frontier is incomplete")?;
        if leaves
            .iter()
            .any(|leaf| matches!(&leaf.lifetime, Some(RegionIdentity::Declared(_))))
        {
            return Err("callable input region has no lexical declaration custody");
        }
        Ok(leaves)
    };
    let inputs = contract
        .parameters
        .iter()
        .map(|parameter| collect(parameter.type_reference, LifetimeFrontier::Input))
        .collect::<Result<Vec<_>, &'static str>>()?;
    let outputs = collect(contract.return_type, LifetimeFrontier::Value)?;
    let sources = frontier_sources(&inputs, &outputs, contract.parameters);
    Ok(ContractFrontier {
        regions,
        inputs,
        sources,
        outputs,
    })
}

fn frontier_sources(
    inputs: &[Vec<CarriedLifetime>],
    outputs: &[CarriedLifetime],
    parameters: &[StateParameter],
) -> ViewReturnSource {
    let source_inputs = inputs
        .iter()
        .zip(parameters)
        .enumerate()
        .flat_map(|(ordinal, (leaves, parameter))| {
            leaves.iter().map(move |leaf| {
                (
                    ordinal,
                    parameter,
                    CarriedLifetime {
                        reference: leaf.reference,
                        owner_path: leaf.owner_path.clone(),
                        lifetime: leaf.lifetime.clone(),
                        access: leaf.access,
                        source: leaf.source,
                    },
                )
            })
        })
        .collect::<Vec<_>>();
    structural_frontier_return_source(outputs.to_vec(), &source_inputs)
}

pub(crate) fn lexical_region_owner(
    program: &dyn crate::CheckingInput,
    mut symbol: SymbolHandle,
) -> SymbolHandle {
    while symbol.is_valid() {
        if program.machine_by_symbol(symbol).is_some()
            || program.trait_definition_by_symbol(symbol).is_some()
            || program.conformance_by_symbol(symbol).is_some()
        {
            return symbol;
        }
        symbol = program.symbols.get(symbol).parent;
    }
    symbol
}

pub(super) fn captured_lifetimes(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
) -> Vec<(String, RegionIdentity)> {
    let mut owner = program.lifetime_declaration_owner(owner);
    // A private executable keeps the authored binder identity, not a new
    // region declaration merely because its body received fresh symbols.
    let mut scopes = Vec::new();
    while owner.is_valid() {
        let parameters = program
            .machine_by_symbol(owner)
            .map(|declaration| &declaration.lifetime_parameters)
            .or_else(|| {
                program
                    .trait_definition_by_symbol(owner)
                    .map(|declaration| &declaration.lifetime_parameters)
            })
            .or_else(|| {
                program
                    .conformance_by_symbol(owner)
                    .map(|declaration| &declaration.lifetime_parameters)
            })
            .or_else(|| {
                program
                    .data_definition_by_symbol(owner)
                    .map(|declaration| &declaration.lifetime_parameters)
            })
            .or_else(|| {
                program
                    .trait_machine_signature(owner)
                    .map(|(_, signature)| &signature.lifetime_parameters)
            })
            .or_else(|| {
                program
                    .retained_static_machine_contract(owner)
                    .map(|contract| &contract.signature().lifetime_parameters)
            });
        scopes.push((owner, parameters));
        owner = program.lifetime_declaration_owner(program.declaration_scope_parent(owner));
    }
    scopes
        .into_iter()
        .rev()
        .flat_map(|(owner, parameters)| {
            parameters
                .into_iter()
                .flatten()
                .enumerate()
                .map(move |(ordinal, parameter)| {
                    (
                        parameter.as_str().to_owned(),
                        RegionIdentity::Captured(owner, ordinal),
                    )
                })
        })
        .collect()
}

/// Callable refinement compares positions in corresponding parameter/result
/// types, not storage in one executable. A prepared nominal carrier has fresh
/// field/case symbols while the scoped requirement still traverses its template.
/// The caller has established type refinement; this operation retains nominal
/// declaration identity and member positions while comparing their loan relation.
/// Region correspondence is still checked independently below. Erased origins
/// supply only member identity here, never field types or lifetime authority.
/// Keep the original symbols on the frontiers: flow and invocation results need
/// exact storage paths, not these relative contract coordinates.
fn contract_paths_correspond(
    program: &dyn crate::CheckingInput,
    required: &[BorrowLoanOwnerSegment],
    actual: &[BorrowLoanOwnerSegment],
) -> bool {
    let declaration_owner = |data_position: u32| {
        let mut declaration = program.data_definitions().get(data_position as usize)?;
        let mut visited = Vec::new();
        while let Some(origin) = declaration.generic_instance {
            if visited.contains(&declaration.symbol) {
                return None;
            }
            visited.push(declaration.symbol);
            let typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } =
                program.type_reference_table.type_reference(origin)
            else {
                return None;
            };
            declaration = program.unique_data_definition_by_symbol(*base_symbol)?;
        }
        Some(declaration.symbol)
    };
    let field_position = |symbol| {
        let positions = program.data_field_positions_for_symbol(symbol);
        let (owner, member, payload) = *positions.first()?;
        let owner = declaration_owner(owner)?;
        positions
            .iter()
            .all(|(other_owner, other_member, other_payload)| {
                declaration_owner(*other_owner) == Some(owner)
                    && (*other_member, *other_payload) == (member, payload)
            })
            .then_some((owner, member, payload))
    };
    let case_position = |symbol| {
        let (owner, member) = program.data_variant_position_for_symbol(symbol)?;
        Some((declaration_owner(owner)?, member))
    };
    required.len() == actual.len()
        && required.iter().zip(actual).all(|(required, actual)| {
            if required == actual {
                return true;
            }
            match (required, actual) {
                (
                    BorrowLoanOwnerSegment::Field(required),
                    BorrowLoanOwnerSegment::Field(actual),
                ) => field_position(*required)
                    .zip(field_position(*actual))
                    .is_some_and(|(required, actual)| required == actual),
                (BorrowLoanOwnerSegment::Case(required), BorrowLoanOwnerSegment::Case(actual)) => {
                    case_position(*required)
                        .zip(case_position(*actual))
                        .is_some_and(|(required, actual)| required == actual)
                }
                _ => false,
            }
        })
}

fn sources_refine(
    program: &dyn crate::CheckingInput,
    required: &ViewReturnSource,
    actual: &ViewReturnSource,
) -> Result<(), &'static str> {
    match (required, actual) {
        (ViewReturnSource::NotApplicable, ViewReturnSource::NotApplicable) => Ok(()),
        (
            ViewReturnSource::Parameter {
                parameter_index: required,
            },
            ViewReturnSource::Parameter {
                parameter_index: actual,
            },
        ) if required == actual => Ok(()),
        (
            ViewReturnSource::Fields { fields: required },
            ViewReturnSource::Fields { fields: actual },
        ) if actual.iter().all(|actual| {
            required.iter().any(|required| {
                contract_paths_correspond(program, &required.owner_path, &actual.owner_path)
                    && contract_paths_correspond(
                        program,
                        &required.source_path,
                        &actual.source_path,
                    )
                    && actual.parameter_index == required.parameter_index
                    && actual.source == required.source
                    && actual.kind == required.kind
            })
        }) =>
        {
            Ok(())
        }
        (ViewReturnSource::Ambiguous(_), _) | (_, ViewReturnSource::Ambiguous(_)) => {
            Err("unimplemented: callable returned-source relation is ambiguous or incomplete")
        }
        _ => Err("callable returned view has a different input-source relation"),
    }
}
