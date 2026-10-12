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

mod payload;
pub use payload::AppliedTraitScopePayload;

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
                    .enumerate()
                    .flat_map(move |(ordinal, _)| {
                        self.machine_conformance_sources(
                            machine,
                            machine.satisfies.handle_at(ordinal as u32),
                        )
                        .into_iter()
                        .map(|source| source.scope)
                    })
            })
            .chain(
                self.conformances()
                    .iter()
                    .filter_map(|edge| AppliedTraitScope::from_conformance(self, edge)),
            )
            .chain(
                self.traits()
                    .iter()
                    .filter(|declaration| self.trait_type_parameters(declaration).is_empty())
                    .map(AppliedTraitScope::abstract_scope),
            )
            .chain(source_fields::service_application_roots(self))
            .collect()
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
    captured_argument_owner:
        Option<crate::typed_trees::product_applications::CapturedStaticLexicalScope>,
}

impl<'program> AppliedTraitScope<'program> {
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

    /// Symbols captured by this exact contract application, including compound
    /// types and static expression DAGs. Source row projection and universal
    /// callable matching use the same dependency owner and traversal.
    pub fn application_parameter_symbols(&self, program: &TypedTrees) -> Vec<SymbolHandle> {
        let mut captured = ApplicationReferences::default();
        for (_, argument) in &self.substitutions {
            crate::references::visit_type_reference(program, &mut captured, *argument);
        }
        captured.symbols
    }

