//! Compare declared callable borrows without consulting body-derived checking facts.

use crate::TypedTrees;

use crate::data::DataDefinition;
use crate::signature::StateParameter;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

use self::BorrowLoanOwnerSegment as BorrowOwnerSegment;

#[derive(Debug, Clone)]
pub enum ViewReturnSource {
    NotApplicable,
    Parameter { parameter_index: usize },
    Fields { fields: Vec<ViewReturnFieldSource> },
    Ambiguous(ViewReturnAmbiguity),
}

#[derive(Debug, Clone)]
pub struct ViewReturnFieldSource {
    pub lifetime_only: bool,
    pub owner_path: Vec<BorrowOwnerSegment>,
    pub source_path: Vec<BorrowOwnerSegment>,
    pub source: ViewReturnInputSource,
    pub source_type: TypeReferenceHandle,
    pub source_reference_hops: usize,
    pub parameter_index: usize,
    pub kind: BorrowAccessKind,
}

#[derive(Debug, Clone)]
pub enum ViewReturnAmbiguity {
    IncompleteStructure { subject: String },
    IncompatibleSourceAccess { input: String },
    ElidedMultipleInputs { candidates: Vec<String> },
    LifetimeMatchesNoInput { lifetime: String },
}

pub fn resolve_signature_view_return_source(
    program: &TypedTrees,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
) -> ViewReturnSource {
    if !returns_borrow(program, return_type) {
        return ViewReturnSource::NotApplicable;
    }

    structural_view_return_source(program, parameters, return_type, &[], &[], &[])
}

pub fn resolve_substituted_view_return_source(
    program: &TypedTrees,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &[usize],
) -> ViewReturnSource {
    if !substituted_returns_borrow(program, return_type, substitutions, scope_starts) {
        return ViewReturnSource::NotApplicable;
    }
    structural_view_return_source(
        program,
        parameters,
        return_type,
        substitutions,
        scope_starts,
        &[],
    )
}

pub fn substituted_returns_borrow(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &[usize],
) -> bool {
    substituted_carried_lifetimes(program, type_reference, substitutions, scope_starts)
        .is_none_or(|leaves| !leaves.is_empty())
}

pub mod callable_refinement {
    use super::BorrowLoanOwnerSegment;
    use super::CallableLifetimeApplication;
    use super::carried_lifetimes::{
        CarriedLifetime, LifetimeFrontier, RegionIdentity, scoped_carried_lifetimes,
    };
    use super::{ViewReturnSource, structural_frontier_return_source};
    use crate::data::TypeParameterKind;
    use crate::name::Identifier;
    use crate::signature::StateParameter;
    use crate::type_identity::AppliedTypeIdentityScope;
    use crate::types::TypeReferenceHandle;
    use symbols::SymbolHandle;

    pub struct CallableBorrowContract<'contract> {
        pub lexical_owner: SymbolHandle,
        pub lifetime_parameters: &'contract [Identifier],
        pub parameters: &'contract [StateParameter],
        pub return_type: TypeReferenceHandle,
        pub substitutions: &'contract [(SymbolHandle, TypeReferenceHandle)],
        pub substitution_scope_starts: &'contract [usize],
        pub application_scopes: &'contract [AppliedTypeIdentityScope<'contract>],
        pub binders: &'contract [(SymbolHandle, String)],
        pub lifetime_applications: &'contract [CallableLifetimeApplication<'contract>],
        pub argument_owner: SymbolHandle,
    }

    pub struct ContractFrontier {
        regions: super::callable_regions::CallableRegions,
        inputs: Vec<Vec<CarriedLifetime>>,
        outputs: Vec<CarriedLifetime>,
        sources: ViewReturnSource,
    }

    pub fn resolve_invocation_view_return_source(
        program: &crate::TypedTrees,
        contract: CallableBorrowContract<'_>,
        scope_starts: &[usize],
    ) -> ViewReturnSource {
        let captured_types = captured_type_parameters(program, contract.lexical_owner);
        let incomplete = || {
            ViewReturnSource::Ambiguous(super::ViewReturnAmbiguity::IncompleteStructure {
                subject: "invocation contract".to_owned(),
            })
        };
        if !super::substituted_result_frontier_is_complete(
            program,
            contract.return_type,
            contract.substitutions,
            scope_starts,
        ) {
            return incomplete();
        }
        let Ok(frontier) = contract_frontier(program, &contract, &captured_types, scope_starts)
        else {
            return incomplete();
        };
        if frontier
            .outputs
            .iter()
            .all(|leaf| matches!(leaf.lifetime, Some(RegionIdentity::Fresh(_))))
        {
            frontier.sources
        } else {
            contract_frontier(program, &contract, &[], scope_starts)
                .map(|frontier| frontier.sources)
                .unwrap_or_else(|_| incomplete())
        }
    }

    pub fn captured_type_parameters(
        program: &crate::TypedTrees,
        signature: SymbolHandle,
    ) -> Vec<SymbolHandle> {
        let mut captured = Vec::new();
        let mut owner = program.declaration_scope_parent(signature);
        while owner.is_valid() {
            let parameters = if let Some(contract) = program.retained_static_machine_contract(owner)
            {
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
                matches!(parameter.kind, TypeParameterKind::Type).then_some(parameter.symbol)
            }));
            owner = program.declaration_scope_parent(owner);
        }
        captured
    }

    fn prepare_callable_inputs(
        program: &crate::TypedTrees,
        required: &CallableBorrowContract<'_>,
        actual: &CallableBorrowContract<'_>,
    ) -> Result<
        (
            ContractFrontier,
            ContractFrontier,
            super::region_inference::InputRegionApplication,
        ),
        String,
    > {
        let symbolic_parameters = program
            .data_type_parameters
            .iter()
            .filter_map(|(_, parameter)| {
                matches!(parameter.kind, TypeParameterKind::Type).then_some(parameter.symbol)
            })
            .collect::<Vec<_>>();
        let required_frontier = contract_frontier(
            program,
            required,
            &symbolic_parameters,
            required.substitution_scope_starts,
        )?;
        let actual_frontier = contract_frontier(
            program,
            actual,
            &symbolic_parameters,
            actual.substitution_scope_starts,
        )?;
        let application = super::region_correspondence::infer_inputs(
            program,
            required,
            actual,
            &required_frontier.regions,
            &actual_frontier.regions,
            &actual_frontier.inputs,
        )?;
        Ok((required_frontier, actual_frontier, application))
    }

    pub struct PreparedCallableBorrowRefinement {
        required_frontier: ContractFrontier,
        actual_frontier: ContractFrontier,
        application: super::region_inference::InputRegionApplication,
        required_substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
        actual_substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    }

    impl PreparedCallableBorrowRefinement {
        pub fn prepare(
            program: &crate::TypedTrees,
            required: &CallableBorrowContract<'_>,
            actual: &CallableBorrowContract<'_>,
        ) -> Result<Self, String> {
            let (required_frontier, actual_frontier, application) =
                prepare_callable_inputs(program, required, actual)?;
            Ok(Self {
                required_frontier,
                actual_frontier,
                application,
                required_substitutions: required.substitutions.to_vec(),
                actual_substitutions: actual.substitutions.to_vec(),
            })
        }

        pub fn lifetime_frame(
            &self,
            program: &crate::TypedTrees,
            required: &CallableBorrowContract<'_>,
            actual: &CallableBorrowContract<'_>,
        ) -> Result<super::CallableLifetimeFrame, String> {
            let own_parameters = |contract: &CallableBorrowContract<'_>| {
                crate::type_identity::declaration_lifetime_parameters(
                    program,
                    program.lifetime_declaration_owner(contract.lexical_owner),
                )
                .ok_or("callable lifetime partition has no retained declaration telescope")
            };
            if own_parameters(required)?.is_empty() && own_parameters(actual)?.is_empty() {
                return Ok(super::CallableLifetimeFrame::default());
            }
            let required_frontier = &self.required_frontier;
            let actual_frontier = &self.actual_frontier;
            let application = &self.application;
            let rows = |contract: &CallableBorrowContract<'_>,
                        regions: &super::callable_regions::CallableRegions,
                        apply: bool|
             -> Result<Vec<super::CallableLifetimeBinding>, String> {
                let declaration = program.lifetime_declaration_owner(contract.lexical_owner);
                let parameters =
                    crate::type_identity::declaration_lifetime_parameters(program, declaration)
                        .ok_or(
                            "callable lifetime partition has no retained declaration telescope",
                        )?;
                if regions.own_lifetimes.len() != parameters.len()
                    || parameters.iter().enumerate().any(|(ordinal, _)| {
                        regions
                            .own_lifetimes
                            .iter()
                            .filter(|(slot, _)| {
                                slot.declaration == declaration && slot.ordinal == ordinal
                            })
                            .count()
                            != 1
                    })
                {
                    return Err(
            "callable lifetime application does not partition its complete source telescope"
                .to_owned(),
        );
                }
                regions
                    .own_lifetimes
                    .iter()
                    .map(|(parameter, region)| {
                        let mut region = region.clone();
                        if apply {
                            application.apply(&mut region);
                        }
                        let argument = match region {
                            RegionIdentity::Declared(_) => {
                                return Err(
                            "callable lifetime partition retains an unresolved declared region"
                                .to_owned(),
                        );
                            }
                            RegionIdentity::Captured(declaration, ordinal) => {
                                crate::type_identity::LifetimeParameterIdentity {
                                    declaration,
                                    ordinal,
                                }
                            }
                            RegionIdentity::Fresh(ordinal) => required_frontier
                                .regions
                                .own_lifetimes
                                .iter()
                                .find_map(|(slot, region)| {
                                    (*region == RegionIdentity::Fresh(ordinal)).then_some(*slot)
                                })
                                .unwrap_or(*parameter),
                        };
                        Ok(super::CallableLifetimeBinding {
                            parameter: *parameter,
                            argument,
                        })
                    })
                    .collect()
            };
            Ok(super::CallableLifetimeFrame {
                required: rows(required, &required_frontier.regions, false)?,
                actual: rows(actual, &actual_frontier.regions, true)?,
            })
        }

        pub fn validate(
            self,
            program: &crate::TypedTrees,
            required: CallableBorrowContract<'_>,
            actual: CallableBorrowContract<'_>,
        ) -> Result<(), String> {
            if self.required_substitutions != required.substitutions
                || self.actual_substitutions != actual.substitutions
            {
                return Err(
                    "nested callable formation changed its parent input substitution premises"
                        .to_owned(),
                );
            }
            let Self {
                required_frontier,
                mut actual_frontier,
                application,
                ..
            } = self;
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
            actual_frontier.sources = frontier_sources(
                &actual_frontier.inputs,
                &actual_frontier.outputs,
                actual.parameters,
            );
            for (ordinal, (required_leaves, actual_leaves)) in required_frontier
                .inputs
                .iter()
                .zip(&actual_frontier.inputs)
                .enumerate()
            {
                for required_leaf in required_leaves {
                    let Some(actual_leaf) = actual_leaves.iter().find(|leaf| {
                        required_leaf.source == leaf.source
                            && required_leaf.reference_hops == leaf.reference_hops
                            && contract_paths_correspond(
                                program,
                                &required_leaf.owner_path,
                                &leaf.owner_path,
                            )
                    }) else {
                        return Err(format!(
                            "callable input has a different carried region frontier [parameter {ordinal}; required path {:?}; required: {}; actual: {}]",
                            required_leaf.owner_path,
                            super::region_correspondence::region_context(
                                program,
                                &required,
                                &required_frontier.regions
                            ),
                            super::region_correspondence::region_context(
                                program,
                                &actual,
                                &actual_frontier.regions
                            ),
                        ));
                    };
                    if let Some(required_region) = &required_leaf.lifetime
                        && actual_leaf.lifetime.as_ref() != Some(required_region)
                    {
                        let reason = match required_region {
                            RegionIdentity::Fresh(_) => {
                                "fresh invocation region is replaced by a captured or different region"
                            }
                            _ => "callable input does not preserve its captured region identity",
                        };
                        return Err(format!(
                            "{reason} [parameter {ordinal}; path {:?}; required region {required_region:?} from {:?}; actual region {:?} from {:?}; required: {}; actual: {}]",
                            required_leaf.owner_path,
                            required_leaf.reference,
                            actual_leaf.lifetime,
                            actual_leaf.reference,
                            super::region_correspondence::region_context(
                                program,
                                &required,
                                &required_frontier.regions
                            ),
                            super::region_correspondence::region_context(
                                program,
                                &actual,
                                &actual_frontier.regions
                            ),
                        ));
                    }
                }
            }
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
                super::region_correspondence::validate(
                    program,
                    &required,
                    &actual,
                    &required_frontier.regions,
                    &actual_frontier.regions,
                    &application.elided,
                    &output_elisions,
                )
                .map_err(|region_error| format!("{region_error}; {error}"))?;
                return Err(format!(
                    "{error}; required sources: {:?}; actual sources: {:?}",
                    required_frontier.sources, actual_frontier.sources,
                ));
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
    }

    pub fn validate_callable_borrow_refinement(
        program: &crate::TypedTrees,
        required: CallableBorrowContract<'_>,
        actual: CallableBorrowContract<'_>,
    ) -> Result<(), String> {
        PreparedCallableBorrowRefinement::prepare(program, &required, &actual)?
            .validate(program, required, actual)
    }

    fn contract_frontier(
        program: &crate::TypedTrees,
        contract: &CallableBorrowContract<'_>,
        symbolic_parameters: &[SymbolHandle],
        scope_starts: &[usize],
    ) -> Result<ContractFrontier, String> {
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
                &regions.bindings,
                contract.application_scopes,
                contract.argument_owner,
                contract.lexical_owner,
                true,
                frontier,
            )
            .ok_or_else(|| {
                "unimplemented: callable input region frontier is incomplete".to_owned()
            })?;
            if let Some((leaf, name)) = leaves.iter().find_map(|leaf| {
                if let Some(RegionIdentity::Declared(name)) = &leaf.lifetime {
                    Some((leaf, name))
                } else {
                    None
                }
            }) {
                let supplier = program
                    .type_reference_table
                    .operand_lifetime_owner(leaf.reference);
                return Err(format!(
                    "callable input region has no lexical declaration custody: region '{name} in `{}` while checking `{}`, supplied by `{}`",
                    program.type_reference_table.display_name(reference),
                    program.symbols.display_path(contract.lexical_owner, "::"),
                    if supplier.is_valid() {
                        program.symbols.display_path(supplier, "::")
                    } else {
                        "<no retained supplier>".to_owned()
                    },
                ));
            }
            Ok(leaves)
        };
        let inputs = contract
            .parameters
            .iter()
            .map(|parameter| collect(parameter.type_reference, LifetimeFrontier::Input))
            .collect::<Result<Vec<_>, String>>()?;
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
                            lifetime_only: leaf.lifetime_only,
                            retained_contract: leaf.retained_contract,
                            reference: leaf.reference,
                            reference_hops: leaf.reference_hops,
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

    pub fn lexical_region_owner(
        program: &crate::TypedTrees,
        mut symbol: SymbolHandle,
    ) -> SymbolHandle {
        while symbol.is_valid() {
            symbol = program.lifetime_declaration_owner(symbol);
            if !symbol.is_valid() {
                return symbol;
            }
            if crate::type_identity::declaration_lifetime_parameters(program, symbol).is_some() {
                return symbol;
            }
            symbol = program.declaration_scope_parent(symbol);
        }
        symbol
    }

    pub fn captured_lifetimes(
        program: &crate::TypedTrees,
        owner: SymbolHandle,
    ) -> Vec<(String, RegionIdentity)> {
        let mut owner = program.lifetime_declaration_owner(owner);
        let mut scopes = Vec::new();
        while owner.is_valid() {
            let parameters = crate::type_identity::declaration_lifetime_parameters(program, owner);
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

    fn contract_paths_correspond(
        program: &crate::TypedTrees,
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
                let crate::types::TypeReferenceNode::Generic { base_symbol, .. } =
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
                    (
                        BorrowLoanOwnerSegment::Case(required),
                        BorrowLoanOwnerSegment::Case(actual),
                    ) => case_position(*required)
                        .zip(case_position(*actual))
                        .is_some_and(|(required, actual)| required == actual),
                    _ => false,
                }
            })
    }

    fn sources_refine(
        program: &crate::TypedTrees,
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
                        && actual.source_reference_hops == required.source_reference_hops
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
}
pub mod callable_regions {
    use super::CallableLifetimeApplication;
    use super::callable_refinement::{CallableBorrowContract, captured_lifetimes};
    use super::carried_lifetimes::RegionIdentity;
    use crate::type_identity::{
        LifetimeParameterIdentity, applied_lifetime_parameter_identity, lifetime_parameter_identity,
    };

