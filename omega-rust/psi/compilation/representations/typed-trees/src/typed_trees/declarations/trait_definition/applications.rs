//! Read-only applied trait scopes shared by operator selection and requirement
//! calls. Each inherited edge extends the lexical substitution chain; it never
//! manufactures an instantiated type. Distinct paths remain distinct until a
//! consumer compares complete applications, rather than a rendered argument or
//! the first reachable declaring trait.

use super::TraitDefinition;
use crate::TypedTrees;
use crate::name::Identifier;
use crate::type_identity::{
    NormalizedTypeIdentity, TypeIdentityQualification, TypeIdentityRequest,
};
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

mod regions;
use regions::AppliedDeclarationRegionScope;
mod enclosing;
mod retention;
mod source_fields;
pub(crate) use retention::RetainedTraitApplicationFrame;
pub use retention::{RetainedTraitScope, TraitApplicationEnvironment};

impl TypedTrees {
    /// Source-owned application roots shared by logical signature preparation
    /// and executable requirement preparation. Keeping the complete supplying
    /// scope prevents either consumer from rebuilding an application from an
    /// erased trait symbol and losing named-map or machine-region custody.
    /// This inventory does not select a provider or admit an open application.
    pub fn applied_trait_roots(&self) -> Vec<AppliedTraitScope<'_>> {
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
            .chain(self.machines().iter().flat_map(|machine| {
                machine.conformance_bounds.iter().filter_map(move |bound| {
                    AppliedTraitScope::new(
                        self,
                        self.trait_definition_by_symbol(bound.trait_symbol)?,
                        TraitApplicationArguments {
                            lexical_owner: machine.symbol,
                            enclosing_data_applications: &[],
                            arguments: &bound.arguments,
                            lifetimes: &[],
                        },
                    )
                })
            }))
            .chain(
                self.traits()
                    .iter()
                    .filter(|declaration| self.trait_type_parameters(declaration).is_empty())
                    .map(AppliedTraitScope::abstract_scope),
            )
            .chain(
                self.closed_conformance_application_sites()
                    .into_iter()
                    .filter_map(|(_, _, _, application)| application.trait_scope.scope(self)),
            )
            .chain(source_fields::service_application_roots(self))
            .chain(
                crate::references::collect_program_call_requirement_applications(self)
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

    /// The trait application `Requirement<Arguments>` written before `::` in
    /// an applied requirement call, supplied by the call's lexical owner. A
    /// qualifier that is not a complete application of a trait has no scope.
    pub fn applied_requirement_scope(
        &self,
        lexical_owner: SymbolHandle,
        application: TypeReferenceHandle,
    ) -> Option<AppliedTraitScope<'_>> {
        let TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = self.type_reference_table.type_reference(application)
        else {
            return None;
        };
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

/// The supplying declaration owns every actual, including regions nested in
/// type arguments. A runtime carrier or a trait declaration cannot supply this
/// custody on behalf of the application site.
#[derive(Clone, Copy)]
pub struct TraitApplicationArguments<'arguments> {
    pub lexical_owner: SymbolHandle,
    /// Exact outer-to-inner record applications containing this occurrence.
    /// A bare declaration cannot stand in for its caller's supplied regions.
    pub enclosing_data_applications: &'arguments [TypeReferenceHandle],
    pub arguments: &'arguments [TypeReferenceHandle],
    pub lifetimes: &'arguments [Identifier],
}

/// A borrowed reference and its remaining lexical scopes. Resolving a binder
/// narrows the visible scopes before its compound argument is traversed: the
/// argument's children must not see the telescope that argument replaces.
/// This is invocation-local inspection, not an instantiated arena node.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AppliedTypeReference<'scope> {
    pub reference: TypeReferenceHandle,
    substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &'scope [usize],
}

impl<'scope> AppliedTypeReference<'scope> {
    /// A static binder's actual sees only preceding simultaneous frames.
    /// Closure and identity consumers borrow this view rather than flattening
    /// a compound actual into a bare arena handle.
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

