/*
Reference and borrow lifetime contract validation (`borrow_contracts`) and
compile-time semantic reflection schema graphs, selection snapshots, and
runtime field visitation plans (`reflection`).
*/

pub mod borrow_contracts {
    use crate::CheckingInput;
    use checked_trees::BorrowAccessKind;
    use symbols::SymbolHandle;
    use typed_trees::data::DataDefinition;
    use typed_trees::signature::StateParameter;
    use typed_trees::state::State;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    use checked_trees::BorrowLoanOwnerSegment as BorrowOwnerSegment;

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

    pub use checked_trees::ViewReturnInputSource;

    #[derive(Debug, Clone)]
    pub enum ViewReturnAmbiguity {
        IncompleteStructure { subject: String },
        IncompatibleSourceAccess { input: String },
        ElidedMultipleInputs { candidates: Vec<String> },
        LifetimeMatchesNoInput { lifetime: String },
    }

    pub fn resolve_view_return_source(
        program: &dyn CheckingInput,
        state: &State,
    ) -> ViewReturnSource {
        if let Some(relation) = installed_context_lifetime_source(program, state.symbol) {
            return relation;
        }
        resolve_signature_view_return_source(
            program,
            program.state_parameters(state),
            state.return_type,
        )
    }

    pub fn resolve_signature_view_return_source(
        program: &dyn CheckingInput,
        parameters: &[StateParameter],
        return_type: TypeReferenceHandle,
    ) -> ViewReturnSource {
        if !returns_borrow(program, return_type) {
            return ViewReturnSource::NotApplicable;
        }

        structural_view_return_source(program, parameters, return_type, &[], &[], &[])
    }

    pub fn resolve_substituted_view_return_source(
        program: &dyn CheckingInput,
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
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        scope_starts: &[usize],
    ) -> bool {
        substituted_carried_lifetimes(program, type_reference, substitutions, scope_starts)
            .is_none_or(|leaves| !leaves.is_empty())
    }