    pub struct AppliedRegionBinding {
        pub parameter: LifetimeParameterIdentity,
        pub argument: RegionIdentity,
    }

    pub fn applied_region<'scope>(
        program: &crate::TypedTrees,
        bindings: &'scope [AppliedRegionBinding],
        parameter: LifetimeParameterIdentity,
    ) -> Option<&'scope RegionIdentity> {
        let owner = program.lifetime_declaration_owner(parameter.declaration);
        if !owner.is_valid() {
            return None;
        }
        bindings
            .iter()
            .rev()
            .find(|binding| {
                binding.parameter.ordinal == parameter.ordinal
                    && binding.parameter.declaration == owner
            })
            .map(|binding| &binding.argument)
    }

    pub struct CallableRegions {
        pub names: Vec<(String, RegionIdentity)>,
        pub arguments: Vec<(String, RegionIdentity)>,
        pub applications: Vec<Vec<(String, RegionIdentity)>>,
        pub fresh_count: usize,
        pub bindings: Vec<AppliedRegionBinding>,
        pub own_lifetimes: Vec<(LifetimeParameterIdentity, RegionIdentity)>,
    }

    pub fn resolve(
        program: &crate::TypedTrees,
        contract: &CallableBorrowContract<'_>,
    ) -> Result<CallableRegions, &'static str> {
        let arguments = captured_lifetimes(program, contract.argument_owner);
        let mut applications = Vec::new();
        for (ordinal, scope) in contract.application_scopes.iter().enumerate() {
            let names = captured_lifetimes(program, scope.declaration)
                .into_iter()
                .map(|(name, _)| {
                    let identity = applied_lifetime_parameter_identity(
                        program,
                        contract.argument_owner,
                        &name,
                        &contract.application_scopes[..=ordinal],
                    )
                    .ok_or("callable application region has no lexical declaration custody")?;
                    Ok((
                        name,
                        RegionIdentity::Captured(identity.declaration, identity.ordinal),
                    ))
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            applications.push(names);
        }
        let mut names = captured_lifetimes(program, contract.lexical_owner);
        if let Some(application) = applications.last() {
            names.extend_from_slice(application);
        }
        let owner = program.lifetime_declaration_owner(contract.lexical_owner);
        let fresh_parameters = contract
            .lifetime_parameters
            .iter()
            .map(|parameter| {
                lifetime_parameter_identity(program, contract.lexical_owner, parameter.as_str())
                    .filter(|parameter| parameter.declaration == owner && owner.is_valid())
                    .ok_or("callable invocation binder has no exact declaration custody")
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        let mut bindings = Vec::new();
        for application in contract.lifetime_applications {
            let supplied = match application {
                CallableLifetimeApplication::Enclosing { bindings }
                | CallableLifetimeApplication::Invocation { bindings } => *bindings,
            };
            for (position, binding) in supplied.iter().enumerate() {
                if program.lifetime_declaration_owner(binding.parameter.declaration)
                    != binding.parameter.declaration
                    || program.lifetime_declaration_owner(binding.argument.declaration)
                        != binding.argument.declaration
                {
                    return Err(
                        "callable lifetime frame lost its canonical declaration coordinates",
                    );
                }
                let parameter_name =
                    crate::type_identity::lifetime_parameter_name(program, binding.parameter)
                        .ok_or("callable capture lost its exact receiving lifetime ordinal")?;
                crate::type_identity::lifetime_parameter_name(program, binding.argument)
                    .ok_or("callable input region lost its exact supplying lifetime ordinal")?;
                if supplied[..position]
                    .iter()
                    .any(|prior| prior.parameter == binding.parameter)
                {
                    return Err("callable capture repeats a receiving lifetime parameter");
                }
                if matches!(application, CallableLifetimeApplication::Enclosing { .. })
                    && fresh_parameters.contains(&binding.parameter)
                {
                    continue;
                }
                let argument = RegionIdentity::Captured(
                    binding.argument.declaration,
                    binding.argument.ordinal,
                );
                names.push((parameter_name.as_str().to_owned(), argument.clone()));
                bindings.push(AppliedRegionBinding {
                    parameter: binding.parameter,
                    argument,
                });
            }
        }
        let mut fresh_count = 0;
        let mut own_lifetimes = Vec::with_capacity(fresh_parameters.len());
        for (parameter, declaration) in contract.lifetime_parameters.iter().zip(fresh_parameters) {
            if let Some(region) = applied_region(program, &bindings, declaration) {
                own_lifetimes.push((declaration, region.clone()));
                continue;
            }
            own_lifetimes.push((declaration, RegionIdentity::Fresh(fresh_count)));
            names.push((
                parameter.as_str().to_owned(),
                RegionIdentity::Fresh(fresh_count),
            ));
            fresh_count += 1;
        }
        Ok(CallableRegions {
            names,
            arguments,
            applications,
            fresh_count,
            bindings,
            own_lifetimes,
        })
    }
}
pub mod carried_lifetimes {
    use super::BorrowLoanOwnerSegment as BorrowOwnerSegment;
    use super::data_definition;
    use super::{SymbolHandle, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Clone)]
    pub struct CarriedLifetime {
        pub lifetime_only: bool,
        pub retained_contract: bool,
        pub reference: TypeReferenceHandle,
        pub reference_hops: usize,
        pub owner_path: Vec<BorrowOwnerSegment>,
        pub lifetime: Option<RegionIdentity>,
        pub access: language_semantics::ReferenceAccess,
        pub source: super::ViewReturnInputSource,
    }

    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    pub enum LifetimeFrontier {
        #[default]
        Value,
        Input,
        ReachableInput,
        InputStorage,
        BorrowedStorage,
        WriteOnlyAuthority,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum RegionIdentity {
        Declared(String),
        Fresh(usize),
        Captured(SymbolHandle, usize),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DeclarationLifetimeFrontier {
        Complete { has_borrows: bool },
        TemplateDependent { has_borrows: bool },
        Incomplete,
    }

    pub struct DeclarationBorrowFrontier {
        pub owner_paths: Vec<Vec<BorrowOwnerSegment>>,
        pub template_dependent: bool,
    }

    #[derive(Clone, Copy, Default)]
    pub struct TemplateFrontier<'a> {
        parameters: &'a [SymbolHandle],
        authority_parameters: &'a [crate::data::TypeParameter],
        dependent: bool,
        declaration_inventory: bool,
        scoped_regions: bool,
        frontier: LifetimeFrontier,
        input_reference_hops: usize,
    }

    #[derive(Clone, Copy, Default)]
    pub struct TypeScope<'scope> {
        declaration: SymbolHandle,
        lifetimes: &'scope [(String, RegionIdentity)],
        bindings: &'scope [(SymbolHandle, TypeReferenceHandle)],
        starts: &'scope [usize],
        outer: Option<&'scope TypeScope<'scope>>,
        argument_lifetimes: &'scope [(String, RegionIdentity)],
        application_lifetimes: &'scope [Vec<(String, RegionIdentity)>],
        captured: &'scope [super::callable_regions::AppliedRegionBinding],
        application_scopes: &'scope [crate::type_identity::AppliedTypeIdentityScope<'scope>],
        argument_owner: SymbolHandle,
        region_outer: Option<&'scope TypeScope<'scope>>,
    }

    impl TypeScope<'_> {
        fn applied_region(
            &self,
            program: &crate::TypedTrees,
            parameter: crate::type_identity::LifetimeParameterIdentity,
        ) -> Option<RegionIdentity> {
            let mut scope = Some(self);
            while let Some(frame) = scope {
                if let Some(argument) =
                    super::callable_regions::applied_region(program, frame.captured, parameter)
                {
                    return Some(argument.clone());
                }
                if let Some((argument, true)) =
                    crate::type_identity::apply_resolved_lifetime_parameter_identity(
                        program,
                        parameter,
                        frame.argument_owner,
                        frame.application_scopes,
                    )
                {
                    return Some(RegionIdentity::Captured(
                        argument.declaration,
                        argument.ordinal,
                    ));
                }
                scope = frame.region_outer.or(frame.outer);
            }
            None
        }
    }

    pub struct DataVisit {
        reference: TypeReferenceHandle,
        cyclic: bool,
    }

    pub fn declaration_lifetime_frontier(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        parameters: &[SymbolHandle],
    ) -> DeclarationLifetimeFrontier {
        match declaration_borrow_frontier(program, reference, parameters) {
            None => DeclarationLifetimeFrontier::Incomplete,
            Some(frontier) if frontier.template_dependent => {
                DeclarationLifetimeFrontier::TemplateDependent {
                    has_borrows: !frontier.owner_paths.is_empty(),
                }
            }
            Some(frontier) => DeclarationLifetimeFrontier::Complete {
                has_borrows: !frontier.owner_paths.is_empty(),
            },
        }
    }

    // Keep known leaves even when exact declaration binders supply the
    // remainder. An unknown declaration or malformed structure is not a
    // symbolic remainder and still returns None.
    pub fn declaration_borrow_frontier(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        parameters: &[SymbolHandle],
    ) -> Option<DeclarationBorrowFrontier> {
        let mut template = TemplateFrontier {
            parameters,
            dependent: false,
            scoped_regions: false,
            frontier: LifetimeFrontier::Value,
            authority_parameters: &[],
            declaration_inventory: false,
            input_reference_hops: 0,
        };
        let mut leaves = Vec::new();
        let complete = collect_type(
            program,
            reference,
            &[],
            TypeScope::default(),
            &[],
            &mut Vec::new(),
            &mut leaves,
            false,
            &mut template,
        );
        complete.then(|| DeclarationBorrowFrontier {
            owner_paths: leaves.into_iter().map(|leaf| leaf.owner_path).collect(),
            template_dependent: template.dependent,
        })
    }

    pub fn carried_lifetimes(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
    ) -> Option<Vec<CarriedLifetime>> {
        storage_lifetimes(program, reference, LifetimeFrontier::Value)
    }

    pub fn stored_lifetimes(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
    ) -> Option<Vec<CarriedLifetime>> {
        storage_lifetimes(program, reference, LifetimeFrontier::BorrowedStorage)
    }

    fn storage_lifetimes(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        frontier: LifetimeFrontier,
    ) -> Option<Vec<CarriedLifetime>> {
        let mut output = Vec::new();
        let mut template = TemplateFrontier {
            frontier,
            ..TemplateFrontier::default()
        };
        let complete = collect_type(
            program,
            reference,
            &[],
            TypeScope::default(),
            &[],
            &mut Vec::new(),
            &mut output,
            false,
            &mut template,
        );
        (complete && !template.dependent).then_some(output)
    }

    fn substitutions_are_registered(
        program: &crate::TypedTrees,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        !substitutions.iter().any(|(_, reference)| {
            reference.is_valid()
                && !program
                    .type_reference_table
                    .contains_type_reference(*reference)
        })
    }

    pub fn substituted_result_is_view_free(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        if !substitutions_are_registered(program, substitutions) {
            return false;
        }
        let mut output = Vec::new();
        collect_type(
            program,
            reference,
            &[],
            TypeScope {
                bindings: substitutions,
                starts: &[0],
                ..TypeScope::default()
            },
            &[],
            &mut Vec::new(),
            &mut output,
            false,
            &mut TemplateFrontier::default(),
        ) && output.is_empty()
    }

    pub fn substituted_result_frontier_is_complete(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        scope_starts: &[usize],
    ) -> bool {
        if !substitutions_are_registered(program, substitutions) {
            return false;
        }
        collect_type(
            program,
            reference,
            &[],
            TypeScope {
                bindings: substitutions,
                starts: scope_starts,
                ..TypeScope::default()
            },
            &[],
            &mut Vec::new(),
            &mut Vec::new(),
            false,
            &mut TemplateFrontier::default(),
        )
    }

    pub fn substituted_carried_lifetimes(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        scope_starts: &[usize],
    ) -> Option<Vec<CarriedLifetime>> {
        symbolic_carried_lifetimes(program, reference, substitutions, scope_starts, &[])
    }

    pub fn symbolic_carried_lifetimes(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        scope_starts: &[usize],
        symbolic_parameters: &[SymbolHandle],
    ) -> Option<Vec<CarriedLifetime>> {
        scoped_carried_lifetimes(
            program,
            reference,
            substitutions,
            scope_starts,
            symbolic_parameters,
            &[],
            &[],
            &[],
            &[],
            &[],
            SymbolHandle::invalid(),
            SymbolHandle::invalid(),
            false,
            LifetimeFrontier::Value,
        )
    }

    pub fn input_write_only_access(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        parameters: &[crate::data::TypeParameter],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> super::InputWriteOnlyAccess {
        let symbols = parameters
            .iter()
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        let mut output = Vec::new();
        let mut template = TemplateFrontier {
            parameters: &symbols,
            authority_parameters: parameters,
            frontier: LifetimeFrontier::WriteOnlyAuthority,
            ..TemplateFrontier::default()
        };
        let complete = collect_type(
            program,
            reference,
            &[],
            TypeScope {
                bindings: substitutions,
                starts: &[0],
                ..TypeScope::default()
            },
            &[],
            &mut Vec::new(),
            &mut output,
            false,
            &mut template,
        );
        if output
            .iter()
            .any(|leaf| leaf.access == language_semantics::ReferenceAccess::WriteOnly)
        {
            super::InputWriteOnlyAccess::Present
        } else if !complete {
            super::InputWriteOnlyAccess::Incomplete
        } else if template.dependent {
            super::InputWriteOnlyAccess::TemplateDependent
        } else {
            super::InputWriteOnlyAccess::Absent
        }
    }

    pub fn symbolic_input_lifetimes(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        scope_starts: &[usize],
        symbolic_parameters: &[SymbolHandle],
    ) -> Option<Vec<CarriedLifetime>> {
        scoped_carried_lifetimes(
            program,
            reference,
            substitutions,
            scope_starts,
            symbolic_parameters,
            &[],
            &[],
            &[],
            &[],
            &[],
            SymbolHandle::invalid(),
            SymbolHandle::invalid(),
            false,
            LifetimeFrontier::Input,
        )
    }

    pub struct CarriedLifetimeInventory {
        pub leaves: Vec<CarriedLifetime>,
        pub template_dependent: bool,
    }

    pub fn declaration_lifetime_inventory(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        frontier: LifetimeFrontier,
    ) -> Option<CarriedLifetimeInventory> {
        let mut template = TemplateFrontier {
            frontier,
            declaration_inventory: true,
            ..TemplateFrontier::default()
        };
        let leaves =
            collect_frontier(program, reference, &[], TypeScope::default(), &mut template)?;
        Some(CarriedLifetimeInventory {
            leaves,
            template_dependent: template.dependent,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn scoped_carried_lifetimes(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        scope_starts: &[usize],
        symbolic_parameters: &[SymbolHandle],
        lifetimes: &[(String, RegionIdentity)],
        argument_lifetimes: &[(String, RegionIdentity)],
        application_lifetimes: &[Vec<(String, RegionIdentity)>],
        captured: &[super::callable_regions::AppliedRegionBinding],
        application_scopes: &[crate::type_identity::AppliedTypeIdentityScope<'_>],
        argument_owner: SymbolHandle,
        declaration: SymbolHandle,
        scoped_regions: bool,
        frontier: LifetimeFrontier,
    ) -> Option<Vec<CarriedLifetime>> {
        collect_frontier(
            program,
            reference,
            lifetimes,
            TypeScope {
                declaration,
                lifetimes,
                bindings: substitutions,
                starts: scope_starts,
                argument_lifetimes,
                application_lifetimes,
                captured,
                application_scopes,
                argument_owner,
                ..TypeScope::default()
            },
            &mut TemplateFrontier {
                parameters: symbolic_parameters,
                dependent: false,
                scoped_regions,
                frontier,
                authority_parameters: &[],
                declaration_inventory: false,
                input_reference_hops: 0,
            },
        )
    }

    fn collect_frontier(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        lifetimes: &[(String, RegionIdentity)],
        types: TypeScope<'_>,
        template: &mut TemplateFrontier<'_>,
    ) -> Option<Vec<CarriedLifetime>> {
        let mut output = Vec::new();
        collect_type(
            program,
            reference,
            lifetimes,
            types,
            &[],
            &mut Vec::new(),
            &mut output,
            false,
            template,
        )
        .then_some(())?;
        if template.frontier == LifetimeFrontier::Input {
            let mut expanded = Vec::new();
            let mut expansion = TemplateFrontier {
                frontier: LifetimeFrontier::ReachableInput,
                ..*template
            };
            if collect_type(
                program,
                reference,
                lifetimes,
                types,
                &[],
                &mut Vec::new(),
                &mut expanded,
                false,
                &mut expansion,
            ) && !expansion.dependent
            {
                *template = expansion;
                return Some(expanded);
            }
        }
        Some(output)
    }

    pub fn whole_elided_result_accesses(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
    ) -> Option<Vec<language_semantics::ReferenceAccess>> {
        let mut output = Vec::new();
        collect_type(
            program,
            reference,
            &[],
            TypeScope::default(),
            &[],
            &mut Vec::new(),
            &mut output,
            true,
            &mut TemplateFrontier::default(),
        )
        .then(|| output.into_iter().map(|leaf| leaf.access).collect())
    }

    fn carried_region(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        slot: usize,
        name: &crate::name::Identifier,
        lifetimes: &[(String, RegionIdentity)],
        types: TypeScope<'_>,
        scoped: bool,
    ) -> Option<RegionIdentity> {
        let lexical = || {
            lifetimes
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == name.as_str())
                .map_or_else(
                    || RegionIdentity::Declared(name.as_str().to_owned()),
                    |(_, region)| region.clone(),
                )
        };
        if !scoped {
            return Some(lexical());
        }
        let Some(region) =
            super::type_regions::retained_lifetime_identity(program, reference, slot, Some(name))
                .ok()?
        else {
            return Some(lexical());
        };
        let owner = program.lifetime_declaration_owner(region.declaration);
        if !owner.is_valid() {
            return None;
        }
        if let Some(argument) = types.applied_region(program, region) {
            return Some(argument);
        }
        let mut scope = Some(types);
        while let Some(frame) = scope {
            if program.lifetime_declaration_owner(frame.declaration) == owner {
                return frame
                    .lifetimes
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == name.as_str())
                    .map(|(_, region)| region.clone());
            }
            scope = frame.region_outer.or(frame.outer).copied();
        }
        Some(RegionIdentity::Captured(owner, region.ordinal))
    }

    fn callable_signature_is_resolved(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        contract: crate::callable_values::RuntimeCallableContractHandle,
        types: TypeScope<'_>,
    ) -> bool {
        use crate::type_identity::identity_context::TypeIdentityContext;
        use crate::type_identity::{LifetimeParameterIdentity, lifetime_parameter_identity};
        if !program.runtime_callable_contracts.is_valid(contract) {
            return false;
        }
        let mut binders = Vec::new();
        let mut resolved = Vec::new();
        let mut frame = Some(types);
        while let Some(scope) = frame {
            for (name, region) in scope.lifetimes {
                let Some(formal) = lifetime_parameter_identity(program, scope.declaration, name)
                else {
                    return false;
                };
                match region {
                    RegionIdentity::Fresh(ordinal) => binders.push((formal, *ordinal)),
                    RegionIdentity::Captured(declaration, ordinal) => resolved.push((
                        formal,
                        LifetimeParameterIdentity {
                            declaration: *declaration,
                            ordinal: *ordinal,
                        },
                    )),
                    RegionIdentity::Declared(_) => return false,
                }
            }
            for binding in scope.captured {
                match &binding.argument {
                    RegionIdentity::Fresh(ordinal) => binders.push((binding.parameter, *ordinal)),
                    RegionIdentity::Captured(declaration, ordinal) => resolved.push((
                        binding.parameter,
                        LifetimeParameterIdentity {
                            declaration: *declaration,
                            ordinal: *ordinal,
                        },
                    )),
                    RegionIdentity::Declared(_) => return false,
                }
            }
            frame = scope.region_outer.or(scope.outer).copied();
        }
        crate::runtime_callable_identity::normalize_type_in_context(
            program,
            reference,
            &TypeIdentityContext {
                substitutions: types.bindings,
                substitution_scope_starts: types.starts,
                application_scopes: types.application_scopes,
                application_owner: types.declaration,
                application_argument_owner: types.argument_owner,
                lifetime_binders: &binders,
                resolved_lifetimes: &resolved,
                ..Default::default()
            },
            false,
        )
        .is_ok()
    }

    fn collect_type(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        lifetimes: &[(String, RegionIdentity)],
        types: TypeScope<'_>,
        owner_path: &[BorrowOwnerSegment],
        visiting: &mut Vec<DataVisit>,
        output: &mut Vec<CarriedLifetime>,
        whole_elided_contract: bool,
        template: &mut TemplateFrontier<'_>,
    ) -> bool {
        let receiver_types = types;
        let operand_owner = if template.scoped_regions {
            program
                .type_reference_table
                .operand_lifetime_owner(reference)
        } else {
            SymbolHandle::invalid()
        };
        let operand_lifetimes =
            super::callable_refinement::captured_lifetimes(program, operand_owner)
                .into_iter()
                .map(|(name, identity)| {
                    let identity = match identity {
                        RegionIdentity::Captured(declaration, ordinal) => receiver_types
                            .applied_region(
                                program,
                                crate::type_identity::LifetimeParameterIdentity {
                                    declaration,
                                    ordinal,
                                },
                            )
                            .unwrap_or(RegionIdentity::Captured(declaration, ordinal)),
                        identity => identity,
                    };
                    (name, identity)
                })
                .collect::<Vec<_>>();
        let (lifetimes, types) = if operand_owner.is_valid() {
            let mut scope = Some(types);
            let mut supplying_scope = None;
            while let Some(frame) = scope {
                if program.lifetime_declaration_owner(frame.declaration)
                    == program.lifetime_declaration_owner(operand_owner)
                {
                    supplying_scope = Some(frame);
                    break;
                }
                let application_owner = program.lifetime_declaration_owner(operand_owner);
                if application_owner.is_valid()
                    && let Some((ordinal, application)) =
                        frame.application_scopes.iter().enumerate().rev().find(
                            |(_, application)| {
                                program.lifetime_declaration_owner(application.declaration)
                                    == application_owner
                            },
                        )
                    && ordinal < frame.starts.len()
                    && ordinal < frame.application_lifetimes.len()
                {
                    let end = frame
                        .starts
                        .get(ordinal + 1)
                        .copied()
                        .unwrap_or(frame.bindings.len());
                    if end <= frame.bindings.len() && frame.starts[ordinal] <= end {
                        supplying_scope = Some(TypeScope {
                            declaration: application.declaration,
                            lifetimes: &frame.application_lifetimes[ordinal],
                            bindings: &frame.bindings[..end],
                            starts: &frame.starts[..=ordinal],
                            application_scopes: &frame.application_scopes[..=ordinal],
                            application_lifetimes: &frame.application_lifetimes[..=ordinal],
                            ..frame
                        });
                        break;
                    }
                }
                scope = frame.outer.copied();
            }
            supplying_scope.map_or(
                (
                    &operand_lifetimes[..],
                    TypeScope {
                        declaration: operand_owner,
                        lifetimes: &operand_lifetimes,
                        region_outer: Some(&receiver_types),
                        ..TypeScope::default()
                    },
                ),
                |frame| (frame.lifetimes, frame),
            )
        } else {
            (lifetimes, types)
        };
        macro_rules! carry_child {
            ($reference:expr $(,)?) => {
                collect_type(
                    program,
                    $reference,
                    lifetimes,
                    types,
                    owner_path,
                    visiting,
                    output,
                    whole_elided_contract,
                    template,
                )
            };
        }
        if !program.contains_type_reference(reference) {
            return false;
        }
        if program.primitive_type_reference(reference).is_some() {
            return true;
        }
        if let TypeReferenceNode::Callable { contract } =
            program.type_reference_table.type_reference(reference)
        {
            return callable_signature_is_resolved(program, reference, *contract, types);
        }
        if template.frontier == LifetimeFrontier::WriteOnlyAuthority
            && crate::source_properties::type_satisfies_declared_property(
                program,
                template.authority_parameters,
                reference,
                crate::source_properties::DeclaredPropertyRequirement::Copy,
            )
        {
            return true;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference {
                access,
                lifetime,
                referee,
            } => {
                if template.frontier == LifetimeFrontier::WriteOnlyAuthority {
                    match access {
                        language_semantics::ReferenceAccess::Shared => return true,
                        language_semantics::ReferenceAccess::Mutable => {
                            return carry_child!(*referee);
                        }
                        language_semantics::ReferenceAccess::WriteOnly => {}
                    }
                }
                if whole_elided_contract && lifetime.is_some() {
                    return false;
                }
                let lifetime = match lifetime {
                    Some(name) => {
                        let Some(region) = carried_region(
                            program,
                            reference,
                            0,
                            name,
                            lifetimes,
                            types,
                            template.scoped_regions,
                        ) else {
                            return false;
                        };
                        Some(region)
                    }
                    None => None,
                };
                output.push(CarriedLifetime {
                    lifetime_only: false,
                    retained_contract: false,
                    reference,
                    reference_hops: template.input_reference_hops,
                    owner_path: owner_path.to_vec(),
                    lifetime,
                    access: *access,
                    source: if matches!(
                        template.frontier,
                        LifetimeFrontier::Input | LifetimeFrontier::ReachableInput
                    ) {
                        super::ViewReturnInputSource::Referent
                    } else {
                        super::ViewReturnInputSource::Stored
                    },
                });
                if !matches!(
                    template.frontier,
                    LifetimeFrontier::Input
                        | LifetimeFrontier::ReachableInput
                        | LifetimeFrontier::InputStorage
                ) || !access.is_readable()
                {
                    return true;
                }
                let first_stored = output.len();
                let frontier = template.frontier;
                let reference_hops = template.input_reference_hops;
                if frontier == LifetimeFrontier::InputStorage
                    && crate::source_borrow_shapes::slice_view_shell(program, reference, &[])
                        .is_none()
                {
                    let Some(next) = reference_hops.checked_add(1) else {
                        return false;
                    };
                    template.input_reference_hops = next;
                }
                template.frontier = if frontier == LifetimeFrontier::Input {
                    LifetimeFrontier::BorrowedStorage
                } else {
                    LifetimeFrontier::InputStorage
                };
                let complete = carry_child!(*referee);
                template.frontier = frontier;
                template.input_reference_hops = reference_hops;
                if *access == language_semantics::ReferenceAccess::Shared {
                    let mut retained = first_stored;
                    for position in first_stored..output.len() {
                        if output[position].access == language_semantics::ReferenceAccess::WriteOnly
                        {
                            continue;
                        }
                        output.swap(retained, position);
                        retained += 1;
                    }
                    output.truncate(retained);
                    for leaf in &mut output[first_stored..] {
                        if leaf.access == language_semantics::ReferenceAccess::Mutable {
                            leaf.access = language_semantics::ReferenceAccess::Shared;
                        }
                    }
                }
                complete
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let _ = constraints;
                carry_child!(*base_type)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                if matches!(length, crate::types::FixedArrayLength::Literal(0)) {
                    return true;
                }
                let mut element_frontier = Vec::new();
                let frontier = template.frontier;
                if frontier == LifetimeFrontier::Input {
                    template.frontier = LifetimeFrontier::Value;
                } else if frontier == LifetimeFrontier::ReachableInput {
                    template.frontier = LifetimeFrontier::InputStorage;
                }
                let complete = collect_type(
                    program,
                    *element_type,
                    lifetimes,
                    types,
                    &[],
                    visiting,
                    &mut element_frontier,
                    whole_elided_contract,
                    template,
                );
                template.frontier = frontier;
                if !complete {
                    return false;
                }
                if element_frontier.is_empty() {
                    return true;
                }
                let crate::types::FixedArrayLength::Literal(length) = length else {
                    return false;
                };
                if frontier == LifetimeFrontier::WriteOnlyAuthority {
                    output.extend(element_frontier);
                    return true;
                }
                for element_index in 0..*length {
                    for leaf in &element_frontier {
                        let mut path = owner_path.to_vec();
                        path.push(BorrowOwnerSegment::FixedIndex(element_index));
                        path.extend_from_slice(&leaf.owner_path);
                        output.push(CarriedLifetime {
                            lifetime_only: leaf.lifetime_only,
                            retained_contract: leaf.retained_contract,
                            reference: leaf.reference,
                            reference_hops: leaf.reference_hops,
                            owner_path: path,
                            lifetime: leaf.lifetime.clone(),
                            access: leaf.access,
                            source: leaf.source,
                        });
                    }
                }
                true
            }
            TypeReferenceNode::Slice { element_type }
                if template.frontier == LifetimeFrontier::WriteOnlyAuthority =>
            {
                carry_child!(*element_type)
            }
            TypeReferenceNode::Slice { element_type }
                if matches!(
                    template.frontier,
                    LifetimeFrontier::BorrowedStorage | LifetimeFrontier::InputStorage
                ) =>
            {
                let mut path = owner_path.to_vec();
                path.push(BorrowOwnerSegment::DynamicIndex);
                collect_type(
                    program,
                    *element_type,
                    lifetimes,
                    types,
                    &path,
                    visiting,
                    output,
                    whole_elided_contract,
                    template,
                )
            }
            TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } => {
                if whole_elided_contract {
                    return false;
                }
                if !template.scoped_regions
                    && let Some(definition) = program.data_application_definition(reference)
                {
                    return collect_data(
                        program,
                        reference,
                        definition,
                        lifetimes,
                        types,
                        owner_path,
                        visiting,
                        output,
                        whole_elided_contract,
                        template,
                    );
                }
                let Some(definition) = data_definition(program, *base_symbol) else {
                    return false;
                };
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                let parameters = program.data_type_parameters(definition);
                if definition.lifetime_parameters.len() != lifetime_arguments.len()
                    || parameters.len() != arguments.len()
                {
                    return false;
                }
                let nested_lifetimes = definition
                    .lifetime_parameters
                    .iter()
                    .zip(lifetime_arguments)
                    .enumerate()
                    .map(|(slot, (parameter, argument))| {
                        let argument = carried_region(
                            program,
                            reference,
                            slot,
                            argument,
                            lifetimes,
                            types,
                            template.scoped_regions,
                        )?;
                        Some((parameter.as_str().to_owned(), argument))
                    })
                    .collect::<Option<Vec<_>>>();
                let Some(nested_lifetimes) = nested_lifetimes else {
                    return false;
                };
                let nested_types = parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| (parameter.symbol, *argument))
                    .collect::<Vec<_>>();
                collect_data(
                    program,
                    reference,
                    definition,
                    &nested_lifetimes,
                    TypeScope {
                        declaration: definition.symbol,
                        lifetimes: &nested_lifetimes,
                        bindings: &nested_types,
                        starts: &[0],
                        outer: Some(&types),
                        argument_lifetimes: lifetimes,
                        application_lifetimes: &[],
                        captured: &[],
                        application_scopes: &[],
                        argument_owner: types.argument_owner,
                        region_outer: None,
                    },
                    owner_path,
                    visiting,
                    output,
                    whole_elided_contract,
                    template,
                )
            }
            TypeReferenceNode::Named { symbol, .. } => {
                let mut scope = Some(&types);
                while let Some(frame) = scope {
                    if let Some((binding_ordinal, (_, concrete))) = frame
                        .bindings
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, (parameter, _))| parameter == symbol)
                    {
                        if *concrete == reference && template.parameters.contains(symbol) {
                            template.dependent = true;
                            return true;
                        }
                        if *concrete == reference {
                            return false;
                        }
                        let Some((scope_ordinal, start)) = frame
                            .starts
                            .iter()
                            .enumerate()
                            .rev()
                            .find(|(_, start)| **start <= binding_ordinal)
                        else {
                            return false;
                        };
                        let outer = if scope_ordinal == 0 {
                            frame.outer.copied().unwrap_or_default()
                        } else {
                            TypeScope {
                                bindings: &frame.bindings[..*start],
                                starts: &frame.starts[..scope_ordinal],
                                application_scopes: &frame.application_scopes
                                    [..frame.application_scopes.len().min(scope_ordinal)],
                                application_lifetimes: &frame.application_lifetimes
                                    [..frame.application_lifetimes.len().min(scope_ordinal)],
                                captured: &[],
                                ..*frame
                            }
                        };
                        return collect_type(
                            program,
                            *concrete,
                            scope_ordinal
                                .checked_sub(1)
                                .and_then(|ordinal| frame.application_lifetimes.get(ordinal))
                                .map_or(frame.argument_lifetimes, Vec::as_slice),
                            outer,
                            owner_path,
                            visiting,
                            output,
                            whole_elided_contract,
                            template,
                        );
                    }
                    scope = frame.outer;
                }
                let Some(definition) = data_definition(program, *symbol) else {
                    if template.parameters.contains(symbol)
                        || ((matches!(
                            template.frontier,
                            LifetimeFrontier::BorrowedStorage | LifetimeFrontier::InputStorage
                        ) || template.declaration_inventory)
                            && program.symbols.get(*symbol).kind
                                == symbols::SymbolKind::TypeParameter)
                    {
                        template.dependent = true;
                        return true;
                    }
                    return false;
                };
                if whole_elided_contract
                    && (!definition.type_parameters.is_empty()
                        || !definition.lifetime_parameters.is_empty())
                {
                    return false;
                }
                if definition.type_parameters.is_empty() {
                    return collect_data(
                        program,
                        reference,
                        definition,
                        lifetimes,
                        types,
                        owner_path,
                        visiting,
                        output,
                        whole_elided_contract,
                        template,
                    );
                }
                collect_family(
                    program, reference, definition, owner_path, visiting, output, template,
                )
            }
            TypeReferenceNode::Callable { contract } => {
                callable_signature_is_resolved(program, reference, *contract, types)
            }
            TypeReferenceNode::Unit | TypeReferenceNode::DynamicTrait { .. } => true,
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Slice { .. } => false,
        }
    }

    fn collect_family(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        definition: &crate::data::DataDefinition,
        owner_path: &[BorrowOwnerSegment],
        visiting: &mut Vec<DataVisit>,
        output: &mut Vec<CarriedLifetime>,
        template: &mut TemplateFrontier<'_>,
    ) -> bool {
        let pack = program.data_type_parameters(definition);
        let comparison_parameters = template
            .parameters
            .iter()
            .copied()
            .filter(|parameter| !pack.iter().any(|binder| binder.symbol == *parameter))
            .collect::<Vec<_>>();
        let mut family_template = TemplateFrontier {
            declaration_inventory: false,
            parameters: &comparison_parameters,
            dependent: false,
            scoped_regions: template.scoped_regions,
            frontier: template.frontier,
            authority_parameters: template.authority_parameters,
            input_reference_hops: template.input_reference_hops,
        };
        let complete = collect_data(
            program,
            reference,
            definition,
            &[],
            TypeScope {
                declaration: definition.symbol,
                ..TypeScope::default()
            },
            owner_path,
            visiting,
            output,
            false,
            &mut family_template,
        );
        template.dependent |= family_template.dependent;
        complete
    }

    fn collect_data(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        definition: &crate::data::DataDefinition,
        lifetimes: &[(String, RegionIdentity)],
        types: TypeScope<'_>,
        owner_path: &[BorrowOwnerSegment],
        visiting: &mut Vec<DataVisit>,
        output: &mut Vec<CarriedLifetime>,
        whole_elided_contract: bool,
        template: &mut TemplateFrontier<'_>,
    ) -> bool {
        // An opaque declaration does not expose the stored lifetime relationships.
        // A completed body used to supply them; source formation cannot assume them.
        if definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
            return false;
        }
        if let Some(ancestor) = visiting
            .iter()
            .position(|visit| visit.reference == reference)
        {
            for visit in &mut visiting[ancestor..] {
                visit.cyclic = true;
            }
            return true;
        }
        let first_leaf = output.len();
        let frontier = template.frontier;
        if frontier == LifetimeFrontier::Input {
            template.frontier = LifetimeFrontier::Value;
        } else if frontier == LifetimeFrontier::ReachableInput {
            template.frontier = LifetimeFrontier::InputStorage;
        }
        visiting.push(DataVisit {
            reference,
            cyclic: false,
        });
        let mut path = Vec::new();
        let complete = program
            .data_member_fields(program.data_members(definition))
            .all(|field| {
                path.clear();
                path.extend_from_slice(owner_path);
                if let Some(variant) = super::payload_variant_for_field(program, field.symbol) {
                    path.push(BorrowOwnerSegment::Case(variant));
                }
                path.push(BorrowOwnerSegment::Field(field.symbol));
                collect_type(
                    program,
                    field.type_reference,
                    lifetimes,
                    types,
                    &path,
                    visiting,
                    output,
                    whole_elided_contract,
                    template,
                )
            });
        let cyclic = visiting.pop().is_some_and(|visit| visit.cyclic);
        template.frontier = frontier;
        complete && (whole_elided_contract || !cyclic || output.len() == first_leaf)
    }

    #[cfg(test)]
    mod frontier_tests {
        use super::*;
        use crate::data::{DataDefinition, DataField, DataMember};
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

        #[test]
        fn symbolic_frontier_keeps_known_leaves_and_rejects_foreign_binders() {
            let mut symbols = SymbolTableBuilder::new();
            let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let declarations = symbols.insert_children(
                root,
                [
                    (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                    (SymbolKind::TypeParameter, SymbolNameRef::Static("Foreign")),
                    (SymbolKind::Data, SymbolNameRef::Static("Carrier")),
                ],
            );
            let parameter = declarations.handle_at(0);
            let foreign = declarations.handle_at(1);
            let carrier = declarations.handle_at(2);
            let fields = symbols.insert_children(
                carrier,
                [
                    (SymbolKind::Field, SymbolNameRef::Static("view")),
                    (SymbolKind::Field, SymbolNameRef::Static("opaque")),
                ],
            );
            let mut program = crate::TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let opaque = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: parameter,
                    name: "T".into(),
                });
            let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
            let view = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::Shared,
                    lifetime: None,
                    referee: unit,
                });
            let members = program.data_members.insert_many([
                DataMember::Field(DataField {
                    symbol: fields.handle_at(0),
                    name: "view".into(),
                    type_reference: view,
                    ..Default::default()
                }),
                DataMember::Field(DataField {
                    symbol: fields.handle_at(1),
                    name: "opaque".into(),
                    type_reference: opaque,
                    ..Default::default()
                }),
            ]);
            program.push_data_definition(DataDefinition {
                symbol: carrier,
                name: "Carrier".into(),
                members,
                ..Default::default()
            });
            let carrier = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: carrier,
                    name: "Carrier".into(),
                });
            let frontier = declaration_borrow_frontier(&program, carrier, &[parameter])
                .expect("known leaves beside an exact open binder");
            assert!(frontier.template_dependent);
            assert_eq!(
                frontier.owner_paths,
                vec![vec![BorrowOwnerSegment::Field(fields.handle_at(0))]]
            );
            assert!(declaration_borrow_frontier(&program, carrier, &[foreign]).is_none());
            assert!(declaration_borrow_frontier(&program, opaque, &[]).is_none());
            assert!(
                declaration_borrow_frontier(
                    &program,
                    TypeReferenceHandle::from_parts(1_000_000, 0),
                    &[parameter]
                )
                .is_none()
            );
            let scalar = declaration_borrow_frontier(&program, unit, &[]).unwrap();
            assert!(!scalar.template_dependent);
            assert!(scalar.owner_paths.is_empty());
        }
    }
}
pub use crate::callable_lifetimes as lifetime_applications;
pub mod owned_carriers {
    use super::CallableBorrowContract;
    use super::carried_lifetimes::{
        CarriedLifetime, LifetimeFrontier, RegionIdentity, scoped_carried_lifetimes,
    };
    use crate::type_identity::{IndexIdentityArgument, NormalizedTypeIdentity};
    use crate::types::{TypeReferenceHandle, TypeReferenceNode};
    use symbols::SymbolHandle;

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum OwnedCarrierLoanRelation {
        ViewFree,
        Preserved,
        Unproven,
    }

