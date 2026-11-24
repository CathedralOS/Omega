use super::AppliedTraitScope;
use crate::source_state::TypedTrees;
use crate::source_state::data::TypeParameterKind;
use crate::source_state::name::Identifier;
use crate::source_state::type_identity::lifetime_parameter_identity;
use crate::source_state::type_identity::{
    AppliedTypeIdentityScope, IndexIdentityArgument, NormalizedTypeIdentity,
    applied_index_identity, applied_lifetime_identity, applied_type_identity,
    local_trait_application_identity, portable_applied_index_identity,
    portable_applied_lifetime_identity, portable_applied_type_identity,
};
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppliedDeclarationRegionScope {
    pub(crate) declaration: SymbolHandle,
    pub(crate) argument_owner: SymbolHandle,
    pub(crate) lifetimes: Vec<(String, String)>,
}

impl AppliedDeclarationRegionScope {
    pub(crate) fn new(
        declaration: SymbolHandle,
        argument_owner: SymbolHandle,
        parameters: &[Identifier],
        arguments: &[Identifier],
    ) -> Self {
        Self {
            declaration,
            argument_owner,
            lifetimes: parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| {
                    (parameter.as_str().to_owned(), argument.as_str().to_owned())
                })
                .collect(),
        }
    }
}

impl AppliedTraitScope<'_> {
    pub fn under_invocation(
        mut self,
        program: &TypedTrees,
        declaration: SymbolHandle,
        argument_owner: SymbolHandle,
        arguments: &[(
            SymbolHandle,
            crate::source_state::types::TypeReferenceHandle,
        )],
        bindings: &[crate::source_state::static_applications::StaticMachineLifetimeBinding],
    ) -> Option<Self> {
        let enclosing = program.enclosing_invocation_frames(argument_owner).ok()?;
        let region = invocation_region_scope(program, declaration, argument_owner, bindings)?;
        self.argument_owner = argument_owner;
        self.substitutions.splice(0..0, arguments.iter().copied());
        for start in &mut self.substitution_scope_starts {
            *start += arguments.len();
        }
        self.substitution_scope_starts.insert(0, 0);
        self.region_scopes.insert(0, region);
        for frame in enclosing {
            // Untyped bindings remain unresolved. They cannot close a
            // demanded trait operand, but unrelated traits still form.
            let arguments = frame
                .bindings
                .iter()
                .filter_map(|binding| {
                    use crate::source_state::static_applications::StaticMachineBindingValue;
                    let reference = match &binding.value {
                        StaticMachineBindingValue::Type(reference)
                        | StaticMachineBindingValue::Const(reference) => *reference,
                        StaticMachineBindingValue::Machine(argument)
                        | StaticMachineBindingValue::Conformance(argument) => {
                            argument.type_reference
                        }
                        // Logical evidence supplies no runtime type or region.
                        StaticMachineBindingValue::Proof(_)
                        | StaticMachineBindingValue::RuntimeValue { .. } => return None,
                    };
                    reference
                        .is_valid()
                        .then_some((binding.parameter, reference))
                })
                .collect::<Vec<_>>();
            let region = invocation_region_scope(
                program,
                frame.declaration,
                frame.argument_owner,
                &frame.lifetimes,
            )?;
            self.argument_owner = frame.argument_owner;
            self.substitutions.splice(0..0, arguments.iter().copied());
            for start in &mut self.substitution_scope_starts {
                *start += arguments.len();
            }
            self.substitution_scope_starts.insert(0, 0);
            self.region_scopes.insert(0, region);
        }
        Some(self)
    }

    /// Whether a member of this trait keeps the signature it was
    /// declared with, so no applied signature exists for it.
    ///
    /// `under_invocation` puts the invoking machine's own bindings
    /// in front. A trait member cannot name another machine's
    /// parameters, so those frames leave its signature alone; only
    /// the frames after them, which apply the trait, can change it.
    pub fn keeps_declared_signatures(&self, program: &TypedTrees) -> bool {
        let invocation_frames = self
            .region_scopes
            .iter()
            .take_while(|frame| {
                frame.declaration.is_valid()
                    && program.symbols.get(frame.declaration).kind == symbols::SymbolKind::Machine
            })
            .count();
        self.declaration.lifetime_parameters.is_empty()
            && self
                .substitution_scope_starts
                .get(invocation_frames)
                .is_some_and(|start| *start == self.substitutions.len())
    }

    pub fn portable_application_identity(
        &self,
        program: &TypedTrees,
    ) -> Option<NormalizedTypeIdentity> {
        self.with_identity_scopes(program, |scopes| {
            self.portable_application_identity_in_scopes(program, self.argument_owner, scopes, 0)
        })
    }

    fn portable_application_identity_in_scopes(
        &self,
        program: &TypedTrees,
        argument_owner: SymbolHandle,
        scopes: &[AppliedTypeIdentityScope<'_>],
        preceding: usize,
    ) -> Option<NormalizedTypeIdentity> {
        let arguments = program
            .trait_type_parameters(self.declaration)
            .iter()
            .map(|parameter| {
                let (reference, outer) = self.parameter_application(parameter.symbol)?;
                let supplying = &scopes[..outer + preceding];
                if parameter.kind.is_const_or_value()
                    || matches!(parameter.kind, TypeParameterKind::Machine { .. })
                {
                    portable_applied_index_identity(
                        program,
                        IndexIdentityArgument::TypeReference(reference),
                        argument_owner,
                        supplying,
                    )
                } else {
                    portable_applied_type_identity(program, reference, argument_owner, supplying)
                }
            })
            .collect::<Option<Vec<_>>>()?;
        let lifetimes = self
            .declaration
            .lifetime_parameters
            .iter()
            .map(|parameter| {
                portable_applied_lifetime_identity(
                    program,
                    parameter.as_str(),
                    self.declaration.symbol,
                    scopes,
                )
            })
            .collect::<Option<Vec<_>>>()?;
        crate::source_state::type_identity::portable_trait_application_identity(
            program,
            self.declaration.symbol,
            &arguments,
            &lifetimes,
        )
    }

    pub fn portable_parameter_identity(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
    ) -> Option<NormalizedTypeIdentity> {
        let (reference, outer) = self.parameter_application(parameter)?;
        self.with_identity_scopes(program, |scopes| {
            let binder = program
                .trait_type_parameters(self.declaration)
                .iter()
                .find(|binder| binder.symbol == parameter)?;
            if matches!(
                binder.kind,
                TypeParameterKind::Const { .. }
                    | TypeParameterKind::Value { .. }
                    | TypeParameterKind::Machine { .. }
            ) {
                portable_applied_index_identity(
                    program,
                    IndexIdentityArgument::TypeReference(reference),
                    self.argument_owner,
                    &scopes[..outer],
                )
            } else {
                portable_applied_type_identity(
                    program,
                    reference,
                    self.argument_owner,
                    &scopes[..outer],
                )
            }
        })
    }

    pub fn portable_lifetime_arguments(
        &self,
        program: &TypedTrees,
    ) -> Option<Vec<NormalizedTypeIdentity>> {
        self.with_identity_scopes(program, |scopes| {
            self.declaration
                .lifetime_parameters
                .iter()
                .map(|parameter| {
                    portable_applied_lifetime_identity(
                        program,
                        parameter.as_str(),
                        self.declaration.symbol,
                        scopes,
                    )
                })
                .collect()
        })
    }

    pub fn parameter_application(
        &self,
        parameter: SymbolHandle,
    ) -> Option<(crate::source_state::types::TypeReferenceHandle, usize)> {
        let (ordinal, (_, reference)) = self
            .substitutions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (binder, _))| *binder == parameter)?;
        let outer = self
            .substitution_scope_starts
            .partition_point(|start| *start <= ordinal)
            .checked_sub(1)?;
        Some((*reference, outer))
    }

    pub fn argument_identity(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
    ) -> Option<NormalizedTypeIdentity> {
        let (reference, outer) = self.parameter_application(parameter)?;
        let binder = program
            .trait_type_parameters(self.declaration)
            .iter()
            .find(|binder| binder.symbol == parameter)?;
        self.with_identity_scopes(program, |scopes| {
            if binder.kind.is_const_or_value()
                || matches!(binder.kind, TypeParameterKind::Machine { .. })
            {
                applied_index_identity(
                    program,
                    IndexIdentityArgument::TypeReference(reference),
                    self.argument_owner,
                    &scopes[..outer],
                )
            } else {
                applied_type_identity(program, reference, self.argument_owner, &scopes[..outer])
            }
        })
    }

    pub fn with_identity_scopes<Result>(
        &self,
        program: &TypedTrees,
        use_scopes: impl FnOnce(&[AppliedTypeIdentityScope<'_>]) -> Result,
    ) -> Result {
        let arguments = self
            .substitution_scope_starts
            .iter()
            .enumerate()
            .map(|(ordinal, start)| {
                let end = self
                    .substitution_scope_starts
                    .get(ordinal + 1)
                    .copied()
                    .unwrap_or(self.substitutions.len());
                self.substitutions[*start..end]
                    .iter()
                    .map(|(parameter, argument)| {
                        let is_type = program
                            .data_type_parameter_by_symbol(*parameter)
                            .is_some_and(|binder| binder.kind == TypeParameterKind::Type);
                        (
                            *parameter,
                            if is_type {
                                IndexIdentityArgument::TypeArgument(*argument)
                            } else {
                                IndexIdentityArgument::TypeReference(*argument)
                            },
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let scopes = self
            .region_scopes
            .iter()
            .zip(&arguments)
            .map(|(region, arguments)| AppliedTypeIdentityScope {
                declaration: region.declaration,
                argument_owner: region.argument_owner,
                arguments,
                lifetimes: &region.lifetimes,
            })
            .collect::<Vec<_>>();
        use_scopes(&scopes)
    }

    pub fn application_identity(&self, program: &TypedTrees) -> Option<NormalizedTypeIdentity> {
        self.with_identity_scopes(program, |scopes| {
            let arguments = program
                .trait_type_parameters(self.declaration)
                .iter()
                .map(|parameter| {
                    let Some((reference, scope)) = self.parameter_application(parameter.symbol)
                    else {
                        return program.normalized_nominal_type_identity(parameter.symbol);
                    };
                    if matches!(
                        parameter.kind,
                        TypeParameterKind::Const { .. }
                            | TypeParameterKind::Value { .. }
                            | TypeParameterKind::Machine { .. }
                    ) {
                        applied_index_identity(
                            program,
                            IndexIdentityArgument::TypeReference(reference),
                            self.argument_owner,
                            &scopes[..scope],
                        )
                    } else {
                        applied_type_identity(
                            program,
                            reference,
                            self.argument_owner,
                            &scopes[..scope],
                        )
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            let lifetimes = self
                .declaration
                .lifetime_parameters
                .iter()
                .map(|parameter| {
                    applied_lifetime_identity(
                        program,
                        parameter.as_str(),
                        self.declaration.symbol,
                        scopes,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            local_trait_application_identity(
                program,
                self.declaration.symbol,
                &arguments,
                &lifetimes,
            )
        })
    }
}

fn invocation_region_scope(
    program: &TypedTrees,
    declaration: SymbolHandle,
    argument_owner: SymbolHandle,
    bindings: &[crate::source_state::static_applications::StaticMachineLifetimeBinding],
) -> Option<AppliedDeclarationRegionScope> {
    let declaration = program.lifetime_declaration_owner(declaration);
    let mut lifetimes = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let parameter =
            lifetime_parameter_identity(program, declaration, binding.parameter.as_str())?;
        if parameter.declaration != declaration
            || lifetimes
                .iter()
                .any(|(previous, _)| previous == binding.parameter.as_str())
        {
            return None;
        }
        lifetime_parameter_identity(program, argument_owner, binding.argument.as_str())?;
        lifetimes.push((
            binding.parameter.as_str().to_owned(),
            binding.argument.as_str().to_owned(),
        ));
    }
    Some(AppliedDeclarationRegionScope {
        declaration,
        argument_owner,
        lifetimes,
    })
}
