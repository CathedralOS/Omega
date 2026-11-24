use super::TraitDefinition;
use crate::source_state::TypedTrees;
use crate::source_state::name::Identifier;
use crate::source_state::type_identity::{
    NormalizedTypeIdentity, TypeIdentityQualification, TypeIdentityRequest,
};
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

pub(crate) mod regions;
use regions::AppliedDeclarationRegionScope;
pub(crate) mod enclosing;
pub(crate) mod evidence;
pub use evidence::GenericBoundRequirement;
pub(crate) mod refinement_views;
pub(crate) mod retention;
mod service_reach;

pub(crate) use retention::RetainedTraitApplicationFrame;
pub use retention::{RetainedTraitScope, TraitApplicationEnvironment};

impl TypedTrees {
    pub fn lexical_conformance_scope(
        &self,
        caller: SymbolHandle,
    ) -> Option<(&super::Conformance, AppliedTraitScope<'_>)> {
        let authored = self.lifetime_declaration_owner(caller);
        let selecting = self.conformance_by_symbol(self.declaration_scope_parent(caller))?;
        if !selecting
            .rows
            .iter()
            .any(|row| row.source.owns_realization() && row.realization_machine == authored)
        {
            return None;
        }
        let scope = if let Some(capture) = self.enclosing_map_capture(caller).ok()? {
            if capture.conformance_application.declaration != selecting.symbol {
                return None;
            }
            capture.conformance_application.trait_scope.scope(self)?
        } else {
            if caller != authored
                && (!self.conformance_type_parameters(selecting).is_empty()
                    || !selecting.lifetime_parameters.is_empty())
            {
                return None;
            }
            AppliedTraitScope::from_conformance(self, selecting)?
        };
        Some((selecting, scope))
    }

    pub fn machine_conformance_requirement_scope(
        &self,
        machine: SymbolHandle,
        declaring_trait: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Result<Option<AppliedTraitScope<'_>>, &'static str> {
        let Some((implementation, scope)) = self.lexical_conformance_scope(machine) else {
            return Ok(None);
        };
        let authored = self.lifetime_declaration_owner(machine);
        let mut rows = implementation.rows.iter().filter(|row| {
            row.source.owns_realization()
                && row.realization_machine == authored
                && row.declaring_trait == declaring_trait
                && row.requirement == requirement
        });
        let Some(row) = rows.next() else {
            return Ok(None);
        };
        if rows.next().is_some() {
            return Err("inline member repeats an exact requirement row");
        }
        let entry = self
            .machine_by_symbol(authored)
            .and_then(|machine| self.machine_states(machine).first())
            .ok_or("inline requirement member lost its authored entry")?;
        if row.realization_state != entry.symbol {
            return Err("inline requirement row does not name its member entry");
        }
        scope
            .declaring_scope(self, declaring_trait)
            .map(Some)
            .ok_or("inline requirement row lost its exact declaring application")
    }

    pub fn applied_trait_roots(&self) -> Vec<AppliedTraitScope<'_>> {
        let occurrences =
            crate::source_state::references::collect_program_application_occurrences(self);
        self.machines()
            .iter()
            .flat_map(|machine| {
                self.machine_trait_conformances(machine)
                    .iter()
                    .filter_map(move |edge| {
                        AppliedTraitScope::from_machine_conformance(self, machine, edge)
                    })
            })
            .chain(
                self.conformances()
                    .iter()
                    .filter_map(|edge| AppliedTraitScope::from_conformance(self, edge)),
            )
            .chain(
                self.machines()
                    .iter()
                    .map(|machine| (machine.symbol, &machine.conformance_bounds))
                    .chain(
                        self.traits().iter().map(|declaration| {
                            (declaration.symbol, &declaration.conformance_bounds)
                        }),
                    )
                    .flat_map(|(owner, bounds)| {
                        bounds.iter().filter_map(move |bound| {
                            AppliedTraitScope::new(
                                self,
                                self.trait_definition_by_symbol(bound.trait_symbol)?,
                                TraitApplicationArguments {
                                    lexical_owner: owner,
                                    enclosing_data_applications: &[],
                                    arguments: &bound.arguments,
                                    lifetimes: &bound.lifetime_arguments,
                                },
                            )
                        })
                    }),
            )
            .chain(
                self.traits()
                    .iter()
                    .filter(|declaration| self.trait_type_parameters(declaration).is_empty())
                    .map(AppliedTraitScope::abstract_scope),
            )
            .chain(
                self.closed_conformance_application_sites_from_occurrences(
                    &occurrences.conformance_applications,
                )
                .into_iter()
                .filter_map(|(_, _, _, application)| application.trait_scope.scope(self)),
            )
            .chain(
                self.live_product_operand_applications()
                    .into_iter()
                    .filter_map(|handle| {
                        let application = self.product_operand_applications.get(handle);
                        matches!(
                            self.product_operand_application_role(application),
                            Ok(Some(
                                crate::declaration_selection::ProductOperandRole::BoundarySubject
                                    | crate::declaration_selection::ProductOperandRole::Requirement
                                    | crate::declaration_selection::ProductOperandRole::TestRequirement
                            ))
                        )
                        .then(|| application.scope.scope(self))
                        .flatten()
                    }),
            )
            .chain(self.service_reaches.all_definitions().filter_map(|(_, row)| {
                row.application.as_ref()?.arguments.scope(self)
            }))
            .chain(
                occurrences
                    .requirement_applications
                    .into_iter()
                    .filter_map(|occurrence| {
                        self.applied_requirement_scope(
                            occurrence.lexical_owner,
                            occurrence.reference,
                        )
                    }),
            )
            .collect()
    }

