//! Structural, declaration-derived lifetime frontiers shared by result and
//! input selection. Incomplete frontiers never establish a partial contract.
//! Fixed arrays share one element-type traversal; only actual reference leaves
//! are expanded into per-element paths. View-free storage therefore costs work
//! proportional to its type structure, not its physical element count.
use super::data_definition;
use super::{SymbolHandle, TypeReferenceHandle, TypeReferenceNode};
use checked_trees::BorrowLoanOwnerSegment as BorrowOwnerSegment;

#[derive(Clone)]
pub(super) struct CarriedLifetime {
    pub(super) lifetime_only: bool,
    pub(super) retained_contract: bool,
    pub(super) reference: TypeReferenceHandle,
    pub(super) reference_hops: usize,
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) lifetime: Option<RegionIdentity>,
    pub(super) access: language_semantics::ReferenceAccess,
    pub(super) source: super::ViewReturnInputSource,
}

/// Value and destination inventories stop at each stored descriptor. Input
/// lifetime inventory also follows readable descriptors, keeping each leaf’s
/// own region separate from the borrow used to reach its containing record.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum LifetimeFrontier {
    #[default]
    Value,
    Input,
    ReachableInput,
    /// Lifetime candidates below an input descriptor. Each pointer boundary
    /// remains a stored source; this inventory grants no permission to load it.
    InputStorage,
    BorrowedStorage,
    /// Authority reach for opaque non-observation, not a returned-loan map.
    /// Shared references cut access; mutable references expose their contents.
    WriteOnlyAuthority,
}

/// Ordinary flow queries retain declaration spellings. Callable refinement
/// and invocation-family flow resolve fresh and captured declaration identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RegionIdentity {
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

#[derive(Clone, Copy, Default)]
struct TemplateFrontier<'a> {
    parameters: &'a [SymbolHandle],
    authority_parameters: &'a [typed_trees::data::TypeParameter],
    dependent: bool,
    /// Declaration inventory retains registered binders without granting leaves.
    declaration_inventory: bool,
    scoped_regions: bool,
    frontier: LifetimeFrontier,
    input_reference_hops: usize,
}

/// Each generic telescope binds simultaneously. Its actual operands belong
/// to the outer scope, even when another application uses the same declaration.
#[derive(Clone, Copy, Default)]
struct TypeScope<'scope> {
    declaration: SymbolHandle,
    lifetimes: &'scope [(String, RegionIdentity)],
    bindings: &'scope [(SymbolHandle, TypeReferenceHandle)],
    starts: &'scope [usize],
    outer: Option<&'scope TypeScope<'scope>>,
    argument_lifetimes: &'scope [(String, RegionIdentity)],
    application_lifetimes: &'scope [Vec<(String, RegionIdentity)>],
    captured: &'scope [super::callable_regions::AppliedRegionBinding],
    /// Declared application rows parallel to starts, retained only through
    /// the supplying prefix selected by a Type substitution.
    application_scopes: &'scope [typed_trees::type_identity::AppliedTypeIdentityScope<'scope>],
    argument_owner: SymbolHandle,
    /// Region scopes survive a foreign operand without reactivating that
    /// operand's receiving Type substitutions.
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

struct DataVisit {
    reference: TypeReferenceHandle,
    cyclic: bool,
}

/// A declaration may describe a type parameter without knowing its carried
/// references. This is not a complete frontier and never supplies a loan.
pub fn declaration_lifetime_frontier(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    parameters: &[SymbolHandle],
) -> DeclarationLifetimeFrontier {
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
    if !complete {
        DeclarationLifetimeFrontier::Incomplete
    } else if template.dependent {
        DeclarationLifetimeFrontier::TemplateDependent {
            has_borrows: !leaves.is_empty(),
        }
    } else {
        DeclarationLifetimeFrontier::Complete {
            has_borrows: !leaves.is_empty(),
        }
    }
}

