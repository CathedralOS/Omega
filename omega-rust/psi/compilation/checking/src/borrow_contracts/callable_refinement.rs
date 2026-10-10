//! Compare callable regions in original declaration and operand scopes.
//! Type substitution drops the callee's fresh telescope. Equal lifetime
//! spellings in a caller and a callback never imply equal region identities.
//! Opaque types remain under the preceding complete formation proof.

use super::carried_lifetimes::{CarriedLifetime, RegionIdentity, scoped_carried_lifetimes};
use super::{ViewReturnSource, structural_frontier_return_source};
use symbols::SymbolHandle;
use typed_trees::name::Identifier;
use typed_trees::signature::StateParameter;
use typed_trees::static_applications::StaticMachineLifetimeBinding;
use typed_trees::types::TypeReferenceHandle;

pub struct CallableBorrowContract<'contract> {
    pub lexical_owner: SymbolHandle,
    pub lifetime_parameters: &'contract [Identifier],
    pub parameters: &'contract [StateParameter],
    pub return_type: TypeReferenceHandle,
    pub substitutions: &'contract [(SymbolHandle, TypeReferenceHandle)],
    pub binders: &'contract [(SymbolHandle, String)],
    pub lifetime_bindings: &'contract [StaticMachineLifetimeBinding],
    pub argument_owner: SymbolHandle,
    pub supplied_telescope: bool,
}

struct ContractFrontier {
    fresh_count: usize,
    regions: Vec<(String, RegionIdentity)>,
    inputs: Vec<Vec<CarriedLifetime>>,
    outputs: Vec<CarriedLifetime>,
    sources: ViewReturnSource,
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
    let required_frontier = contract_frontier(program, &required, &symbolic_parameters)?;
    let actual_frontier = contract_frontier(program, &actual, &symbolic_parameters)?;
    if required_frontier.fresh_count != 0
        && required_frontier.fresh_count != actual_frontier.fresh_count
    {
        return Err("fresh invocation lifetime telescope is not universally refined");
    }
    for (required_leaves, actual_leaves) in
        required_frontier.inputs.iter().zip(&actual_frontier.inputs)
    {
        for required_leaf in required_leaves {
            let Some(actual_leaf) = actual_leaves
                .iter()
                .find(|leaf| leaf.owner_path == required_leaf.owner_path)
            else {
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
    sources_refine(&required_frontier.sources, &actual_frontier.sources)?;
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
    super::region_correspondence::validate(
        program,
        &required,
        &actual,
        &required_frontier.regions,
        &actual_frontier.regions,
        &output_elisions,
    )
}

fn contract_frontier(
    program: &dyn crate::CheckingInput,
    contract: &CallableBorrowContract<'_>,
    symbolic_parameters: &[SymbolHandle],
) -> Result<ContractFrontier, &'static str> {
    let arguments = captured_lifetimes(program, contract.argument_owner);
    let mut lifetimes = captured_lifetimes(program, contract.lexical_owner);
    for binding in contract.lifetime_bindings {
        let identity = arguments
            .iter()
            .find(|(name, _)| name == binding.argument.as_str())
            .map(|(_, identity)| identity.clone())
            .ok_or("callable input region has no lexical declaration custody")?;
        lifetimes.push((binding.parameter.as_str().to_owned(), identity));
    }
    let mut fresh_count = 0;
    for parameter in contract.lifetime_parameters {
        if contract.supplied_telescope
            && contract
                .lifetime_bindings
                .iter()
                .any(|binding| binding.parameter == *parameter)
        {
            continue;
        }
        // Requirement-local fresh binders shadow the enclosing mapping.
        lifetimes.push((
            parameter.as_str().to_owned(),
            RegionIdentity::Fresh(fresh_count),
        ));
        fresh_count += 1;
    }
    let collect = |reference| {
        let leaves = scoped_carried_lifetimes(
            program,
            reference,
            contract.substitutions,
            &[0],
            symbolic_parameters,
            &lifetimes,
            &arguments,
            contract.lexical_owner,
            true,
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
        .map(|parameter| collect(parameter.type_reference))
        .collect::<Result<Vec<_>, &'static str>>()?;
    let outputs = collect(contract.return_type)?;
    let source_inputs = inputs
        .iter()
        .zip(contract.parameters)
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
                    },
                )
            })
        })
        .collect::<Vec<_>>();
    Ok(ContractFrontier {
        fresh_count,
        regions: lifetimes,
        inputs,
        sources: structural_frontier_return_source(outputs.clone(), &source_inputs),
        outputs,
    })
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
    let owner = program.lifetime_declaration_owner(lexical_region_owner(program, owner));
    // A private executable keeps the authored binder identity, not a new
    // region declaration merely because its body received fresh symbols.
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
        });
    parameters
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(ordinal, parameter)| {
            (
                parameter.as_str().to_owned(),
                RegionIdentity::Captured(owner, ordinal),
            )
        })
        .collect()
}

fn sources_refine(
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
                actual.owner_path == required.owner_path
                    && actual.source_path == required.source_path
                    && actual.parameter_index == required.parameter_index
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