    pub fn applied_requirement_scope(
        &self,
        lexical_owner: SymbolHandle,
        application: TypeReferenceHandle,
    ) -> Option<AppliedTraitScope<'_>> {
        if let TypeReferenceNode::Named { symbol, .. } =
            self.type_reference_table.type_reference(application)
        {
            let selected = self.conformance_by_symbol(*symbol)?;
            // A missing generic tuple cannot become a zero-argument
            // selection. Only a genuinely closed declaration has this view.
            if !selected.type_parameters.is_empty() || !selected.lifetime_parameters.is_empty() {
                return None;
            }
            return AppliedTraitScope::from_conformance(self, selected);
        }
        let TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = self.type_reference_table.type_reference(application)
        else {
            return None;
        };
        // A named map's application binds its own telescope first;
        // its trait arguments can then refer to those exact binders.
        if let Some(conformance) = self.conformance_by_symbol(*base_symbol) {
            return AppliedTraitScope::from_conformance_application(
                self,
                conformance,
                TraitApplicationArguments {
                    lexical_owner,
                    enclosing_data_applications: &[],
                    arguments: self.type_reference_table.type_reference_handles(*arguments),
                    lifetimes: lifetime_arguments,
                },
            );
        }
        AppliedTraitScope::new(
            self,
            self.trait_definition_by_symbol(*base_symbol)?,
            TraitApplicationArguments {
                lexical_owner,
                enclosing_data_applications: &[],
                arguments: self.type_reference_table.type_reference_handles(*arguments),
                lifetimes: lifetime_arguments,
            },
        )
    }
}

#[derive(Clone, Copy)]
pub struct TraitApplicationArguments<'arguments> {
    pub lexical_owner: SymbolHandle,
    pub enclosing_data_applications: &'arguments [TypeReferenceHandle],
    pub arguments: &'arguments [TypeReferenceHandle],
    pub lifetimes: &'arguments [Identifier],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AppliedTypeReference<'scope> {
    pub reference: TypeReferenceHandle,
    substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &'scope [usize],
}

impl<'scope> AppliedTypeReference<'scope> {
    pub fn parameter_argument(self, program: &TypedTrees, parameter: SymbolHandle) -> Option<Self> {
        let (ordinal, (_, reference)) = self
            .substitutions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (binder, _))| *binder == parameter)?;
        let visible_scopes = self
            .scope_starts
            .partition_point(|start| *start <= ordinal)
            .checked_sub(1)?;
        let visible = self.scope_starts.get(visible_scopes).copied()?;
        Some(project_type_reference(
            program,
            *reference,
            &self.substitutions[..visible],
            &self.scope_starts[..visible_scopes],
        ))
    }

    pub fn child(self, program: &TypedTrees, reference: TypeReferenceHandle) -> Self {
        project_type_reference(program, reference, self.substitutions, self.scope_starts)
    }

    pub fn type_identity(self, program: &TypedTrees) -> NormalizedTypeIdentity {
        program.scoped_type_identity(
            TypeIdentityRequest {
                substitutions: self.substitutions,
                ..TypeIdentityRequest::ordinary(self.reference)
            },
            self.scope_starts,
        )
    }

    pub fn multiplicity(self, program: &TypedTrees) -> language_core::Multiplicity {
        if !self.reference.is_valid() {
            return language_core::Multiplicity::Affine;
        }
        match program.type_reference_table.type_reference(self.reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.child(program, *base_type).multiplicity(program)
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                self.child(program, *element_type).multiplicity(program)
            }
            _ => program.type_multiplicity(self.reference),
        }
    }
}

