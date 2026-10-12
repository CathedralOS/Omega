//! Region identity refines the complete type, not just its physical loans.
//! Reuse conformance's structural query for phantom nominal arguments and
//! reference descendants. Simultaneous operands re-enter their supplying
//! scope; source names never equate a captured region with a fresh binder.

use super::callable_refinement::{CallableBorrowContract, captured_lifetimes};
use super::callable_regions::CallableRegions;
use super::carried_lifetimes::{CarriedLifetime, RegionIdentity};
use super::region_inference::InputRegionApplication;
use crate::type_regions::{
    TypeRegionRelationship, TypeRegionScope, type_reference_region_applications,
    type_reference_regions_match,
};
use symbols::SymbolHandle;
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Copy)]
enum RegionEnvironment<'scope> {
    Resolved(&'scope [(String, RegionIdentity)]),
    Captured(SymbolHandle),
}

impl RegionEnvironment<'_> {
    fn identity(
        self,
        program: &dyn crate::CheckingInput,
        name: &Identifier,
    ) -> Option<RegionIdentity> {
        match self {
            Self::Resolved(regions) => regions
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == name.as_str())
                .map(|(_, identity)| identity.clone()),
            Self::Captured(owner) => captured_lifetimes(program, owner)
                .into_iter()
                .rev()
                .find(|(parameter, _)| parameter == name.as_str())
                .map(|(_, identity)| identity),
        }
    }
}

#[derive(Clone, Copy)]
struct RegionScope<'scope> {
    declaration: SymbolHandle,
    // Only a declaration application quantifies these original formals. A
    // callable's supplied operand still re-enters its rigid capture normally.
    formal_declaration: SymbolHandle,
    formal_regions: &'scope [(String, RegionIdentity)],
    substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
    starts: &'scope [usize],
    applications: &'scope [Vec<(String, RegionIdentity)>],
    binders: &'scope [(SymbolHandle, String)],
    regions: RegionEnvironment<'scope>,
    arguments: RegionEnvironment<'scope>,
    output_elisions: &'scope [(TypeReferenceHandle, RegionIdentity)],
}

impl TypeRegionScope for RegionScope<'_> {
    fn erased_identity(
        self,
        program: &dyn crate::CheckingInput,
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

    fn resolve(
        mut self,
        program: &dyn crate::CheckingInput,
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
            if owner.is_valid() && owner == self.formal_declaration {
                self.regions = RegionEnvironment::Resolved(self.formal_regions);
            } else if owner.is_valid() {
                self.regions = RegionEnvironment::Captured(owner);
                self.output_elisions = &[];
                if program.lifetime_declaration_owner(owner)
                    != program.lifetime_declaration_owner(self.declaration)
                {
                    self.substitutions = &[];
                    self.starts = &[];
                    self.applications = &[];
                }
            }
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return Some((reference, self));
            };
            if let Some((binding_ordinal, (_, operand))) = self
                .substitutions
                .iter()
                .enumerate()
                .rev()
                .find(|(_, (parameter, _))| parameter == symbol)
            {
                if *operand == reference {
                    return Some((reference, self));
                }
                reference = *operand;
                let (scope_ordinal, start) = self
                    .starts
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, start)| **start <= binding_ordinal)?;
                self.regions = scope_ordinal
                    .checked_sub(1)
                    .and_then(|ordinal| self.applications.get(ordinal))
                    .map_or(self.arguments, |regions| {
                        RegionEnvironment::Resolved(regions)
                    });
                self.substitutions = &self.substitutions[..*start];
                self.starts = &self.starts[..scope_ordinal];
                self.applications =
                    &self.applications[..self.applications.len().min(scope_ordinal)];
                self.output_elisions = &[];
                continue;
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

fn contract_scope<'scope>(
    contract: &'scope CallableBorrowContract<'_>,
    regions: &'scope CallableRegions,
) -> RegionScope<'scope> {
    RegionScope {
        declaration: contract.lexical_owner,
        formal_declaration: SymbolHandle::invalid(),
        formal_regions: &[],
        substitutions: contract.substitutions,
        starts: contract.substitution_scope_starts,
        applications: &regions.applications,
        binders: contract.binders,
        regions: RegionEnvironment::Resolved(&regions.names),
        arguments: RegionEnvironment::Resolved(&regions.arguments),
        output_elisions: &[],
    }
}