    /// A source satisfaction owns a universal member family independently of
    /// the invocation that happens to share its executable body. The original
    /// edge graph is interpreted under that occurrence's complete bindings and
    /// enclosing data scopes. Its region ordinals still name the source family.
    pub fn from_completed_machine_conformance(
        program: &'program TypedTrees,
        family: &crate::machine::Machine,
        conformance: &crate::machine::TraitConformance,
        application: crate::static_applications::StaticMachineApplicationHandle,
    ) -> Option<Self> {
        if !program
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return None;
        }
        let retained = program
            .expression_table
            .static_machine_application(application);
        if retained.binding_declaration != family.symbol {
            return None;
        }
        let declaration =
            crate::static_applications::StaticApplicationDeclaration::for_application(
                program, retained,
            )
            .ok()?;
        let bindings = declaration.bind_arguments(program, application).ok()?;
        let lifetimes = conformance
            .trait_lifetime_arguments
            .iter()
            .map(|ordinal| {
                family
                    .lifetime_parameters
                    .get(usize::try_from(*ordinal).ok()?)
                    .cloned()
            })
            .collect::<Option<Vec<_>>>()?;
        let mut scope = Self {
            declaration: program.trait_definition_by_symbol(conformance.symbol)?,
            substitutions: Vec::new(),
            substitution_scope_starts: Vec::new(),
            argument_owner: retained.lexical_owner,
            captured_argument_owner: match retained.type_identity_owner(program)? {
                crate::type_identity::AppliedTypeIdentityOwner::Declaration(_) => None,
                crate::type_identity::AppliedTypeIdentityOwner::Captured(capture) => {
                    Some(capture.clone())
                }
            },
            region_scopes: Vec::new(),
        };
        for reference in program
            .type_reference_table
            .type_reference_handles(retained.enclosing_data_applications)
        {
            enclosing::append_data_application(program, *reference, &mut scope)?;
        }
        // A generic attached data declaration needs its actual receiving graph.
        // A generated storage definition's tuple is not a source occurrence.
        if let Some(data) = program.data_definition_by_symbol(family.attached_data_symbol) {
            let original = data
                .generic_instance
                .and_then(|reference| {
                    enclosing::data_application(program, reference)
                        .map(|application| application.declaration)
                })
                .unwrap_or(data);
            if (!program.data_type_parameters(original).is_empty()
                || !original.lifetime_parameters.is_empty())
                && !scope
                    .region_scopes
                    .iter()
                    .any(|region| region.declaration == original.symbol)
            {
                return None;
            }
        }
        scope
            .substitution_scope_starts
            .push(scope.substitutions.len());
        // The exact explicit satisfaction application fixes contract coordinates,
        // including compound/static arguments. Other member binders remain
        // universal invocation coordinates; a sibling edge cannot capture them.
        let mut contract_parameters = Self::from_machine_conformance(program, family, conformance)?
            .application_parameter_symbols(program);
        if program
            .type_reference_table
            .contains_type_reference(family.attached_data_application)
            && let TypeReferenceNode::Generic { arguments, .. } = program
                .type_reference_table
                .type_reference(family.attached_data_application)
        {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                if let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(*argument)
                    && !contract_parameters.contains(symbol)
                {
                    contract_parameters.push(*symbol);
                }
            }
        }
        scope
            .substitutions
            .extend(bindings.iter().filter_map(|binding| {
                if !contract_parameters.contains(&binding.parameter) {
                    return None;
                }
                use crate::static_applications::StaticMachineBindingValue;
                match &binding.value {
                    StaticMachineBindingValue::Type(reference)
                    | StaticMachineBindingValue::Const(reference)
                    | StaticMachineBindingValue::RuntimeValue {
                        type_reference: reference,
                        ..
                    } => Some((binding.parameter, *reference)),
                    _ => None,
                }
            }));
        scope
            .region_scopes
            .push(AppliedDeclarationRegionScope::declaration(
                family.symbol,
                application,
                contract_parameters,
            ));
        let arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let parameters = program.trait_type_parameters(scope.declaration);
        if parameters.len() != arguments.len()
            || scope.declaration.lifetime_parameters.len() != lifetimes.len()
        {
            return None;
        }
        scope
            .substitution_scope_starts
            .push(scope.substitutions.len());
        scope.substitutions.extend(
            parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.symbol, *argument)),
        );
        scope.region_scopes.push(AppliedDeclarationRegionScope::new(
            scope.declaration.symbol,
            &scope.declaration.lifetime_parameters,
            &lifetimes,
        ));
        Some(scope)
    }

    pub fn new(
        program: &TypedTrees,
        declaration: &'program TraitDefinition,
        application: TraitApplicationArguments<'_>,
    ) -> Option<Self> {
        Self::new_with_owner(
            program,
            declaration,
            application,
            crate::type_identity::AppliedTypeIdentityOwner::Declaration(application.lexical_owner),
        )
    }

    /// The operand graph and its lexical region issuer have separate custody
    /// when Build retains an original caller before relocating Product payloads.
    pub fn new_with_owner(
        program: &TypedTrees,
        declaration: &'program TraitDefinition,
        application: TraitApplicationArguments<'_>,
        owner: crate::type_identity::AppliedTypeIdentityOwner<'_>,
    ) -> Option<Self> {
        let (owner_symbol, capture) = match owner {
            crate::type_identity::AppliedTypeIdentityOwner::Declaration(symbol) => (symbol, None),
            crate::type_identity::AppliedTypeIdentityOwner::Captured(capture) => {
                (capture.symbol(), Some(capture.clone()))
            }
        };
        if application.lexical_owner != owner_symbol {
            return None;
        }
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
            captured_argument_owner: capture,
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

    /// Retained application bindings supply regions in original declaration
    /// order. A captured caller keeps its frozen lexical issuer independently
    /// of relocated operand handles in this program.
    pub fn from_static_application(
        program: &'program TypedTrees,
        declaration: &'program TraitDefinition,
        application: crate::static_applications::StaticMachineApplicationHandle,
        arguments: &[TypeReferenceHandle],
    ) -> Option<Self> {
        let retained = program
            .expression_table
            .static_machine_application(application);
        if retained.selected_interface.is_valid() {
            if retained.selected_interface != declaration.symbol {
                return None;
            }
        } else if program
            .trait_machine_signature(retained.binding_declaration)?
            .0
            .symbol
            != declaration.symbol
        {
            // An original directly declared member can predate selected-interface
            // retention. Its declaration is the exact namespace, never a name
            // search or a choice of inherited application.
            return None;
        }
        let bindings = program
            .expression_table
            .static_machine_application_lifetimes(application);
        let lifetimes = declaration
            .lifetime_parameters
            .iter()
            .enumerate()
            .map(|(ordinal, parameter)| {
                let binding = bindings.get(ordinal)?;
                (&binding.parameter == parameter)
                    .then(|| binding.argument.named().cloned())
                    .flatten()
            })
            .collect::<Option<Vec<_>>>()?;
        Self::new_with_owner(
            program,
            declaration,
            TraitApplicationArguments {
                lexical_owner: retained.lexical_owner,
                enclosing_data_applications: &[],
                arguments,
                lifetimes: &lifetimes,
            },
            retained.type_identity_owner(program)?,
        )
    }

    /// Borrow a complete selected interface from its original binding payload.
    /// Static declaration operands stay in the retained binding frame; this
    /// query creates no type nodes or caller operands to stand in for them.
    pub fn from_binding_application(
        program: &'program TypedTrees,
        declaration: &'program TraitDefinition,
        application: crate::static_applications::StaticMachineApplicationHandle,
    ) -> Option<Self> {
        use crate::static_applications::{StaticApplicationView, StaticMachineBindingValue};
        let view = StaticApplicationView::new(program, application).ok()?;
        let retained = view.application();
        let selected = if retained.selected_interface.is_valid() {
            retained.selected_interface
        } else {
            program
                .trait_machine_signature(retained.binding_declaration)?
                .0
                .symbol
        };
        if selected != declaration.symbol {
            return None;
        }
        let capture = match retained.type_identity_owner(program)? {
            crate::type_identity::AppliedTypeIdentityOwner::Declaration(owner) => {
                if owner != retained.lexical_owner {
                    return None;
                }
                None
            }
            crate::type_identity::AppliedTypeIdentityOwner::Captured(capture) => {
                if capture.symbol() != retained.lexical_owner {
                    return None;
                }
                Some(capture.clone())
            }
        };
        let mut scope = Self {
            declaration,
            substitutions: Vec::new(),
            substitution_scope_starts: Vec::new(),
            argument_owner: retained.lexical_owner,
            captured_argument_owner: capture,
            region_scopes: Vec::new(),
        };
        for reference in program
            .type_reference_table
            .type_reference_handles(retained.enclosing_data_applications)
        {
            enclosing::append_data_application(program, *reference, &mut scope)?;
        }
        scope
            .substitution_scope_starts
            .push(scope.substitutions.len());
        let mut parameters = program
            .trait_type_parameters(declaration)
            .iter()
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        for bound in &declaration.conformance_bounds {
            if !parameters.contains(&bound.binder) {
                parameters.push(bound.binder);
            }
        }
        for parameter in &parameters {
            let mut rows = view
                .bindings()
                .iter()
                .filter(|binding| binding.parameter == *parameter);
            let binding = rows.next()?;
            if rows.next().is_some() {
                return None;
            }
            match &binding.value {
                StaticMachineBindingValue::Type(reference)
                | StaticMachineBindingValue::Const(reference) => {
                    if !program
                        .type_reference_table
                        .contains_type_reference(*reference)
                    {
                        return None;
                    }
                    scope.substitutions.push((*parameter, *reference));
                }
                StaticMachineBindingValue::Machine(_)
                | StaticMachineBindingValue::Conformance(_) => {}
                // This scope compares static interface tuples. A runtime subject
                // needs its own index correspondence, not its carrier alone.
                StaticMachineBindingValue::RuntimeValue { .. } => return None,
            }
        }
        let captured = view
            .lifetimes()
            .get(..declaration.lifetime_parameters.len())?;
        if captured
            .iter()
            .zip(&declaration.lifetime_parameters)
            .any(|((formal, _), parameter)| formal.name != parameter)
        {
            return None;
        }
        scope.region_scopes.push(AppliedDeclarationRegionScope {
            declaration: declaration.symbol,
            lifetimes: captured
                .iter()
                .map(|(formal, binding)| {
                    (formal.name.to_string(), binding.argument.scope_argument())
                })
                .collect(),
            role: crate::type_identity::AppliedTypeIdentityRegionRole::Supplied,
            binding_application: application,
            binding_parameters: parameters,
        });
        Some(scope)
    }

    pub(crate) fn argument_identity_owner(
        &self,
    ) -> crate::type_identity::AppliedTypeIdentityOwner<'_> {
        match &self.captured_argument_owner {
            Some(capture) => crate::type_identity::AppliedTypeIdentityOwner::Captured(capture),
            None => {
                crate::type_identity::AppliedTypeIdentityOwner::Declaration(self.argument_owner)
            }
        }
    }

    /// Extend one exact declaring member's simultaneous telescope. Every start
    /// has a matching region row, including an uninstantiated universal region
    /// roster; operands still see only earlier telescopes.
    pub fn extend_member(
        &mut self,
        program: &TypedTrees,
        member: &crate::signature::StateSignature,
        arguments: &[(SymbolHandle, TypeReferenceHandle)],
        lifetimes: &[impl crate::static_applications::StaticLifetimeOperand],
    ) -> Option<()> {
        program.trait_machine_signature_by_symbol(self.declaration, member.symbol)?;
        let parameters = program.state_signature_type_parameters(member);
        if parameters.len() != arguments.len()
            || parameters
                .iter()
                .zip(arguments)
                .any(|(parameter, (symbol, _))| parameter.symbol != *symbol)
        {
            return None;
        }
        if !lifetimes.is_empty() && lifetimes.len() != member.lifetime_parameters.len() {
            return None;
        }
        self.substitution_scope_starts
            .push(self.substitutions.len());
        self.substitutions.extend_from_slice(arguments);
        self.region_scopes.push(AppliedDeclarationRegionScope::new(
            member.symbol,
            &member.lifetime_parameters,
            lifetimes,
        ));
        Some(())
    }

    /// An authored trait contract keeps its own parameters rigid and open.
    pub fn abstract_scope(declaration: &'program TraitDefinition) -> Self {
        Self {
            declaration,
            substitutions: Vec::new(),
            substitution_scope_starts: vec![0],
            argument_owner: declaration.symbol,
            captured_argument_owner: None,
            region_scopes: vec![AppliedDeclarationRegionScope::new(
                declaration.symbol,
                &declaration.lifetime_parameters,
                &[] as &[Identifier],
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
    pub fn region_free_parameter_identities(
        &self,
        program: &TypedTrees,
        qualification: TypeIdentityQualification,
    ) -> Option<Vec<NormalizedTypeIdentity>> {
        self.with_identity_scopes(program, |scopes| {
            program
                .trait_type_parameters(self.declaration)
                .iter()
                .map(|original| {
                    self.parameter_application(original.symbol)?;
                    crate::type_identity::region_free_static_parameter_identity(
                        program,
                        original,
                        scopes,
                        qualification,
                    )
                })
                .collect()
        })
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

    /// Map the declaring trait's erased region slots into this original root
    /// telescope. Source argument names are resolved only against the owning
    /// declaration's binder roster; the result contains declaration ordinals.
    /// Distinct inherited paths must agree, just as complete type applications
    /// must agree in `declaring_scope`. Missing custody never selects a path.
    pub fn declaring_lifetime_ordinals(
        &self,
        program: &TypedTrees,
        declaration: SymbolHandle,
    ) -> Option<Vec<u32>> {
        let root = (0..self.declaration.lifetime_parameters.len())
            .map(|ordinal| u32::try_from(ordinal).ok())
            .collect::<Option<Vec<_>>>()?;
        let mut selected = None;
        let mut ambiguous = false;
        visit_lifetime_applications(
            program,
            self.declaration,
            &root,
            &mut Vec::new(),
            &mut |owner, ordinals| {
                if owner != declaration {
                    return;
                }
                if let Some(known) = &selected {
                    ambiguous |= known != ordinals;
                } else {
                    selected = Some(ordinals.to_vec());
                }
            },
        )?;
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
            scope.captured_argument_owner = self.captured_argument_owner.clone();
            scope.visit_inherited(program, active, visit);
        }
        active.pop();
    }
}

/// Each inherited region operand belongs to the current declaration, never
/// to the receiving trait. Resolve that simultaneous telescope before walking
/// another edge, preserving shared slots and distinct sibling applications.
fn visit_lifetime_applications(
    program: &TypedTrees,
    declaration: &TraitDefinition,
    ordinals: &[u32],
    active: &mut Vec<SymbolHandle>,
    visit: &mut impl FnMut(SymbolHandle, &[u32]),
) -> Option<()> {
    if active.contains(&declaration.symbol) {
        return Some(());
    }
    if ordinals.len() != declaration.lifetime_parameters.len() {
        return None;
    }
    active.push(declaration.symbol);
    visit(declaration.symbol, ordinals);
    for parent in program
        .trait_requirements(declaration)
        .iter()
        .chain(declaration.refines.iter())
    {
        let inherited = program.trait_definition_by_symbol(parent.symbol)?;
        if parent.lifetime_arguments.len() != inherited.lifetime_parameters.len() {
            return None;
        }
        let inherited_ordinals = parent
            .lifetime_arguments
            .iter()
            .map(|argument| {
                declaration
                    .lifetime_parameters
                    .iter()
                    .position(|parameter| parameter == argument)
                    .and_then(|ordinal| ordinals.get(ordinal).copied())
            })
            .collect::<Option<Vec<_>>>()?;
        visit_lifetime_applications(program, inherited, &inherited_ordinals, active, visit)?;
    }
    active.pop();
    Some(())
}

#[derive(Default)]
struct ApplicationReferences {
    symbols: Vec<SymbolHandle>,
    types: Vec<crate::types::TypeReferenceHandle>,
    expressions: Vec<crate::expression::ExpressionHandle>,
}

impl Extend<SymbolHandle> for ApplicationReferences {
    fn extend<References: IntoIterator<Item = SymbolHandle>>(&mut self, references: References) {
        self.symbols.extend(references);
    }
}

impl crate::references::TypedReferences for ApplicationReferences {
    fn type_reference(&mut self, reference: crate::types::TypeReferenceHandle) -> bool {
        if self.types.contains(&reference) {
            return false;
        }
        self.types.push(reference);
        true
    }

    fn expression(&mut self, expression: crate::expression::ExpressionHandle) -> bool {
        if self.expressions.contains(&expression) {
            return false;
        }
        self.expressions.push(expression);
        true
    }
}