fn project_type_reference<'scope>(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &'scope [usize],
) -> AppliedTypeReference<'scope> {
    let mut visible = substitutions.len();
    let mut visible_scopes = scope_starts.len();
    if reference.is_valid() {
        while let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        {
            let Some((ordinal, (_, replacement))) = substitutions[..visible]
                .iter()
                .enumerate()
                .rev()
                .find(|(_, (parameter, _))| parameter == symbol)
            else {
                break;
            };
            visible_scopes = scope_starts
                .partition_point(|start| *start <= ordinal)
                .saturating_sub(1);
            visible = scope_starts.get(visible_scopes).copied().unwrap_or(0);
            reference = *replacement;
        }
    }
    AppliedTypeReference {
        reference,
        substitutions: &substitutions[..visible],
        scope_starts: &scope_starts[..visible_scopes],
    }
}

#[derive(Clone)]
pub struct AppliedTraitScope<'program> {
    pub declaration: &'program TraitDefinition,
    pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    pub substitution_scope_starts: Vec<usize>,
    pub argument_owner: SymbolHandle,
    region_scopes: Vec<AppliedDeclarationRegionScope>,
}

impl<'program> AppliedTraitScope<'program> {
    pub fn from_type_reference(
        program: &'program TypedTrees,
        reference: TypeReferenceHandle,
        lexical_owner: SymbolHandle,
    ) -> Option<Self> {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                Self::from_type_reference(program, *referee, lexical_owner)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                Self::from_type_reference(program, *base_type, lexical_owner)
            }
            TypeReferenceNode::Named { symbol, .. } => Some(Self::abstract_scope(
                program.trait_definition_by_symbol(*symbol)?,
            )),
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                lifetime_arguments,
                ..
            } => Self::new(
                program,
                program.trait_definition_by_symbol(*base_symbol)?,
                TraitApplicationArguments {
                    lexical_owner,
                    enclosing_data_applications: &[],
                    arguments: program.type_reference_handles(*arguments),
                    lifetimes: lifetime_arguments,
                },
            ),
            _ => None,
        }
    }

    pub fn from_conformance_application(
        program: &'program TypedTrees,
        conformance: &super::Conformance,
        application: TraitApplicationArguments<'_>,
    ) -> Option<Self> {
        let parameters = program.conformance_type_parameters(conformance);
        if parameters.len() != application.arguments.len()
            || conformance.lifetime_parameters.len() != application.lifetimes.len()
        {
            return None;
        }
        let mut scope = Self::from_conformance(program, conformance)?;
        for start in &mut scope.substitution_scope_starts {
            *start += parameters.len();
        }
        scope.substitution_scope_starts.insert(0, 0);
        scope.substitutions.splice(
            0..0,
            parameters
                .iter()
                .zip(application.arguments)
                .map(|(parameter, actual)| (parameter.symbol, *actual)),
        );
        scope.region_scopes.insert(
            0,
            AppliedDeclarationRegionScope::new(
                conformance.symbol,
                application.lexical_owner,
                &conformance.lifetime_parameters,
                application.lifetimes,
            ),
        );
        scope.argument_owner = application.lexical_owner;
        Some(scope)
    }

    pub fn from_conformance(
        program: &'program TypedTrees,
        conformance: &super::Conformance,
    ) -> Option<Self> {
        let declaration = program.trait_definition_by_symbol(conformance.trait_symbol)?;
        let lifetimes = conformance
            .trait_lifetime_arguments
            .iter()
            .map(|ordinal| {
                conformance
                    .lifetime_parameters
                    .get(usize::try_from(*ordinal).ok()?)
                    .cloned()
            })
            .collect::<Option<Vec<_>>>()?;
        Self::new(
            program,
            declaration,
            TraitApplicationArguments {
                lexical_owner: conformance.symbol,
                enclosing_data_applications: &[],
                arguments: program.type_reference_handles(conformance.arguments),
                lifetimes: &lifetimes,
            },
        )
    }

    pub fn from_machine_conformance(
        program: &'program TypedTrees,
        machine: &crate::source_state::machine::Machine,
        conformance: &crate::source_state::machine::TraitConformance,
    ) -> Option<Self> {
        let declaration = program.trait_definition_by_symbol(conformance.symbol)?;
        let owner = program.lifetime_declaration_owner(machine.symbol);
        let original = program.machine_by_symbol(owner)?;
        let lifetimes = conformance
            .trait_lifetime_arguments
            .iter()
            .map(|ordinal| {
                original
                    .lifetime_parameters
                    .get(usize::try_from(*ordinal).ok()?)
                    .cloned()
            })
            .collect::<Option<Vec<_>>>()?;
        Self::new(
            program,
            declaration,
            TraitApplicationArguments {
                lexical_owner: owner,
                enclosing_data_applications: &[],
                arguments: program.type_reference_handles(conformance.arguments),
                lifetimes: &lifetimes,
            },
        )
    }

    pub fn new(
        program: &TypedTrees,
        declaration: &'program TraitDefinition,
        application: TraitApplicationArguments<'_>,
    ) -> Option<Self> {
        let parameters = program.trait_type_parameters(declaration);
        if parameters.len() != application.arguments.len()
            || declaration.lifetime_parameters.len() != application.lifetimes.len()
        {
            return None;
        }
        let mut scope = Self {
            declaration,
            substitutions: Vec::new(),
            substitution_scope_starts: Vec::new(),
            argument_owner: application.lexical_owner,
            region_scopes: Vec::new(),
        };
        for reference in application.enclosing_data_applications {
            enclosing::append_data_application(program, *reference, &mut scope)?;
        }
        scope
            .substitution_scope_starts
            .push(scope.substitutions.len());
        scope.substitutions.extend(
            parameters
                .iter()
                .zip(application.arguments)
                .map(|(parameter, argument)| (parameter.symbol, *argument)),
        );
        scope.region_scopes.push(AppliedDeclarationRegionScope::new(
            declaration.symbol,
            application.lexical_owner,
            &declaration.lifetime_parameters,
            application.lifetimes,
        ));
        Some(scope)
    }

    pub fn abstract_scope(declaration: &'program TraitDefinition) -> Self {
        Self {
            declaration,
            substitutions: Vec::new(),
            substitution_scope_starts: vec![0],
            argument_owner: declaration.symbol,
            region_scopes: vec![AppliedDeclarationRegionScope::new(
                declaration.symbol,
                declaration.symbol,
                &declaration.lifetime_parameters,
                &[],
            )],
        }
    }

    pub fn type_identity(
        &self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        program.scoped_type_identity(
            TypeIdentityRequest {
                substitutions: &self.substitutions,
                ..TypeIdentityRequest::ordinary(reference)
            },
            &self.substitution_scope_starts,
        )
    }

    pub fn package_qualified_type_identity(
        &self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        program.scoped_type_identity(
            TypeIdentityRequest {
                substitutions: &self.substitutions,
                ..TypeIdentityRequest::package_qualified(reference)
            },
            &self.substitution_scope_starts,
        )
    }

    pub fn argument_reference(
        &self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        self.type_reference(program, reference).reference
    }

    pub fn type_reference(
        &self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> AppliedTypeReference<'_> {
        project_type_reference(
            program,
            reference,
            &self.substitutions,
            &self.substitution_scope_starts,
        )
    }

    pub(crate) fn supplied_type_reference(
        &self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        preceding_frames: usize,
    ) -> Option<AppliedTypeReference<'_>> {
        let starts = self.substitution_scope_starts.get(..preceding_frames)?;
        let end = self
            .substitution_scope_starts
            .get(preceding_frames)
            .copied()
            .unwrap_or(self.substitutions.len());
        Some(project_type_reference(
            program,
            reference,
            self.substitutions.get(..end)?,
            starts,
        ))
    }

    pub fn parameter_argument(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
    ) -> TypeReferenceHandle {
        self.supplied_parameter_application(program, parameter)
            .map_or(TypeReferenceHandle::invalid(), |(reference, _)| reference)
    }

    /// A forwarded argument belongs to the frame that supplied its source
    /// reference. Nested types and lifetimes still depend on the outer frames.
    pub fn supplied_parameter_application(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
    ) -> Option<(TypeReferenceHandle, usize)> {
        let (reference, outer) = self.parameter_application(parameter)?;
        let supplied = self.supplied_type_reference(program, reference, outer)?;
        Some((supplied.reference, supplied.scope_starts.len()))
    }

    pub fn same_application(&self, program: &TypedTrees, other: &Self) -> bool {
        self.declaration.symbol == other.declaration.symbol
            && self
                .application_identity(program)
                .zip(other.application_identity(program))
                .is_some_and(|(left, right)| left == right)
    }

    /// Nominate the closed operation record supplied to this exact
    /// implementation binder. The retained substitution fixes source
    /// identity; this proves no refinement, law, or body property.
    pub fn named_implementation_argument(
        &self,
        program: &'program TypedTrees,
        parameter: SymbolHandle,
    ) -> Result<&'program super::Conformance, &'static str> {
        let declared = program
            .trait_type_parameters(self.declaration)
            .iter()
            .find(|declared| declared.symbol == parameter)
            .ok_or("implementation binder is not declared by this trait scope")?;
        if declared.kind != crate::source_state::data::TypeParameterKind::Implementation {
            return Err("trait argument is not an implementation binder");
        }
        let mut bounds = self
            .declaration
            .conformance_bounds
            .iter()
            .filter(|bound| bound.binder == parameter);
        let (Some(bound), None) = (bounds.next(), bounds.next()) else {
            return Err("implementation binder has no unique exact bound");
        };
        if !bound.arguments.is_empty() || !bound.lifetime_arguments.is_empty() {
            return Err("applied implementation bound needs closed source preparation");
        }
        let argument = self.parameter_argument(program, parameter);
        if !program
            .type_reference_table
            .contains_type_reference(argument)
        {
            return Err("implementation argument is not closed in its source scope");
        }
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(argument)
        else {
            return Err("applied implementation argument needs closed source preparation");
        };
        let selected = program
            .conformance_by_symbol(*symbol)
            .ok_or("implementation argument does not name an exact operation record")?;
        if selected.trait_symbol != bound.trait_symbol {
            return Err("implementation argument supplies another nominal trait");
        }
        if !selected.arguments.is_empty()
            || !selected.trait_lifetime_arguments.is_empty()
            || !selected.lifetime_parameters.is_empty()
            || !program.conformance_type_parameters(selected).is_empty()
        {
            return Err("generic implementation argument needs closed source preparation");
        }
        Ok(selected)
    }

    pub fn parameter_identity(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
        qualification: TypeIdentityQualification,
    ) -> NormalizedTypeIdentity {
        let Some((request, scopes)) = self.parameter_identity_request(parameter, qualification)
        else {
            return program
                .normalized_nominal_type_identity(parameter)
                .unwrap_or_default();
        };
        program.scoped_type_identity(request, scopes)
    }

    fn parameter_identity_request(
        &self,
        parameter: SymbolHandle,
        qualification: TypeIdentityQualification,
    ) -> Option<(TypeIdentityRequest<'_>, &[usize])> {
        let (ordinal, (_, reference)) = self
            .substitutions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (binder, _))| *binder == parameter)?;
        let outer = self
            .substitution_scope_starts
            .iter()
            .rev()
            .find(|start| **start <= ordinal)
            .copied()
            .unwrap_or(0);
        Some((
            TypeIdentityRequest {
                substitutions: &self.substitutions[..outer],
                qualification,
                ..TypeIdentityRequest::ordinary(*reference)
            },
            &self.substitution_scope_starts[..self
                .substitution_scope_starts
                .partition_point(|start| *start < outer)],
        ))
    }

    pub fn declaring_scope(
        &self,
        program: &'program TypedTrees,
        declaration: SymbolHandle,
    ) -> Option<Self> {
        let mut selected: Option<Self> = None;
        let mut ambiguous = false;
        self.visit(program, &mut |scope| {
            if scope.declaration.symbol != declaration {
                return;
            }
            if let Some(known) = &selected {
                ambiguous |= !known.same_application(program, scope);
            } else {
                selected = Some(scope.clone());
            }
        });
        if ambiguous { None } else { selected }
    }

    pub fn bound_scope(&self, program: &'program TypedTrees, binder: SymbolHandle) -> Option<Self> {
        let mut bounds = self
            .declaration
            .conformance_bounds
            .iter()
            .filter(|bound| bound.binder == binder);
        let bound = bounds.next()?;
        if bounds.next().is_some() {
            return None;
        }
        let nested = Self::new(
            program,
            program.trait_definition_by_symbol(bound.trait_symbol)?,
            TraitApplicationArguments {
                lexical_owner: self.declaration.symbol,
                enclosing_data_applications: &[],
                arguments: &bound.arguments,
                lifetimes: &bound.lifetime_arguments,
            },
        )?;
        Some(nested.preceded_by(self))
    }

    fn preceded_by(mut self, outer: &Self) -> Self {
        let mut substitutions = outer.substitutions.clone();
        self.substitution_scope_starts = outer.substitution_scope_starts.clone();
        self.substitution_scope_starts.push(substitutions.len());
        substitutions.append(&mut self.substitutions);
        self.substitutions = substitutions;
        let mut regions = outer.region_scopes.clone();
        regions.append(&mut self.region_scopes);
        self.region_scopes = regions;
        self.argument_owner = outer.argument_owner;
        self
    }

    /// A retained service needs every declared parent application.
    /// Return missing declarations or incomplete argument frames to
    /// that producer instead of silently publishing a smaller graph.
    pub fn parent_scopes(&self, program: &'program TypedTrees) -> Result<Vec<Self>, &'static str> {
        let parents = program.trait_requirements(self.declaration);
        if parents.len() != self.declaration.requires.len() {
            return Err("applied trait lost its complete parent declaration span");
        }
        parents
            .iter()
            .chain(self.declaration.refines.iter())
            .map(|parent| self.parent_scope(program, parent))
            .collect()
    }

    fn parent_scope(
        &self,
        program: &'program TypedTrees,
        parent: &super::TraitRequirement,
    ) -> Result<Self, &'static str> {
        let declaration = program
            .trait_definition_by_symbol(parent.symbol)
            .ok_or("applied trait parent has no exact declaration")?;
        let scope = Self::new(
            program,
            declaration,
            TraitApplicationArguments {
                lexical_owner: self.declaration.symbol,
                enclosing_data_applications: &[],
                arguments: program.type_reference_handles(parent.arguments),
                lifetimes: &parent.lifetime_arguments,
            },
        )
        .ok_or("applied trait parent has no complete type and lifetime application")?;
        Ok(scope.preceded_by(self))
    }

    /// Visit the complete applied inheritance graph or report its
    /// missing relationship. Parent formation has one owner.
    pub fn try_visit(
        &self,
        program: &'program TypedTrees,
        visit: &mut impl FnMut(&Self) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        fn walk<'p>(
            scope: &AppliedTraitScope<'p>,
            program: &'p TypedTrees,
            active: &mut Vec<SymbolHandle>,
            visit: &mut impl FnMut(&AppliedTraitScope<'p>) -> Result<(), &'static str>,
        ) -> Result<(), &'static str> {
            if active.contains(&scope.declaration.symbol) {
                return Err("applied trait traversal has an inherited declaration cycle");
            }
            active.push(scope.declaration.symbol);
            visit(scope)?;
            for parent in scope.parent_scopes(program)? {
                walk(&parent, program, active, visit)?;
            }
            active.pop();
            Ok(())
        }
        walk(self, program, &mut Vec::new(), visit)
    }

    pub fn visit(&self, program: &'program TypedTrees, visit: &mut impl FnMut(&Self)) {
        self.visit_inherited(program, &mut Vec::new(), visit, &mut |_| {});
    }

    pub fn visit_parents_first(
        &self,
        program: &'program TypedTrees,
        visit: &mut impl FnMut(&Self),
    ) {
        self.visit_inherited(program, &mut Vec::new(), &mut |_| {}, visit);
    }

    fn visit_inherited(
        &self,
        program: &'program TypedTrees,
        active: &mut Vec<SymbolHandle>,
        visit: &mut impl FnMut(&Self),
        after: &mut impl FnMut(&Self),
    ) {
        if active.contains(&self.declaration.symbol) {
            return;
        }
        active.push(self.declaration.symbol);
        visit(self);
        for parent in program
            .trait_requirements(self.declaration)
            .iter()
            .chain(self.declaration.refines.iter())
        {
            let Ok(scope) = self.parent_scope(program, parent) else {
                continue;
            };
            scope.visit_inherited(program, active, visit, after);
        }
        after(self);
        active.pop();
    }
}