/// Input-established application of one original declaration's lifetime slot.
/// Invocation coordinates remain distinct from captured source declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputLifetimeArgument {
    Captured(typed_trees::type_identity::LifetimeParameterIdentity),
    Invocation(usize),
    AnonymousInvocation(typed_trees::type_identity::LifetimeParameterIdentity),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputLifetimeBinding {
    pub parameter: Identifier,
    pub argument: InputLifetimeArgument,
}

/// A declaration application inspects original type inputs, without inventing
/// runtime parameters. Supplied operand subtrees retain their own owner; only
/// this declaration's original lifetime formals are quantified for application.
pub struct InputTypeRegionScope<'scope> {
    pub declaration: SymbolHandle,
    pub lifetime_parameters: &'scope [Identifier],
    pub substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
    pub argument_owner: SymbolHandle,
}

pub fn infer_type_input_lifetime_application(
    program: &dyn crate::CheckingInput,
    actual: InputTypeRegionScope<'_>,
    required_owner: SymbolHandle,
    inputs: &[(TypeReferenceHandle, TypeReferenceHandle)],
    invocation_count: usize,
    constraints: &[(usize, InputLifetimeArgument)],
) -> Result<Vec<InputLifetimeBinding>, &'static str> {
    let mut actual_names = captured_lifetimes(program, actual.declaration);
    actual_names.extend(actual.lifetime_parameters.iter().enumerate().map(
        |(ordinal, parameter)| {
            (
                parameter.as_str().to_owned(),
                RegionIdentity::Fresh(ordinal),
            )
        },
    ));
    let required_names = captured_lifetimes(program, required_owner);
    let starts = [0];
    let actual_scope = RegionScope {
        declaration: actual.declaration,
        formal_declaration: actual.declaration,
        formal_regions: &actual_names,
        substitutions: actual.substitutions,
        starts: &starts,
        applications: &[],
        binders: &[],
        regions: RegionEnvironment::Resolved(&actual_names),
        arguments: RegionEnvironment::Captured(actual.argument_owner),
        output_elisions: &[],
    };
    let required_scope = RegionScope {
        declaration: required_owner,
        formal_declaration: SymbolHandle::invalid(),
        formal_regions: &[],
        substitutions: &[],
        starts: &[],
        applications: &[],
        binders: &[],
        regions: RegionEnvironment::Resolved(&required_names),
        arguments: RegionEnvironment::Captured(required_owner),
        output_elisions: &[],
    };
    let mut initial = InputRegionApplication::default();
    for (ordinal, argument) in constraints {
        if *ordinal >= actual.lifetime_parameters.len()
            || !initial.constrain(
                0,
                TypeReferenceHandle::invalid(),
                Some(RegionIdentity::Fresh(*ordinal)),
                argument.region(),
            )
        {
            return Err("declaration input region constraints conflict");
        }
    }
    let application = infer_correspondences(
        program,
        inputs,
        actual_scope,
        required_scope,
        &[],
        actual.lifetime_parameters.len(),
        invocation_count,
        initial,
    )?;
    project_application(actual.lifetime_parameters, &application)
}