    pub(crate) mod callable_refinement {
        use super::CallableLifetimeApplication;
        use super::carried_lifetimes::{
            CarriedLifetime, LifetimeFrontier, RegionIdentity, scoped_carried_lifetimes,
        };
        use super::{ViewReturnSource, structural_frontier_return_source};
        use checked_trees::BorrowLoanOwnerSegment;
        use symbols::SymbolHandle;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::name::Identifier;
        use typed_trees::signature::StateParameter;
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
            pub lifetime_applications: &'contract [CallableLifetimeApplication<'contract>],
            pub argument_owner: SymbolHandle,
        }

        pub(crate) struct ContractFrontier {
            regions: super::callable_regions::CallableRegions,
            inputs: Vec<Vec<CarriedLifetime>>,
            outputs: Vec<CarriedLifetime>,
            sources: ViewReturnSource,
        }

        pub(crate) fn resolve_invocation_view_return_source(
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
            program: &dyn crate::CheckingInput,
            signature: SymbolHandle,
        ) -> Vec<SymbolHandle> {
            let mut captured = Vec::new();
            let mut owner = program.declaration_scope_parent(signature);
            while owner.is_valid() {
                let parameters =
                    if let Some(contract) = program.retained_static_machine_contract(owner) {
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
            program: &dyn crate::CheckingInput,
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

        pub(crate) struct PreparedCallableBorrowRefinement {
            required_frontier: ContractFrontier,
            actual_frontier: ContractFrontier,
            application: super::region_inference::InputRegionApplication,
            required_substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
            actual_substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
        }

        impl PreparedCallableBorrowRefinement {
            pub(crate) fn prepare(
                program: &dyn crate::CheckingInput,
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

            pub(crate) fn lifetime_frame(
                &self,
                program: &dyn crate::CheckingInput,
                required: &CallableBorrowContract<'_>,
                actual: &CallableBorrowContract<'_>,
            ) -> Result<super::CallableLifetimeFrame, String> {
                let own_parameters = |contract: &CallableBorrowContract<'_>| {
                    typed_trees::type_identity::declaration_lifetime_parameters(
                        program.typed_program(),
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
                    let parameters = typed_trees::type_identity::declaration_lifetime_parameters(
                        program.typed_program(),
                        declaration,
                    )
                    .ok_or("callable lifetime partition has no retained declaration telescope")?;
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
                                    typed_trees::type_identity::LifetimeParameterIdentity {
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

            pub(crate) fn validate(
                self,
                program: &dyn crate::CheckingInput,
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
                                _ => {
                                    "callable input does not preserve its captured region identity"
                                }
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
                    .filter_map(|output| {
                        input_region.map(|region| (output.reference, region.clone()))
                    })
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
            program: &dyn crate::CheckingInput,
            required: CallableBorrowContract<'_>,
            actual: CallableBorrowContract<'_>,
        ) -> Result<(), String> {
            PreparedCallableBorrowRefinement::prepare(program, &required, &actual)?
                .validate(program, required, actual)
        }

        fn contract_frontier(
            program: &dyn crate::CheckingInput,
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

        pub(crate) fn lexical_region_owner(
            program: &dyn crate::CheckingInput,
            mut symbol: SymbolHandle,
        ) -> SymbolHandle {
            while symbol.is_valid() {
                symbol = program.lifetime_declaration_owner(symbol);
                if !symbol.is_valid() {
                    return symbol;
                }
                if typed_trees::type_identity::declaration_lifetime_parameters(
                    program.typed_program(),
                    symbol,
                )
                .is_some()
                {
                    return symbol;
                }
                symbol = program.declaration_scope_parent(symbol);
            }
            symbol
        }

        pub(crate) fn captured_lifetimes(
            program: &dyn crate::CheckingInput,
            owner: SymbolHandle,
        ) -> Vec<(String, RegionIdentity)> {
            let mut owner = program.lifetime_declaration_owner(owner);
            let mut scopes = Vec::new();
            while owner.is_valid() {
                let parameters = typed_trees::type_identity::declaration_lifetime_parameters(
                    program.typed_program(),
                    owner,
                );
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
                            && actual.source_reference_hops == required.source_reference_hops
                            && actual.kind == required.kind
                    })
                }) =>
                {
                    Ok(())
                }
                (ViewReturnSource::Ambiguous(_), _) | (_, ViewReturnSource::Ambiguous(_)) => Err(
                    "unimplemented: callable returned-source relation is ambiguous or incomplete",
                ),
                _ => Err("callable returned view has a different input-source relation"),
            }
        }
    }
    pub(crate) mod callable_regions {
        use super::CallableLifetimeApplication;
        use super::callable_refinement::{CallableBorrowContract, captured_lifetimes};
        use super::carried_lifetimes::RegionIdentity;
        use typed_trees::type_identity::{
            LifetimeParameterIdentity, applied_lifetime_parameter_identity,
            lifetime_parameter_identity,
        };

        pub(crate) struct AppliedRegionBinding {
            pub(crate) parameter: LifetimeParameterIdentity,
            pub(crate) argument: RegionIdentity,
        }

        pub(crate) fn applied_region<'scope>(
            program: &dyn crate::CheckingInput,
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

        pub(crate) struct CallableRegions {
            pub(crate) names: Vec<(String, RegionIdentity)>,
            pub(crate) arguments: Vec<(String, RegionIdentity)>,
            pub(crate) applications: Vec<Vec<(String, RegionIdentity)>>,
            pub(crate) fresh_count: usize,
            pub(crate) bindings: Vec<AppliedRegionBinding>,
            pub(crate) own_lifetimes: Vec<(LifetimeParameterIdentity, RegionIdentity)>,
        }

        pub(crate) fn resolve(
            program: &dyn crate::CheckingInput,
            contract: &CallableBorrowContract<'_>,
        ) -> Result<CallableRegions, &'static str> {
            let arguments = captured_lifetimes(program, contract.argument_owner);
            let mut applications = Vec::new();
            for (ordinal, scope) in contract.application_scopes.iter().enumerate() {
                let names = captured_lifetimes(program, scope.declaration)
                    .into_iter()
                    .map(|(name, _)| {
                        let identity = applied_lifetime_parameter_identity(
                            program.typed_program(),
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
                    lifetime_parameter_identity(
                        program.typed_program(),
                        contract.lexical_owner,
                        parameter.as_str(),
                    )
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
                    let parameter_name = typed_trees::type_identity::lifetime_parameter_name(
                        program.typed_program(),
                        binding.parameter,
                    )
                    .ok_or("callable capture lost its exact receiving lifetime ordinal")?;
                    typed_trees::type_identity::lifetime_parameter_name(
                        program.typed_program(),
                        binding.argument,
                    )
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
            for (parameter, declaration) in
                contract.lifetime_parameters.iter().zip(fresh_parameters)
            {
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
    pub(crate) mod carried_lifetimes {
        use super::data_definition;
        use super::{SymbolHandle, TypeReferenceHandle, TypeReferenceNode};
        use checked_trees::BorrowLoanOwnerSegment as BorrowOwnerSegment;

        #[derive(Clone)]
        pub(crate) struct CarriedLifetime {
            pub(crate) lifetime_only: bool,
            pub(crate) retained_contract: bool,
            pub(crate) reference: TypeReferenceHandle,
            pub(crate) reference_hops: usize,
            pub(crate) owner_path: Vec<BorrowOwnerSegment>,
            pub(crate) lifetime: Option<RegionIdentity>,
            pub(crate) access: language_semantics::ReferenceAccess,
            pub(crate) source: super::ViewReturnInputSource,
        }

        #[derive(Clone, Copy, Default, PartialEq, Eq)]
        pub(crate) enum LifetimeFrontier {
            #[default]
            Value,
            Input,
            ReachableInput,
            InputStorage,
            BorrowedStorage,
            WriteOnlyAuthority,
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(crate) enum RegionIdentity {
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
        pub(crate) struct TemplateFrontier<'a> {
            parameters: &'a [SymbolHandle],
            authority_parameters: &'a [typed_trees::data::TypeParameter],
            dependent: bool,
            declaration_inventory: bool,
            scoped_regions: bool,
            frontier: LifetimeFrontier,
            input_reference_hops: usize,
        }

        #[derive(Clone, Copy, Default)]
        pub(crate) struct TypeScope<'scope> {
            declaration: SymbolHandle,
            lifetimes: &'scope [(String, RegionIdentity)],
            bindings: &'scope [(SymbolHandle, TypeReferenceHandle)],
            starts: &'scope [usize],
            outer: Option<&'scope TypeScope<'scope>>,
            argument_lifetimes: &'scope [(String, RegionIdentity)],
            application_lifetimes: &'scope [Vec<(String, RegionIdentity)>],
            captured: &'scope [super::callable_regions::AppliedRegionBinding],
            application_scopes:
                &'scope [typed_trees::type_identity::AppliedTypeIdentityScope<'scope>],
            argument_owner: SymbolHandle,
            region_outer: Option<&'scope TypeScope<'scope>>,
        }

        impl TypeScope<'_> {
            fn applied_region(
                &self,
                program: &dyn crate::CheckingInput,
                parameter: typed_trees::type_identity::LifetimeParameterIdentity,
            ) -> Option<RegionIdentity> {
                let mut scope = Some(self);
                while let Some(frame) = scope {
                    if let Some(argument) =
                        super::callable_regions::applied_region(program, frame.captured, parameter)
                    {
                        return Some(argument.clone());
                    }
                    if let Some((argument, true)) =
                        typed_trees::type_identity::apply_resolved_lifetime_parameter_identity(
                            program.typed_program(),
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

        pub(crate) struct DataVisit {
            reference: TypeReferenceHandle,
            cyclic: bool,
        }

        pub fn declaration_lifetime_frontier(
            program: &dyn crate::CheckingInput,
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
            program: &dyn crate::CheckingInput,
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

        pub(crate) fn carried_lifetimes(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Option<Vec<CarriedLifetime>> {
            storage_lifetimes(program, reference, LifetimeFrontier::Value)
        }

        pub(crate) fn stored_lifetimes(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Option<Vec<CarriedLifetime>> {
            storage_lifetimes(program, reference, LifetimeFrontier::BorrowedStorage)
        }

        fn storage_lifetimes(
            program: &dyn crate::CheckingInput,
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
            program: &dyn crate::CheckingInput,
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
            program: &dyn crate::CheckingInput,
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
            program: &dyn crate::CheckingInput,
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

        pub(crate) fn substituted_carried_lifetimes(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            scope_starts: &[usize],
        ) -> Option<Vec<CarriedLifetime>> {
            symbolic_carried_lifetimes(program, reference, substitutions, scope_starts, &[])
        }

        pub(crate) fn symbolic_carried_lifetimes(
            program: &dyn crate::CheckingInput,
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

        pub(crate) fn input_write_only_access(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            parameters: &[typed_trees::data::TypeParameter],
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

        pub(crate) fn symbolic_input_lifetimes(
            program: &dyn crate::CheckingInput,
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

        pub(crate) struct CarriedLifetimeInventory {
            pub(crate) leaves: Vec<CarriedLifetime>,
            pub(crate) template_dependent: bool,
        }

        pub(crate) fn declaration_lifetime_inventory(
            program: &dyn crate::CheckingInput,
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
        pub(crate) fn scoped_carried_lifetimes(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            scope_starts: &[usize],
            symbolic_parameters: &[SymbolHandle],
            lifetimes: &[(String, RegionIdentity)],
            argument_lifetimes: &[(String, RegionIdentity)],
            application_lifetimes: &[Vec<(String, RegionIdentity)>],
            captured: &[super::callable_regions::AppliedRegionBinding],
            application_scopes: &[typed_trees::type_identity::AppliedTypeIdentityScope<'_>],
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
            program: &dyn crate::CheckingInput,
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

        pub(crate) fn whole_elided_result_accesses(
            program: &dyn crate::CheckingInput,
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
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            slot: usize,
            name: &typed_trees::name::Identifier,
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
            let Some(region) = crate::queries::type_regions::retained_lifetime_identity(
                program,
                reference,
                slot,
                Some(name),
            )
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

        fn collect_type(
            program: &dyn crate::CheckingInput,
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
                                    typed_trees::type_identity::LifetimeParameterIdentity {
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
            if reference.is_valid()
                && !program
                    .type_reference_table
                    .contains_type_reference(reference)
            {
                return false;
            }
            if program.primitive_type_reference(reference).is_some() {
                return true;
            }
            if template.frontier == LifetimeFrontier::WriteOnlyAuthority
                && crate::type_satisfies_declared_property(
                    program,
                    template.authority_parameters,
                    reference,
                    crate::DeclaredPropertyRequirement::Copy,
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
                        && crate::slice_view_shell(program, reference, &[]).is_none()
                        && !crate::byte_sequence_carrier(program, reference, &[])
                            .is_some_and(|carrier| carrier.is_borrowed())
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
                            if output[position].access
                                == language_semantics::ReferenceAccess::WriteOnly
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
                    let first = output.len();
                    let complete = carry_child!(*base_type);
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        if let typed_trees::types::TypeConstraintNode::Domain(domain) = constraint
                            && program.retained_borrow_custodies().iter().any(|custody| {
                                custody.retained_semantic_domain == domain.semantic_id
                            })
                        {
                            for leaf in &mut output[first..] {
                                if leaf.lifetime_only && leaf.owner_path == owner_path {
                                    leaf.retained_contract = true;
                                }
                            }
                        }
                    }
                    complete
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    if matches!(length, typed_trees::types::FixedArrayLength::Literal(0)) {
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
                    let typed_trees::types::FixedArrayLength::Literal(length) = length else {
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
                    if template.frontier != LifetimeFrontier::WriteOnlyAuthority
                        && let Some(custody) = program
                            .retained_borrow_custodies()
                            .iter()
                            .find(|custody| custody.result_data == *base_symbol)
                        && let Some(lifetime) = lifetime_arguments
                            .get(custody.result_lifetime_argument_ordinal as usize)
                    {
                        output.push(CarriedLifetime {
                            lifetime_only: true,
                            retained_contract: false,
                            reference,
                            reference_hops: template.input_reference_hops,
                            owner_path: owner_path.to_vec(),
                            lifetime: {
                                let Some(region) = carried_region(
                                    program,
                                    reference,
                                    custody.result_lifetime_argument_ordinal as usize,
                                    lifetime,
                                    lifetimes,
                                    types,
                                    template.scoped_regions,
                                ) else {
                                    return false;
                                };
                                Some(region)
                            },
                            access: language_semantics::ReferenceAccess::Shared,
                            source: super::ViewReturnInputSource::Stored,
                        });
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
                    if template.frontier != LifetimeFrontier::WriteOnlyAuthority
                        && program
                            .retained_borrow_custodies()
                            .iter()
                            .any(|custody| custody.result_data == definition.symbol)
                    {
                        output.push(CarriedLifetime {
                            lifetime_only: true,
                            retained_contract: false,
                            reference,
                            reference_hops: template.input_reference_hops,
                            owner_path: owner_path.to_vec(),
                            lifetime: None,
                            access: language_semantics::ReferenceAccess::Shared,
                            source: super::ViewReturnInputSource::Stored,
                        });
                    }
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
                TypeReferenceNode::Unit | TypeReferenceNode::DynamicTrait { .. } => true,
                TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Slice { .. } => false,
            }
        }

        fn collect_family(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            definition: &typed_trees::data::DataDefinition,
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
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            definition: &typed_trees::data::DataDefinition,
            lifetimes: &[(String, RegionIdentity)],
            types: TypeScope<'_>,
            owner_path: &[BorrowOwnerSegment],
            visiting: &mut Vec<DataVisit>,
            output: &mut Vec<CarriedLifetime>,
            whole_elided_contract: bool,
            template: &mut TemplateFrontier<'_>,
        ) -> bool {
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
                    if let Some(variant) = facts::payload_variant_for_field(program, field.symbol) {
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
            use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
            use typed_trees::data::{DataDefinition, DataField, DataMember};

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
                let mut program = typed_trees::TypedTrees {
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
    pub(crate) mod lifetime_applications {
        use symbols::SymbolHandle;
        use typed_trees::static_applications::StaticMachineLifetimeBinding;
        use typed_trees::type_identity::LifetimeParameterIdentity;

        #[derive(Clone, Copy)]
        pub struct CallableLifetimeBinding {
            pub parameter: LifetimeParameterIdentity,
            pub argument: LifetimeParameterIdentity,
        }

        #[derive(Clone, Default)]
        pub(crate) struct CallableLifetimeFrame {
            pub(crate) required: Vec<CallableLifetimeBinding>,
            pub(crate) actual: Vec<CallableLifetimeBinding>,
        }

        #[derive(Clone, Copy)]
        pub enum CallableLifetimeApplication<'scope> {
            Enclosing {
                bindings: &'scope [CallableLifetimeBinding],
            },
            Invocation {
                bindings: &'scope [CallableLifetimeBinding],
            },
        }

        pub fn resolve_callable_lifetime_bindings(
            program: &dyn crate::CheckingInput,
            declaration: SymbolHandle,
            argument_owner: SymbolHandle,
            rows: &[StaticMachineLifetimeBinding],
            preceding: &[CallableLifetimeApplication<'_>],
        ) -> Result<Vec<CallableLifetimeBinding>, &'static str> {
            let owner = program.lifetime_declaration_owner(declaration);
            let mut resolved: Vec<CallableLifetimeBinding> = Vec::with_capacity(rows.len());
            for row in rows {
                let parameter = typed_trees::type_identity::lifetime_parameter_identity(
                    program.typed_program(),
                    declaration,
                    row.parameter.as_str(),
                )
                .filter(|parameter| parameter.declaration == owner && owner.is_valid())
                .ok_or("callable application lost its exact receiving lifetime telescope")?;
                if resolved.iter().any(|prior| prior.parameter == parameter) {
                    return Err("callable application repeats a receiving lifetime parameter");
                }
                let mut argument = typed_trees::type_identity::lifetime_parameter_identity(
                    program.typed_program(),
                    argument_owner,
                    row.argument.as_str(),
                )
                .ok_or("callable application lost its exact supplying lifetime scope")?;
                for application in preceding.iter().rev() {
                    let bindings = match application {
                        CallableLifetimeApplication::Enclosing { bindings }
                        | CallableLifetimeApplication::Invocation { bindings } => *bindings,
                    };
                    if let Some(binding) = bindings
                        .iter()
                        .rev()
                        .find(|binding| binding.parameter == argument)
                    {
                        argument = binding.argument;
                        break;
                    }
                }
                resolved.push(CallableLifetimeBinding {
                    parameter,
                    argument,
                });
            }
            Ok(resolved)
        }
    }
    pub(crate) mod owned_carriers {
        use super::CallableBorrowContract;
        use super::carried_lifetimes::{
            CarriedLifetime, LifetimeFrontier, RegionIdentity, scoped_carried_lifetimes,
        };
        use symbols::SymbolHandle;
        use typed_trees::type_identity::{IndexIdentityArgument, NormalizedTypeIdentity};
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        #[derive(Clone, Copy, PartialEq, Eq)]
        pub enum OwnedCarrierLoanRelation {
            ViewFree,
            Preserved,
            Unproven,
        }

        pub fn owned_carrier_loan_relation(
            program: &dyn crate::CheckingInput,
            callable: SymbolHandle,
            source: TypeReferenceHandle,
            source_applications: &[TypeReferenceHandle],
            result: TypeReferenceHandle,
            result_applications: &[TypeReferenceHandle],
        ) -> OwnedCarrierLoanRelation {
            let Some(source) = applied_frontier(program, callable, source, source_applications)
            else {
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
            let Some(result) = applied_frontier(program, callable, result, result_applications)
            else {
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

        pub(crate) struct AppliedCarrierFrontier {
            identity: NormalizedTypeIdentity,
            loans: Vec<CarriedLifetime>,
            borrowed: bool,
            linear: bool,
        }

        fn applied_frontier(
            program: &dyn crate::CheckingInput,
            callable: SymbolHandle,
            reference: TypeReferenceHandle,
            applications: &[TypeReferenceHandle],
        ) -> Option<AppliedCarrierFrontier> {
            if !reference.is_valid() {
                return None;
            }
            let reference = crate::peel_declared_constraints(program, reference);
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
                    let identity = typed_trees::type_identity::applied_type_identity_in_scope(
                        program.typed_program(),
                        reference,
                        callable,
                        callable,
                        scopes,
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
                            && program.lifetime_declaration_owner(scope.declaration)
                                == operand_owner
                    });
                    let (declaration, lifetimes) = receiving.map_or(
                        (callable, regions.names.as_slice()),
                        |(ordinal, scope)| {
                            (scope.declaration, regions.applications[ordinal].as_slice())
                        },
                    );
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
    pub(crate) mod provider_outputs {
        use super::carried_lifetimes::{self, RegionIdentity};
        use super::{
            BorrowOwnerSegment, StateParameter, TypeReferenceHandle, ViewReturnAmbiguity,
            ViewReturnInputSource,
        };
        use language_semantics::ReferenceAccess;

        #[derive(Debug, Clone)]
        pub struct ProviderLifetimeDependency {
            pub parameter_index: usize,
            pub source_type: TypeReferenceHandle,
            pub source_path: Vec<BorrowOwnerSegment>,
            pub source: ViewReturnInputSource,
            pub access: ReferenceAccess,
        }

        #[derive(Debug, Clone)]
        pub struct ProviderOutput {
            pub destination: ProviderOutputDestination,
            pub owner_path: Vec<BorrowOwnerSegment>,
            pub access: ReferenceAccess,
            pub dependencies: Vec<ProviderLifetimeDependency>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ProviderOutputDestination {
            Parameter(usize),
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum BorrowRelationReadiness {
            Complete,
            TemplateDependent,
        }

        pub struct ProviderOutputRelation {
            pub outputs: Vec<ProviderOutput>,
            pub readiness: BorrowRelationReadiness,
        }

        pub fn resolve_provider_outputs(
            program: &dyn crate::CheckingInput,
            parameters: &[StateParameter],
        ) -> Result<ProviderOutputRelation, ViewReturnAmbiguity> {
            let mut destinations = Vec::new();
            for (parameter_index, parameter) in parameters.iter().enumerate() {
                let Some(shell) = crate::reference_shell(program, parameter.type_reference) else {
                    continue;
                };
                if !shell.access.is_exclusive() {
                    continue;
                }
                destinations.push((
                    ProviderOutputDestination::Parameter(parameter_index),
                    shell.referee,
                    format!("output `{}`", parameter.name),
                ));
            }
            if destinations.is_empty() {
                let readiness =
                    crate::machine_calls::calls::provider_completion_covers_writable_slots(
                        program,
                        parameters,
                        &[],
                    )
                    .ok_or_else(|| {
                        ViewReturnAmbiguity::IncompleteStructure {
                            subject: "provider writable reference frontier".to_owned(),
                        }
                    })?;
                return Ok(ProviderOutputRelation {
                    outputs: Vec::new(),
                    readiness,
                });
            }
            let mut inputs = Vec::new();
            let mut template_dependent = false;
            for (parameter_index, parameter) in parameters.iter().enumerate() {
                let leaves = carried_lifetimes::declaration_lifetime_inventory(
                    program,
                    parameter.type_reference,
                    carried_lifetimes::LifetimeFrontier::Input,
                )
                .ok_or_else(|| ViewReturnAmbiguity::IncompleteStructure {
                    subject: format!("input `{}`", parameter.name),
                })?;
                template_dependent |= leaves.template_dependent;
                for leaf in leaves.leaves.into_iter().filter(|leaf| !leaf.lifetime_only) {
                    inputs.push((parameter_index, parameter, leaf));
                }
            }
            let inputs_dependent = template_dependent;
            let mut outputs = Vec::new();
            for (destination, reference, subject) in destinations {
                let leaves = carried_lifetimes::declaration_lifetime_inventory(
                    program,
                    reference,
                    carried_lifetimes::LifetimeFrontier::BorrowedStorage,
                )
                .ok_or(ViewReturnAmbiguity::IncompleteStructure { subject })?;
                template_dependent |= leaves.template_dependent;
                for leaf in leaves.leaves.into_iter().filter(|leaf| !leaf.lifetime_only) {
                    let matching: Vec<_> = inputs
                        .iter()
                        .filter(|(_, _, input)| {
                            leaf.lifetime
                                .as_ref()
                                .is_none_or(|region| input.lifetime.as_ref() == Some(region))
                        })
                        .collect();
                    if matching.is_empty() && !inputs_dependent {
                        return Err(ViewReturnAmbiguity::LifetimeMatchesNoInput {
                            lifetime: match leaf.lifetime {
                                Some(RegionIdentity::Declared(name)) => name,
                                Some(region) => format!("{region:?}"),
                                None => "elided".to_owned(),
                            },
                        });
                    }
                    if leaf.lifetime.is_none() && matching.len() > 1 {
                        return Err(ViewReturnAmbiguity::ElidedMultipleInputs {
                            candidates: matching
                                .iter()
                                .map(|(_, parameter, input)| {
                                    format!(
                                        "{} carried source {:?}",
                                        parameter.name, input.owner_path
                                    )
                                })
                                .collect(),
                        });
                    }
                    outputs.push(ProviderOutput {
                        destination,
                        owner_path: leaf.owner_path,
                        access: leaf.access,
                        dependencies: matching
                            .into_iter()
                            .map(
                                |(parameter_index, parameter, input)| ProviderLifetimeDependency {
                                    parameter_index: *parameter_index,
                                    source_type: parameter.type_reference,
                                    source_path: input.owner_path.clone(),
                                    source: input.source,
                                    access: input.access,
                                },
                            )
                            .collect(),
                    });
                }
            }
            let writable = crate::machine_calls::calls::provider_completion_covers_writable_slots(
                program, parameters, &outputs,
            )
            .ok_or_else(|| ViewReturnAmbiguity::IncompleteStructure {
                subject: "provider writable reference frontier".to_owned(),
            })?;
            let readiness =
                if template_dependent || writable == BorrowRelationReadiness::TemplateDependent {
                    BorrowRelationReadiness::TemplateDependent
                } else {
                    BorrowRelationReadiness::Complete
                };
            Ok(ProviderOutputRelation { outputs, readiness })
        }
    }
    pub(crate) mod region_correspondence {
        use super::callable_refinement::{CallableBorrowContract, captured_lifetimes};
        use super::callable_regions::CallableRegions;
        use super::carried_lifetimes::{CarriedLifetime, RegionIdentity};
        use super::region_inference::InputRegionApplication;
        use crate::CheckingInput;
        use crate::queries::type_regions::{
            ReferenceStep, TypeRegionRelationship, TypeRegionScope,
            type_reference_region_applications, type_reference_regions_match,
        };
        use symbols::SymbolHandle;
        use typed_trees::name::Identifier;
        use typed_trees::types::TypeReferenceHandle;

        #[derive(Clone, Copy)]
        pub(crate) enum RegionEnvironment<'scope> {
            Resolved(&'scope [(String, RegionIdentity)]),
            Captured {
                owner: SymbolHandle,
                bindings: &'scope [super::callable_regions::AppliedRegionBinding],
            },
            Bound(&'scope RegionIdentity),
            CapturedSlot(typed_trees::type_identity::LifetimeParameterIdentity),
        }

        pub(crate) struct RejectedRegionComparison {
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
            fn identity(
                self,
                program: &dyn CheckingInput,
                name: &Identifier,
            ) -> Option<RegionIdentity> {
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
                                    typed_trees::type_identity::LifetimeParameterIdentity {
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
        pub(crate) struct RegionScope<'scope> {
            declaration: SymbolHandle,
            substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
            starts: &'scope [usize],
            applications: &'scope [Vec<(String, RegionIdentity)>],
            binders: &'scope [(SymbolHandle, String)],
            regions: RegionEnvironment<'scope>,
            arguments: RegionEnvironment<'scope>,
            output_elisions: &'scope [(TypeReferenceHandle, RegionIdentity)],
            captured: &'scope [super::callable_regions::AppliedRegionBinding],
            application_scopes:
                &'scope [typed_trees::type_identity::AppliedTypeIdentityScope<'scope>],
            argument_owner: SymbolHandle,
        }

        impl TypeRegionScope for RegionScope<'_> {
            fn erased_identity(
                self,
                program: &dyn CheckingInput,
                reference: TypeReferenceHandle,
            ) -> typed_trees::type_identity::NormalizedTypeIdentity {
                program.scoped_type_identity(
                    typed_trees::type_identity::TypeIdentityRequest {
                        binders: self.binders,
                        substitutions: self.substitutions,
                        ..typed_trees::type_identity::TypeIdentityRequest::ordinary(reference)
                    },
                    self.starts,
                )
            }

            fn enter_operand(
                mut self,
                program: &dyn CheckingInput,
                owner: symbols::SymbolHandle,
            ) -> Self {
                let declaration = program.lifetime_declaration_owner(self.declaration);
                if declaration.is_valid()
                    && program.lifetime_declaration_owner(owner) == declaration
                {
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
                program: &dyn CheckingInput,
                region: typed_trees::type_identity::LifetimeParameterIdentity,
            ) -> Self {
                if let Some(argument) =
                    super::callable_regions::applied_region(program, self.captured, region)
                {
                    self.regions = RegionEnvironment::Bound(argument);
                    self
                } else if let Some((argument, true)) =
                    typed_trees::type_identity::apply_resolved_lifetime_parameter_identity(
                        program.typed_program(),
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
                _program: &dyn CheckingInput,
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

        pub(crate) fn region_context(
            program: &dyn CheckingInput,
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

        pub(crate) fn infer_inputs(
            program: &dyn CheckingInput,
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
                let mut constrain =
                    |application: &mut InputRegionApplication,
                     actual_scope: RegionScope<'_>,
                     required_scope: RegionScope<'_>,
                     reference,
                     actual: Option<&Identifier>,
                     required: Option<&Identifier>| {
                        let Some(required) = required else {
                            return true;
                        };
                        let Some(required) = required_scope.regions.identity(program, required)
                        else {
                            return false;
                        };
                        let actual = match actual {
                            Some(name) => {
                                let Some(region) = actual_scope.regions.identity(program, name)
                                else {
                                    return false;
                                };
                                Some(region)
                            }
                            None => {
                                if !actual_inputs.get(ordinal).is_some_and(|leaves| {
                                    leaves.iter().any(|leaf| {
                                        leaf.reference == reference && leaf.lifetime.is_none()
                                    })
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
                        if application.constrain(
                            ordinal,
                            reference,
                            actual.clone(),
                            required.clone(),
                        ) {
                            return true;
                        }
                        failure = if matches!(actual, Some(RegionIdentity::Captured(_, _)))
                            && matches!(required, RegionIdentity::Fresh(_))
                        {
                            "fresh invocation lifetime telescope is not universally refined"
                        } else if actual_inputs.get(ordinal).is_some_and(|leaves| {
                            leaves.iter().any(|leaf| leaf.reference == reference)
                        }) && (matches!(
                            instantiated,
                            Some(RegionIdentity::Captured(_, _))
                        ) || matches!(required, RegionIdentity::Captured(_, _)))
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

        pub(crate) fn validate(
            program: &dyn CheckingInput,
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
    pub(crate) mod region_inference {
        use super::carried_lifetimes::RegionIdentity;
        use typed_trees::types::TypeReferenceHandle;

        #[derive(Clone, Debug, Default)]
        pub(crate) struct InputRegionApplication {
            fresh: Vec<(usize, RegionIdentity)>,
            pub(crate) elided: Vec<(usize, TypeReferenceHandle, RegionIdentity)>,
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
            pub(crate) fn constrain(
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

            pub(crate) fn finish(
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
                    if let Some((_, region)) =
                        self.fresh.iter().find(|(ordinal, _)| *ordinal == source)
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

            pub(crate) fn apply(&self, region: &mut RegionIdentity) {
                if let RegionIdentity::Fresh(ordinal) = region
                    && let Some((_, target)) =
                        self.fresh.iter().find(|(source, _)| source == ordinal)
                {
                    *region = target.clone();
                }
            }
        }
    }

    pub use provider_outputs::{
        BorrowRelationReadiness, ProviderLifetimeDependency, ProviderOutput,
        ProviderOutputDestination, ProviderOutputRelation, resolve_provider_outputs,
    };

    pub(crate) use callable_refinement::lexical_region_owner;
    pub use callable_refinement::{
        CallableBorrowContract, captured_type_parameters, validate_callable_borrow_refinement,
    };
    pub use owned_carriers::{OwnedCarrierLoanRelation, owned_carrier_loan_relation};

    pub(crate) use callable_refinement::PreparedCallableBorrowRefinement;
    pub(crate) use lifetime_applications::CallableLifetimeFrame;
    pub use lifetime_applications::{
        CallableLifetimeApplication, CallableLifetimeBinding, resolve_callable_lifetime_bindings,
    };

    pub fn resolve_applied_signature_view_return_source(
        program: &dyn CheckingInput,
        signature: &typed_trees::signature::StateSignature,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        scope_starts: &[usize],
        argument_owner: SymbolHandle,
    ) -> ViewReturnSource {
        if let Some(relation) = installed_context_lifetime_source(program, signature.symbol) {
            return relation;
        }
        if let Some(custody) = program
            .retained_borrow_custodies()
            .iter()
            .find(|custody| custody.callable == signature.symbol)
            && let language_semantics::content::ContentPlaceRoot::Parameter { position, .. } =
                &custody.source.root
            && custody.source.segments.is_empty()
            && custody.result.segments.is_empty()
            && let Some(parameter) = program
                .state_signature_parameters(signature)
                .get(*position as usize)
        {
            return ViewReturnSource::Fields {
                fields: vec![ViewReturnFieldSource {
                    lifetime_only: true,
                    owner_path: Vec::new(),
                    source_path: Vec::new(),
                    source: ViewReturnInputSource::Referent,
                    source_type: parameter.type_reference,
                    source_reference_hops: 0,
                    parameter_index: *position as usize,
                    kind: BorrowAccessKind::from(custody.access),
                }],
            };
        }
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
        program: &dyn CheckingInput,
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
                return ViewReturnSource::Ambiguous(
                    ViewReturnAmbiguity::IncompatibleSourceAccess {
                        input: first.1.name.as_str().to_owned(),
                    },
                );
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
                        language_semantics::ReferenceAccess::WriteOnly => {
                            BorrowAccessKind::WriteOnly
                        }
                    },
                });
            }
        }
        ViewReturnSource::Fields { fields }
    }

    fn whole_elided_result_source(
        program: &dyn CheckingInput,
        parameters: &[StateParameter],
        return_type: TypeReferenceHandle,
    ) -> Option<ViewReturnSource> {
        let output_accesses =
            carried_lifetimes::whole_elided_result_accesses(program, return_type)?;
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

    pub fn is_reference_type(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { .. } => true,
            TypeReferenceNode::Constrained { base_type, .. } => {
                is_reference_type(program, *base_type)
            }
            _ => false,
        }
    }

    pub fn returns_borrow(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        is_reference_type(program, type_reference)
            || is_borrow_carrying_data(program, type_reference)
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum InputWriteOnlyAccess {
        Absent,
        Present,
        TemplateDependent,
        Incomplete,
    }

    pub(crate) fn input_write_only_access(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
        parameters: &[typed_trees::data::TypeParameter],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> InputWriteOnlyAccess {
        carried_lifetimes::input_write_only_access(
            program,
            type_reference,
            parameters,
            substitutions,
        )
    }

    pub fn borrow_carrying_owner_paths(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<Vec<Vec<BorrowOwnerSegment>>> {
        carried_lifetimes(program, type_reference)
            .map(|leaves| leaves.into_iter().map(|leaf| leaf.owner_path).collect())
    }

    pub fn carried_reference_paths(
        program: &dyn CheckingInput,
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

    pub fn structural_owner_path(
        program: &dyn CheckingInput,
        path: &[BorrowOwnerSegment],
    ) -> Option<Vec<checked_trees::CheckedUnitStructuralPathSegment>> {
        use checked_trees::CheckedUnitStructuralPathSegment as Segment;
        path.iter()
            .map(|segment| match segment {
                BorrowOwnerSegment::Field(symbol) => Some(Segment::Field(
                    program.any_field_owner(*symbol)?.1.member_identity(),
                )),
                BorrowOwnerSegment::Case(symbol) => Some(Segment::Case(
                    program.variant_owner(*symbol)?.1.member_identity(),
                )),
                BorrowOwnerSegment::FixedIndex(position) => {
                    Some(Segment::FixedIndex(u64::try_from(*position).ok()?))
                }
                BorrowOwnerSegment::DynamicIndex => None,
            })
            .collect()
    }

    pub fn is_borrow_carrying_data(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        carried_lifetimes(program, type_reference).is_none_or(|leaves| !leaves.is_empty())
    }

    pub fn is_mutably_borrow_carrying_data(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        carried_lifetimes(program, type_reference)
            .is_none_or(|leaves| leaves.iter().any(|leaf| leaf.access.is_exclusive()))
    }

    fn data_definition(
        program: &dyn CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&DataDefinition> {
        program.data_definition_by_symbol(symbol)
    }

    pub fn retained_lifetime_owner_paths(
        program: &dyn CheckingInput,
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

    pub fn installed_context_lifetime_source(
        program: &dyn CheckingInput,
        target: SymbolHandle,
    ) -> Option<ViewReturnSource> {
        let context = match crate::compiler_operations::static_interrupt_context(program, target) {
            Ok(Some(context)) => context,
            Ok(None) => return None,
            Err(reason) => {
                return Some(ViewReturnSource::Ambiguous(
                    ViewReturnAmbiguity::IncompleteStructure {
                        subject: reason.to_owned(),
                    },
                ));
            }
        };
        Some(ViewReturnSource::Fields {
            fields: vec![ViewReturnFieldSource {
                lifetime_only: true,
                owner_path: Vec::new(),
                source_path: Vec::new(),
                source: ViewReturnInputSource::Referent,
                source_type: context.acknowledgement,
                source_reference_hops: 0,
                parameter_index: 0,
                kind: BorrowAccessKind::Read,
            }],
        })
    }
}

pub mod reflection {
    pub(crate) mod fingerprint {
        pub(crate) const OFFSET_BASIS: u64 = semantic_vocabulary::Fnv1a::OFFSET_BASIS;

        pub(crate) fn byte(hash: &mut u64, value: u8) {
            *hash ^= u64::from(value);
            *hash = hash.wrapping_mul(semantic_vocabulary::Fnv1a::PRIME);
        }
        pub(crate) fn bytes(hash: &mut u64, value: &[u8]) {
            for value in value {
                byte(hash, *value);
            }
        }
        pub(crate) fn uint(hash: &mut u64, value: u64) {
            bytes(hash, &value.to_le_bytes());
        }
        pub(crate) fn text(hash: &mut u64, value: &str) {
            uint(hash, value.len() as u64);
            bytes(hash, value.as_bytes());
        }
        pub(crate) fn optional_text(hash: &mut u64, value: &Option<String>) {
            match value {
                Some(value) => {
                    byte(hash, 1);
                    text(hash, value);
                }
                None => byte(hash, 0),
            }
        }
        pub(crate) fn optional_uint(hash: &mut u64, value: Option<u64>) {
            match value {
                Some(value) => {
                    byte(hash, 1);
                    uint(hash, value);
                }
                None => byte(hash, 0),
            }
        }

        pub(crate) fn query_authority(hash: &mut u64, authority: &super::SchemaQueryAuthority) {
            match authority {
                super::SchemaQueryAuthority::OwningScope { requester_identity } => {
                    byte(hash, 0);
                    text(hash, requester_identity);
                }
                super::SchemaQueryAuthority::ForeignScope { requester_identity } => {
                    byte(hash, 1);
                    text(hash, requester_identity);
                }
            }
        }
    }
    pub(crate) mod schema_graph {
        pub(crate) mod graph_construction {
            use crate::reflection::schema_graph::graph_replay::graph_revision;
            use crate::reflection::schema_graph::{
                CaseDescription, DeclarationDescription, FieldDescription,
                NominalReferenceDescription, SchemaNode, SchemaNodeHandle, SchemaQueryAuthority,
                SchemaShape, SemanticSchemaGraph, TypeParameterDescription,
            };
            use symbols::{SymbolHandle, SymbolKind};
            use typed_trees::TypedTrees;
            use typed_trees::data::{
                DataDefinition, DataField, DataMember, DataShapeKind, TypeParameter,
                TypeParameterKind,
            };
            use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

            pub(crate) fn exact_symbol_identity(
                typed: &TypedTrees,
                symbol: SymbolHandle,
            ) -> Result<(String, bool), String> {
                if !symbol.is_valid() {
                    return Err("a schema identity requires a resolved declaration".to_owned());
                }
                if let Ok(identity) = typed.normalized_hermetic_symbol_identity(symbol) {
                    return Ok((identity, true));
                }
                let path = typed.symbols.display_path(symbol, "::");
                if path.is_empty() {
                    return Err("a schema identity requires a named declaration".to_owned());
                }
                Ok((path, false))
            }

            #[derive(Debug, Clone, PartialEq)]
            pub(crate) struct NominalExpectation {
                pub(crate) owner_identity: String,
                pub(crate) owner_kind: String,
                pub(crate) name: String,
                pub(crate) application_identity: String,
                pub(crate) is_self_edge: bool,
            }

            fn nominal_expectations(
                typed: &TypedTrees,
                type_reference: TypeReferenceHandle,
                root_owner_identity: &str,
                root_application_identity: Option<&str>,
            ) -> Result<Vec<NominalExpectation>, String> {
                let mut found = Vec::new();
                collect_nominal_expectations(
                    typed,
                    type_reference,
                    root_owner_identity,
                    root_application_identity,
                    &mut found,
                )?;
                Ok(found)
            }

            fn collect_nominal_expectations(
                typed: &TypedTrees,
                type_reference: TypeReferenceHandle,
                root_owner_identity: &str,
                root_application_identity: Option<&str>,
                found: &mut Vec<NominalExpectation>,
            ) -> Result<(), String> {
                let mut intern = |symbol: SymbolHandle,
                                  name: &str,
                                  application: TypeReferenceHandle,
                                  bare_named: bool|
                 -> Result<(), String> {
                    let (owner_identity, _) =
                        exact_symbol_identity(typed, symbol).map_err(|message| {
                            format!(
                                "{message}: nominal reference `{name}` in `{}`",
                                typed.package_qualified_type_identity_text(application)
                            )
                        })?;
                    let application_identity =
                        typed.package_qualified_type_identity_text(application);
                    let is_self_edge = owner_identity == root_owner_identity
                        && match root_application_identity {
                            Some(root_application) => application_identity == root_application,
                            None => bare_named,
                        };
                    found.push(NominalExpectation {
                        owner_identity,
                        owner_kind: format!("{:?}", typed.symbols.get(symbol).kind),
                        name: name.to_owned(),
                        application_identity,
                        is_self_edge,
                    });
                    Ok(())
                };

                match typed.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Named { symbol, name } => {
                        intern(*symbol, name.as_str(), type_reference, true)?;
                    }
                    TypeReferenceNode::Generic {
                        base_symbol,
                        base_name,
                        arguments,
                        ..
                    } => {
                        intern(*base_symbol, base_name.as_str(), type_reference, false)?;
                        let parameters = typed.declaration_type_parameters(*base_symbol);
                        for (index, argument) in typed
                            .type_reference_table
                            .type_reference_handles(*arguments)
                            .iter()
                            .enumerate()
                        {
                            if parameters
                                .get(index)
                                .is_some_and(|parameter| parameter.kind.is_const_or_value())
                            {
                                continue;
                            }
                            collect_nominal_expectations(
                                typed,
                                *argument,
                                root_owner_identity,
                                root_application_identity,
                                found,
                            )?;
                        }
                    }
                    TypeReferenceNode::DynamicTrait { symbol, name, .. } => {
                        intern(*symbol, name.as_str(), type_reference, false)?;
                    }
                    TypeReferenceNode::Constrained { base_type, .. } => {
                        collect_nominal_expectations(
                            typed,
                            *base_type,
                            root_owner_identity,
                            root_application_identity,
                            found,
                        )?;
                    }
                    TypeReferenceNode::Reference { referee, .. } => {
                        collect_nominal_expectations(
                            typed,
                            *referee,
                            root_owner_identity,
                            root_application_identity,
                            found,
                        )?;
                    }
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => {
                        collect_nominal_expectations(
                            typed,
                            *element_type,
                            root_owner_identity,
                            root_application_identity,
                            found,
                        )?;
                    }
                    TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {}
                }
                Ok(())
            }

            pub(crate) fn describe_field(
                typed: &TypedTrees,
                field: &DataField,
                owner_identity: &str,
                declaration_position: u32,
                owner_case: SchemaNodeHandle,
                root_application_identity: Option<&str>,
            ) -> Result<(FieldDescription, Vec<NominalExpectation>), String> {
                let member_identity = if field.symbol.is_valid() {
                    exact_symbol_identity(typed, field.symbol)?.0
                } else {
                    format!("{owner_identity}::{}", field.name.as_str())
                };
                let expectations = nominal_expectations(
                    typed,
                    field.type_reference,
                    owner_identity,
                    root_application_identity,
                )?;
                Ok((
                    FieldDescription {
                        member_identity,
                        name: field.name.as_str().to_owned(),
                        declaration_position,
                        stable_number: field.identity,
                        erased: field.relevance.is_erased(),
                        qualified_type: typed
                            .package_qualified_type_identity_text(field.type_reference),
                        nominal_references: Vec::new(),
                        owner_case,
                    },
                    expectations,
                ))
            }

            pub(crate) fn describe_type_parameter(
                typed: &TypedTrees,
                parameter: &TypeParameter,
            ) -> TypeParameterDescription {
                let (kind, type_reference) = match &parameter.kind {
                    TypeParameterKind::Type => ("type", None),
                    TypeParameterKind::Const { type_reference } => ("const", Some(*type_reference)),
                    TypeParameterKind::Value { type_reference } => ("value", Some(*type_reference)),
                    TypeParameterKind::Machine { .. } => ("machine", None),
                    TypeParameterKind::Implementation => ("implementation", None),
                    TypeParameterKind::Proposition { .. } => ("proposition", None),
                };
                TypeParameterDescription {
                    name: parameter.name.as_str().to_owned(),
                    kind,
                    type_identity: type_reference
                        .map(|reference| typed.package_qualified_type_identity_text(reference)),
                    bounds: format!("{:?}", parameter.bounds),
                }
            }

            pub(crate) fn resolve_subject<'program>(
                typed: &'program TypedTrees,
                owner_identity: &str,
            ) -> Result<&'program DataDefinition, String> {
                let mut matches = typed.data_definitions().iter().filter(|data| {
                    exact_symbol_identity(typed, data.symbol)
                        .is_ok_and(|(identity, _)| identity == owner_identity)
                });
                let Some(data) = matches.next() else {
                    return Err(format!(
                        "schema owner `{owner_identity}` does not resolve to a data declaration in this program"
                    ));
                };
                if matches.next().is_some() {
                    return Err(format!(
                        "schema owner `{owner_identity}` resolves to more than one data declaration"
                    ));
                }
                Ok(data)
            }

            pub fn construct_semantic_schema_graph(
                typed: &TypedTrees,
                subject: SymbolHandle,
                authority: SchemaQueryAuthority,
            ) -> Result<SemanticSchemaGraph, String> {
                if !subject.is_valid() || typed.symbols.get(subject).kind != SymbolKind::Data {
                    return Err(
            "a semantic schema query selects an exact data declaration, not another symbol kind"
                .to_owned(),
        );
                }
                let data = typed.data_definition_by_symbol(subject).ok_or_else(|| {
                    format!(
                        "schema subject `{}` has no retained data definition",
                        typed.symbols.display_path(subject, "::")
                    )
                })?;
                if data.quotient.is_some() {
                    return Err(format!(
                        "schema reflection cannot observe quotient `{}`: a quotient does not expose its representative outside its checked quotient contract",
                        data.name.as_str()
                    ));
                }
                let (owner_identity, identity_is_hermetic) = exact_symbol_identity(typed, subject)?;
                if let SchemaQueryAuthority::ForeignScope { .. } = &authority {
                    let visible = typed_trees::visibility::declaration_visibility(typed, subject)
                        .is_some_and(|visibility| visibility.is_public());
                    if !visible {
                        return Err(format!(
                            "schema subject `{}` is not public to a foreign package; a query may not hide inaccessible members while claiming complete coverage",
                            data.name.as_str()
                        ));
                    }
                }
                let application_identity = data
                    .generic_instance
                    .map(|instance| typed.package_qualified_type_identity_text(instance));

                let mut nodes = vec![SchemaNode::Invalid, SchemaNode::Invalid];
                let root = SchemaNodeHandle(1);
                let mut interned: Vec<(String, String, SchemaNodeHandle)> = Vec::new();
                let mut member_handles = Vec::new();
                for (position, member) in typed.data_members(data).iter().enumerate() {
                    match member {
                        DataMember::Field(field) => {
                            let handle = push_field_node(
                                typed,
                                field,
                                &owner_identity,
                                position as u32,
                                SchemaNodeHandle::INVALID,
                                application_identity.as_deref(),
                                root,
                                &mut interned,
                                &mut nodes,
                            )?;
                            member_handles.push(handle);
                        }
                        DataMember::Variant(variant) => {
                            let case_handle = nodes.len() as u32;
                            nodes.push(SchemaNode::Invalid);
                            let case_handle = SchemaNodeHandle(case_handle);
                            let mut payload_handles = Vec::new();
                            for (payload_position, payload_field) in
                                typed.data_payload_fields(variant).iter().enumerate()
                            {
                                payload_handles.push(push_field_node(
                                    typed,
                                    payload_field,
                                    &owner_identity,
                                    payload_position as u32,
                                    case_handle,
                                    application_identity.as_deref(),
                                    root,
                                    &mut interned,
                                    &mut nodes,
                                )?);
                            }
                            let (member_identity, _) = exact_symbol_identity(typed, variant.symbol)
                                .unwrap_or_else(|_| {
                                    (
                                        format!("{owner_identity}::{}", variant.name.as_str()),
                                        false,
                                    )
                                });
                            nodes[case_handle.index()] = SchemaNode::Case(CaseDescription {
                                member_identity,
                                name: variant.name.as_str().to_owned(),
                                declaration_position: position as u32,
                                stable_number: variant.identity,
                                payload_fields: payload_handles,
                                retired_payload_identities: sorted(
                                    variant.retired_payload_identities.clone(),
                                ),
                            });
                            member_handles.push(case_handle);
                        }
                    }
                }

                nodes[root.index()] = SchemaNode::Declaration(DeclarationDescription {
                    owner_identity,
                    identity_is_hermetic,
                    name: data.name.as_str().to_owned(),
                    shape: match DataDefinition::shape_kind_from_members(typed.data_members(data)) {
                        DataShapeKind::Empty => SchemaShape::Empty,
                        DataShapeKind::Record => SchemaShape::Record,
                        DataShapeKind::Enum => SchemaShape::Sum,
                        DataShapeKind::Mixed => SchemaShape::Mixed,
                    },
                    supply: format!("{:?}", data.supply_mode),
                    multiplicity: format!("{:?}", data.properties.multiplicity),
                    carry: data.properties.carry.map(|carry| format!("{carry:?}")),
                    type_parameters: typed
                        .data_type_parameters(data)
                        .iter()
                        .map(|parameter| describe_type_parameter(typed, parameter))
                        .collect(),
                    lifetime_parameters: data
                        .lifetime_parameters
                        .iter()
                        .map(|name| name.as_str().to_owned())
                        .collect(),
                    application_identity,
                    members: member_handles,
                    retired_identities: sorted(data.retired_identities.clone()),
                });
                let revision = graph_revision(&authority, &nodes);
                Ok(SemanticSchemaGraph {
                    nodes,
                    root,
                    authority,
                    revision,
                })
            }

            #[allow(clippy::too_many_arguments)]
            fn push_field_node(
                typed: &TypedTrees,
                field: &DataField,
                owner_identity: &str,
                declaration_position: u32,
                owner_case: SchemaNodeHandle,
                root_application_identity: Option<&str>,
                root: SchemaNodeHandle,
                interned: &mut Vec<(String, String, SchemaNodeHandle)>,
                nodes: &mut Vec<SchemaNode>,
            ) -> Result<SchemaNodeHandle, String> {
                let (mut description, expectations) = describe_field(
                    typed,
                    field,
                    owner_identity,
                    declaration_position,
                    owner_case,
                    root_application_identity,
                )?;
                description.nominal_references = expectations
                    .iter()
                    .map(|expectation| {
                        if expectation.is_self_edge {
                            return Ok(root);
                        }
                        if let Some((_, _, handle)) =
                            interned.iter().find(|(owner, application, _)| {
                                owner == &expectation.owner_identity
                                    && application == &expectation.application_identity
                            })
                        {
                            return Ok(*handle);
                        }
                        let handle = SchemaNodeHandle(nodes.len() as u32);
                        nodes.push(SchemaNode::NominalReference(NominalReferenceDescription {
                            owner_identity: expectation.owner_identity.clone(),
                            owner_kind: expectation.owner_kind.clone(),
                            name: expectation.name.clone(),
                            application_identity: expectation.application_identity.clone(),
                        }));
                        interned.push((
                            expectation.owner_identity.clone(),
                            expectation.application_identity.clone(),
                            handle,
                        ));
                        Ok(handle)
                    })
                    .collect::<Result<_, String>>()?;
                let handle = SchemaNodeHandle(nodes.len() as u32);
                nodes.push(SchemaNode::Field(description));
                Ok(handle)
            }

            pub(crate) fn sorted(mut identities: Vec<u64>) -> Vec<u64> {
                identities.sort_unstable();
                identities
            }
        }
        pub(crate) mod graph_replay {
            use crate::reflection::extend_unique;
            use crate::reflection::schema_graph::graph_construction::{
                NominalExpectation, describe_field, describe_type_parameter, exact_symbol_identity,
                resolve_subject, sorted,
            };
            use crate::reflection::schema_graph::{
                CaseDescription, DeclarationDescription, FieldDescription, SchemaNode,
                SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, SemanticSchemaGraph,
            };
            use symbols::SymbolHandle;
            use typed_trees::TypedTrees;
            use typed_trees::data::{DataDefinition, DataField, DataMember, DataShapeKind};

            pub fn replay_semantic_schema_graph(
                typed: &TypedTrees,
                graph: &SemanticSchemaGraph,
            ) -> Result<(), String> {
                let SchemaNode::Declaration(root) = graph.node(graph.root) else {
                    return Err("schema graph root is not a declaration node".to_owned());
                };
                let data = resolve_subject(typed, &root.owner_identity)?;

                check_authority_claim(typed, data, &graph.authority, "schema", &root.name)?;
                check_subject_application(
                    typed,
                    data,
                    &root.application_identity,
                    "schema",
                    &root.name,
                )?;

                let (_, expected_hermetic) = exact_symbol_identity(typed, data.symbol)?;
                if root.identity_is_hermetic != expected_hermetic || root.name != data.name.as_str()
                {
                    return Err(format!(
                        "schema `{}` name or identity strength does not match the bound declaration",
                        root.name
                    ));
                }

                let expected_shape =
                    match DataDefinition::shape_kind_from_members(typed.data_members(data)) {
                        DataShapeKind::Empty => SchemaShape::Empty,
                        DataShapeKind::Record => SchemaShape::Record,
                        DataShapeKind::Enum => SchemaShape::Sum,
                        DataShapeKind::Mixed => SchemaShape::Mixed,
                    };
                if root.shape != expected_shape {
                    return Err(format!(
                        "schema `{}` claims shape `{:?}` but the bound declaration is `{:?}`",
                        root.name, root.shape, expected_shape
                    ));
                }
                if root.supply != format!("{:?}", data.supply_mode)
                    || root.multiplicity != format!("{:?}", data.properties.multiplicity)
                    || root.carry != data.properties.carry.map(|carry| format!("{carry:?}"))
                {
                    return Err(format!(
                        "schema `{}` does not match the bound declaration's construction qualifications",
                        root.name
                    ));
                }
                let expected_parameters: Vec<_> = typed
                    .data_type_parameters(data)
                    .iter()
                    .map(|parameter| describe_type_parameter(typed, parameter))
                    .collect();
                if root.type_parameters != expected_parameters {
                    return Err(format!(
                        "schema `{}` type parameters do not match the bound declaration",
                        root.name
                    ));
                }
                let expected_lifetimes: Vec<_> = data
                    .lifetime_parameters
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect();
                if root.lifetime_parameters != expected_lifetimes {
                    return Err(format!(
                        "schema `{}` lifetime parameters do not match the bound declaration",
                        root.name
                    ));
                }
                if root.retired_identities != sorted(data.retired_identities.clone()) {
                    return Err(format!(
                        "schema `{}` retired identities do not match the bound declaration's tombstones",
                        root.name
                    ));
                }

                let members = typed.data_members(data);
                if root.members.len() != members.len() {
                    return Err(format!(
                        "schema `{}` describes {} top-level members but the bound declaration has {}",
                        root.name,
                        root.members.len(),
                        members.len()
                    ));
                }
                let mut expected_nominals: Vec<(String, String)> = Vec::new();
                for (position, (member, handle)) in
                    members.iter().zip(root.members.iter()).enumerate()
                {
                    if !handle.is_valid() {
                        return Err(format!(
                            "schema `{}` member {position} carries an invalid node handle",
                            root.name
                        ));
                    }
                    match member {
                        DataMember::Field(field) => {
                            let (mut expected_field, expectations) = expect_field_node(
                                typed,
                                field,
                                &root.owner_identity,
                                position as u32,
                                SchemaNodeHandle::INVALID,
                                root.application_identity.as_deref(),
                            )?;
                            let SchemaNode::Field(stored) = graph.node(*handle) else {
                                return Err(format!(
                                    "schema `{}` member {position} `{}` is not a field node",
                                    root.name, field.name
                                ));
                            };
                            expected_field.nominal_references = stored.nominal_references.clone();
                            if *stored != expected_field {
                                return Err(format!(
                                    "schema `{}` member `{name}` does not match its bound field",
                                    root.name,
                                    name = field.name
                                ));
                            }
                            check_nominal_edges(
                                graph,
                                root,
                                stored,
                                &expectations,
                                &mut expected_nominals,
                            )?;
                        }
                        DataMember::Variant(variant) => {
                            let SchemaNode::Case(stored_case) = graph.node(*handle) else {
                                return Err(format!(
                                    "schema `{}` member {position} `{}` is not a case node",
                                    root.name, variant.name
                                ));
                            };
                            let expected_case = CaseDescription {
                                member_identity: member_identity_or_name(
                                    typed,
                                    variant.symbol,
                                    &root.owner_identity,
                                    variant.name.as_str(),
                                ),
                                name: variant.name.as_str().to_owned(),
                                declaration_position: position as u32,
                                stable_number: variant.identity,
                                payload_fields: stored_case.payload_fields.clone(),
                                retired_payload_identities: sorted(
                                    variant.retired_payload_identities.clone(),
                                ),
                            };
                            if *stored_case != expected_case {
                                return Err(format!(
                                    "schema `{}` case `{}` does not match its bound variant",
                                    root.name, variant.name
                                ));
                            }
                            let payload = typed.data_payload_fields(variant);
                            if stored_case.payload_fields.len() != payload.len() {
                                return Err(format!(
                                    "schema `{}` case `{}` describes {} payload members but the bound case has {}",
                                    root.name,
                                    variant.name,
                                    stored_case.payload_fields.len(),
                                    payload.len()
                                ));
                            }
                            for (payload_position, (payload_field, payload_handle)) in payload
                                .iter()
                                .zip(stored_case.payload_fields.iter())
                                .enumerate()
                            {
                                if !payload_handle.is_valid() {
                                    return Err(format!(
                                        "schema `{}` case `{}` payload {payload_position} carries an invalid node handle",
                                        root.name, variant.name
                                    ));
                                }
                                let (mut expected_field, expectations) = expect_field_node(
                                    typed,
                                    payload_field,
                                    &root.owner_identity,
                                    payload_position as u32,
                                    *handle,
                                    root.application_identity.as_deref(),
                                )?;
                                let SchemaNode::Field(stored_field) = graph.node(*payload_handle)
                                else {
                                    return Err(format!(
                                        "schema `{}` case `{}` payload {payload_position} is not a field node",
                                        root.name, variant.name
                                    ));
                                };
                                expected_field.nominal_references =
                                    stored_field.nominal_references.clone();
                                if *stored_field != expected_field {
                                    return Err(format!(
                                        "schema `{}` case `{}` payload member `{}` does not match its bound field",
                                        root.name, variant.name, payload_field.name
                                    ));
                                }
                                check_nominal_edges(
                                    graph,
                                    root,
                                    stored_field,
                                    &expectations,
                                    &mut expected_nominals,
                                )?;
                            }
                        }
                    }
                }

                let mut reachable = vec![false; graph.nodes.len()];
                reachable[graph.root.index()] = true;
                for handle in &root.members {
                    reachable[handle.index()] = true;
                    if let SchemaNode::Case(case) = graph.node(*handle) {
                        for payload in &case.payload_fields {
                            reachable[payload.index()] = true;
                        }
                    }
                }
                let mut referenced_nominals = vec![false; graph.nodes.len()];
                for node in graph.nodes.iter() {
                    if let SchemaNode::Field(field) = node {
                        for reference in &field.nominal_references {
                            if !reference.is_valid() || reference.index() >= graph.nodes.len() {
                                return Err(format!(
                                    "schema `{}` field `{}` references a node outside the owned graph",
                                    root.name, field.name
                                ));
                            }
                            referenced_nominals[reference.index()] = true;
                        }
                    }
                }
                for (index, _) in graph.nodes.iter().enumerate() {
                    if index > 1 && !reachable[index] && !referenced_nominals[index] {
                        return Err(format!(
                            "schema `{}` contains an orphan member node `{index}` reachable through no member list",
                            root.name
                        ));
                    }
                }
                let mut stored_nominals: Vec<(String, String)> = Vec::new();
                for (index, node) in graph.nodes.iter().enumerate() {
                    let SchemaNode::NominalReference(reference) = node else {
                        continue;
                    };
                    if !referenced_nominals[index] {
                        return Err(format!(
                            "schema `{}` nominal reference `{}` is not referenced by any member",
                            root.name, reference.name
                        ));
                    }
                    let key = (
                        reference.owner_identity.clone(),
                        reference.application_identity.clone(),
                    );
                    if stored_nominals.contains(&key) {
                        return Err(format!(
                            "schema `{}` duplicates nominal reference `{}`",
                            root.name, reference.name
                        ));
                    }
                    stored_nominals.push(key);
                }
                let mut expected_unique: Vec<(String, String)> = Vec::new();
                extend_unique(&mut expected_unique, expected_nominals);
                if stored_nominals != expected_unique {
                    return Err(format!(
                        "schema `{}` nominal references do not match the bound member types",
                        root.name
                    ));
                }

                if graph.revision != graph_revision(&graph.authority, &graph.nodes) {
                    return Err(format!(
                        "schema `{}` report fingerprint does not match its frozen contents",
                        root.name
                    ));
                }
                Ok(())
            }

            pub(crate) fn check_authority_claim(
                typed: &TypedTrees,
                data: &DataDefinition,
                authority: &SchemaQueryAuthority,
                artifact: &str,
                subject_name: &str,
            ) -> Result<(), String> {
                match authority {
                    SchemaQueryAuthority::ForeignScope { .. } => {
                        let visible =
                            typed_trees::visibility::declaration_visibility(typed, data.symbol)
                                .is_some_and(|visibility| visibility.is_public());
                        if !visible {
                            return Err(format!(
                                "{artifact} `{subject_name}` claims a foreign-scope query over a non-public subject; complete structural visibility is impossible",
                            ));
                        }
                    }
                    SchemaQueryAuthority::OwningScope { requester_identity } => {
                        if let Some(requester) = resolve_requester(typed, requester_identity)?
                            && !typed
                                .symbols
                                .same_symbol_source_package(requester, data.symbol)
                        {
                            return Err(format!(
                                "{artifact} `{subject_name}` claims an owning scope but its requester is outside the subject's package",
                            ));
                        }
                    }
                }
                Ok(())
            }

            pub(crate) fn check_subject_application(
                typed: &TypedTrees,
                data: &DataDefinition,
                recorded: &Option<String>,
                artifact: &str,
                subject_name: &str,
            ) -> Result<(), String> {
                if let Some(application_identity) = recorded {
                    let expected = data
            .generic_instance
            .map(|instance| {
                typed
                    .package_qualified_type_identity_text(instance)
            })
            .ok_or_else(|| {
                format!(
                    "{artifact} `{subject_name}` claims application `{application_identity}` but the bound declaration is not a generated instance",
                )
            })?;
                    if *application_identity != expected {
                        return Err(format!(
                            "{artifact} `{subject_name}` application `{application_identity}` does not match the bound instance `{expected}`",
                        ));
                    }
                } else if data.generic_instance.is_some() {
                    return Err(format!(
                        "{artifact} `{subject_name}` records no application for a generated instance",
                    ));
                }
                Ok(())
            }

            pub(crate) fn member_identity_or_name(
                typed: &TypedTrees,
                symbol: SymbolHandle,
                owner_identity: &str,
                name: &str,
            ) -> String {
                if symbol.is_valid() {
                    exact_symbol_identity(typed, symbol)
                        .map(|(identity, _)| identity)
                        .unwrap_or_else(|_| format!("{owner_identity}::{name}"))
                } else {
                    format!("{owner_identity}::{name}")
                }
            }

            fn expect_field_node(
                typed: &TypedTrees,
                field: &DataField,
                owner_identity: &str,
                declaration_position: u32,
                owner_case: SchemaNodeHandle,
                root_application_identity: Option<&str>,
            ) -> Result<(FieldDescription, Vec<NominalExpectation>), String> {
                let (mut description, expectations) = describe_field(
                    typed,
                    field,
                    owner_identity,
                    declaration_position,
                    owner_case,
                    root_application_identity,
                )?;
                description.nominal_references.clear();
                Ok((description, expectations))
            }

            fn check_nominal_edges(
                graph: &SemanticSchemaGraph,
                root: &DeclarationDescription,
                stored: &FieldDescription,
                expectations: &[NominalExpectation],
                expected_nominals: &mut Vec<(String, String)>,
            ) -> Result<(), String> {
                if stored.nominal_references.len() != expectations.len() {
                    return Err(format!(
                        "schema `{}` field `{}` records {} nominal edges but its bound type has {}",
                        root.name,
                        stored.name,
                        stored.nominal_references.len(),
                        expectations.len()
                    ));
                }
                for (handle, expectation) in
                    stored.nominal_references.iter().zip(expectations.iter())
                {
                    if expectation.is_self_edge {
                        if *handle != graph.root {
                            return Err(format!(
                                "schema `{}` field `{}` self-references the selected application through a non-root handle",
                                root.name, stored.name
                            ));
                        }
                        continue;
                    }
                    expected_nominals.push((
                        expectation.owner_identity.clone(),
                        expectation.application_identity.clone(),
                    ));
                    let SchemaNode::NominalReference(reference) = graph.node(*handle) else {
                        return Err(format!(
                            "schema `{}` field `{}` references `{}` through a non-nominal node",
                            root.name, stored.name, expectation.name
                        ));
                    };
                    if reference.owner_identity != expectation.owner_identity
                        || reference.owner_kind != expectation.owner_kind
                        || reference.name != expectation.name
                        || reference.application_identity != expectation.application_identity
                    {
                        return Err(format!(
                            "schema `{}` field `{}` nominal edge `{}` does not match its bound type",
                            root.name, stored.name, expectation.name
                        ));
                    }
                }
                Ok(())
            }

            fn resolve_requester(
                typed: &TypedTrees,
                requester_identity: &str,
            ) -> Result<Option<SymbolHandle>, String> {
                let mut found = None;
                for index in 0..typed.symbols.symbols().len() {
                    let symbol = SymbolHandle::from_arena_index(index as u32);
                    if !symbol.is_valid() {
                        continue;
                    }
                    if let Ok((identity, _)) = exact_symbol_identity(typed, symbol)
                        && identity == requester_identity
                    {
                        if found.is_some() {
                            return Err(format!(
                                "schema requester `{requester_identity}` resolves to more than one symbol"
                            ));
                        }
                        found = Some(symbol);
                    }
                }
                Ok(found)
            }

            pub(crate) fn graph_revision(
                authority: &SchemaQueryAuthority,
                nodes: &[SchemaNode],
            ) -> u64 {
                use crate::reflection::fingerprint::{
                    byte, bytes, optional_text, optional_uint, query_authority, text, uint,
                };
                fn handle(hash: &mut u64, value: SchemaNodeHandle) {
                    uint(hash, value.0 as u64);
                }
                fn handles(hash: &mut u64, values: &[SchemaNodeHandle]) {
                    uint(hash, values.len() as u64);
                    for value in values {
                        handle(hash, *value);
                    }
                }
                fn identities(hash: &mut u64, values: &[u64]) {
                    uint(hash, values.len() as u64);
                    for value in values {
                        uint(hash, *value);
                    }
                }

                let mut hash = crate::reflection::fingerprint::OFFSET_BASIS;
                bytes(&mut hash, b"omega.reflect.schema.v1");
                query_authority(&mut hash, authority);
                uint(&mut hash, nodes.len() as u64);
                for node in nodes {
                    match node {
                        SchemaNode::Invalid => byte(&mut hash, 0),
                        SchemaNode::Declaration(declaration) => {
                            byte(&mut hash, 1);
                            text(&mut hash, &declaration.owner_identity);
                            byte(&mut hash, declaration.identity_is_hermetic as u8);
                            text(&mut hash, &declaration.name);
                            byte(
                                &mut hash,
                                match declaration.shape {
                                    SchemaShape::Empty => 0,
                                    SchemaShape::Record => 1,
                                    SchemaShape::Sum => 2,
                                    SchemaShape::Mixed => 3,
                                },
                            );
                            text(&mut hash, &declaration.supply);
                            text(&mut hash, &declaration.multiplicity);
                            optional_text(&mut hash, &declaration.carry);
                            uint(&mut hash, declaration.type_parameters.len() as u64);
                            for parameter in &declaration.type_parameters {
                                text(&mut hash, parameter.name.as_str());
                                text(&mut hash, parameter.kind);
                                optional_text(&mut hash, &parameter.type_identity);
                                text(&mut hash, &parameter.bounds);
                            }
                            uint(&mut hash, declaration.lifetime_parameters.len() as u64);
                            for name in &declaration.lifetime_parameters {
                                text(&mut hash, name);
                            }
                            optional_text(&mut hash, &declaration.application_identity);
                            handles(&mut hash, &declaration.members);
                            identities(&mut hash, &declaration.retired_identities);
                        }
                        SchemaNode::Field(field) => {
                            byte(&mut hash, 2);
                            text(&mut hash, &field.member_identity);
                            text(&mut hash, &field.name);
                            uint(&mut hash, u64::from(field.declaration_position));
                            optional_uint(&mut hash, field.stable_number);
                            byte(&mut hash, field.erased as u8);
                            text(&mut hash, &field.qualified_type);
                            handles(&mut hash, &field.nominal_references);
                            handle(&mut hash, field.owner_case);
                        }
                        SchemaNode::Case(case) => {
                            byte(&mut hash, 3);
                            text(&mut hash, &case.member_identity);
                            text(&mut hash, &case.name);
                            uint(&mut hash, u64::from(case.declaration_position));
                            optional_uint(&mut hash, case.stable_number);
                            handles(&mut hash, &case.payload_fields);
                            identities(&mut hash, &case.retired_payload_identities);
                        }
                        SchemaNode::NominalReference(reference) => {
                            byte(&mut hash, 4);
                            text(&mut hash, &reference.owner_identity);
                            text(&mut hash, &reference.owner_kind);
                            text(&mut hash, &reference.name);
                            text(&mut hash, &reference.application_identity);
                        }
                    }
                }
                if hash == 0 { 1 } else { hash }
            }
        }
        pub(crate) mod runtime_image {
            use super::{SchemaNode, SchemaQueryAuthority, SchemaShape, SemanticSchemaGraph};

            const HEADER_WORDS: usize = 8;
            const NODE_WORDS: usize = 34;

            fn word(bytes: &mut [u8], slot: usize, value: u64) {
                bytes[slot * 8..slot * 8 + 8].copy_from_slice(&value.to_le_bytes());
            }

            fn span(bytes: &mut Vec<u8>, slot: usize, value: &[u8]) {
                let start = bytes.len() as u64;
                bytes.extend_from_slice(value);
                word(bytes, slot, start);
                word(bytes, slot + 1, value.len() as u64);
            }

            fn numbers(values: impl IntoIterator<Item = u64>) -> Vec<u8> {
                values.into_iter().flat_map(u64::to_le_bytes).collect()
            }

            fn strings(values: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<u8> {
                let mut bytes = Vec::new();
                for value in values {
                    let value = value.as_ref().as_bytes();
                    bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
                    bytes.extend_from_slice(value);
                }
                bytes
            }

            fn record(node: &SchemaNode) -> Vec<u8> {
                let mut bytes = vec![0; NODE_WORDS * 8];
                match node {
                    SchemaNode::Invalid => {}
                    SchemaNode::Declaration(value) => {
                        word(&mut bytes, 0, 1);
                        word(
                            &mut bytes,
                            6,
                            match value.shape {
                                SchemaShape::Empty => 0,
                                SchemaShape::Record => 1,
                                SchemaShape::Sum => 2,
                                SchemaShape::Mixed => 3,
                            },
                        );
                        word(&mut bytes, 7, u64::from(value.identity_is_hermetic));
                        span(&mut bytes, 8, value.owner_identity.as_bytes());
                        span(&mut bytes, 10, value.name.as_bytes());
                        span(
                            &mut bytes,
                            16,
                            value
                                .application_identity
                                .as_deref()
                                .unwrap_or("")
                                .as_bytes(),
                        );
                        span(&mut bytes, 18, value.supply.as_bytes());
                        span(&mut bytes, 20, value.multiplicity.as_bytes());
                        span(
                            &mut bytes,
                            22,
                            value.carry.as_deref().unwrap_or("").as_bytes(),
                        );
                        span(
                            &mut bytes,
                            24,
                            &numbers(value.members.iter().map(|h| h.index() as u64)),
                        );
                        span(
                            &mut bytes,
                            26,
                            &numbers(value.retired_identities.iter().copied()),
                        );
                        let mut parameters = Vec::new();
                        for parameter in &value.type_parameters {
                            parameters.extend(strings([
                                parameter.name.as_str(),
                                parameter.kind,
                                parameter.type_identity.as_deref().unwrap_or(""),
                                parameter.bounds.as_str(),
                            ]));
                        }
                        span(&mut bytes, 30, &parameters);
                        span(&mut bytes, 32, &strings(&value.lifetime_parameters));
                    }
                    SchemaNode::Field(value) => {
                        word(&mut bytes, 0, 2);
                        word(&mut bytes, 1, u64::from(value.declaration_position));
                        word(&mut bytes, 2, u64::from(value.stable_number.is_some()));
                        word(&mut bytes, 3, value.stable_number.unwrap_or(0));
                        word(&mut bytes, 4, u64::from(value.erased));
                        word(&mut bytes, 5, value.owner_case.index() as u64);
                        span(&mut bytes, 8, value.member_identity.as_bytes());
                        span(&mut bytes, 10, value.name.as_bytes());
                        span(&mut bytes, 12, value.qualified_type.as_bytes());
                        span(
                            &mut bytes,
                            28,
                            &numbers(value.nominal_references.iter().map(|h| h.index() as u64)),
                        );
                    }
                    SchemaNode::Case(value) => {
                        word(&mut bytes, 0, 3);
                        word(&mut bytes, 1, u64::from(value.declaration_position));
                        word(&mut bytes, 2, u64::from(value.stable_number.is_some()));
                        word(&mut bytes, 3, value.stable_number.unwrap_or(0));
                        span(&mut bytes, 8, value.member_identity.as_bytes());
                        span(&mut bytes, 10, value.name.as_bytes());
                        span(
                            &mut bytes,
                            24,
                            &numbers(value.payload_fields.iter().map(|h| h.index() as u64)),
                        );
                        span(
                            &mut bytes,
                            26,
                            &numbers(value.retired_payload_identities.iter().copied()),
                        );
                    }
                    SchemaNode::NominalReference(value) => {
                        word(&mut bytes, 0, 4);
                        span(&mut bytes, 8, value.owner_identity.as_bytes());
                        span(&mut bytes, 10, value.name.as_bytes());
                        span(&mut bytes, 14, value.owner_kind.as_bytes());
                        span(&mut bytes, 16, value.application_identity.as_bytes());
                    }
                }
                bytes
            }

            impl SemanticSchemaGraph {
                pub fn runtime_image(&self) -> Vec<u8> {
                    let mut bytes = vec![0; HEADER_WORDS * 8 + self.nodes.len() * 16];
                    word(&mut bytes, 0, 1);
                    word(&mut bytes, 1, self.nodes.len() as u64);
                    word(&mut bytes, 2, self.root.index() as u64);
                    word(&mut bytes, 3, self.revision);
                    let (authority, requester) = match &self.authority {
                        SchemaQueryAuthority::OwningScope { requester_identity } => {
                            (0, requester_identity)
                        }
                        SchemaQueryAuthority::ForeignScope { requester_identity } => {
                            (1, requester_identity)
                        }
                    };
                    word(&mut bytes, 4, authority);
                    span(&mut bytes, 5, requester.as_bytes());
                    for (index, node) in self.nodes.iter().enumerate() {
                        span(&mut bytes, HEADER_WORDS + index * 2, &record(node));
                    }
                    bytes
                }
            }
        }
        pub(crate) mod schema_nodes {
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
            pub struct SchemaNodeHandle(pub(crate) u32);

            impl SchemaNodeHandle {
                pub const INVALID: Self = Self(0);

                pub const fn invalid() -> Self {
                    Self::INVALID
                }

                pub const fn is_valid(self) -> bool {
                    self.0 != 0
                }

                pub const fn index(self) -> usize {
                    self.0 as usize
                }
            }

            #[derive(Debug, Clone, PartialEq)]
            pub enum SchemaNode {
                Invalid,
                Declaration(DeclarationDescription),
                Field(FieldDescription),
                Case(CaseDescription),
                NominalReference(NominalReferenceDescription),
            }

            #[derive(Debug, Clone, PartialEq)]
            pub struct DeclarationDescription {
                pub owner_identity: String,
                pub identity_is_hermetic: bool,
                pub name: String,
                pub shape: SchemaShape,
                pub supply: String,
                pub multiplicity: String,
                pub carry: Option<String>,
                pub type_parameters: Vec<TypeParameterDescription>,
                pub lifetime_parameters: Vec<String>,
                pub application_identity: Option<String>,
                pub members: Vec<SchemaNodeHandle>,
                pub retired_identities: Vec<u64>,
            }

            #[derive(Debug, Clone, PartialEq)]
            pub struct TypeParameterDescription {
                pub name: String,
                pub kind: &'static str,
                pub type_identity: Option<String>,
                pub bounds: String,
            }

            #[derive(Debug, Clone, PartialEq)]
            pub struct FieldDescription {
                pub member_identity: String,
                pub name: String,
                pub declaration_position: u32,
                pub stable_number: Option<u64>,
                pub erased: bool,
                pub qualified_type: String,
                pub nominal_references: Vec<SchemaNodeHandle>,
                pub owner_case: SchemaNodeHandle,
            }

            #[derive(Debug, Clone, PartialEq)]
            pub struct CaseDescription {
                pub member_identity: String,
                pub name: String,
                pub declaration_position: u32,
                pub stable_number: Option<u64>,
                pub payload_fields: Vec<SchemaNodeHandle>,
                pub retired_payload_identities: Vec<u64>,
            }

            #[derive(Debug, Clone, PartialEq)]
            pub struct NominalReferenceDescription {
                pub owner_identity: String,
                pub owner_kind: String,
                pub name: String,
                pub application_identity: String,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum SchemaShape {
                Empty,
                Record,
                Sum,
                Mixed,
            }

            #[derive(Debug, Clone, PartialEq)]
            pub enum SchemaQueryAuthority {
                OwningScope { requester_identity: String },
                ForeignScope { requester_identity: String },
            }

            impl SchemaQueryAuthority {}
        }

        pub use graph_construction::construct_semantic_schema_graph;
        pub(crate) use graph_construction::{
            describe_field, exact_symbol_identity, resolve_subject,
        };
        pub use graph_replay::replay_semantic_schema_graph;
        pub(crate) use graph_replay::{
            check_authority_claim, check_subject_application, member_identity_or_name,
        };
        pub use schema_nodes::{
            CaseDescription, DeclarationDescription, FieldDescription, NominalReferenceDescription,
            SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SchemaShape,
            TypeParameterDescription,
        };

        #[derive(Debug, Clone, PartialEq)]
        pub struct SemanticSchemaGraph {
            nodes: Vec<SchemaNode>,
            root: SchemaNodeHandle,
            authority: SchemaQueryAuthority,
            revision: u64,
        }

        impl SemanticSchemaGraph {
            pub fn root(&self) -> SchemaNodeHandle {
                self.root
            }

            pub fn authority(&self) -> &SchemaQueryAuthority {
                &self.authority
            }

            pub fn revision(&self) -> u64 {
                self.revision
            }

            pub fn node(&self, handle: SchemaNodeHandle) -> &SchemaNode {
                self.nodes
                    .get(handle.index())
                    .unwrap_or(&SchemaNode::Invalid)
            }

            pub fn declaration(&self) -> &DeclarationDescription {
                let SchemaNode::Declaration(declaration) = self.node(self.root) else {
                    unreachable!("the schema root is always a declaration node");
                };
                declaration
            }

            pub fn fields(&self) -> impl Iterator<Item = (SchemaNodeHandle, &FieldDescription)> {
                self.declaration()
                    .members
                    .iter()
                    .copied()
                    .filter_map(|handle| match self.node(handle) {
                        SchemaNode::Field(field) => Some((handle, field)),
                        _ => None,
                    })
            }

            pub fn cases(&self) -> impl Iterator<Item = (SchemaNodeHandle, &CaseDescription)> {
                self.declaration()
                    .members
                    .iter()
                    .copied()
                    .filter_map(|handle| match self.node(handle) {
                        SchemaNode::Case(case) => Some((handle, case)),
                        _ => None,
                    })
            }
        }
    }

    fn extend_unique<T: PartialEq>(target: &mut Vec<T>, additions: impl IntoIterator<Item = T>) {
        for addition in additions {
            if !target.contains(&addition) {
                target.push(addition);
            }
        }
    }
    pub(crate) mod selection {
        pub(crate) mod requirement_resolution {
            use crate::reflection::schema_graph::exact_symbol_identity;
            use crate::reflection::selection::scoped_members::ScopedMember;
            use crate::reflection::selection::{
                DemandedTraitApplication, SelectionChoice, SelectionRequirement,
                first_occurrence_normalized,
            };
            use symbols::SymbolHandle;
            use typed_trees::TypedTrees;
            use typed_trees::machine::Machine;
            use typed_trees::trait_definition::{Conformance, TraitDefinition};
            use typed_trees::types::TypeReferenceHandle;

            pub(crate) struct ResolvedRequirement<'program> {
                definition: &'program TraitDefinition,
                requirement_symbol: Option<SymbolHandle>,
                trait_application: Option<DemandedTraitApplication>,
            }

            pub(crate) fn resolve_requirement<'program>(
                typed: &'program TypedTrees,
                requirement: &SelectionRequirement,
            ) -> Result<ResolvedRequirement<'program>, String> {
                let mut matches = typed.traits().iter().filter(|definition| {
                    exact_symbol_identity(typed, definition.symbol)
                        .is_ok_and(|(identity, _)| identity == requirement.trait_identity)
                });
                let Some(definition) = matches.next() else {
                    return Err(format!(
                        "selection requirement `{}` does not resolve to a trait declaration in this program",
                        requirement.trait_identity
                    ));
                };
                if matches.next().is_some() {
                    return Err(format!(
                        "selection requirement `{}` resolves to more than one trait declaration",
                        requirement.trait_identity
                    ));
                }
                let requirement_symbol = match &requirement.requirement_identity {
        Some(identity) => Some(
            typed
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| {
                    exact_symbol_identity(typed, signature.symbol)
                        .is_ok_and(|(candidate, _)| candidate == *identity)
                })
                .map(|signature| signature.symbol)
                .ok_or_else(|| {
                    format!(
                        "selection requirement `{identity}` is not a requirement of trait `{}`",
                        requirement.trait_identity
                    )
                })?,
        ),
        None => None,
    };
                Ok(ResolvedRequirement {
                    definition,
                    requirement_symbol,
                    trait_application: requirement.trait_application.clone(),
                })
            }

            fn resolve_unique<'program, T>(
                items: impl Iterator<Item = &'program T>,
                symbol_of: impl Fn(&'program T) -> SymbolHandle,
                typed: &'program TypedTrees,
                identity: &str,
                domain: &str,
            ) -> Result<&'program T, String> {
                let mut matches = items.filter(|item| {
                    exact_symbol_identity(typed, symbol_of(item))
                        .is_ok_and(|(candidate, _)| candidate == identity)
                });
                match (matches.next(), matches.next()) {
                    (Some(item), None) => Ok(item),
                    (Some(_), Some(_)) => Err(format!(
                        "selection {domain} `{identity}` resolves to more than one declaration"
                    )),
                    _ => Err(format!(
                        "selection {domain} `{identity}` does not resolve to a {domain} in this program"
                    )),
                }
            }

            fn resolve_conformance<'program>(
                typed: &'program TypedTrees,
                identity: &str,
            ) -> Result<&'program Conformance, String> {
                resolve_unique(
                    typed.conformances().iter(),
                    |conformance| conformance.symbol,
                    typed,
                    identity,
                    "conformance",
                )
            }

            fn resolve_machine<'program>(
                typed: &'program TypedTrees,
                identity: &str,
            ) -> Result<&'program Machine, String> {
                resolve_unique(
                    typed.machines().iter(),
                    |machine| machine.symbol,
                    typed,
                    identity,
                    "machine",
                )
            }

            fn check_machine_owner_covers_member(
                typed: &TypedTrees,
                machine: &Machine,
                member: &ScopedMember,
            ) -> Result<(), String> {
                let Some(head) = &member.head_nominal_identity else {
                    return Err(format!(
                        "selection for `{}` requires an operation on the member's declared type, which has no nominal head",
                        member.member_identity
                    ));
                };
                if !machine.attached_data_symbol.is_valid() {
                    return Err(format!(
                        "selection for `{}` names an operation owned by no data declaration",
                        member.member_identity
                    ));
                }
                let owner_identity = exact_symbol_identity(typed, machine.attached_data_symbol)?.0;
                if owner_identity != *head {
                    return Err(format!(
                        "selection for `{}` names an operation on `{owner_identity}` which does not cover the member's declared type",
                        member.member_identity
                    ));
                }
                Ok(())
            }

            fn application_satisfied(
                typed: &TypedTrees,
                demanded: &DemandedTraitApplication,
                type_arguments: &[TypeReferenceHandle],
                lifetime_arguments: &[u32],
            ) -> bool {
                let carried: Vec<String> = type_arguments
                    .iter()
                    .map(|argument| typed.package_qualified_type_identity_text(*argument))
                    .collect();
                carried == demanded.type_argument_identities
                    && first_occurrence_normalized(lifetime_arguments)
                        == demanded.lifetime_arguments
            }

            fn carried_application_matches(
                typed: &TypedTrees,
                demanded: &Option<DemandedTraitApplication>,
                arguments: arena::HandleSpan<TypeReferenceHandle>,
                lifetime_arguments: &[u32],
            ) -> bool {
                match demanded {
                    None => true,
                    Some(demanded) => application_satisfied(
                        typed,
                        demanded,
                        typed.type_reference_table.type_reference_handles(arguments),
                        lifetime_arguments,
                    ),
                }
            }

            pub(crate) fn check_conformance_refines(
                typed: &TypedTrees,
                conformance_identity: &str,
                requirement: &ResolvedRequirement,
                member: &ScopedMember,
            ) -> Result<(), String> {
                let conformance = resolve_conformance(typed, conformance_identity)?;
                if conformance.trait_symbol != requirement.definition.symbol {
                    return Err(format!(
                        "selection for `{}` names conformance `{conformance_identity}` of a different trait; insufficient contract",
                        member.member_identity
                    ));
                }
                if !carried_application_matches(
                    typed,
                    &requirement.trait_application,
                    conformance.arguments,
                    &conformance.trait_lifetime_arguments,
                ) {
                    return Err(format!(
                        "selection for `{}` names conformance `{conformance_identity}` at a different trait application than demanded",
                        member.member_identity
                    ));
                }
                if let Some(requirement_symbol) = requirement.requirement_symbol
                    && !conformance.rows.iter().any(|row| {
                        row.requirement == requirement_symbol && row.realization_machine.is_valid()
                    })
                {
                    return Err(format!(
                        "selection for `{}` names conformance `{conformance_identity}` which does not realize the pinned requirement",
                        member.member_identity
                    ));
                }
                Err(format!(
                    "selection for `{}` names named implementation `{conformance_identity}`, which has no data subject; select the realizing machine whose owner covers the member",
                    member.member_identity
                ))
            }

            pub(crate) fn check_machine_refines(
                typed: &TypedTrees,
                machine_identity: &str,
                requirement: &ResolvedRequirement,
                member: &ScopedMember,
            ) -> Result<(), String> {
                let machine = resolve_machine(typed, machine_identity)?;
                if typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|edge| {
                        edge.symbol == requirement.definition.symbol
                            && requirement
                                .requirement_symbol
                                .is_none_or(|required| edge.requirement_symbol == required)
                            && carried_application_matches(
                                typed,
                                &requirement.trait_application,
                                edge.arguments,
                                &edge.trait_lifetime_arguments,
                            )
                    })
                {
                    return check_machine_owner_covers_member(typed, machine, member);
                }
                for conformance in typed
                    .conformances()
                    .iter()
                    .filter(|conformance| conformance.trait_symbol == requirement.definition.symbol)
                    .filter(|conformance| {
                        carried_application_matches(
                            typed,
                            &requirement.trait_application,
                            conformance.arguments,
                            &conformance.trait_lifetime_arguments,
                        )
                    })
                {
                    let realizes = conformance.rows.iter().any(|row| {
                        row.realization_machine == machine.symbol
                            && requirement
                                .requirement_symbol
                                .is_none_or(|required| row.requirement == required)
                    });
                    if realizes {
                        return check_machine_owner_covers_member(typed, machine, member);
                    }
                }
                Err(format!(
                    "selection for `{}` names machine `{machine_identity}` which does not realize the required contract",
                    member.member_identity
                ))
            }

            pub(crate) fn check_choice_refines(
                typed: &TypedTrees,
                requirement: &SelectionRequirement,
                member: &ScopedMember,
                choice: &SelectionChoice,
            ) -> Result<(), String> {
                let requirement = resolve_requirement(typed, requirement)?;
                match choice {
                    SelectionChoice::Conformance {
                        conformance_identity,
                    } => {
                        check_conformance_refines(typed, conformance_identity, &requirement, member)
                    }
                    SelectionChoice::Machine { machine_identity } => {
                        check_machine_refines(typed, machine_identity, &requirement, member)
                    }
                    SelectionChoice::Excluded => Ok(()),
                }
            }
        }
        pub(crate) mod scoped_members {
            use crate::reflection::schema_graph::{
                FieldDescription, SchemaNode, SchemaNodeHandle, SemanticSchemaGraph,
                describe_field, member_identity_or_name,
            };
            use crate::reflection::selection::{
                SelectionChoice, SelectionProjection, SelectionRecord,
            };
            use typed_trees::TypedTrees;
            use typed_trees::data::{DataDefinition, DataMember};

            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct MemberSelectionKey {
                pub(crate) member_identity: String,
                pub(crate) qualified_type: Option<String>,
                owner_case_identity: Option<String>,
                erased: bool,
            }

            impl MemberSelectionKey {
                pub fn member_identity(&self) -> &str {
                    &self.member_identity
                }

                pub fn qualified_type(&self) -> Option<&str> {
                    self.qualified_type.as_deref()
                }

                pub fn is_erased(&self) -> bool {
                    self.erased
                }
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum MemberKind {
                Field,
                Case,
                PayloadField,
            }

            #[derive(Debug, Clone, PartialEq, Eq)]
            pub(crate) struct ScopedMember {
                pub(crate) member_identity: String,
                pub(crate) name: String,
                pub(crate) kind: MemberKind,
                pub(crate) declaration_position: u32,
                pub(crate) stable_number: Option<u64>,
                pub(crate) qualified_type: Option<String>,
                pub(crate) owner_case_identity: Option<String>,
                pub(crate) erased: bool,
                pub(crate) head_nominal_identity: Option<String>,
            }

            impl ScopedMember {
                pub(crate) fn key(&self) -> MemberSelectionKey {
                    MemberSelectionKey {
                        member_identity: self.member_identity.clone(),
                        qualified_type: self.qualified_type.clone(),
                        owner_case_identity: self.owner_case_identity.clone(),
                        erased: self.erased,
                    }
                }

                pub(crate) fn matches_key(&self, key: &MemberSelectionKey) -> bool {
                    self.member_identity == key.member_identity
                        && self.qualified_type == key.qualified_type
                        && self.owner_case_identity == key.owner_case_identity
                        && self.erased == key.erased
                }

                pub(crate) fn record(&self, choice: SelectionChoice) -> SelectionRecord {
                    SelectionRecord {
                        member_identity: self.member_identity.clone(),
                        qualified_type: self.qualified_type.clone(),
                        owner_case_identity: self.owner_case_identity.clone(),
                        erased: self.erased,
                        choice,
                    }
                }
            }

            pub(crate) fn record_matches(record: &SelectionRecord, member: &ScopedMember) -> bool {
                record.member_identity == member.member_identity
                    && record.qualified_type == member.qualified_type
                    && record.owner_case_identity == member.owner_case_identity
                    && record.erased == member.erased
            }

            pub(crate) fn project(
                members: &[ScopedMember],
                projection: &SelectionProjection,
            ) -> Result<Vec<ScopedMember>, String> {
                match projection {
                    SelectionProjection::DeclaredMembers => Ok(members.to_vec()),
                    SelectionProjection::RuntimeMembers => Ok(members
                        .iter()
                        .filter(|member| !member.erased)
                        .cloned()
                        .collect()),
                    SelectionProjection::Members(identities) => {
                        let mut scoped = Vec::with_capacity(identities.len());
                        let mut seen: Vec<&str> = Vec::with_capacity(identities.len());
                        for identity in identities {
                            if seen.contains(&identity.as_str()) {
                                return Err(format!(
                                    "selection projection lists member `{identity}` twice"
                                ));
                            }
                            seen.push(identity.as_str());
                            let mut matches = members
                                .iter()
                                .filter(|member| member.member_identity == *identity);
                            match (matches.next(), matches.next()) {
                                (Some(member), None) => scoped.push(member.clone()),
                                (Some(_), Some(_)) => {
                                    return Err(format!(
                                        "selection projection member `{identity}` is ambiguous in the bound schema"
                                    ));
                                }
                                _ => {
                                    return Err(format!(
                                        "selection projection member `{identity}` is not a member of the bound schema"
                                    ));
                                }
                            }
                        }
                        Ok(scoped)
                    }
                }
            }

            pub(crate) fn graph_members(
                graph: &SemanticSchemaGraph,
            ) -> Result<Vec<ScopedMember>, String> {
                let declaration = graph.declaration();
                let mut members = Vec::new();
                for handle in &declaration.members {
                    match graph.node(*handle) {
                        SchemaNode::Field(field) => {
                            members.push(graph_field_member(graph, field, MemberKind::Field, None)?)
                        }
                        SchemaNode::Case(case) => {
                            members.push(ScopedMember {
                                member_identity: case.member_identity.clone(),
                                name: case.name.clone(),
                                kind: MemberKind::Case,
                                declaration_position: case.declaration_position,
                                stable_number: case.stable_number,
                                qualified_type: None,
                                owner_case_identity: None,
                                erased: false,
                                head_nominal_identity: Some(declaration.owner_identity.clone()),
                            });
                            for payload in &case.payload_fields {
                                let SchemaNode::Field(field) = graph.node(*payload) else {
                                    return Err(format!(
                                        "schema `{}` case `{}` payload is not a field node",
                                        declaration.name, case.name
                                    ));
                                };
                                members.push(graph_field_member(
                                    graph,
                                    field,
                                    MemberKind::PayloadField,
                                    Some(case.member_identity.clone()),
                                )?);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(members)
            }

            fn graph_field_member(
                graph: &SemanticSchemaGraph,
                field: &FieldDescription,
                kind: MemberKind,
                owner_case_identity: Option<String>,
            ) -> Result<ScopedMember, String> {
                let head_nominal_identity = match field.nominal_references.first() {
                    None => None,
                    Some(handle) if *handle == graph.root() => {
                        Some(graph.declaration().owner_identity.clone())
                    }
                    Some(handle) => match graph.node(*handle) {
                        SchemaNode::NominalReference(reference) => {
                            Some(reference.owner_identity.clone())
                        }
                        _ => {
                            return Err(format!(
                                "schema member `{}` nominal edge is not a reference node",
                                field.name
                            ));
                        }
                    },
                };
                Ok(ScopedMember {
                    member_identity: field.member_identity.clone(),
                    name: field.name.clone(),
                    kind,
                    declaration_position: field.declaration_position,
                    stable_number: field.stable_number,
                    qualified_type: Some(field.qualified_type.clone()),
                    owner_case_identity,
                    erased: field.erased,
                    head_nominal_identity,
                })
            }

            pub(crate) fn expected_members(
                typed: &TypedTrees,
                data: &DataDefinition,
                owner_identity: &str,
                application_identity: Option<&str>,
            ) -> Result<Vec<ScopedMember>, String> {
                let mut members = Vec::new();
                for (position, member) in typed.data_members(data).iter().enumerate() {
                    match member {
                        DataMember::Field(field) => {
                            let (description, expectations) = describe_field(
                                typed,
                                field,
                                owner_identity,
                                position as u32,
                                SchemaNodeHandle::INVALID,
                                application_identity,
                            )?;
                            members.push(ScopedMember {
                                member_identity: description.member_identity,
                                name: description.name,
                                kind: MemberKind::Field,
                                declaration_position: description.declaration_position,
                                stable_number: description.stable_number,
                                qualified_type: Some(description.qualified_type),
                                owner_case_identity: None,
                                erased: description.erased,
                                head_nominal_identity: expectations
                                    .first()
                                    .map(|expectation| expectation.owner_identity.clone()),
                            });
                        }
                        DataMember::Variant(variant) => {
                            let case_identity = member_identity_or_name(
                                typed,
                                variant.symbol,
                                owner_identity,
                                variant.name.as_str(),
                            );
                            members.push(ScopedMember {
                                member_identity: case_identity.clone(),
                                name: variant.name.as_str().to_owned(),
                                kind: MemberKind::Case,
                                declaration_position: position as u32,
                                stable_number: variant.identity,
                                qualified_type: None,
                                owner_case_identity: None,
                                erased: false,
                                head_nominal_identity: Some(owner_identity.to_owned()),
                            });
                            for (payload_position, payload_field) in
                                typed.data_payload_fields(variant).iter().enumerate()
                            {
                                let (description, expectations) = describe_field(
                                    typed,
                                    payload_field,
                                    owner_identity,
                                    payload_position as u32,
                                    SchemaNodeHandle::INVALID,
                                    application_identity,
                                )?;
                                members.push(ScopedMember {
                                    member_identity: description.member_identity,
                                    name: description.name,
                                    kind: MemberKind::PayloadField,
                                    declaration_position: description.declaration_position,
                                    stable_number: description.stable_number,
                                    qualified_type: Some(description.qualified_type),
                                    owner_case_identity: Some(case_identity.clone()),
                                    erased: description.erased,
                                    head_nominal_identity: expectations
                                        .first()
                                        .map(|expectation| expectation.owner_identity.clone()),
                                });
                            }
                        }
                    }
                }
                Ok(members)
            }
        }
        pub(crate) mod selection_receiver {
            use crate::reflection::schema_graph::{SchemaQueryAuthority, SemanticSchemaGraph};
            use crate::reflection::selection::requirement_resolution::{
                check_choice_refines, resolve_requirement,
            };
            use crate::reflection::selection::scoped_members::{
                ScopedMember, graph_members, project, record_matches,
            };
            use crate::reflection::selection::snapshots::snapshot_revision;
            use crate::reflection::selection::{
                MemberSelectionKey, SelectionChoice, SelectionCoverage, SelectionProjection,
                SelectionRecord, SelectionRequirement, SelectionSnapshot,
            };
            use typed_trees::TypedTrees;

            #[derive(Debug)]
            pub struct ScopedSelectionReceiver<'program> {
                typed: &'program TypedTrees,
                subject_owner_identity: String,
                subject_application_identity: Option<String>,
                authority: SchemaQueryAuthority,
                projection: SelectionProjection,
                coverage: SelectionCoverage,
                requirement: SelectionRequirement,
                members: Vec<ScopedMember>,
                records: Vec<SelectionRecord>,
            }

            impl<'program> ScopedSelectionReceiver<'program> {
                pub fn new(
                    typed: &'program TypedTrees,
                    graph: &SemanticSchemaGraph,
                    projection: SelectionProjection,
                    coverage: SelectionCoverage,
                    requirement: SelectionRequirement,
                ) -> Result<Self, String> {
                    resolve_requirement(typed, &requirement)?;
                    let declaration = graph.declaration();
                    let members = project(&graph_members(graph)?, &projection)?;
                    Ok(Self {
                        typed,
                        subject_owner_identity: declaration.owner_identity.clone(),
                        subject_application_identity: declaration.application_identity.clone(),
                        authority: graph.authority().clone(),
                        projection,
                        coverage,
                        requirement,
                        members,
                        records: Vec::new(),
                    })
                }

                pub fn member(&self, name: &str) -> Result<MemberSelectionKey, String> {
                    let mut matches = self.members.iter().filter(|member| member.name == name);
                    match (matches.next(), matches.next()) {
                        (Some(member), None) => Ok(member.key()),
                        (Some(_), Some(_)) => Err(format!(
                            "selection member name `{name}` is ambiguous inside the projection"
                        )),
                        _ => Err(format!(
                            "no member named `{name}` is in the receiver's scoped projection"
                        )),
                    }
                }

                pub fn members(&self) -> impl Iterator<Item = MemberSelectionKey> + '_ {
                    self.members.iter().map(ScopedMember::key)
                }

                pub fn select(
                    &mut self,
                    key: &MemberSelectionKey,
                    choice: SelectionChoice,
                ) -> Result<(), String> {
                    if matches!(choice, SelectionChoice::Excluded) {
                        return Err(
                            "an explicit exclusion is recorded with `exclude`, not `select`"
                                .to_owned(),
                        );
                    }
                    let member = self.scoped_member_for_key(key)?;
                    self.check_unrecorded(&member)?;
                    check_choice_refines(self.typed, &self.requirement, &member, &choice)?;
                    self.records.push(member.record(choice));
                    Ok(())
                }

                pub fn exclude(&mut self, key: &MemberSelectionKey) -> Result<(), String> {
                    if self.coverage == SelectionCoverage::Complete {
                        return Err(format!(
                            "member `{}` cannot be excluded under a complete-coverage contract; every in-scope member needs exactly one selection",
                            key.member_identity
                        ));
                    }
                    let member = self.scoped_member_for_key(key)?;
                    self.check_unrecorded(&member)?;
                    self.records.push(member.record(SelectionChoice::Excluded));
                    Ok(())
                }

                fn scoped_member_for_key(
                    &self,
                    key: &MemberSelectionKey,
                ) -> Result<ScopedMember, String> {
                    if let Some(member) = self.members.iter().find(|member| member.matches_key(key))
                    {
                        return Ok(member.clone());
                    }
                    if self
                        .members
                        .iter()
                        .any(|member| member.member_identity == key.member_identity)
                    {
                        Err(format!(
                            "selection key for `{}` is stale: it does not match the projected member's type and scope",
                            key.member_identity
                        ))
                    } else {
                        Err(format!(
                            "selection member `{}` is outside the receiver's scoped projection",
                            key.member_identity
                        ))
                    }
                }

                fn check_unrecorded(&self, member: &ScopedMember) -> Result<(), String> {
                    if self
                        .records
                        .iter()
                        .any(|record| record_matches(record, member))
                    {
                        return Err(format!(
                            "member `{}` already has a recorded choice; selecting twice is not last-write-wins",
                            member.member_identity
                        ));
                    }
                    Ok(())
                }

                pub fn freeze(self) -> Result<SelectionSnapshot, String> {
                    let mut records = self.records;
                    let mut ordered = Vec::with_capacity(self.members.len());
                    for member in &self.members {
                        let Some(position) = records
                            .iter()
                            .position(|record| record_matches(record, member))
                        else {
                            return Err(format!(
                                "member `{}` has no recorded selection or exclusion; a scoped selection covers every in-scope member exactly once",
                                member.member_identity
                            ));
                        };
                        ordered.push(records.swap_remove(position));
                    }
                    if !records.is_empty() {
                        return Err(
                            "selection records name members outside the projection".to_owned()
                        );
                    }
                    let mut snapshot = SelectionSnapshot {
                        subject_owner_identity: self.subject_owner_identity,
                        subject_application_identity: self.subject_application_identity,
                        authority: self.authority,
                        projection: self.projection,
                        coverage: self.coverage,
                        requirement: self.requirement,
                        records: ordered,
                        revision: 0,
                    };
                    snapshot.revision = snapshot_revision(&snapshot);
                    Ok(snapshot)
                }
            }
        }
        pub(crate) mod snapshots {
            use crate::reflection::schema_graph::{
                SchemaQueryAuthority, check_authority_claim, check_subject_application,
                resolve_subject,
            };
            use crate::reflection::selection::requirement_resolution::{
                check_conformance_refines, check_machine_refines, resolve_requirement,
            };
            use crate::reflection::selection::scoped_members::{
                expected_members, project, record_matches,
            };
            use crate::reflection::selection::{
                SelectionChoice, SelectionCoverage, SelectionProjection, SelectionRecord,
                SelectionRequirement,
            };
            use typed_trees::TypedTrees;

            #[derive(Debug, Clone, PartialEq)]
            pub struct SelectionSnapshot {
                pub(crate) subject_owner_identity: String,
                pub(crate) subject_application_identity: Option<String>,
                pub(crate) authority: SchemaQueryAuthority,
                pub(crate) projection: SelectionProjection,
                pub(crate) coverage: SelectionCoverage,
                pub(crate) requirement: SelectionRequirement,
                pub(crate) records: Vec<SelectionRecord>,
                pub(crate) revision: u64,
            }

            impl SelectionSnapshot {
                pub fn subject_owner_identity(&self) -> &str {
                    &self.subject_owner_identity
                }

                pub fn subject_application_identity(&self) -> Option<&str> {
                    self.subject_application_identity.as_deref()
                }

                pub fn authority(&self) -> &SchemaQueryAuthority {
                    &self.authority
                }

                pub fn projection(&self) -> &SelectionProjection {
                    &self.projection
                }

                pub fn coverage(&self) -> SelectionCoverage {
                    self.coverage
                }

                pub fn requirement(&self) -> &SelectionRequirement {
                    &self.requirement
                }

                pub fn records(&self) -> &[SelectionRecord] {
                    &self.records
                }

                pub fn revision(&self) -> u64 {
                    self.revision
                }
            }

            pub fn replay_selection_snapshot(
                typed: &TypedTrees,
                snapshot: &SelectionSnapshot,
            ) -> Result<(), String> {
                let data = resolve_subject(typed, &snapshot.subject_owner_identity)?;
                if data.quotient.is_some() {
                    return Err(format!(
                        "selection snapshot `{}` binds a quotient declaration, which exposes no representative to reflection",
                        data.name.as_str()
                    ));
                }
                check_authority_claim(
                    typed,
                    data,
                    &snapshot.authority,
                    "selection snapshot",
                    data.name.as_str(),
                )?;
                check_subject_application(
                    typed,
                    data,
                    &snapshot.subject_application_identity,
                    "selection snapshot",
                    data.name.as_str(),
                )?;
                let requirement = resolve_requirement(typed, &snapshot.requirement)?;
                let expected = expected_members(
                    typed,
                    data,
                    &snapshot.subject_owner_identity,
                    snapshot.subject_application_identity.as_deref(),
                )?;
                let in_scope = project(&expected, &snapshot.projection).map_err(|error| {
        format!(
            "selection snapshot `{}` projection does not resolve against the bound declaration: {error}",
            data.name.as_str()
        )
    })?;
                if snapshot.records.len() != in_scope.len() {
                    return Err(format!(
                        "selection snapshot `{}` records {} members but its projection covers {}",
                        data.name.as_str(),
                        snapshot.records.len(),
                        in_scope.len()
                    ));
                }
                for (record, member) in snapshot.records.iter().zip(in_scope.iter()) {
                    if !record_matches(record, member) {
                        return Err(format!(
                            "selection snapshot `{}` record for `{}` does not match the bound member",
                            data.name.as_str(),
                            record.member_identity
                        ));
                    }
                    match &record.choice {
                        SelectionChoice::Excluded => {
                            if snapshot.coverage == SelectionCoverage::Complete {
                                return Err(format!(
                                    "selection snapshot `{}` excludes member `{}` under a complete-coverage contract",
                                    data.name.as_str(),
                                    record.member_identity
                                ));
                            }
                        }
                        SelectionChoice::Conformance {
                            conformance_identity,
                        } => check_conformance_refines(
                            typed,
                            conformance_identity,
                            &requirement,
                            member,
                        )?,
                        SelectionChoice::Machine { machine_identity } => {
                            check_machine_refines(typed, machine_identity, &requirement, member)?
                        }
                    }
                }
                if snapshot.revision != snapshot_revision(snapshot) {
                    return Err(format!(
                        "selection snapshot `{}` report fingerprint does not match its frozen contents",
                        data.name.as_str()
                    ));
                }
                Ok(())
            }

            pub(crate) fn snapshot_revision(snapshot: &SelectionSnapshot) -> u64 {
                use crate::reflection::fingerprint::{
                    byte, bytes, optional_text, query_authority, text, uint,
                };

                let mut hash = crate::reflection::fingerprint::OFFSET_BASIS;
                bytes(&mut hash, b"omega.reflect.selection.v1");
                text(&mut hash, &snapshot.subject_owner_identity);
                optional_text(&mut hash, &snapshot.subject_application_identity);
                query_authority(&mut hash, &snapshot.authority);
                match &snapshot.projection {
                    SelectionProjection::DeclaredMembers => byte(&mut hash, 0),
                    SelectionProjection::RuntimeMembers => byte(&mut hash, 1),
                    SelectionProjection::Members(identities) => {
                        byte(&mut hash, 2);
                        uint(&mut hash, identities.len() as u64);
                        for identity in identities {
                            text(&mut hash, identity);
                        }
                    }
                }
                byte(
                    &mut hash,
                    match snapshot.coverage {
                        SelectionCoverage::Complete => 0,
                        SelectionCoverage::Partial => 1,
                    },
                );
                text(&mut hash, &snapshot.requirement.trait_identity);
                optional_text(&mut hash, &snapshot.requirement.requirement_identity);
                match &snapshot.requirement.trait_application {
                    Some(application) => {
                        byte(&mut hash, 1);
                        uint(&mut hash, application.type_argument_identities.len() as u64);
                        for identity in &application.type_argument_identities {
                            text(&mut hash, identity);
                        }
                        uint(&mut hash, application.lifetime_arguments.len() as u64);
                        for ordinal in &application.lifetime_arguments {
                            uint(&mut hash, u64::from(*ordinal));
                        }
                    }
                    None => byte(&mut hash, 0),
                }
                uint(&mut hash, snapshot.records.len() as u64);
                for record in &snapshot.records {
                    text(&mut hash, &record.member_identity);
                    optional_text(&mut hash, &record.qualified_type);
                    optional_text(&mut hash, &record.owner_case_identity);
                    byte(&mut hash, record.erased as u8);
                    match &record.choice {
                        SelectionChoice::Conformance {
                            conformance_identity,
                        } => {
                            byte(&mut hash, 0);
                            text(&mut hash, conformance_identity);
                        }
                        SelectionChoice::Machine { machine_identity } => {
                            byte(&mut hash, 1);
                            text(&mut hash, machine_identity);
                        }
                        SelectionChoice::Excluded => byte(&mut hash, 2),
                    }
                }
                if hash == 0 { 1 } else { hash }
            }
        }

        pub use scoped_members::{MemberKind, MemberSelectionKey};
        pub(crate) use scoped_members::{ScopedMember, expected_members, project};
        pub use selection_receiver::ScopedSelectionReceiver;
        pub use snapshots::{SelectionSnapshot, replay_selection_snapshot};

        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;

        use super::schema_graph::exact_symbol_identity;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum SelectionProjection {
            DeclaredMembers,
            RuntimeMembers,
            Members(Vec<String>),
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum SelectionCoverage {
            Complete,
            Partial,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct DemandedTraitApplication {
            pub type_argument_identities: Vec<String>,
            pub lifetime_arguments: Vec<u32>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct SelectionRequirement {
            pub trait_identity: String,
            pub requirement_identity: Option<String>,
            pub trait_application: Option<DemandedTraitApplication>,
        }

        pub(crate) fn first_occurrence_normalized(ordinals: &[u32]) -> Vec<u32> {
            let mut seen: Vec<u32> = Vec::with_capacity(ordinals.len());
            ordinals
                .iter()
                .map(
                    |ordinal| match seen.iter().position(|known| known == ordinal) {
                        Some(position) => position as u32,
                        None => {
                            seen.push(*ordinal);
                            (seen.len() - 1) as u32
                        }
                    },
                )
                .collect()
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum SelectionChoice {
            Conformance { conformance_identity: String },
            Machine { machine_identity: String },
            Excluded,
        }

        impl SelectionChoice {
            pub fn conformance(typed: &TypedTrees, symbol: SymbolHandle) -> Result<Self, String> {
                if !typed.conformance_by_symbol(symbol).is_some() {
                    return Err(
                        "a conformance selection names an exact conformance declaration".to_owned(),
                    );
                }
                Ok(Self::Conformance {
                    conformance_identity: exact_symbol_identity(typed, symbol)?.0,
                })
            }

            pub fn machine(typed: &TypedTrees, symbol: SymbolHandle) -> Result<Self, String> {
                if !typed.machine(symbol).is_some() {
                    return Err("a machine selection names an exact machine declaration".to_owned());
                }
                Ok(Self::Machine {
                    machine_identity: exact_symbol_identity(typed, symbol)?.0,
                })
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct SelectionRecord {
            pub member_identity: String,
            pub qualified_type: Option<String>,
            pub owner_case_identity: Option<String>,
            pub erased: bool,
            pub choice: SelectionChoice,
        }
    }
    pub(crate) mod visitation {
        use typed_trees::TypedTrees;

        use super::schema_graph::{SchemaQueryAuthority, resolve_subject};
        use super::selection::{
            MemberKind, ScopedMember, SelectionChoice, SelectionRecord, SelectionRequirement,
            SelectionSnapshot, expected_members, project, replay_selection_snapshot,
        };

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct FieldInfo {
            member_identity: String,
            name: String,
            kind: MemberKind,
            declaration_position: u32,
            stable_number: Option<u64>,
            qualified_type: Option<String>,
            owner_case_identity: Option<String>,
            carrier_identity: Option<String>,
        }

        impl FieldInfo {
            pub fn member_identity(&self) -> &str {
                &self.member_identity
            }

            pub fn name(&self) -> &str {
                &self.name
            }

            pub fn kind(&self) -> MemberKind {
                self.kind
            }

            pub fn declaration_position(&self) -> u32 {
                self.declaration_position
            }

            pub fn stable_number(&self) -> Option<u64> {
                self.stable_number
            }

            pub fn qualified_type(&self) -> Option<&str> {
                self.qualified_type.as_deref()
            }

            pub fn owner_case_identity(&self) -> Option<&str> {
                self.owner_case_identity.as_deref()
            }

            pub fn carrier_identity(&self) -> Option<&str> {
                self.carrier_identity.as_deref()
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum VisitationOperation {
            Conformance { conformance_identity: String },
            Machine { machine_identity: String },
        }

        impl VisitationOperation {
            pub fn identity(&self) -> &str {
                match self {
                    Self::Conformance {
                        conformance_identity,
                    } => conformance_identity,
                    Self::Machine { machine_identity } => machine_identity,
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct VisitationCall {
            field: FieldInfo,
            operation: VisitationOperation,
        }

        impl VisitationCall {
            pub fn field(&self) -> &FieldInfo {
                &self.field
            }

            pub fn operation(&self) -> &VisitationOperation {
                &self.operation
            }
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct RuntimeVisitationPlan {
            subject_owner_identity: String,
            subject_application_identity: Option<String>,
            authority: SchemaQueryAuthority,
            requirement: SelectionRequirement,
            calls: Vec<VisitationCall>,
            revision: u64,
        }

        impl RuntimeVisitationPlan {
            pub fn compose(
                typed: &TypedTrees,
                snapshot: &SelectionSnapshot,
            ) -> Result<Self, String> {
                replay_selection_snapshot(typed, snapshot)?;
                let data = resolve_subject(typed, snapshot.subject_owner_identity())?;
                let members = expected_members(
                    typed,
                    data,
                    snapshot.subject_owner_identity(),
                    snapshot.subject_application_identity(),
                )?;
                let in_scope = project(&members, snapshot.projection())?;
                let calls = expand_calls(&in_scope, snapshot.records())?;
                let mut plan = Self {
                    subject_owner_identity: snapshot.subject_owner_identity().to_owned(),
                    subject_application_identity: snapshot
                        .subject_application_identity()
                        .map(str::to_owned),
                    authority: snapshot.authority().clone(),
                    requirement: snapshot.requirement().clone(),
                    calls,
                    revision: 0,
                };
                plan.revision = plan_revision(&plan);
                Ok(plan)
            }

            pub fn subject_owner_identity(&self) -> &str {
                &self.subject_owner_identity
            }

            pub fn subject_application_identity(&self) -> Option<&str> {
                self.subject_application_identity.as_deref()
            }

            pub fn authority(&self) -> &SchemaQueryAuthority {
                &self.authority
            }

            pub fn requirement(&self) -> &SelectionRequirement {
                &self.requirement
            }

            pub fn calls(&self) -> &[VisitationCall] {
                &self.calls
            }

            pub fn revision(&self) -> u64 {
                self.revision
            }
        }

        fn field_info(member: &ScopedMember) -> FieldInfo {
            FieldInfo {
                member_identity: member.member_identity.clone(),
                name: member.name.clone(),
                kind: member.kind,
                declaration_position: member.declaration_position,
                stable_number: member.stable_number,
                qualified_type: member.qualified_type.clone(),
                owner_case_identity: member.owner_case_identity.clone(),
                carrier_identity: member.head_nominal_identity.clone(),
            }
        }

        fn expand_calls(
            members: &[ScopedMember],
            records: &[SelectionRecord],
        ) -> Result<Vec<VisitationCall>, String> {
            let mut calls = Vec::new();
            for (member, record) in members.iter().zip(records.iter()) {
                let operation = match &record.choice {
                    SelectionChoice::Conformance {
                        conformance_identity,
                    } => VisitationOperation::Conformance {
                        conformance_identity: conformance_identity.clone(),
                    },
                    SelectionChoice::Machine { machine_identity } => VisitationOperation::Machine {
                        machine_identity: machine_identity.clone(),
                    },
                    SelectionChoice::Excluded => continue,
                };
                if member.erased {
                    return Err(format!(
                        "member `{}` is erased and receives no runtime borrow; its selected operation cannot be visited",
                        member.member_identity
                    ));
                }
                calls.push(VisitationCall {
                    field: field_info(member),
                    operation,
                });
            }
            Ok(calls)
        }

        pub fn replay_runtime_visitation_plan(
            typed: &TypedTrees,
            snapshot: &SelectionSnapshot,
            plan: &RuntimeVisitationPlan,
        ) -> Result<(), String> {
            if plan.subject_owner_identity != snapshot.subject_owner_identity()
                || plan.subject_application_identity.as_deref()
                    != snapshot.subject_application_identity()
                || plan.authority != *snapshot.authority()
                || plan.requirement != *snapshot.requirement()
            {
                return Err(
                    "visitation plan header does not match the snapshot it claims to expand"
                        .to_owned(),
                );
            }
            replay_selection_snapshot(typed, snapshot)?;
            let data = resolve_subject(typed, snapshot.subject_owner_identity())?;
            let members = expected_members(
                typed,
                data,
                snapshot.subject_owner_identity(),
                snapshot.subject_application_identity(),
            )?;
            let in_scope = project(&members, snapshot.projection()).map_err(|error| {
        format!(
            "visitation plan projection does not resolve against the bound declaration: {error}"
        )
    })?;
            let expected = expand_calls(&in_scope, snapshot.records())?;
            if plan.calls != expected {
                return Err(format!(
                    "visitation plan for `{}` does not match the snapshot's checked call expansion",
                    data.name.as_str()
                ));
            }
            if plan.revision != plan_revision(plan) {
                return Err(format!(
                    "visitation plan for `{}` report fingerprint does not match its frozen contents",
                    data.name.as_str()
                ));
            }
            Ok(())
        }

        fn plan_revision(plan: &RuntimeVisitationPlan) -> u64 {
            use crate::reflection::fingerprint::{
                byte, bytes, optional_text, optional_uint, query_authority, text, uint,
            };

            let mut hash = crate::reflection::fingerprint::OFFSET_BASIS;
            bytes(&mut hash, b"omega.reflect.visitation.v1");
            text(&mut hash, &plan.subject_owner_identity);
            optional_text(&mut hash, &plan.subject_application_identity);
            query_authority(&mut hash, &plan.authority);
            text(&mut hash, &plan.requirement.trait_identity);
            optional_text(&mut hash, &plan.requirement.requirement_identity);
            uint(&mut hash, plan.calls.len() as u64);
            for call in &plan.calls {
                let field = &call.field;
                text(&mut hash, &field.member_identity);
                text(&mut hash, &field.name);
                byte(
                    &mut hash,
                    match field.kind {
                        MemberKind::Field => 0,
                        MemberKind::Case => 1,
                        MemberKind::PayloadField => 2,
                    },
                );
                uint(&mut hash, u64::from(field.declaration_position));
                optional_uint(&mut hash, field.stable_number);
                optional_text(&mut hash, &field.qualified_type);
                optional_text(&mut hash, &field.owner_case_identity);
                optional_text(&mut hash, &field.carrier_identity);
                match &call.operation {
                    VisitationOperation::Conformance {
                        conformance_identity,
                    } => {
                        byte(&mut hash, 0);
                        text(&mut hash, conformance_identity);
                    }
                    VisitationOperation::Machine { machine_identity } => {
                        byte(&mut hash, 1);
                        text(&mut hash, machine_identity);
                    }
                }
            }
            if hash == 0 { 1 } else { hash }
        }
    }

    pub use schema_graph::{
        CaseDescription, DeclarationDescription, FieldDescription, NominalReferenceDescription,
        SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, SemanticSchemaGraph,
        TypeParameterDescription, construct_semantic_schema_graph, replay_semantic_schema_graph,
    };
    pub use selection::{
        MemberKind, MemberSelectionKey, ScopedSelectionReceiver, SelectionChoice,
        SelectionCoverage, SelectionProjection, SelectionRecord, SelectionRequirement,
        SelectionSnapshot, replay_selection_snapshot,
    };
    pub use visitation::{
        FieldInfo, RuntimeVisitationPlan, VisitationCall, VisitationOperation,
        replay_runtime_visitation_plan,
    };
}