    pub fn owned_carrier_loan_relation(
        program: &crate::TypedTrees,
        callable: SymbolHandle,
        source: TypeReferenceHandle,
        source_applications: &[TypeReferenceHandle],
        result: TypeReferenceHandle,
        result_applications: &[TypeReferenceHandle],
    ) -> OwnedCarrierLoanRelation {
        let Some(source) = applied_frontier(program, callable, source, source_applications) else {
            return OwnedCarrierLoanRelation::Unproven;
        };
        if source.borrowed {
            return OwnedCarrierLoanRelation::Unproven;
        }
        if source.loans.is_empty() {
            return OwnedCarrierLoanRelation::ViewFree;
        }
        if !source.linear {
            return OwnedCarrierLoanRelation::Unproven;
        }
        let Some(result) = applied_frontier(program, callable, result, result_applications) else {
            return OwnedCarrierLoanRelation::Unproven;
        };
        if source.identity == result.identity
            && source.loans.len() == result.loans.len()
            && source
                .loans
                .iter()
                .zip(&result.loans)
                .all(|(source, result)| {
                    source.lifetime_only == result.lifetime_only
                        && source.retained_contract == result.retained_contract
                        && source.owner_path == result.owner_path
                        && source.lifetime == result.lifetime
                        && source.access == result.access
                        && source.source == result.source
                })
        {
            OwnedCarrierLoanRelation::Preserved
        } else {
            OwnedCarrierLoanRelation::Unproven
        }
    }