    /// Applied array/constrained carriers take the resolved child's
    /// multiplicity; nominal carriers keep the declaration's own judgment.
    pub fn multiplicity(self, program: &TypedTrees) -> language_semantics::Multiplicity {
        if !self.reference.is_valid() {
            return language_semantics::Multiplicity::Affine;
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
            // Even the same binder handle can denote an outer occurrence of
            // a repeated declaration. The supplying prefix shrinks on every
            // replacement; handle equality alone is not a substitution cycle.
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

/// Invocation-local traversal storage, not a new durable compiler stage.
#[derive(Clone)]
pub struct AppliedTraitScope<'program> {
    pub declaration: &'program TraitDefinition,
    pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    /// Each simultaneous telescope starts here in `substitutions`. Arguments
    /// belong to outer telescopes, not to the telescope replacing them.
    pub substitution_scope_starts: Vec<usize>,
    pub argument_owner: SymbolHandle,
    region_scopes: Vec<AppliedDeclarationRegionScope>,
}

impl<'program> AppliedTraitScope<'program> {
    /// Compose a selected named map's own telescope before its declared trait
    /// application. The simultaneous frames prevent map operands from being
    /// captured by equally spelled trait binders, including inherited edges.
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
                &conformance.lifetime_parameters,
                application.lifetimes,
            ),
        );
        scope.argument_owner = application.lexical_owner;
        Some(scope)
    }

    /// Decode a named map's declaration-relative lifetime ordinals. This
    /// constructs an inspection scope, not conformance admission.
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
                arguments: program
                    .type_reference_table
                    .type_reference_handles(conformance.arguments),
                lifetimes: &lifetimes,
            },
        )
    }

    /// An exact machine edge supplies its own owner and explicit ordinal map;
    /// it does not borrow a similarly shaped named implementation's scope.
    pub fn from_machine_conformance(
        program: &'program TypedTrees,
        machine: &crate::machine::Machine,
        conformance: &crate::machine::TraitConformance,
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
                arguments: program
                    .type_reference_table
                    .type_reference_handles(conformance.arguments),
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
            &declaration.lifetime_parameters,
            application.lifetimes,
        ));
        Some(scope)
    }

    /// An authored trait contract keeps its own parameters rigid and open.
    pub fn abstract_scope(declaration: &'program TraitDefinition) -> Self {
        Self {
            declaration,
            substitutions: Vec::new(),
            substitution_scope_starts: vec![0],
            argument_owner: declaration.symbol,
            region_scopes: vec![AppliedDeclarationRegionScope::new(
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

    /// Only a bare parameter projects to another existing node. Composite
    /// arguments retain their declaration node and this entire scope.
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

    /// Borrow an incoming actual before the simultaneous frame it supplies.
    /// Enclosing data and inherited frames remain visible; the replacing trait
    /// cannot capture a caller region with the same spelling.
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

    /// Project one declaration binder through its outer scopes. A compound
    /// keeps its existing node; consumers must keep this scope alongside it.
    pub fn parameter_argument(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
    ) -> TypeReferenceHandle {
        let Some((ordinal, (_, reference))) = self
            .substitutions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (binder, _))| *binder == parameter)
        else {
            return TypeReferenceHandle::invalid();
        };
        let visible = self
            .substitution_scope_starts
            .iter()
            .rev()
            .find(|start| **start <= ordinal)
            .copied()
            .unwrap_or(0);
        project_type_reference(
            program,
            *reference,
            &self.substitutions[..visible],
            &self.substitution_scope_starts[..self
                .substitution_scope_starts
                .partition_point(|start| *start < visible)],
        )
        .reference
    }

    pub fn same_application(&self, program: &TypedTrees, other: &Self) -> bool {
        self.declaration.symbol == other.declaration.symbol
            && self
                .application_identity(program)
                .zip(other.application_identity(program))
                .is_some_and(|(left, right)| left == right)
    }

    /// Normalize one actual argument in its outer lexical scopes, not in the
    /// simultaneous telescope whose binder it replaces. A compound argument
    /// needs this complete chain; projecting only its existing node loses the
    /// inherited application. An open declaration keeps its own binder rigid.
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

    /// Portable exact argument matching is unavailable when ordinary identity
    /// erases regions or the argument remains an uninstantiated binder.
    pub fn region_free_parameter_identity(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
        qualification: TypeIdentityQualification,
    ) -> Option<NormalizedTypeIdentity> {
        let (request, scopes) = self.parameter_identity_request(parameter, qualification)?;
        program.region_free_scoped_type_identity(request, scopes)
    }

    fn parameter_identity_request(
        &self,
        parameter: SymbolHandle,
        qualification: TypeIdentityQualification,
    ) -> Option<(TypeIdentityRequest<'_>, &[usize])> {
        let Some((ordinal, (_, reference))) = self
            .substitutions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (binder, _))| *binder == parameter)
        else {
            return None;
        };
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

    /// Rejoin a declaring trait only when every reaching path agrees on its
    /// complete application. A diamond does not choose its first parent.
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

    /// Visit each reachable application, including transparent refinement
    /// heads. An active-path guard prevents cycles without discarding a sibling
    /// path's different application of the same declaring trait.
    pub fn visit(&self, program: &'program TypedTrees, visit: &mut impl FnMut(&Self)) {
        self.visit_inherited(program, &mut Vec::new(), visit);
    }

    fn visit_inherited(
        &self,
        program: &'program TypedTrees,
        active: &mut Vec<SymbolHandle>,
        visit: &mut impl FnMut(&Self),
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
            let Some(declaration) = program.trait_definition_by_symbol(parent.symbol) else {
                continue;
            };
            let Some(mut scope) = Self::new(
                program,
                declaration,
                TraitApplicationArguments {
                    lexical_owner: self.argument_owner,
                    enclosing_data_applications: &[],
                    arguments: program
                        .type_reference_table
                        .type_reference_handles(parent.arguments),
                    lifetimes: &parent.lifetime_arguments,
                },
            ) else {
                continue;
            };
            let mut substitutions = self.substitutions.clone();
            scope.substitution_scope_starts = self.substitution_scope_starts.clone();
            scope.substitution_scope_starts.push(substitutions.len());
            substitutions.append(&mut scope.substitutions);
            scope.substitutions = substitutions;
            let mut regions = self.region_scopes.clone();
            regions.append(&mut scope.region_scopes);
            scope.region_scopes = regions;
            scope.visit_inherited(program, active, visit);
        }
        active.pop();
    }
}