pub(super) fn carried_lifetimes(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<Vec<CarriedLifetime>> {
    storage_lifetimes(program, reference, LifetimeFrontier::Value)
}

/// A destination inventory walks stored elements, including the unknown-index
/// paths of a slice referent. This differs from a value frontier: a bare slice
/// is storage rather than a complete value descriptor. Symbolic elements still
/// leave this query incomplete; they do not establish an empty debt roster.
pub(super) fn stored_lifetimes(
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

/// Every substitution maps to a valid, registered type reference.
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

/// Exact call substitutions may prove that a formerly dependent result has
/// no carried views. A closed nonempty frontier still requires per-call loan
/// attribution and cannot use this deliberately narrower result.
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

/// Exact call substitutions may instead close a formerly dependent result
/// frontier completely. A closed view-bearing frontier is not view-free:
/// every instantiated leaf still borrows an exact caller input, which is the
/// caller-side loan relation `resolve_substituted_view_return_source`
/// carries. An unresolved structure is never a complete frontier.
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

/// The instantiated structural frontier of one substituted type: the same
/// leaf walk as [`carried_lifetimes`], with each declaration type parameter
/// resolved through the call's own closed bindings. `None` means the
/// instantiated frontier cannot be enumerated completely.
pub(super) fn substituted_carried_lifetimes(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &[usize],
) -> Option<Vec<CarriedLifetime>> {
    symbolic_carried_lifetimes(program, reference, substitutions, scope_starts, &[])
}

/// Known region leaves of a universal callable. Opaque type operands are
/// compared by the caller's complete type-correspondence judgment, not
/// asserted view-free. Ordinary flow queries require complete concrete loan
/// frontiers. Invocation-family flow may exclude opaque captured inputs only
/// for complete fresh outputs; the scoped contract owner proves that separation.
pub(super) fn symbolic_carried_lifetimes(
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

/// Classify reachable write-only access under exact substitutions. This uses
/// the same scoped declaration traversal as lifetime frontiers, with shared
/// attenuation and copy bounds cutting access. Dependence is retained until
/// ordinary call admission supplies its concrete application.
pub(super) fn input_write_only_access(
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

pub(super) fn symbolic_input_lifetimes(
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

/// Known declaration leaves and the obligation to close remaining binders.
/// A dependent inventory supplies no complete relation or reference-free proof.
pub(super) struct CarriedLifetimeInventory {
    pub(super) leaves: Vec<CarriedLifetime>,
    pub(super) template_dependent: bool,
}

pub(super) fn declaration_lifetime_inventory(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    frontier: LifetimeFrontier,
) -> Option<CarriedLifetimeInventory> {
    let mut template = TemplateFrontier {
        frontier,
        declaration_inventory: true,
        ..TemplateFrontier::default()
    };
    let leaves = collect_frontier(program, reference, &[], TypeScope::default(), &mut template)?;
    Some(CarriedLifetimeInventory {
        leaves,
        template_dependent: template.dependent,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn scoped_carried_lifetimes(
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

/// A recursive input can still return a reference described by its finite
/// input list. Add deeper pointer sources only when all can be resolved;
/// otherwise keep the original complete list unchanged. Body checking still
/// rejects a returned reference with no source in that retained list.
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

/// A nongeneric recursive result can retain one whole input loan under ordinary
/// elision. Repeated declarations need no repeated path expansion here, because
/// no projected frontier is published. Explicit lifetimes or substitutions
/// require the ordinary complete contract and cannot use this fallback.
pub(super) fn whole_elided_result_accesses(
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

/// A slot's declaration identity selects its own region frame. Whole-node
/// ownership cannot collapse mixed captured/fresh nominal arguments, and a
/// Type-operand prefix must not inherit its receiver's lifetime application.
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
    let operand_lifetimes = super::callable_refinement::captured_lifetimes(program, operand_owner)
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
    let (lifetimes, types) =
        if operand_owner.is_valid() {
            // A declaration-owned field uses the region application of its active
            // traversal frame, just as it uses that frame's Type substitutions.
            // A supplied operand outside those frames re-enters its captured scope.
            // The owner stamp alone does not distinguish a schema from an operand.
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
    // Zero remains the ordinary absence/Unit convention. A stale nonzero
    // generational handle resolving to the same dummy node is not a type.
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
                // Shared access cannot load a write-only descriptor or lend
                // its exclusive permission; neither read nor write is gained.
                let mut retained = first_stored;
                for position in first_stored..output.len() {
                    if output[position].access == language_semantics::ReferenceAccess::WriteOnly {
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
                    && program
                        .retained_borrow_custodies()
                        .iter()
                        .any(|custody| custody.retained_semantic_domain == domain.semantic_id)
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
            // The path does not participate in frontier admission. Check the
            // element once in the same substitution/recursion context, then
            // prefix its complete relative leaves. Incompleteness must not
            // become an empty (view-free) frontier.
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
                // A symbolic extent does not hide references when the element
                // frontier is empty. Only view-bearing arrays require an exact
                // count to publish their per-element owner paths.
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
            // Authority classification needs the element type, not a runtime
            // inventory of indices. Shared views have already cut access.
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
            // A checked retained nominal carrier constrains its source lifetime
            // without containing a pointer. A phantom lifetime alone does not.
            // The shared placeholder below describes no spatial permission:
            // exact producer contracts supply access, and caller transport keeps
            // each actual debt's access instead of attenuating it to this slot.
            // One nominal carrier may have both shared and mutable issuers.
            if template.frontier != LifetimeFrontier::WriteOnlyAuthority
                && let Some(custody) = program
                    .retained_borrow_custodies()
                    .iter()
                    .find(|custody| custody.result_data == *base_symbol)
                && let Some(lifetime) =
                    lifetime_arguments.get(custody.result_lifetime_argument_ordinal as usize)
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
            // Preparation owns the nominal field substitution. Its fields
            // already live in the application's supplying scope: retain the
            // incoming call bindings without replaying the nominal telescope.
            // An open prepared field can still name an evidence-bound Type;
            // clearing those bindings would erase its aggregate result loan.
            // Copied fields have lost the operand's lexical lifetime scope.
            // Scoped callable queries traverse the original declaration and
            // simultaneous application instead of the prepared carrier view.
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
                    // An unresolved self-substitution is not a finite concrete type.
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
                    // Earlier applications can bind lifetimes without binding
                    // any Types. Their position, rather than the Type offset,
                    // decides which supplying scopes this operand keeps.
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
                        && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter)
                {
                    // The input reference still lends its declared region
                    // when its referee is abstract. This supplies no stored
                    // descriptor source: those require the instantiated
                    // carrier. Value/output queries remain incomplete, never
                    // classifying an opaque owned value as reference-free.
                    template.dependent = true;
                    return true;
                }
                return false;
            };
            // Erasing a nominal application's region at a binding preserves
            // its obligation; the initializer still supplies the exact source.
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
        // Implementation selection stores no user-state lifetime. References
        // to selector storage were handled above as ordinary references.
        TypeReferenceNode::Unit | TypeReferenceNode::DynamicTrait { .. } => true,
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Slice { .. } => false,
    }
}

/// A generic declaration named without arguments is a carrier family: its
/// subject is the total carrier `Σ (I : IndexPack). C<I>` (quotients.md,
/// "Families, relations, and evidence"), so its frontier is the union over
/// every admissible index pack. The pack is existential, never the symbolic
/// template of the surrounding comparison, and no outer application supplies
/// it. Machine, proposition, implementation and value binders select code or
/// facts, not stored field types, so the declaration's fields describe the
/// frontier exactly. A field whose type is one of the family's own Type
/// binders depends on the unknown pack and leaves the frontier incomplete; a
/// reference over one of its own regions names no lexical declaration, which
/// callable refinement refuses.
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
        // A cycle with no reference leaves is still borrow-free (for example
        // Nat). A cycle carrying views has no finite projected frontier. Mark
        // the participating frames, then decide only after all members have
        // contributed their leaves; member order must not decide completeness.
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
    // Each field's owner path is transient: leaves copy what they keep. One
    // buffer serves every field rather than a fresh copy of the prefix each.
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