/// Original source inputs have their own simultaneous supplying applications.
/// The reference may include an actual ordinary formed-borrow shell; that
/// shell is type evidence, not permission to execute or retain the loan.
pub struct InvocationInputTypeApplication {
    pub reference: TypeReferenceHandle,
    pub enclosing_data_applications: Vec<TypeReferenceHandle>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum InvocationRegionOperand {
    Captured(typed_trees::type_identity::LifetimeParameterIdentity),
    Anonymous,
}

/// Instantiate one original invocation from its complete source inputs. This
/// is distinct from universal body refinement: genuine anonymous input regions
/// may supply invocation slots, while captured receiver regions remain rigid.
/// Results, provider bodies and equal lifetime spellings supply no constraints.
pub fn infer_requirement_input_lifetime_application(
    program: &typed_trees::TypedTrees,
    signature: &typed_trees::signature::StateSignature,
    receiver: &typed_trees::trait_definition::AppliedTraitScope<'_>,
    member_arguments: &[(SymbolHandle, TypeReferenceHandle)],
    caller: SymbolHandle,
    original_application: typed_trees::static_applications::StaticMachineApplicationHandle,
    inputs: &[InvocationInputTypeApplication],
) -> Result<Vec<typed_trees::static_applications::StaticMachineLifetimeBinding>, &'static str> {
    let parameters = program.state_signature_parameters(signature);
    if parameters.len() != inputs.len()
        || program
            .trait_machine_signature_by_symbol(receiver.declaration, signature.symbol)
            .is_none()
    {
        return Err("requirement input application lost its original signature or arity");
    }
    let arguments = member_arguments
        .iter()
        .map(|(parameter, actual)| {
            (
                *parameter,
                typed_trees::type_identity::IndexIdentityArgument::TypeReference(*actual),
            )
        })
        .collect::<Vec<_>>();
    let mut initial = vec![None; signature.lifetime_parameters.len()];
    if program
        .expression_table
        .static_machine_application_is_valid(original_application)
    {
        let original = program
            .expression_table
            .static_machine_application(original_application);
        if original.registration_operation != signature.symbol
            || (original.binding_declaration.is_valid()
                && original.binding_declaration != signature.symbol)
        {
            return Err("input application lost its original callable occurrence");
        }
        let owner = original
            .type_identity_owner(program)
            .ok_or("input application lost its authentic source owner")?;
        for (ordinal, binding) in initial.iter_mut().enumerate() {
            if let Some(previous) = program.static_application_lifetime_binding(
                original_application,
                signature.symbol,
                ordinal,
            ) {
                *binding = Some(match &previous.argument {
                    typed_trees::static_applications::StaticMachineLifetimeArgument::Named(
                        name,
                    ) => {
                        let region =
                            typed_trees::type_identity::scoped_lifetime_parameter_identity(
                                program,
                                owner,
                                name.as_str(),
                                &[],
                            )
                            .ok_or("retained invocation region lost its supplying owner")?;
                        InvocationRegionOperand::Captured(region)
                    }
                    typed_trees::static_applications::StaticMachineLifetimeArgument::Anonymous => {
                        InvocationRegionOperand::Anonymous
                    }
                });
            }
        }
    } else if original_application.is_valid() {
        return Err("input application carries an unavailable original occurrence");
    }
    let fixed = initial.iter().map(Option::is_some).collect::<Vec<_>>();
    let mut applications = vec![initial];
    let matched = receiver.with_identity_scopes(program, |outer| {
        let mut scopes = outer.to_vec();
        scopes.push(typed_trees::type_identity::AppliedTypeIdentityScope {
            region_role: typed_trees::type_identity::AppliedTypeIdentityRegionRole::Declaration,
            declaration: signature.symbol,
            arguments: &arguments,
            lifetimes: &[],
        });
        let formal = crate::type_regions::ApplicationRegions::source(
            receiver.argument_identity_owner(),
            &scopes,
        );
        for (parameter, input) in parameters.iter().zip(inputs) {
            if !input.reference.is_valid() {
                // This query prepares invocation metadata, not value admission.
                // A formal with no original invocation region contributes no
                // binding constraint. Ordinary checking still owes this value's
                // formation, carrier, access and qualifications. Unknown or
                // regional formals cannot be treated as that empty relation.
                let mut own_region = false;
                let mut complete = vec![()];
                let formed = type_reference_region_applications(
                    program,
                    parameter.type_reference,
                    parameter.type_reference,
                    formal,
                    formal,
                    TypeRegionRelationship::ExactApplication,
                    &mut complete,
                    &mut |_: &mut (),
                          left: crate::type_regions::ApplicationRegions<'_>,
                          _: crate::type_regions::ApplicationRegions<'_>,
                          _: TypeReferenceHandle,
                          name: Option<&Identifier>,
                          _: Option<&Identifier>| {
                        if let Some(name) = name {
                            let Some(region) = left.region(program, name.as_str()) else {
                                return false;
                            };
                            own_region |= region.declaration == signature.symbol;
                        }
                        true
                    },
                );
                if !formed || own_region {
                    return false;
                }
                continue;
            }
            let matched = program.with_enclosing_data_identity_scopes(
                &input.enclosing_data_applications,
                |input_scopes| {
                    let actual = crate::type_regions::ApplicationRegions::source(
                        typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(caller),
                        input_scopes,
                    );
                    let mut constrain =
                        |bindings: &mut Vec<Option<InvocationRegionOperand>>,
                         actual: crate::type_regions::ApplicationRegions<'_>,
                         formal: crate::type_regions::ApplicationRegions<'_>,
                         _reference,
                         actual_name: Option<&Identifier>,
                         formal_name: Option<&Identifier>| {
                            let Some(formal_name) = formal_name else {
                                return true;
                            };
                            let Some(required) = formal.region(program, formal_name.as_str())
                            else {
                                return false;
                            };
                            let supplied = match actual_name {
                                Some(name) => match actual.region(program, name.as_str()) {
                                    Some(region) => InvocationRegionOperand::Captured(region),
                                    None => return false,
                                },
                                None => InvocationRegionOperand::Anonymous,
                            };
                            if required.declaration != signature.symbol {
                                return supplied == InvocationRegionOperand::Captured(required);
                            }
                            let Some(binding) = bindings.get_mut(required.ordinal) else {
                                return false;
                            };
                            if fixed[required.ordinal] {
                                return binding.as_ref() == Some(&supplied);
                            }
                            // Preserve ordinary input elision: a genuinely named
                            // input constrains an otherwise anonymous invocation.
                            // This relation grants no backing or output loan, and
                            // an already-retained occurrence is never overwritten.
                            match (binding.as_ref(), &supplied) {
                                (
                                    Some(InvocationRegionOperand::Captured(previous)),
                                    InvocationRegionOperand::Captured(actual),
                                ) => previous == actual,
                                (
                                    Some(InvocationRegionOperand::Captured(_)),
                                    InvocationRegionOperand::Anonymous,
                                ) => true,
                                _ => {
                                    *binding = Some(supplied);
                                    true
                                }
                            }
                        };
                    type_reference_region_applications(
                        program,
                        input.reference,
                        parameter.type_reference,
                        actual,
                        formal,
                        TypeRegionRelationship::CallableRefinement,
                        &mut applications,
                        &mut constrain,
                    )
                },
            );
            if matched != Some(true) {
                return false;
            }
        }
        true
    });
    if !matched {
        return Err("requirement inputs do not retain their original region correspondence");
    }
    let mut complete = Vec::new();
    for application in applications {
        if let Some(application) = application.into_iter().collect::<Option<Vec<_>>>()
            && !complete.contains(&application)
        {
            complete.push(application);
        }
    }
    let [application] = complete.as_slice() else {
        return Err("requirement inputs do not determine one complete invocation application");
    };
    signature
        .lifetime_parameters
        .iter()
        .zip(application)
        .map(|(parameter, argument)| {
            let argument = match argument {
                InvocationRegionOperand::Captured(region) => {
                    let name =
                        typed_trees::type_identity::lifetime_parameter_name(program, *region)
                            .ok_or("input region lost its original declaration payload")?;
                    if typed_trees::type_identity::lifetime_parameter_identity(
                        program,
                        caller,
                        name.as_str(),
                    ) != Some(*region)
                    {
                        return Err(
                            "input region cannot be represented in its original call owner",
                        );
                    }
                    typed_trees::static_applications::StaticMachineLifetimeArgument::Named(
                        name.clone(),
                    )
                }
                InvocationRegionOperand::Anonymous => {
                    typed_trees::static_applications::StaticMachineLifetimeArgument::Anonymous
                }
            };
            Ok(
                typed_trees::static_applications::StaticMachineLifetimeBinding {
                    parameter: parameter.clone(),
                    argument,
                },
            )
        })
        .collect()
}