    pub struct AppliedCarrierFrontier {
        identity: NormalizedTypeIdentity,
        loans: Vec<CarriedLifetime>,
        borrowed: bool,
        linear: bool,
    }

    fn applied_frontier(
        program: &crate::TypedTrees,
        callable: SymbolHandle,
        reference: TypeReferenceHandle,
        applications: &[TypeReferenceHandle],
    ) -> Option<AppliedCarrierFrontier> {
        if !reference.is_valid() {
            return None;
        }
        let reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        let (reference, frames) =
            program.project_enclosing_data_type_reference(reference, applications)?;
        let applications = &applications[..frames];
        let linear =
            program.type_multiplicity(reference) == language_semantics::Multiplicity::Linear;
        let borrowed = matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Reference { .. }
        );
        program
            .with_enclosing_data_identity_scopes(applications, |scopes| {
                let identity = crate::type_identity::applied_type_identity_in_scope(
                    program, reference, callable, callable, scopes,
                )?;
                let mut substitutions = Vec::new();
                let mut starts = Vec::new();
                for scope in scopes {
                    starts.push(substitutions.len());
                    substitutions.extend(scope.arguments.iter().filter_map(
                        |(parameter, argument)| match argument {
                            IndexIdentityArgument::TypeArgument(reference) => {
                                Some((*parameter, *reference))
                            }
                            _ => None,
                        },
                    ));
                }
                let contract = CallableBorrowContract {
                    lexical_owner: callable,
                    lifetime_parameters: &[],
                    parameters: &[],
                    return_type: reference,
                    substitutions: &substitutions,
                    substitution_scope_starts: &starts,
                    application_scopes: scopes,
                    binders: &[],
                    lifetime_applications: &[],
                    argument_owner: callable,
                };
                let regions = super::callable_regions::resolve(program, &contract).ok()?;
                let operand_owner = program.lifetime_declaration_owner(
                    program
                        .type_reference_table
                        .operand_lifetime_owner(reference),
                );
                let receiving = scopes.iter().enumerate().rev().find(|(_, scope)| {
                    operand_owner.is_valid()
                        && program.lifetime_declaration_owner(scope.declaration) == operand_owner
                });
                let (declaration, lifetimes) =
                    receiving.map_or((callable, regions.names.as_slice()), |(ordinal, scope)| {
                        (scope.declaration, regions.applications[ordinal].as_slice())
                    });
                let frontier = scoped_carried_lifetimes(
                    program,
                    reference,
                    &substitutions,
                    &starts,
                    &[],
                    lifetimes,
                    &regions.arguments,
                    &regions.applications,
                    &regions.bindings,
                    scopes,
                    callable,
                    declaration,
                    true,
                    LifetimeFrontier::Value,
                )?;
                (!frontier
                    .iter()
                    .any(|leaf| matches!(leaf.lifetime, Some(RegionIdentity::Declared(_)))))
                .then_some(AppliedCarrierFrontier {
                    identity,
                    loans: frontier,
                    borrowed,
                    linear,
                })
            })
            .flatten()
    }
}
pub mod region_correspondence {
    use super::callable_refinement::{CallableBorrowContract, captured_lifetimes};
    use super::callable_regions::CallableRegions;
    use super::carried_lifetimes::{CarriedLifetime, RegionIdentity};
    use super::region_inference::InputRegionApplication;
    use crate::TypedTrees;
    use crate::name::Identifier;
    use crate::source_borrow_contracts::type_regions::{
        ReferenceStep, TypeRegionRelationship, TypeRegionScope, type_reference_region_applications,
        type_reference_regions_match,
    };
    use crate::types::TypeReferenceHandle;
    use symbols::SymbolHandle;

    #[derive(Clone, Copy)]
    pub enum RegionEnvironment<'scope> {
        Resolved(&'scope [(String, RegionIdentity)]),
        Captured {
            owner: SymbolHandle,
            bindings: &'scope [super::callable_regions::AppliedRegionBinding],
        },
        Bound(&'scope RegionIdentity),
        CapturedSlot(crate::type_identity::LifetimeParameterIdentity),
    }

    pub struct RejectedRegionComparison {
        reference: TypeReferenceHandle,
        actual_declaration: SymbolHandle,
        required_declaration: SymbolHandle,
        actual: Option<RegionIdentity>,
        required: Option<RegionIdentity>,
    }

    impl std::fmt::Debug for RejectedRegionComparison {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("RejectedRegionComparison")
                .field("reference", &self.reference)
                .field("actual_declaration", &self.actual_declaration)
                .field("required_declaration", &self.required_declaration)
                .field("actual", &self.actual)
                .field("required", &self.required)
                .finish()
        }
    }

    impl RegionEnvironment<'_> {
        fn identity(self, program: &TypedTrees, name: &Identifier) -> Option<RegionIdentity> {
            match self {
                Self::Resolved(regions) => regions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == name.as_str())
                    .map(|(_, identity)| identity.clone()),
                Self::Bound(region) => Some(region.clone()),
                Self::CapturedSlot(region) => {
                    Some(RegionIdentity::Captured(region.declaration, region.ordinal))
                }
                Self::Captured { owner, bindings } => captured_lifetimes(program, owner)
                    .into_iter()
                    .rev()
                    .find(|(parameter, _)| parameter == name.as_str())
                    .map(|(_, identity)| match identity {
                        RegionIdentity::Captured(declaration, ordinal) => {
                            super::callable_regions::applied_region(
                                program,
                                bindings,
                                crate::type_identity::LifetimeParameterIdentity {
                                    declaration,
                                    ordinal,
                                },
                            )
                            .cloned()
                            .unwrap_or(RegionIdentity::Captured(declaration, ordinal))
                        }
                        identity => identity,
                    }),
            }
        }
    }

    #[derive(Clone, Copy)]
    pub struct RegionScope<'scope> {
        declaration: SymbolHandle,
        substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
        starts: &'scope [usize],
        applications: &'scope [Vec<(String, RegionIdentity)>],
        binders: &'scope [(SymbolHandle, String)],
        regions: RegionEnvironment<'scope>,
        arguments: RegionEnvironment<'scope>,
        output_elisions: &'scope [(TypeReferenceHandle, RegionIdentity)],
        captured: &'scope [super::callable_regions::AppliedRegionBinding],
        application_scopes: &'scope [crate::type_identity::AppliedTypeIdentityScope<'scope>],
        argument_owner: SymbolHandle,
    }

    impl TypeRegionScope for RegionScope<'_> {
        fn erased_identity(
            self,
            program: &TypedTrees,
            reference: TypeReferenceHandle,
        ) -> crate::type_identity::NormalizedTypeIdentity {
            program.scoped_type_identity(
                crate::type_identity::TypeIdentityRequest {
                    binders: self.binders,
                    substitutions: self.substitutions,
                    ..crate::type_identity::TypeIdentityRequest::ordinary(reference)
                },
                self.starts,
            )
        }

        fn enter_operand(mut self, program: &TypedTrees, owner: symbols::SymbolHandle) -> Self {
            let declaration = program.lifetime_declaration_owner(self.declaration);
            if declaration.is_valid() && program.lifetime_declaration_owner(owner) == declaration {
                return self;
            }
            let application_owner = program.lifetime_declaration_owner(owner);
            if application_owner.is_valid()
                && let Some((ordinal, application)) = self
                    .application_scopes
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, application)| {
                        program.lifetime_declaration_owner(application.declaration)
                            == application_owner
                    })
                && ordinal < self.starts.len()
                && ordinal < self.applications.len()
            {
                let end = self
                    .starts
                    .get(ordinal + 1)
                    .copied()
                    .unwrap_or(self.substitutions.len());
                if end <= self.substitutions.len() && self.starts[ordinal] <= end {
                    self.declaration = application.declaration;
                    self.regions = RegionEnvironment::Resolved(&self.applications[ordinal]);
                    self.substitutions = &self.substitutions[..end];
                    self.starts = &self.starts[..=ordinal];
                    self.applications = &self.applications[..=ordinal];
                    self.application_scopes = &self.application_scopes[..=ordinal];
                    self.output_elisions = &[];
                    return self;
                }
            }
            self.regions = RegionEnvironment::Captured {
                owner,
                bindings: self.captured,
            };
            self.output_elisions = &[];
            if program.lifetime_declaration_owner(owner)
                != program.lifetime_declaration_owner(self.declaration)
            {
                self.substitutions = &[];
                self.starts = &[];
                self.applications = &[];
                self.application_scopes = &[];
            }
            self
        }

        fn enter_lifetime(
            mut self,
            program: &TypedTrees,
            region: crate::type_identity::LifetimeParameterIdentity,
        ) -> Self {
            if let Some(argument) =
                super::callable_regions::applied_region(program, self.captured, region)
            {
                self.regions = RegionEnvironment::Bound(argument);
                self
            } else if let Some((argument, true)) =
                crate::type_identity::apply_resolved_lifetime_parameter_identity(
                    program,
                    region,
                    self.argument_owner,
                    self.application_scopes,
                )
            {
                self.regions = RegionEnvironment::CapturedSlot(argument);
                self
            } else {
                self.enter_operand(program, region.declaration)
            }
        }

        fn named_alias(
            mut self,
            _program: &TypedTrees,
            symbol: symbols::SymbolHandle,
            reference: TypeReferenceHandle,
        ) -> Option<ReferenceStep<Self>> {
            let substitutions = self.substitutions;
            let starts = self.starts;
            let applications = self.applications;
            let (binding_ordinal, operand) = substitutions
                .iter()
                .enumerate()
                .rev()
                .find(|(_, (parameter, _))| *parameter == symbol)
                .map(|(ordinal, (_, operand))| (ordinal, *operand))?;
            if operand == reference {
                return Some(ReferenceStep::Stop);
            }
            let Some((scope_ordinal, start)) = starts
                .iter()
                .enumerate()
                .rev()
                .find(|(_, start)| **start <= binding_ordinal)
            else {
                return Some(ReferenceStep::Refuse);
            };
            self.regions = scope_ordinal
                .checked_sub(1)
                .and_then(|ordinal| applications.get(ordinal))
                .map_or(self.arguments, |regions| {
                    RegionEnvironment::Resolved(regions)
                });
            self.substitutions = &substitutions[..*start];
            self.starts = &starts[..scope_ordinal];
            self.applications = &applications[..applications.len().min(scope_ordinal)];
            self.application_scopes =
                &self.application_scopes[..self.application_scopes.len().min(scope_ordinal)];
            self.output_elisions = &[];
            self.captured = &[];
            Some(ReferenceStep::Chase(self, operand))
        }
    }

    fn contract_scope<'scope>(
        contract: &'scope CallableBorrowContract<'_>,
        regions: &'scope CallableRegions,
    ) -> RegionScope<'scope> {
        RegionScope {
            declaration: contract.lexical_owner,
            substitutions: contract.substitutions,
            starts: contract.substitution_scope_starts,
            applications: &regions.applications,
            binders: contract.binders,
            regions: RegionEnvironment::Resolved(&regions.names),
            arguments: RegionEnvironment::Resolved(&regions.arguments),
            output_elisions: &[],
            captured: &regions.bindings,
            application_scopes: contract.application_scopes,
            argument_owner: contract.argument_owner,
        }
    }

    pub fn region_context(
        program: &TypedTrees,
        contract: &CallableBorrowContract<'_>,
        regions: &CallableRegions,
    ) -> String {
        let bindings = regions
            .bindings
            .iter()
            .map(|binding| (binding.parameter, &binding.argument))
            .collect::<Vec<_>>();
        format!(
            "owner {} {:?}, supplier {} {:?}, fresh {}, names {:?}, own slots {:?}, bindings {:?}, applications {:?}, substitutions {:?}, scope starts {:?}",
            program.symbols.name(contract.lexical_owner),
            contract.lexical_owner,
            program.symbols.name(contract.argument_owner),
            contract.argument_owner,
            regions.fresh_count,
            regions.names,
            regions.own_lifetimes,
            bindings,
            regions.applications,
            contract.substitutions,
            contract.substitution_scope_starts,
        )
    }

    pub fn infer_inputs(
        program: &TypedTrees,
        required: &CallableBorrowContract<'_>,
        actual: &CallableBorrowContract<'_>,
        required_regions: &CallableRegions,
        actual_regions: &CallableRegions,
        actual_inputs: &[Vec<CarriedLifetime>],
    ) -> Result<InputRegionApplication, String> {
        if required.parameters.len() != actual.parameters.len() {
            return Err("callable input arity does not match its requirement".to_owned());
        }
        let mut applications = vec![InputRegionApplication::default()];
        for (ordinal, (required_parameter, actual_parameter)) in required
            .parameters
            .iter()
            .zip(actual.parameters)
            .enumerate()
        {
            let mut failure = "callable input does not preserve its region identity";
            let mut rejected = None;
            let mut constrain = |application: &mut InputRegionApplication,
                                 actual_scope: RegionScope<'_>,
                                 required_scope: RegionScope<'_>,
                                 reference,
                                 actual: Option<&Identifier>,
                                 required: Option<&Identifier>| {
                let Some(required) = required else {
                    return true;
                };
                let Some(required) = required_scope.regions.identity(program, required) else {
                    return false;
                };
                let actual = match actual {
                    Some(name) => {
                        let Some(region) = actual_scope.regions.identity(program, name) else {
                            return false;
                        };
                        Some(region)
                    }
                    None => {
                        if !actual_inputs.get(ordinal).is_some_and(|leaves| {
                            leaves
                                .iter()
                                .any(|leaf| leaf.reference == reference && leaf.lifetime.is_none())
                        }) {
                            return false;
                        }
                        None
                    }
                };
                let mut instantiated = actual.clone();
                if let Some(region) = &mut instantiated {
                    application.apply(region);
                }
                if application.constrain(ordinal, reference, actual.clone(), required.clone()) {
                    return true;
                }
                failure = if matches!(actual, Some(RegionIdentity::Captured(_, _)))
                    && matches!(required, RegionIdentity::Fresh(_))
                {
                    "fresh invocation lifetime telescope is not universally refined"
                } else if actual_inputs
                    .get(ordinal)
                    .is_some_and(|leaves| leaves.iter().any(|leaf| leaf.reference == reference))
                    && (matches!(instantiated, Some(RegionIdentity::Captured(_, _)))
                        || matches!(required, RegionIdentity::Captured(_, _)))
                {
                    "callable input does not preserve its captured region identity"
                } else {
                    "callable type does not preserve its region identity"
                };
                rejected = Some(RejectedRegionComparison {
                    reference,
                    actual_declaration: actual_scope.declaration,
                    required_declaration: required_scope.declaration,
                    actual,
                    required: Some(required),
                });
                false
            };
            if !type_reference_region_applications(
                program,
                actual_parameter.type_reference,
                required_parameter.type_reference,
                contract_scope(actual, actual_regions),
                contract_scope(required, required_regions),
                TypeRegionRelationship::CallableRefinement,
                &mut applications,
                &mut constrain,
            ) {
                return Err(format!(
                    "{failure} [input-inference parameter {ordinal}; actual reference {:?}; required reference {:?}; last rejected comparison {rejected:?}; required: {}; actual: {}]",
                    actual_parameter.type_reference,
                    required_parameter.type_reference,
                    region_context(program, required, required_regions),
                    region_context(program, actual, actual_regions),
                ));
            }
        }
        let mut completed = Vec::new();
        let mut failure = "callable input regions do not determine one application";
        let mut rejected = None;
        for mut application in applications {
            match application.finish(actual_regions.fresh_count, required_regions.fresh_count) {
                Ok(()) if !completed.contains(&application) => completed.push(application),
                Ok(()) => {}
                Err(reason) => {
                    failure = reason;
                    rejected = Some(application);
                }
            }
        }
        if completed.len() != 1 {
            let reason = if completed.is_empty() {
                failure
            } else {
                "callable input regions do not determine one application"
            };
            return Err(format!(
                "{reason} [input-telescope-finish; completed {}; last rejected application {rejected:?}; required: {}; actual: {}]",
                completed.len(),
                region_context(program, required, required_regions),
                region_context(program, actual, actual_regions),
            ));
        }
        completed.pop().ok_or_else(|| failure.to_owned())
    }

    pub fn validate(
        program: &TypedTrees,
        required: &CallableBorrowContract<'_>,
        actual: &CallableBorrowContract<'_>,
        required_regions: &CallableRegions,
        actual_regions: &CallableRegions,
        input_elisions: &[(usize, TypeReferenceHandle, RegionIdentity)],
        output_elisions: &[(TypeReferenceHandle, RegionIdentity)],
    ) -> Result<(), String> {
        let required_scope = contract_scope(required, required_regions);
        let actual_scope = contract_scope(actual, actual_regions);
        let matches = |rejected: &mut Option<RejectedRegionComparison>,
                       actual_scope: RegionScope<'_>,
                       required_scope: RegionScope<'_>,
                       reference: TypeReferenceHandle,
                       actual: Option<&Identifier>,
                       required: Option<&Identifier>| {
            let Some(required) = required else {
                return true;
            };
            let required = required_scope.regions.identity(program, required);
            let actual = match actual {
                Some(actual) => actual_scope.regions.identity(program, actual),
                None => actual_scope
                    .output_elisions
                    .iter()
                    .find(|(output, _)| *output == reference)
                    .map(|(_, region)| region.clone()),
            };
            let agrees = required
                .as_ref()
                .zip(actual.as_ref())
                .is_some_and(|(required, actual)| required == actual);
            if !agrees {
                *rejected = Some(RejectedRegionComparison {
                    reference,
                    actual_declaration: actual_scope.declaration,
                    required_declaration: required_scope.declaration,
                    actual,
                    required,
                });
            }
            agrees
        };
        for (ordinal, (required_reference, actual_reference)) in required
            .parameters
            .iter()
            .map(|parameter| parameter.type_reference)
            .zip(
                actual
                    .parameters
                    .iter()
                    .map(|parameter| parameter.type_reference),
            )
            .enumerate()
        {
            let elisions = input_elisions
                .iter()
                .filter(|(parameter, _, _)| *parameter == ordinal)
                .map(|(_, reference, region)| (*reference, region.clone()))
                .collect::<Vec<_>>();
            let mut rejected = None;
            if !type_reference_regions_match(
                program,
                actual_reference,
                required_reference,
                RegionScope {
                    output_elisions: &elisions,
                    ..actual_scope
                },
                required_scope,
                &mut (),
                &mut |_: &mut (), a, r, reference, actual, required| {
                    matches(&mut rejected, a, r, reference, actual, required)
                },
            ) {
                return Err(format!(
                    "callable type does not preserve its region identity [input-validation parameter {ordinal}; actual reference {actual_reference:?}; required reference {required_reference:?}; last rejected comparison {rejected:?}; required: {}; actual: {}]",
                    region_context(program, required, required_regions),
                    region_context(program, actual, actual_regions),
                ));
            }
        }
        let mut rejected = None;
        if !type_reference_regions_match(
            program,
            actual.return_type,
            required.return_type,
            RegionScope {
                output_elisions,
                ..actual_scope
            },
            required_scope,
            &mut (),
            &mut |_: &mut (), a, r, reference, actual, required| {
                matches(&mut rejected, a, r, reference, actual, required)
            },
        ) {
            return Err(format!(
                "callable type does not preserve its region identity [result-validation; actual reference {:?}; required reference {:?}; last rejected comparison {rejected:?}; required: {}; actual: {}]",
                actual.return_type,
                required.return_type,
                region_context(program, required, required_regions),
                region_context(program, actual, actual_regions),
            ));
        }
        Ok(())
    }
}
pub mod region_inference {
    use super::carried_lifetimes::RegionIdentity;
    use crate::types::TypeReferenceHandle;