impl InputLifetimeArgument {
    fn region(&self) -> RegionIdentity {
        match self {
            Self::Captured(identity) => {
                RegionIdentity::Captured(identity.declaration, identity.ordinal)
            }
            Self::Invocation(ordinal) => RegionIdentity::Fresh(*ordinal),
            Self::AnonymousInvocation(identity) => {
                RegionIdentity::AnonymousInvocation(identity.declaration, identity.ordinal)
            }
        }
    }
}

pub(super) fn project_application(
    parameters: &[Identifier],
    application: &InputRegionApplication,
) -> Result<Vec<InputLifetimeBinding>, &'static str> {
    parameters
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| {
            let argument = match application
                .fresh_argument(ordinal)
                .ok_or("input lifetime application lost a declared slot")?
            {
                RegionIdentity::Fresh(ordinal) => InputLifetimeArgument::Invocation(*ordinal),
                RegionIdentity::Captured(declaration, ordinal) => InputLifetimeArgument::Captured(
                    typed_trees::type_identity::LifetimeParameterIdentity {
                        declaration: *declaration,
                        ordinal: *ordinal,
                    },
                ),
                RegionIdentity::AnonymousInvocation(declaration, ordinal) => {
                    InputLifetimeArgument::AnonymousInvocation(
                        typed_trees::type_identity::LifetimeParameterIdentity {
                            declaration: *declaration,
                            ordinal: *ordinal,
                        },
                    )
                }
                RegionIdentity::Declared(_) => {
                    return Err("input lifetime has no declaration custody");
                }
            };
            Ok(InputLifetimeBinding {
                parameter: parameter.clone(),
                argument,
            })
        })
        .collect()
}