    #[derive(Clone, Debug, Default)]
    pub struct InputRegionApplication {
        fresh: Vec<(usize, RegionIdentity)>,
        pub elided: Vec<(usize, TypeReferenceHandle, RegionIdentity)>,
    }

    impl PartialEq for InputRegionApplication {
        fn eq(&self, other: &Self) -> bool {
            self.fresh.len() == other.fresh.len()
                && self
                    .fresh
                    .iter()
                    .all(|binding| other.fresh.contains(binding))
                && self.elided.len() == other.elided.len()
                && self
                    .elided
                    .iter()
                    .all(|binding| other.elided.contains(binding))
        }
    }

    impl InputRegionApplication {
        pub fn constrain(
            &mut self,
            parameter: usize,
            reference: TypeReferenceHandle,
            actual: Option<RegionIdentity>,
            required: RegionIdentity,
        ) -> bool {
            match actual {
                Some(RegionIdentity::Fresh(ordinal)) => {
                    if let Some((_, region)) =
                        self.fresh.iter().find(|(source, _)| *source == ordinal)
                    {
                        *region == required
                    } else {
                        self.fresh.push((ordinal, required));
                        true
                    }
                }
                Some(actual) => actual == required,
                None => {
                    if let Some((_, _, region)) =
                        self.elided.iter().find(|(input_parameter, input, _)| {
                            *input_parameter == parameter && *input == reference
                        })
                    {
                        *region == required
                    } else {
                        self.elided.push((parameter, reference, required));
                        true
                    }
                }
            }
        }

        pub fn finish(
            &mut self,
            actual_count: usize,
            required_count: usize,
        ) -> Result<(), &'static str> {
            let mut invocation_count = 0;
            for source in 0..actual_count {
                if self.fresh.iter().any(|(ordinal, region)| {
                    *ordinal == source && matches!(region, RegionIdentity::Captured(_, _))
                }) {
                    continue;
                }
                let expected = RegionIdentity::Fresh(invocation_count);
                if let Some((_, region)) = self.fresh.iter().find(|(ordinal, _)| *ordinal == source)
                {
                    if *region != expected {
                        return Err(
                            "fresh invocation lifetime telescope is not universally refined",
                        );
                    }
                } else {
                    self.fresh.push((source, expected));
                }
                invocation_count += 1;
            }
            let mut implicit = Vec::new();
            for (_, _, region) in &self.elided {
                if let RegionIdentity::Fresh(ordinal) = region
                    && *ordinal >= invocation_count
                    && !implicit.contains(ordinal)
                {
                    implicit.push(*ordinal);
                }
            }
            if required_count != 0 && invocation_count + implicit.len() != required_count {
                if self
                    .fresh
                    .iter()
                    .any(|(_, region)| matches!(region, RegionIdentity::Captured(_, _)))
                {
                    return Err("callable type does not preserve its region identity");
                }
                return Err("fresh invocation lifetime telescope is not universally refined");
            }
            Ok(())
        }

        pub fn apply(&self, region: &mut RegionIdentity) {
            if let RegionIdentity::Fresh(ordinal) = region
                && let Some((_, target)) = self.fresh.iter().find(|(source, _)| source == ordinal)
            {
                *region = target.clone();
            }
        }
    }
}

pub use callable_refinement::lexical_region_owner;
pub use callable_refinement::{
    CallableBorrowContract, captured_type_parameters, validate_callable_borrow_refinement,
};
pub use owned_carriers::{OwnedCarrierLoanRelation, owned_carrier_loan_relation};

pub use callable_refinement::PreparedCallableBorrowRefinement;
pub use lifetime_applications::CallableLifetimeFrame;
pub use lifetime_applications::{
    CallableLifetimeApplication, CallableLifetimeBinding, resolve_callable_lifetime_bindings,
};

pub fn resolve_applied_signature_view_return_source(
    program: &TypedTrees,
    signature: &crate::signature::StateSignature,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &[usize],
    argument_owner: SymbolHandle,
) -> ViewReturnSource {
    if signature.lifetime_parameters.is_empty() {
        return resolve_substituted_view_return_source(
            program,
            program.state_signature_parameters(signature),
            signature.return_type,
            substitutions,
            scope_starts,
        );
    }
    callable_refinement::resolve_invocation_view_return_source(
        program,
        CallableBorrowContract {
            lexical_owner: signature.symbol,
            lifetime_parameters: &signature.lifetime_parameters,
            parameters: program.state_signature_parameters(signature),
            return_type: signature.return_type,
            substitutions,
            substitution_scope_starts: scope_starts,
            application_scopes: &[],
            binders: &[],
            lifetime_applications: &[],
            argument_owner,
        },
        scope_starts,
    )
}

pub use carried_lifetimes::{
    DeclarationBorrowFrontier, DeclarationLifetimeFrontier, declaration_borrow_frontier,
    declaration_lifetime_frontier, substituted_result_frontier_is_complete,
    substituted_result_is_view_free,
};
use carried_lifetimes::{carried_lifetimes, substituted_carried_lifetimes};

fn structural_view_return_source(
    program: &TypedTrees,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &[usize],
    symbolic_parameters: &[SymbolHandle],
) -> ViewReturnSource {
    let Some(outputs) = carried_lifetimes::symbolic_carried_lifetimes(
        program,
        return_type,
        substitutions,
        scope_starts,
        symbolic_parameters,
    ) else {
        if substitutions.is_empty()
            && let Some(source) = whole_elided_result_source(program, parameters, return_type)
        {
            return source;
        }
        return ViewReturnSource::Ambiguous(ViewReturnAmbiguity::IncompleteStructure {
            subject: "result".to_owned(),
        });
    };
    if outputs.is_empty() {
        return ViewReturnSource::NotApplicable;
    }
    let mut inputs = Vec::new();
    for (index, parameter) in parameters.iter().enumerate() {
        if !substituted_returns_borrow(
            program,
            parameter.type_reference,
            substitutions,
            scope_starts,
        ) {
            continue;
        }
        let Some(leaves) = carried_lifetimes::symbolic_input_lifetimes(
            program,
            parameter.type_reference,
            substitutions,
            scope_starts,
            symbolic_parameters,
        ) else {
            return ViewReturnSource::Ambiguous(ViewReturnAmbiguity::IncompleteStructure {
                subject: format!("input `{}`", parameter.name),
            });
        };
        for leaf in leaves {
            inputs.push((index, parameter, leaf));
        }
    }
    structural_frontier_return_source(outputs, &inputs)
}

fn structural_frontier_return_source(
    outputs: Vec<carried_lifetimes::CarriedLifetime>,
    inputs: &[(usize, &StateParameter, carried_lifetimes::CarriedLifetime)],
) -> ViewReturnSource {
    if inputs.is_empty()
        && outputs
            .iter()
            .all(|output| output.lifetime_only || output.lifetime.is_none())
    {
        return ViewReturnSource::NotApplicable;
    }
    let mut fields = Vec::new();
    for output in outputs {
        let matching = inputs
            .iter()
            .filter(|(_, _, input)| {
                (if output.lifetime_only {
                    input.lifetime_only || output.retained_contract
                } else {
                    !input.lifetime_only
                }) && output
                    .lifetime
                    .as_ref()
                    .is_none_or(|lifetime| input.lifetime.as_ref() == Some(lifetime))
            })
            .collect::<Vec<_>>();
        let Some(first) = matching.first() else {
            if output.lifetime_only {
                continue;
            }
            return ViewReturnSource::Ambiguous(ViewReturnAmbiguity::LifetimeMatchesNoInput {
                lifetime: match output.lifetime {
                    Some(carried_lifetimes::RegionIdentity::Declared(name)) => name,
                    Some(identity) => format!("{identity:?}"),
                    None => "elided".to_owned(),
                },
            });
        };
        if output.lifetime.is_none() && matching.len() != 1 {
            return ViewReturnSource::Ambiguous(ViewReturnAmbiguity::ElidedMultipleInputs {
                candidates: matching
                    .iter()
                    .map(|(_, parameter, input)| {
                        format!("{} carried source {:?}", parameter.name, input.owner_path)
                    })
                    .collect(),
            });
        }
        if matching.iter().any(|(_, _, input)| {
            use language_semantics::ReferenceAccess;
            input.access != output.access && input.access != ReferenceAccess::Mutable
        }) {
            return ViewReturnSource::Ambiguous(ViewReturnAmbiguity::IncompatibleSourceAccess {
                input: first.1.name.as_str().to_owned(),
            });
        }
        for (index, parameter, input) in matching {
            fields.push(ViewReturnFieldSource {
                lifetime_only: output.lifetime_only,
                owner_path: output.owner_path.clone(),
                source_path: input.owner_path.clone(),
                source: input.source,
                source_type: parameter.type_reference,
                source_reference_hops: input.reference_hops,
                parameter_index: *index,
                kind: match output.access {
                    language_semantics::ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
                    language_semantics::ReferenceAccess::Shared => BorrowAccessKind::Read,
                    language_semantics::ReferenceAccess::WriteOnly => BorrowAccessKind::WriteOnly,
                },
            });
        }
    }
    ViewReturnSource::Fields { fields }
}

fn whole_elided_result_source(
    program: &TypedTrees,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
) -> Option<ViewReturnSource> {
    let output_accesses = carried_lifetimes::whole_elided_result_accesses(program, return_type)?;
    if output_accesses.contains(&language_semantics::ReferenceAccess::WriteOnly) {
        return None;
    }
    let mut candidates = parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| returns_borrow(program, parameter.type_reference));
    let (index, parameter) = candidates.next()?;
    if candidates.next().is_some() || !is_reference_type(program, parameter.type_reference) {
        return None;
    }
    let inputs = carried_lifetimes(program, parameter.type_reference)?;
    let [input] = inputs.as_slice() else {
        return None;
    };
    if !output_accesses.iter().all(|access| {
        *access == input.access || input.access == language_semantics::ReferenceAccess::Mutable
    }) {
        return Some(ViewReturnSource::Ambiguous(
            ViewReturnAmbiguity::IncompatibleSourceAccess {
                input: parameter.name.as_str().to_owned(),
            },
        ));
    }
    Some(ViewReturnSource::Parameter {
        parameter_index: index,
    })
}

pub fn is_reference_type(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    if !program.contains_type_reference(type_reference) {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => is_reference_type(program, *base_type),
        _ => false,
    }
}