pub(super) fn infer_inputs(
    program: &dyn crate::CheckingInput,
    required: &CallableBorrowContract<'_>,
    actual: &CallableBorrowContract<'_>,
    required_regions: &CallableRegions,
    actual_regions: &CallableRegions,
    actual_inputs: &[Vec<CarriedLifetime>],
) -> Result<InputRegionApplication, &'static str> {
    if required.parameters.len() != actual.parameters.len() {
        return Err("callable input arity does not match its requirement");
    }
    let inputs = actual
        .parameters
        .iter()
        .zip(required.parameters)
        .map(|(actual, required)| (actual.type_reference, required.type_reference))
        .collect::<Vec<_>>();
    infer_correspondences(
        program,
        &inputs,
        contract_scope(actual, actual_regions),
        contract_scope(required, required_regions),
        actual_inputs,
        actual_regions.fresh_count,
        required_regions.fresh_count,
        InputRegionApplication::default(),
    )
}

#[allow(clippy::too_many_arguments)]
fn infer_correspondences(
    program: &dyn crate::CheckingInput,
    inputs: &[(TypeReferenceHandle, TypeReferenceHandle)],
    actual_scope: RegionScope<'_>,
    required_scope: RegionScope<'_>,
    actual_inputs: &[Vec<CarriedLifetime>],
    actual_count: usize,
    required_count: usize,
    initial: InputRegionApplication,
) -> Result<InputRegionApplication, &'static str> {
    let mut applications = vec![initial];
    for (ordinal, (actual_type, required_type)) in inputs.iter().copied().enumerate() {
        let mut failure = "callable input does not preserve its region identity";
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
                    // Nonphysical operands cannot supply an input region. Keep
                    // elision on this parameter's enumerated physical frontier.
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
            // Preserve the semantic axis of the failed application. A rigid
            // captured input cannot supply a universally fresh invocation;
            // conflicting inferred binders cannot merge captured storage with
            // that invocation either. Nonphysical operands carry identity but
            // never acquire a physical loan diagnostic.
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
            false
        };
        if !type_reference_region_applications(
            program,
            actual_type,
            required_type,
            actual_scope,
            required_scope,
            TypeRegionRelationship::CallableRefinement,
            &mut applications,
            &mut constrain,
        ) {
            return Err(failure);
        }
    }
    // Only the complete input tuple and universal telescope settle inference.
    // A domain alternative may look ambiguous before a later input fixes a
    // binder. Results consume this application; they never choose among it.
    let mut completed = Vec::new();
    let mut failure = "callable input regions do not determine one application";
    for mut application in applications {
        match application.finish(actual_count, required_count) {
            Ok(()) if !completed.contains(&application) => completed.push(application),
            Ok(()) => {}
            Err(reason) => failure = reason,
        }
    }
    if completed.len() != 1 {
        return Err(if completed.is_empty() {
            failure
        } else {
            "callable input regions do not determine one application"
        });
    }
    completed.pop().ok_or(failure)
}

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    required: &CallableBorrowContract<'_>,
    actual: &CallableBorrowContract<'_>,
    required_regions: &CallableRegions,
    actual_regions: &CallableRegions,
    input_elisions: &[(usize, TypeReferenceHandle, RegionIdentity)],
    output_elisions: &[(TypeReferenceHandle, RegionIdentity)],
) -> Result<(), &'static str> {
    let required_scope = contract_scope(required, required_regions);
    let actual_scope = contract_scope(actual, actual_regions);
    let mut matches = |_: &mut (),
                       actual_scope: RegionScope<'_>,
                       required_scope: RegionScope<'_>,
                       reference: TypeReferenceHandle,
                       actual: Option<&Identifier>,
                       required: Option<&Identifier>| {
        let Some(required) = required else {
            return true;
        };
        required_scope
            .regions
            .identity(program, required)
            .zip(match actual {
                Some(actual) => actual_scope.regions.identity(program, actual),
                None => actual_scope
                    .output_elisions
                    .iter()
                    .find(|(output, _)| *output == reference)
                    .map(|(_, region)| region.clone()),
            })
            .is_some_and(|(required, actual)| required == actual)
    };
    for (ordinal, (required, actual)) in required
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
        if !type_reference_regions_match(
            program,
            actual,
            required,
            RegionScope {
                output_elisions: &elisions,
                ..actual_scope
            },
            required_scope,
            &mut (),
            &mut matches,
        ) {
            return Err("callable type does not preserve its region identity");
        }
    }
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
        &mut matches,
    ) {
        return Err("callable type does not preserve its region identity");
    }
    Ok(())
}