pub fn returns_borrow(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    is_reference_type(program, type_reference) || is_borrow_carrying_data(program, type_reference)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputWriteOnlyAccess {
    Absent,
    Present,
    TemplateDependent,
    Incomplete,
}

pub fn input_write_only_access(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    parameters: &[crate::data::TypeParameter],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> InputWriteOnlyAccess {
    carried_lifetimes::input_write_only_access(program, type_reference, parameters, substitutions)
}

pub fn borrow_carrying_owner_paths(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<Vec<Vec<BorrowOwnerSegment>>> {
    carried_lifetimes(program, type_reference)
        .map(|leaves| leaves.into_iter().map(|leaf| leaf.owner_path).collect())
}

pub fn carried_reference_paths(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<Vec<(Vec<BorrowOwnerSegment>, BorrowAccessKind)>> {
    carried_lifetimes(program, type_reference).map(|leaves| {
        leaves
            .into_iter()
            .filter(|leaf| !leaf.lifetime_only)
            .map(|leaf| (leaf.owner_path, leaf.access.into()))
            .collect()
    })
}

pub fn is_borrow_carrying_data(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    carried_lifetimes(program, type_reference).is_none_or(|leaves| !leaves.is_empty())
}

pub fn is_mutably_borrow_carrying_data(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    carried_lifetimes(program, type_reference)
        .is_none_or(|leaves| leaves.iter().any(|leaf| leaf.access.is_exclusive()))
}

fn data_definition(program: &TypedTrees, symbol: SymbolHandle) -> Option<&DataDefinition> {
    program.data_definition_by_symbol(symbol)
}

pub fn retained_lifetime_owner_paths(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<Vec<Vec<BorrowOwnerSegment>>> {
    carried_lifetimes::stored_lifetimes(program, reference).map(|leaves| {
        leaves
            .into_iter()
            .filter(|leaf| leaf.lifetime_only)
            .map(|leaf| leaf.owner_path)
            .collect()
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum BorrowAccessKind {
    #[default]
    Read,
    Mutable,
    WriteOnly,
}

impl From<language_core::ReferenceAccess> for BorrowAccessKind {
    fn from(access: language_core::ReferenceAccess) -> Self {
        match access {
            language_core::ReferenceAccess::Shared => Self::Read,
            language_core::ReferenceAccess::Mutable => Self::Mutable,
            language_core::ReferenceAccess::WriteOnly => Self::WriteOnly,
        }
    }
}

impl BorrowAccessKind {
    pub fn is_exclusive(&self) -> bool {
        matches!(self, Self::Mutable | Self::WriteOnly)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum BorrowLoanOwnerSegment {
    Field(SymbolHandle),
    Case(SymbolHandle),
    FixedIndex(usize),
    #[default]
    DynamicIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum ViewReturnInputSource {
    Referent,
    Stored,
}

fn payload_variant_for_field(program: &TypedTrees, field: SymbolHandle) -> Option<SymbolHandle> {
    if !program.symbols.symbols().nodes().is_valid(field) {
        return None;
    }
    let parent = program.symbols.get(field).parent;
    (program.symbols.symbols().nodes().is_valid(parent)
        && program.symbols.get(parent).kind == symbols::SymbolKind::Variant)
        .then_some(parent)
}

pub mod type_regions {
    use crate::name::Identifier;
    use crate::types::{
        DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };

    pub trait TypeRegionScope: Copy {
        fn erased_identity(
            self,
            program: &crate::TypedTrees,
            reference: TypeReferenceHandle,
        ) -> crate::type_identity::NormalizedTypeIdentity {
            program.normalized_type_identity(reference)
        }

        fn enter_operand(
            self,
            _program: &crate::TypedTrees,
            _owner: symbols::SymbolHandle,
        ) -> Self {
            self
        }

        fn at_lifetime(
            self,
            program: &crate::TypedTrees,
            reference: TypeReferenceHandle,
            slot: usize,
            spelling: Option<&Identifier>,
        ) -> Option<Self> {
            Some(
                match retained_lifetime_identity(program, reference, slot, spelling).ok()? {
                    Some(region) => self.enter_lifetime(program, region),
                    None => self,
                },
            )
        }

        fn enter_lifetime(
            self,
            program: &crate::TypedTrees,
            region: crate::type_identity::LifetimeParameterIdentity,
        ) -> Self {
            self.enter_operand(program, region.declaration)
        }

        fn named_alias(
            self,
            _program: &crate::TypedTrees,
            _symbol: symbols::SymbolHandle,
            _reference: TypeReferenceHandle,
        ) -> Option<ReferenceStep<Self>> {
            None
        }

        fn resolve(
            mut self,
            program: &crate::TypedTrees,
            mut reference: TypeReferenceHandle,
        ) -> Option<(TypeReferenceHandle, Self)> {
            let mut visited = Vec::new();
            loop {
                if !reference.is_valid() {
                    return Some((reference, self));
                }
                if visited.contains(&reference)
                    || !program
                        .type_reference_table
                        .contains_type_reference(reference)
                {
                    return None;
                }
                visited.push(reference);
                let owner = program
                    .type_reference_table
                    .operand_lifetime_owner(reference);
                if owner.is_valid() {
                    self = self.enter_operand(program, owner);
                }
                let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    return Some((reference, self));
                };
                match self.named_alias(program, *symbol, reference) {
                    Some(ReferenceStep::Refuse) => return None,
                    Some(ReferenceStep::Stop) => return Some((reference, self)),
                    Some(ReferenceStep::Chase(scope, next)) => {
                        self = scope;
                        reference = next;
                        continue;
                    }
                    None => {}
                }
                if let Some(application) = program
                    .data_definition_by_symbol(*symbol)
                    .and_then(|definition| definition.generic_instance)
                {
                    reference = application;
                    continue;
                }
                return Some((reference, self));
            }
        }
    }

    pub fn retained_lifetime_identity(
        program: &crate::TypedTrees,
        reference: TypeReferenceHandle,
        slot: usize,
        spelling: Option<&Identifier>,
    ) -> Result<Option<crate::type_identity::LifetimeParameterIdentity>, &'static str> {
        let regions = program.type_reference_table.operand_lifetimes(reference);
        if regions.is_empty() {
            return Ok(None);
        }
        let expected = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference {
                lifetime: Some(_), ..
            } => 1,
            TypeReferenceNode::Generic {
                lifetime_arguments, ..
            } => lifetime_arguments.len(),
            _ => 0,
        };
        if regions.len() != expected {
            return Err("retained lifetime slots do not match the type's telescope");
        }
        let region = *regions
            .get(slot)
            .ok_or("retained lifetime slot is absent")?;
        if !region.declaration.is_valid() {
            return Ok(None);
        }
        if crate::type_identity::lifetime_parameter_name(program, region) != spelling {
            return Err("retained lifetime slot lost its exact declaration ordinal");
        }
        Ok(Some(region))
    }

    pub enum ReferenceStep<Scope> {
        Chase(Scope, TypeReferenceHandle),
        Stop,
        Refuse,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum TypeRegionRelationship {
        CallableRefinement,
        ExactApplication,
    }

    pub fn type_reference_regions_match<Scope: TypeRegionScope, State: Clone + PartialEq>(
        program: &crate::TypedTrees,
        actual: TypeReferenceHandle,
        required: TypeReferenceHandle,
        actual_scope: Scope,
        required_scope: Scope,
        state: &mut State,
        lifetime_matches: &mut impl FnMut(
            &mut State,
            Scope,
            Scope,
            TypeReferenceHandle,
            Option<&Identifier>,
            Option<&Identifier>,
        ) -> bool,
    ) -> bool {
        let mut applications = vec![state.clone()];
        if !type_reference_region_applications(
            program,
            actual,
            required,
            actual_scope,
            required_scope,
            TypeRegionRelationship::CallableRefinement,
            &mut applications,
            lifetime_matches,
        ) {
            return false;
        }
        let [application] = applications.as_slice() else {
            return false;
        };
        *state = application.clone();
        true
    }

    pub fn type_reference_region_applications<Scope: TypeRegionScope, State: Clone + PartialEq>(
        program: &crate::TypedTrees,
        actual: TypeReferenceHandle,
        required: TypeReferenceHandle,
        actual_scope: Scope,
        required_scope: Scope,
        relationship: TypeRegionRelationship,
        applications: &mut Vec<State>,
        lifetime_matches: &mut impl FnMut(
            &mut State,
            Scope,
            Scope,
            TypeReferenceHandle,
            Option<&Identifier>,
            Option<&Identifier>,
        ) -> bool,
    ) -> bool {
        if applications.is_empty() {
            return false;
        }
        if !actual.is_valid() || !required.is_valid() {
            return actual.is_valid() == required.is_valid();
        }
        let Some((actual, actual_scope, actual_domains)) =
            type_structure(program, actual, actual_scope)
        else {
            return false;
        };
        let Some((required, required_scope, required_domains)) =
            type_structure(program, required, required_scope)
        else {
            return false;
        };
        macro_rules! region_child {
            ($actual:expr, $required:expr $(,)?) => {
                type_reference_region_applications(
                    program,
                    $actual,
                    $required,
                    actual_scope,
                    required_scope,
                    relationship,
                    applications,
                    lifetime_matches,
                )
            };
        }
        if !domain_region_applications(
            program,
            &actual_domains,
            &required_domains,
            DomainCoverage::Required,
            relationship,
            applications,
            lifetime_matches,
        ) || (relationship == TypeRegionRelationship::ExactApplication
            && !domain_region_applications(
                program,
                &actual_domains,
                &required_domains,
                DomainCoverage::Actual,
                relationship,
                applications,
                lifetime_matches,
            ))
        {
            return false;
        }
        match (
            program.type_reference_table.type_reference(actual),
            program.type_reference_table.type_reference(required),
        ) {
            (
                TypeReferenceNode::Reference {
                    referee: actual_referee,
                    lifetime: actual_lifetime,
                    ..
                },
                TypeReferenceNode::Reference {
                    referee: required_referee,
                    lifetime: required_lifetime,
                    ..
                },
            ) => {
                let Some(actual_region_scope) =
                    actual_scope.at_lifetime(program, actual, 0, actual_lifetime.as_ref())
                else {
                    return false;
                };
                let Some(required_region_scope) =
                    required_scope.at_lifetime(program, required, 0, required_lifetime.as_ref())
                else {
                    return false;
                };
                applications.retain_mut(|state| {
                    lifetime_matches(
                        state,
                        actual_region_scope,
                        required_region_scope,
                        actual,
                        actual_lifetime.as_ref(),
                        required_lifetime.as_ref(),
                    )
                });
                region_child!(*actual_referee, *required_referee)
            }
            (
                TypeReferenceNode::FixedArray {
                    element_type: actual_element,
                    ..
                },
                TypeReferenceNode::FixedArray {
                    element_type: required_element,
                    ..
                },
            )
            | (
                TypeReferenceNode::Slice {
                    element_type: actual_element,
                },
                TypeReferenceNode::Slice {
                    element_type: required_element,
                },
            ) => region_child!(*actual_element, *required_element),
            (
                TypeReferenceNode::Generic {
                    lifetime_arguments: actual_lifetimes,
                    arguments: actual_arguments,
                    ..
                },
                TypeReferenceNode::Generic {
                    lifetime_arguments: required_lifetimes,
                    arguments: required_arguments,
                    ..
                },
            ) => {
                actual_lifetimes.len() == required_lifetimes.len()
                    && actual_lifetimes
                        .iter()
                        .zip(required_lifetimes)
                        .enumerate()
                        .all(|(slot, (actual_lifetime, required_lifetime))| {
                            let Some(actual_region_scope) = actual_scope.at_lifetime(
                                program,
                                actual,
                                slot,
                                Some(actual_lifetime),
                            ) else {
                                return false;
                            };
                            let Some(required_region_scope) = required_scope.at_lifetime(
                                program,
                                required,
                                slot,
                                Some(required_lifetime),
                            ) else {
                                return false;
                            };
                            applications.retain_mut(|state| {
                                lifetime_matches(
                                    state,
                                    actual_region_scope,
                                    required_region_scope,
                                    actual,
                                    Some(actual_lifetime),
                                    Some(required_lifetime),
                                )
                            });
                            !applications.is_empty()
                        })
                    && actual_arguments.count() == required_arguments.count()
                    && program
                        .type_reference_table
                        .type_reference_handles(*actual_arguments)
                        .len()
                        == actual_arguments.len()
                    && program
                        .type_reference_table
                        .type_reference_handles(*required_arguments)
                        .len()
                        == required_arguments.len()
                    && program
                        .type_reference_table
                        .type_reference_handles(*actual_arguments)
                        .iter()
                        .zip(
                            program
                                .type_reference_table
                                .type_reference_handles(*required_arguments),
                        )
                        .all(|(actual_argument, required_argument)| {
                            region_child!(*actual_argument, *required_argument)
                        })
            }
            (
                TypeReferenceNode::DynamicTrait {
                    arguments: actual, ..
                },
                TypeReferenceNode::DynamicTrait {
                    arguments: required,
                    ..
                },
            ) => {
                actual.len() == required.len()
                    && program
                        .type_reference_table
                        .type_reference_handles(*actual)
                        .len()
                        == actual.len()
                    && program
                        .type_reference_table
                        .type_reference_handles(*required)
                        .len()
                        == required.len()
                    && program
                        .type_reference_table
                        .type_reference_handles(*actual)
                        .iter()
                        .zip(
                            program
                                .type_reference_table
                                .type_reference_handles(*required),
                        )
                        .all(|(actual, required)| region_child!(*actual, *required))
            }
            _ => true,
        }
    }

    #[derive(Clone, Copy)]
    pub enum DomainCoverage {
        Required,
        Actual,
    }

    fn domain_region_applications<Scope: TypeRegionScope, State: Clone + PartialEq>(
        program: &crate::TypedTrees,
        actual_domains: &[(&DomainConstraint, Scope)],
        required_domains: &[(&DomainConstraint, Scope)],
        coverage: DomainCoverage,
        relationship: TypeRegionRelationship,
        applications: &mut Vec<State>,
        lifetime_matches: &mut impl FnMut(
            &mut State,
            Scope,
            Scope,
            TypeReferenceHandle,
            Option<&Identifier>,
            Option<&Identifier>,
        ) -> bool,
    ) -> bool {
        let (obligations, candidates) = match coverage {
            DomainCoverage::Required => (required_domains, actual_domains),
            DomainCoverage::Actual => (actual_domains, required_domains),
        };
        for (obligation, obligation_scope) in obligations {
            let corresponding = candidates.iter().filter(|(candidate, candidate_scope)| {
                candidate.subject == obligation.subject
                    && candidate.symbol == obligation.symbol
                    && candidate.arguments.len() == obligation.arguments.len()
                    && candidate.arguments.iter().zip(&obligation.arguments).all(
                        |(candidate, obligation)| {
                            candidate_scope.erased_identity(program, *candidate)
                                == obligation_scope.erased_identity(program, *obligation)
                        },
                    )
            });
            let mut found = false;
            let mut alternatives = Vec::new();
            for (candidate, candidate_scope) in corresponding {
                found = true;
                let (actual, actual_scope, required, required_scope) = match coverage {
                    DomainCoverage::Required => {
                        (*candidate, *candidate_scope, *obligation, *obligation_scope)
                    }
                    DomainCoverage::Actual => {
                        (*obligation, *obligation_scope, *candidate, *candidate_scope)
                    }
                };
                let mut candidate_applications = applications.clone();
                if actual
                    .arguments
                    .iter()
                    .zip(&required.arguments)
                    .all(|(actual, required)| {
                        type_reference_region_applications(
                            program,
                            *actual,
                            *required,
                            actual_scope,
                            required_scope,
                            relationship,
                            &mut candidate_applications,
                            lifetime_matches,
                        )
                    })
                {
                    for application in candidate_applications {
                        if !alternatives.contains(&application) {
                            alternatives.push(application);
                        }
                    }
                }
            }
            if !found && relationship == TypeRegionRelationship::CallableRefinement {
                continue;
            }
            *applications = alternatives;
            if applications.is_empty() {
                return false;
            }
        }
        true
    }

    fn type_structure<Scope: TypeRegionScope>(
        program: &crate::TypedTrees,
        mut reference: TypeReferenceHandle,
        mut scope: Scope,
    ) -> Option<(TypeReferenceHandle, Scope, Vec<(&DomainConstraint, Scope)>)> {
        let mut domains = Vec::new();
        let mut visited = Vec::new();
        loop {
            let (resolved, resolved_scope) = scope.resolve(program, reference)?;
            reference = resolved;
            scope = resolved_scope;
            if visited.contains(&reference) {
                return None;
            }
            visited.push(reference);
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program.type_reference_table.type_reference(reference)
            else {
                return Some((reference, scope, domains));
            };
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    domains.push((domain, scope));
                }
            }
            reference = *base_type;
        }
    }
}
