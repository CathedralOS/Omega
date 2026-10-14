//! The catalogs a Unit plan family publishes, collected from the types its
//! signatures, locals, results and cleanup control name. Cleanup conditions
//! retain their own arenas beside the type catalog; they never alter a shape's
//! declaration or grant custody.
//!
//! Collection describes storage, not permission to copy, construct, or discard
//! it. Fields and array elements collect the same stored value shapes,
//! including provider receipts and stored reference shells. Their multiplicity
//! and cleanup remain obligations of the operations that use that storage.
//! Plain finite source contents additionally retain their no-code disposal
//! capability. A selected move may remove a common exit disposition while the
//! skipped alternative still owns the value; the graph's existing custody
//! solver needs this independent authority to settle that live complement.
//! Nominal hooks, linear descendants and stored loans never gain this capability.
//! Shared descriptors have a separate exact-type disposal judgment. A replacement
//! must settle their resident cohort and preserve external loans before installing
//! the incoming roster; this bit never permits ordinary resource-free cleanup.
//!
//! A stored reference owns its pointer carrier, not its referent's storage.
//! Record its exact type edge without collecting the target's fields. Root
//! storage demands collect their own shapes; otherwise `Nest<T>` containing
//! `&Nest<[T; 2]>` would require an infinite catalog for one finite pointer.

use super::*;

pub(crate) struct ShapeCollector<'program> {
    pub(in crate::execution::terminal_unit) program: &'program dyn ::validation::CheckingInput,
    pub(in crate::execution::terminal_unit) types:
        BTreeMap<typed_trees::type_identity::TypeIdentityId, CheckedUnitStructuralTypePlan>,
    pub(in crate::execution::terminal_unit) domains: Vec<CheckedUnitStructuralDomainPlan>,
    pub(in crate::execution::terminal_unit) cleanup_control:
        checked_trees::CheckedCleanupControlPlans,
    in_progress: BTreeSet<typed_trees::type_identity::TypeIdentityId>,
    /// The element rows registered beside borrowed byte views. A byte
    /// carrier's plan keeps no element field, so `retain_transitive` follows
    /// this edge where it follows a `BorrowedSliceView`'s element identity.
    byte_view_elements: BTreeSet<typed_trees::type_identity::TypeIdentityId>,
}

impl<'program> ShapeCollector<'program> {
    pub(crate) fn new(program: &'program dyn ::validation::CheckingInput) -> Self {
        Self {
            program,
            types: BTreeMap::new(),
            domains: Vec::new(),
            cleanup_control: Default::default(),
            in_progress: BTreeSet::new(),
            byte_view_elements: BTreeSet::new(),
        }
    }

    pub(in crate::execution::terminal_unit) fn add_domain(
        &mut self,
        domain: SemanticDomainId,
        carrier: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        let mut reference = carrier;
        let mut has_domain_constraints = false;
        let mut selected_symbol = None;
        loop {
            match self.program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference { referee, .. } => reference = *referee,
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    for constraint in self.program.type_reference_table.constraints(*constraints) {
                        let TypeConstraintNode::Domain(constraint) = constraint else {
                            continue;
                        };
                        has_domain_constraints = true;
                        if constraint.semantic_id == domain
                            && selected_symbol.replace(constraint.symbol).is_some()
                        {
                            return None;
                        }
                    }
                    reference = *base_type;
                }
                _ => break,
            }
        }
        let definition = if !has_domain_constraints {
            // Legacy unqualified carriers carry only the semantic family.
            self.program
                .domain_definitions()
                .iter()
                .find(|definition| definition.semantic_id == domain)
        } else {
            // Qualified byte carriers preserve the actual declaration selected
            // by typing. Another capacity in the same family is not its proof
            // theory, even when both declarations happen to classify bytes.
            let definition = typed_trees::domain::domain_by_symbol(self.program, selected_symbol?)?;
            if definition.semantic_id != domain {
                return None;
            }
            Some(definition)
        };
        self.add_domain_definition(domain, carrier, binders, definition)
    }

    /// A contract may establish a domain independently of the qualifications
    /// already declared on its result carrier. Keep the declaration selected
    /// by that contract's authorization instead of searching those unrelated
    /// qualifications for it. Registration describes its theory, not evidence
    /// that the result satisfies it; the calling contract owns that judgment.
    pub(in crate::execution::terminal_unit) fn add_selected_domain(
        &mut self,
        domain: SemanticDomainId,
        selected_symbol: SymbolHandle,
        carrier: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        let definition = typed_trees::domain::domain_by_symbol(self.program, selected_symbol)?;
        if definition.semantic_id != domain {
            return None;
        }
        self.add_domain_definition(domain, carrier, binders, Some(definition))
    }

    fn add_domain_definition(
        &mut self,
        domain: SemanticDomainId,
        carrier: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        definition: Option<&typed_trees::domain::DomainDefinition>,
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        let carrier_type_identity = self.add_type(carrier, binders, &[])?;
        let identity = self.program.semantic_domains.name(domain)?.to_owned();
        let establishment_routes = definition
            .map(|definition| definition.establishment_routes.clone())
            .unwrap_or_default();
        let byte_predicate = definition.and_then(|definition| {
            typed_trees::byte_predicates::domain_byte_predicate(self.program, definition.symbol)
        });
        let plan = CheckedUnitStructuralDomainPlan {
            byte_predicate,
            domain,
            name: identity,
            carrier_type_identity,
            establishment_routes,
        };
        if let Some(existing) = self.domains.iter().find(|existing| {
            existing.domain == domain
                && existing.carrier_type_identity == plan.carrier_type_identity
        }) {
            return (existing == &plan).then_some(carrier_type_identity);
        }
        // A semantic domain names a family, not one storage carrier. Separate
        // bounded capacities retain that family under distinct exact carriers;
        // conflicting descriptions of the same pair still reject above.
        self.domains.push(plan);
        Some(carrier_type_identity)
    }

    /// A machine attached to generic data retains the exact owner application
    /// its binder scope resolved — `Task<Token>` on a receiver-specialized
    /// clone where the declared `Task<T>` data still names the open
    /// parameter. Collect that applied shape like any other carrier; the
    /// declared-data fallback still serves attachments that named no generic
    /// actuals.
    pub(in crate::execution::terminal_unit) fn add_attached_application(
        &mut self,
        machine: &typed_trees::machine::Machine,
        binders: &[(SymbolHandle, String)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        let attached = self
            .program
            .data_definition_by_symbol(machine.attached_data_symbol)
            .or_else(|| {
                // A compiler-synthesized placed accessor attaches by a
                // generated name that carries no source-resolved symbol; the
                // signature builder joins that spelling to the exact shell
                // declaration in the same way.
                machine
                    .attached_data
                    .as_ref()
                    .and_then(|name| self.program.data_definition_by_name(name))
            });
        if let Some(data) = attached
            && let Some(identity) = self.add_placed_accessor_attachment(data, binders)
        {
            return Some(identity);
        }
        if machine.attached_data_application.is_valid() {
            return self.add_type(machine.attached_data_application, binders, &[]);
        }
        self.add_attached_data(attached?, binders)
    }

    pub(in crate::execution::terminal_unit) fn add_attached_data(
        &mut self,
        data: &typed_trees::data::DataDefinition,
        binders: &[(SymbolHandle, String)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        if let Some(identity) = self.add_placed_accessor_attachment(data, binders) {
            return Some(identity);
        }
        if !self.program.data_type_parameters(data).is_empty() {
            // A static attached machine does not carry an instantiated type
            // argument tuple. Generic attached data therefore needs a later
            // explicit checked identity fact rather than guessed binding.
            return None;
        }
        let identity = closed_data_identity(self.program, data, binders);
        self.add_data_shape(identity, data.clone(), binders, Vec::new())
    }

    /// A placed-view plan's generated accessor shell stays `BoundaryOpaque`
    /// to source construction, yet each cloned `read`/`take`/`write` target
    /// still attaches to it. Admit that attachment through the exact plan
    /// row — not the generated name — as the empty nominal record the layout
    /// catalog already assigns the same definition. The shell keeps no
    /// members a source carrier could construct; this only publishes the
    /// attachment identity its boundary operations and their callers join.
    fn add_placed_accessor_attachment(
        &mut self,
        data: &typed_trees::data::DataDefinition,
        binders: &[(SymbolHandle, String)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        if data.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque
            || !self
                .program
                .placed_view_plans
                .iter()
                .flat_map(|view| view.fields.iter())
                .any(|field| field.accessor_data_symbol == data.symbol)
        {
            return None;
        }
        let identity = closed_data_identity(self.program, data, binders);
        let plan = CheckedUnitStructuralTypePlan {
            scalar_restriction: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            identity: identity,
            shape: CheckedUnitStructuralTypeShape::Record {
                storage: checked_trees::CheckedUnitRecordStorage::Packed,
                fields: Vec::new(),
            },
        };
        if self
            .types
            .get(&identity)
            .is_some_and(|existing| *existing != plan)
        {
            return None;
        }
        self.types.insert(identity, plan);
        Some(identity)
    }

    /// Read a const actual bound through a substitution as a usize array
    /// length. Literal spellings are `Named` rows whose identity is the
    /// decimal text; const-evaluated applications keep a `ConstExpression`.
    fn const_length_reference_value(&self, reference: TypeReferenceHandle) -> Option<usize> {
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { name, .. } => name.as_str().parse::<usize>().ok(),
            TypeReferenceNode::ConstExpression(expression) => usize::try_from(
                self.program
                    .expression_table
                    .constant_integer_value(*expression)?,
            )
            .ok(),
            _ => None,
        }
    }

    /// A const actual's extent: its literal, or the recipe a still-pending
    /// const-generic argument retains for the target closure.
    fn const_length_extent(&self, reference: TypeReferenceHandle) -> Option<CheckedExtent> {
        if let Some(value) = self.const_length_reference_value(reference) {
            return u64::try_from(value).ok().map(CheckedExtent::Literal);
        }
        let recipe = self
            .program
            .constant_recipe_for_const_application(reference);
        recipe.is_valid().then_some(CheckedExtent::Recipe(recipe))
    }

    /// Register an admitted reference carrier with its declared access.
    /// Access selects the loan contract, not whether its descriptor exists.
    pub(in crate::execution::terminal_unit) fn add_reference_type(
        &mut self,
        reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        let shell = validation::reference_shell(self.program, reference)?;
        // A bounded byte loan already owns a distinct carrier identity. Its
        // descriptor is the byte view, rather than a second Reference plan
        // competing for that same identity. Reuse the ordinary structural
        // descriptor; access remains on the field or parameter loan contract.
        if matches!(
            byte_sequence_carrier(self.program, reference, &[]),
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. })
        ) {
            return self.add_type(reference, binders, &[]);
        }
        self.add_view_type(reference, binders, shell.access)
    }

    /// A shared, mutable or write-only view spells
    /// its own type reference: the reference shell is the loan custody the
    /// plan carries, so unlike `add_type` it is retained as a `Reference`
    /// shape over the peeled referee rather than peeled away. The accesses
    /// admit different referees. An exclusive shell lends whatever it
    /// borrows — a `&mut u8`, a `&mut Record` or a `&mut [T]` — while a
    /// shared shell registers here only over a named or generic referee;
    /// `&'a [T]` shared views register through `add_slice_view_type`.
    pub(in crate::execution::terminal_unit) fn add_view_type(
        &mut self,
        reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        access: language_semantics::ReferenceAccess,
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        let identity = self.add_view_type_for_substitutions(reference, binders, &[], access)?;
        // A root view lends actual backing storage. Its independently demanded
        // shape is distinct from the pointer carrier recorded above; stored
        // reference fields do not recursively demand their targets' storage.
        let shell = validation::reference_shell(self.program, reference)?;
        self.add_type(shell.referee, binders, &[])?;
        Some(identity)
    }

    fn add_view_type_for_substitutions(
        &mut self,
        reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        access: language_semantics::ReferenceAccess,
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        let shell = validation::reference_shell(self.program, reference)?;
        if shell.access != access {
            return None;
        }
        // A stored pointer does not depend on the referent's source family.
        // Storage demands collect that referent independently; classifying it
        // as a named view here would reject fixed-array backing during cleanup.
        let referee = shell.referee;
        let referent_identity =
            validation::structural_type_identity(self.program, referee, binders, substitutions);
        self.add_reference_shape(
            reference,
            binders,
            substitutions,
            referent_identity,
            access.into(),
        )
    }

    /// A `&'a [T]` borrowed view also spells its own type reference — the same
    /// `Reference` shell `add_view_type` keeps for `&'a V`. Its referent is
    /// the view carrier `add_type` mints from the whole reference, so `&[u8]`
    /// registers the `slice(u8)` byte view and `&[T]` its element view.
    pub(in crate::execution::terminal_unit) fn add_slice_view_type(
        &mut self,
        reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        if !borrowed_slice_view(self.program, reference) {
            return None;
        }
        let referent_identity = self.add_type(reference, binders, &[])?;
        self.add_reference_shape(
            reference,
            binders,
            &[],
            referent_identity,
            CheckedStructuralAccess::SharedBorrow,
        )
    }

    /// Register the pointer carrier and its exact referent identity. The edge
    /// does not demand a referent storage definition. One identity names one
    /// shape: a conflicting earlier plan declines the registration.
    fn add_reference_shape(
        &mut self,
        reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        referent_identity: typed_trees::type_identity::TypeIdentityId,
        access: CheckedStructuralAccess,
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        let identity = self.program.type_identity(TypeIdentityRequest {
            binders,
            substitutions,
            ..TypeIdentityRequest::ordinary(reference)
        });
        let plan = CheckedUnitStructuralTypePlan {
            scalar_restriction: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: access != CheckedStructuralAccess::Owned,
            identity: identity,
            shape: CheckedUnitStructuralTypeShape::Reference {
                referent_identity,
                access,
            },
        };
        if self
            .types
            .get(&identity)
            .is_some_and(|existing| existing != &plan)
        {
            return None;
        }
        self.types.insert(identity, plan);
        Some(identity)
    }

    /// A checked reference formation demands the storage of its exact source
    /// leaf. An equal stored carrier transports the existing descriptor.
    /// Otherwise peel only the newly formed loan: an inner reference remains a
    /// stored descriptor. This is an operation dependency, not permission to
    /// expand the pointees of every reference field in the type catalog.
    pub(in crate::execution::terminal_unit) fn add_reference_backing(
        &mut self,
        reference: TypeReferenceHandle,
        source_identity: typed_trees::type_identity::TypeIdentityId,
        binders: &[(SymbolHandle, String)],
    ) -> Option<()> {
        if validation::stored_structural_type_identity(self.program, reference, binders, &[])
            == source_identity
        {
            return (self.add_stored_type(reference, binders, &[])? == source_identity)
                .then_some(());
        }
        let identity = self.add_type(reference, binders, &[])?;
        (identity == source_identity).then_some(())
    }

    /// The primitive a registered identity names when its catalog row is a
    /// primitive scalar. A borrowed formal registers its peeled referent, so
    /// this is the primitive a `&u8` or `&mut i64` formal lends.
    pub(in crate::execution::terminal_unit) fn primitive_scalar(
        &self,
        identity: typed_trees::type_identity::TypeIdentityId,
    ) -> Option<PrimitiveType> {
        match self.types.get(&identity).map(|plan| &plan.shape) {
            Some(CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive)) => Some(*primitive),
            _ => None,
        }
    }

    /// Collect the value held in a field or element, rather than the storage
    /// borrowed by an argument. Sequence shells retain their descriptor route;
    /// every other stored loan must retain a Reference shape. Keeping this
    /// distinction here prevents an aggregate boundary from inlining a referent
    /// or making an unsupported loan look like owned storage.
    pub(in crate::execution::terminal_unit) fn add_stored_type(
        &mut self,
        reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        let reference = substituted_formal_type(self.program, reference, substitutions);
        if validation::stores_reference_shape(self.program, reference, substitutions) {
            structural_access_for_type_reference(self.program, reference)?;
            let shell = validation::reference_shell(self.program, reference)?;
            let referee = substituted_formal_type(self.program, shell.referee, substitutions);
            if matches!(
                self.program.type_reference_table.type_reference(referee),
                TypeReferenceNode::Constrained { .. } | TypeReferenceNode::Reference { .. }
            ) {
                // Reference shapes have no referent qualification slot, and
                // nested reference shells must not collapse after substitution.
                return None;
            }
            return self.add_view_type_for_substitutions(
                reference,
                binders,
                substitutions,
                shell.access,
            );
        }
        self.add_type(reference, binders, substitutions)
    }

    pub(in crate::execution::terminal_unit) fn add_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        // One identity names one plan for the whole module, and lowering
        // rejoins this plan by recomputing the identity from source types, so
        // both use the one rule: a bounded byte owner keeps its qualification
        // and a bounded view its loan, which keeps either from claiming the
        // plain array's plan.
        let identity = validation::structural_type_identity(
            self.program,
            type_reference,
            binders,
            substitutions,
        );
        let no_code_affine_cleanup = validation::has_contents(
            self.program,
            validation::structural_identity_type(self.program, type_reference, substitutions),
            substitutions,
            validation::ContentsRequirement::RepresentedOwned,
        );
        let descriptor_cleanup = validation::has_contents(
            self.program,
            validation::structural_identity_type(self.program, type_reference, substitutions),
            substitutions,
            validation::ContentsRequirement::DescriptorCleanup,
        );
        if let Some(carrier) = byte_sequence_carrier(self.program, type_reference, substitutions) {
            // Byte views and bounded byte carriers spell their implicit
            // element through the collection establishment operation. The
            // carrier has no element field, so retain the declaration's u8
            // row beside it, just as an element view retains its element.
            if matches!(
                carrier,
                checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
                    | checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }
                    | checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
            ) {
                let mut referee = self
                    .program
                    .type_reference_table
                    .peeled_type_reference(type_reference);
                loop {
                    match self.program.type_reference_table.type_reference(referee) {
                        TypeReferenceNode::Named { symbol, .. } => {
                            let Some((_, replacement)) = substitutions
                                .iter()
                                .rev()
                                .find(|(parameter, _)| parameter == symbol)
                            else {
                                break;
                            };
                            if *replacement == referee {
                                break;
                            }
                            referee = *replacement;
                        }
                        _ => break,
                    }
                }
                if let TypeReferenceNode::Slice { element_type }
                | TypeReferenceNode::FixedArray { element_type, .. } =
                    self.program.type_reference_table.type_reference(referee)
                {
                    let element = self.add_type(*element_type, binders, substitutions)?;
                    self.byte_view_elements.insert(element);
                }
            }
            let plan = CheckedUnitStructuralTypePlan {
                scalar_restriction: None,
                // A borrowed byte carrier owns only its descriptor. As with
                // Reference shapes, replacement may retire that descriptor,
                // never its backing or any still-live child permission.
                descriptor_cleanup: carrier.is_borrowed(),
                no_code_affine_cleanup: no_code_affine_cleanup
                    && matches!(
                        carrier,
                        checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                    ),
                identity: identity,
                // A view type's identity peels the reference shell, so
                // `&mut [u8]` and `&write [u8]` register under one identity.
                // The access binds on the field or parameter that uses the
                // type, not on the type, and keeping it here would make the
                // second registration conflict with the first.
                shape: CheckedUnitStructuralTypeShape::ByteSequence(carrier.without_access()),
            };
            if self
                .types
                .get(&identity)
                .is_some_and(|existing| existing != &plan)
            {
                return None;
            }
            self.types.insert(identity, plan);
            return Some(identity);
        }
        if let Some(element) =
            borrowed_slice_view_element(self.program, type_reference, substitutions)
        {
            if !self.in_progress.insert(identity) {
                return None;
            }
            let element_type_identity = self.add_type(element, binders, substitutions);
            self.in_progress.remove(&identity);
            let plan = CheckedUnitStructuralTypePlan {
                scalar_restriction: None,
                no_code_affine_cleanup: false,
                // The element type describes borrowed backing, not contents
                // disposed with this address/extent descriptor.
                descriptor_cleanup: true,
                identity: identity,
                shape: CheckedUnitStructuralTypeShape::BorrowedSliceView {
                    element_type_identity: element_type_identity?,
                },
            };
            if self
                .types
                .get(&identity)
                .is_some_and(|existing| existing != &plan)
            {
                return None;
            }
            self.types.insert(identity, plan);
            return Some(identity);
        }
        // Every other carrier names the shared semantic identity beneath its
        // loan. Scalar receiving restrictions retain their own identity and
        // declaration; membership-only qualifications remain separate.
        let type_reference =
            validation::structural_identity_type(self.program, type_reference, substitutions);
        // The same nonconstructible carrier can be a field, a parameter or
        // reference backing. Storage collection must not depend on which
        // owner first demanded its shape. Occurrence closure and established
        // binding custody remain with their independent source judgments.
        if typed_trees::service::inspect_service_carrier_contract(self.program, type_reference)
            .ok()?
            .is_some()
        {
            return self.add_binding_carrier_type(type_reference, binders, substitutions);
        }
        if self.types.contains_key(&identity) {
            return Some(identity);
        }
        if let Some(primitive) = self.program.primitive_type_reference(type_reference) {
            let scalar_restriction = match scalar_fields::retain_scalar_field(
                self.program,
                type_reference,
                substitutions,
                primitive,
                None,
            )? {
                CheckedUnitStructuralFieldType::Scalar(_) => None,
                restriction @ (CheckedUnitStructuralFieldType::BoundedInteger(_)
                | CheckedUnitStructuralFieldType::RefinedScalar { .. }) => Some(restriction),
                _ => return None,
            };
            self.types.insert(
                identity,
                CheckedUnitStructuralTypePlan {
                    scalar_restriction,
                    no_code_affine_cleanup,
                    descriptor_cleanup,
                    identity: identity,
                    shape: CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive),
                },
            );
            return Some(identity);
        }
        if let TypeReferenceNode::FixedArray {
            element_type,
            length,
        } = self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            // An element carrier spelled with a const parameter binds its
            // declared length at each application. Literal actuals resolve
            // here; an open binder keeps no numeric extent and declines.
            // A rostered recipe keeps its symbolic extent in the plan; the
            // target closure fixes it. An unrostered call is still pending
            // evaluation and declines exactly as an open binder does.
            let Some(length) = (match length {
                typed_trees::types::FixedArrayLength::Literal(value) => {
                    u64::try_from(*value).ok().map(CheckedExtent::Literal)
                }
                typed_trees::types::FixedArrayLength::ConstParameter { symbol, .. } => {
                    substitutions
                        .iter()
                        .rev()
                        .find(|(parameter, _)| parameter == symbol)
                        .and_then(|(_, replacement)| self.const_length_extent(*replacement))
                }
                typed_trees::types::FixedArrayLength::Expression(_) => None,
                typed_trees::types::FixedArrayLength::ConstCall { .. } => {
                    let recipe = self
                        .program
                        .constant_recipe_for_array_length(type_reference);
                    recipe.is_valid().then_some(CheckedExtent::Recipe(recipe))
                }
            }) else {
                return None;
            };
            // Array elements own their stored value, including a reference's
            // permission carrier. Their referents do not become inline data.
            // Recursion determines representability; construction, transfer,
            // and cleanup independently check each operation's custody.
            if !self.in_progress.insert(identity) {
                return None;
            }
            let element = substituted_formal_type(self.program, *element_type, substitutions);
            // Each scalar element now retains its own declaration predicate.
            // Recursive collection rejects an unrepresentable scalar contract.
            // Byte element domains use the ordinary projected qualification
            // contract; completed operands must carry those memberships.
            let missing_membership =
                self.program
                    .primitive_type_reference(element)
                    .is_some_and(|primitive| {
                        matches!(
                            scalar_fields::retain_scalar_field(
                                self.program,
                                element,
                                substitutions,
                                primitive,
                                None
                            ),
                            Some(CheckedUnitStructuralFieldType::Scalar(_))
                        ) && validation::unrestricted_builtin_primitive(self.program, element)
                            != Some(primitive)
                    });
            let element_type_identity = if missing_membership {
                None
            } else {
                self.add_stored_type(element, binders, substitutions)
            };
            let Some(element_type_identity) = element_type_identity else {
                self.in_progress.remove(&identity);
                return None;
            };
            self.types.insert(
                identity,
                CheckedUnitStructuralTypePlan {
                    scalar_restriction: None,
                    no_code_affine_cleanup,
                    descriptor_cleanup,
                    identity: identity,
                    shape: CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        length,
                    },
                },
            );
            self.in_progress.remove(&identity);
            return Some(identity);
        }
        // Preparation owns closed field substitution. A view may retain its
        // template symbol, so nominal lookup would reopen those fields under
        // a flattened telescope and capture a recursive application's binder.
        if let Some(data) = self.program.closed_data_definition(type_reference) {
            return self.add_data_shape(identity, data.clone(), binders, Vec::new());
        }
        let (data_symbol, arguments) = match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Named { symbol, name }
                if PrimitiveType::from_name(name.as_str()).is_none() =>
            {
                (*symbol, Vec::new())
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => (
                *base_symbol,
                self.program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .to_vec(),
            ),
            _ => {
                return None;
            }
        };
        let data = self.program.data_definition_by_symbol(data_symbol)?.clone();
        let members = self.program.data_members(&data);
        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || !matches!(
                typed_trees::data::DataDefinition::shape_kind_from_members(members),
                DataShapeKind::Empty
                    | DataShapeKind::Record
                    | DataShapeKind::Enum
                    | DataShapeKind::Mixed
            )
        {
            return None;
        }
        let data_parameters = self.program.data_type_parameters(&data);
        if data_parameters.len() != arguments.len() {
            return None;
        }
        let mut local_substitutions = substitutions.to_vec();
        local_substitutions.extend(
            data_parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.symbol, argument)),
        );
        self.add_data_shape(identity, data, binders, local_substitutions)
    }

    pub(in crate::execution::terminal_unit) fn add_partial_affine_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        let identity = self.add_type(type_reference, binders, &[])?;
        // Shape collection also serves borrowed carriers. Check authored
        // ownership before a referent-oriented shape can erase that distinction.
        if !partial_affine_source_contents_are_owned(self.program, type_reference, &mut Vec::new())
        {
            return None;
        }
        Some(identity)
    }

    pub(in crate::execution::terminal_unit) fn add_affine_array_construction_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        // Construction-prefix admission remains owned by its separate caller.
        self.add_type(type_reference, binders, &[])
    }

    pub(in crate::execution::terminal_unit) fn add_data_shape(
        &mut self,
        identity: typed_trees::type_identity::TypeIdentityId,
        data: typed_trees::data::DataDefinition,
        binders: &[(SymbolHandle, String)],
        substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        if let Some(accessor) = self.add_placed_accessor_attachment(&data, binders) {
            return (accessor == identity).then_some(identity);
        }
        if self.types.contains_key(&identity) {
            return Some(identity);
        }
        if !self.in_progress.insert(identity) {
            return None;
        }
        // A boundary-opaque datum authors no storage of its own. The
        // authoritative build's exact `OpaqueRepresentation` application picks
        // the concrete carrier, and that carrier alone supplies the physical
        // shape collected under the opaque's own identity; an opaque value
        // without a selected application still cannot claim storage.
        let mut data = data;
        if data.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
            let Some(selection) = representation_selections::selection_for_opaque(
                self.program.opaque_representation_selections(),
                data.symbol,
            ) else {
                self.in_progress.remove(&identity);
                return None;
            };
            let Some(carrier) = self.program.data_definition_by_symbol(selection.carrier()) else {
                self.in_progress.remove(&identity);
                return None;
            };
            data = carrier.clone();
        }
        let no_code_affine_cleanup = validation::data_has_contents(
            self.program,
            &data,
            &substitutions,
            validation::ContentsRequirement::RepresentedOwned,
        );
        let descriptor_cleanup = validation::data_has_contents(
            self.program,
            &data,
            &substitutions,
            validation::ContentsRequirement::DescriptorCleanup,
        );
        let members = self.program.data_members(&data).to_vec();
        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || !matches!(
                typed_trees::data::DataDefinition::shape_kind_from_members(&members),
                DataShapeKind::Empty
                    | DataShapeKind::Record
                    | DataShapeKind::Enum
                    | DataShapeKind::Mixed
            )
        {
            self.in_progress.remove(&identity);
            return None;
        }
        let shape_kind = typed_trees::data::DataDefinition::shape_kind_from_members(&members);
        // Interval-only `where` facts become the named fields' bounds.
        let stated_intervals =
            validation::data_where_field_intervals(self.program, &data).unwrap_or_default();
        let stated = |field: &typed_trees::data::DataField| {
            stated_intervals
                .iter()
                .find(|(symbol, _, _)| *symbol == field.symbol)
                .map(|(_, low, high)| (*low, *high))
        };
        if matches!(shape_kind, DataShapeKind::Enum | DataShapeKind::Mixed) {
            let mut fields = Vec::new();
            let mut cases = Vec::with_capacity(members.len());
            for member in &members {
                match member {
                    DataMember::Field(field) => {
                        let Some(field) = self.structural_field_plan(
                            field,
                            binders,
                            &substitutions,
                            identity,
                            stated(field),
                        ) else {
                            self.in_progress.remove(&identity);
                            return None;
                        };
                        fields.push(field);
                    }
                    DataMember::Variant(variant) => {
                        // The checked shape rosters the cases this instance can
                        // carry. A case its `where` fact refutes needs no tag,
                        // payload or dispatch successor (`impossible_variant`).
                        if ::validation::impossible_variant(self.program, variant) {
                            continue;
                        }
                        // Case-local intervals use the same bounded field
                        // representation as record facts. Keeping the owning
                        // case here lets Terminal check every constructor and
                        // expose the bounds only on its selected payload edge.
                        let intervals =
                            validation::case_where_field_intervals(self.program, variant)
                                .unwrap_or_default();
                        let mut payload_fields = Vec::new();
                        for field in self.program.data_payload_fields(variant) {
                            let bounds = intervals
                                .iter()
                                .find(|(symbol, _, _)| *symbol == field.symbol)
                                .map(|(_, low, high)| (*low, *high));
                            let Some(field) = self.structural_field_plan(
                                field,
                                binders,
                                &substitutions,
                                identity,
                                bounds,
                            ) else {
                                self.in_progress.remove(&identity);
                                return None;
                            };
                            payload_fields.push(field);
                        }
                        cases.push(checked_trees::CheckedUnitStructuralCasePlan {
                            identity: checked_trees::CheckedMemberIdentity::of_member(
                                variant.identity,
                                variant.name.clone(),
                            ),
                            fields: payload_fields,
                        });
                    }
                }
            }
            self.types.insert(
                identity,
                CheckedUnitStructuralTypePlan {
                    scalar_restriction: None,
                    no_code_affine_cleanup,
                    descriptor_cleanup,
                    identity: identity,
                    shape: if matches!(shape_kind, DataShapeKind::Mixed) {
                        CheckedUnitStructuralTypeShape::Mixed { fields, cases }
                    } else {
                        CheckedUnitStructuralTypeShape::Sum { cases }
                    },
                },
            );
            self.in_progress.remove(&identity);
            return Some(identity);
        }

        let program = self.program;
        let authored_layout = program.plan_laid_layout(data.symbol);
        let record_storage = match authored_layout {
            Some(layout) => match layout.invariant_geometry() {
                Some(geometry) => checked_trees::CheckedUnitRecordStorage::Authored(
                    semantic_vocabulary::AuthoredRecordStorage {
                        byte_size: u64::try_from(geometry.size).ok()?,
                        byte_alignment: u64::try_from(geometry.align).ok()?,
                    },
                ),
                None => {
                    let schema = program.data_definition_by_symbol(layout.schema_symbol)?;
                    let schema_type_identity = self.add_attached_data(schema, binders)?;
                    checked_trees::CheckedUnitRecordStorage::TargetLayout {
                        application: layout.data_symbol,
                        schema_type_identity,
                    }
                }
            },
            None => checked_trees::CheckedUnitRecordStorage::Packed,
        };
        let mut fields = Vec::new();
        for member in &members {
            let Some(field) = member.as_field() else {
                self.in_progress.remove(&identity);
                return None;
            };
            let bounds = stated(field);
            let Some(mut field_plan) =
                self.structural_field_plan(field, binders, &substitutions, identity, bounds)
            else {
                self.in_progress.remove(&identity);
                return None;
            };
            if let Some(layout) = authored_layout {
                if !field.relevance.is_erased() && layout.invariant_geometry().is_some() {
                    field_plan.storage = Some(authored_field_storage(
                        layout,
                        field.symbol,
                        &field_plan.field_type,
                    )?);
                }
            }
            fields.push(field_plan);
        }
        self.types.insert(
            identity,
            CheckedUnitStructuralTypePlan {
                scalar_restriction: None,
                no_code_affine_cleanup,
                descriptor_cleanup,
                identity: identity,
                shape: CheckedUnitStructuralTypeShape::Record {
                    fields,
                    storage: record_storage,
                },
            },
        );
        self.in_progress.remove(&identity);
        Some(identity)
    }

    fn structural_field_plan(
        &mut self,
        field: &typed_trees::data::DataField,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        owner_identity: typed_trees::type_identity::TypeIdentityId,
        stated_bounds: Option<(Option<i64>, Option<i64>)>,
    ) -> Option<CheckedUnitStructuralFieldPlan> {
        // Classify the actual member before choosing its storage route. A
        // formal T instantiated with a reference or Binding must retain that
        // shell, not reach add_type's referent-oriented fallback as plain T.
        let field_type_reference =
            substituted_formal_type(self.program, field.type_reference, substitutions);
        let field_type = if field.relevance.is_erased() {
            CheckedUnitStructuralFieldType::Erased {
                type_identity: self.program.type_identity(TypeIdentityRequest {
                    binders,
                    substitutions,
                    ..TypeIdentityRequest::ordinary(field_type_reference)
                }),
            }
        } else if let Some((binding_carrier, provider_node)) =
            provider_backed_field(self.program, field_type_reference)
        {
            let provider_type_identity = self.program.type_identity(TypeIdentityRequest {
                binders,
                substitutions,
                ..TypeIdentityRequest::ordinary(provider_node)
            });
            match binding_carrier {
                Some(receipt) => {
                    self.add_type(provider_node, binders, &[])?;
                    CheckedUnitStructuralFieldType::BindingBacked {
                        provider_type_identity,
                        receipt,
                    }
                }
                None => CheckedUnitStructuralFieldType::ProviderBacked {
                    provider_type_identity,
                },
            }
        } else if let Some(carrier) =
            byte_sequence_carrier(self.program, field_type_reference, substitutions)
        {
            let declared = typed_trees::byte_predicates::type_reference_domain_predicates(
                self.program,
                field_type_reference,
            );
            let mut predicates =
                language_semantics::byte_predicates::ByteSequencePredicateSet::EMPTY;
            for (_, predicate) in declared {
                predicates.insert(predicate?);
            }
            if predicates.is_empty() {
                CheckedUnitStructuralFieldType::ByteSequence(carrier)
            } else {
                CheckedUnitStructuralFieldType::PredicateByteSequence {
                    carrier,
                    predicates,
                }
            }
        } else if is_reference(self.program, field_type_reference) {
            CheckedUnitStructuralFieldType::Structural {
                type_identity: self.add_stored_type(
                    field_type_reference,
                    binders,
                    substitutions,
                )?,
            }
        } else {
            match scalar_type(self.program, field_type_reference, substitutions) {
                Some(primitive) => scalar_fields::retain_scalar_field(
                    self.program,
                    field_type_reference,
                    substitutions,
                    primitive,
                    stated_bounds,
                )?,
                None => {
                    let nested =
                        self.add_stored_type(field_type_reference, binders, substitutions)?;
                    if nested == owner_identity {
                        return None;
                    }
                    CheckedUnitStructuralFieldType::Structural {
                        type_identity: nested,
                    }
                }
            }
        };
        // A qualified field's domain theories join the roster with its shape,
        // so a value constructed into the field (`Tag { label: "amulet" }`)
        // carries the membership checking decided for it even when no
        // signature or local spells the qualified carrier.
        let mut qualification_carriers = Vec::new();
        if !field.relevance.is_erased()
            && matches!(
                field_type,
                CheckedUnitStructuralFieldType::Structural { .. }
                    | CheckedUnitStructuralFieldType::ByteSequence(_)
                    | CheckedUnitStructuralFieldType::PredicateByteSequence { .. }
            )
        {
            let carriers = super::signature_contracts::qualification_carriers(
                self.program,
                self,
                field_type_reference,
                binders,
            )?;
            // A structural field's carrier is its own retained type; a byte
            // field names none, so its plan carries the carriers instead.
            if field_type.byte_sequence_carrier().is_some() {
                for (_, carrier) in carriers {
                    if !qualification_carriers.contains(&carrier) {
                        qualification_carriers.push(carrier);
                    }
                }
            }
        }
        Some(CheckedUnitStructuralFieldPlan {
            identity: checked_trees::CheckedMemberIdentity::of_member(
                field.identity,
                field.name.clone(),
            ),
            relevance: field.relevance,
            field_type,
            storage: None,
            qualification_carriers,
        })
    }

    /// Register the semantic binding independently from its occurrence receipt.
    /// Keeping a nonconstructible type lets fields and parameters share ordinary
    /// loan paths without turning zero-sized layout into establishment authority.
    fn add_binding_carrier_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        // Physical sharing requires the nominal core shell, not a proof that
        // this occurrence's provider or regions have been established. The
        // parameter/field receipt retains that separate semantic association.
        typed_trees::service::inspect_service_carrier_contract(self.program, type_reference)
            .ok()??;
        let identity = self.program.type_identity(TypeIdentityRequest {
            binders,
            substitutions,
            ..TypeIdentityRequest::ordinary(type_reference)
        });
        let plan = CheckedUnitStructuralTypePlan {
            scalar_restriction: None,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            identity: identity,
            shape: CheckedUnitStructuralTypeShape::BindingCarrier,
        };
        if self
            .types
            .get(&identity)
            .is_some_and(|existing| existing != &plan)
        {
            return None;
        }
        self.types.insert(identity, plan);
        Some(identity)
    }

    /// Retain the exact source parameter, including any borrow shell, beside
    /// its logical semantic carrier. The receipt grants no new binding.
    pub(in crate::execution::terminal_unit) fn add_binding_parameter_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        source_parameter: SymbolHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<(
        typed_trees::type_identity::TypeIdentityId,
        checked_trees::CheckedBindingParameterReceipt,
    )> {
        let parameter_carrier = typed_trees::service::classify_exact_service_parameter_carrier(
            self.program,
            typed_trees::service::ServiceCarrierOccurrence {
                type_reference,
                lexical_owner: source_parameter,
                enclosing_data_applications: &[],
            },
        )
        .ok()??;
        let carrier = parameter_carrier.carrier;
        if !source_parameter.is_valid() {
            return None;
        }

        let type_identity =
            self.add_binding_carrier_type(parameter_carrier.carrier_type, binders, &[])?;

        Some((
            type_identity,
            checked_trees::CheckedBindingParameterReceipt {
                source_parameter,
                carrier_type_identity: self.program.type_identity(TypeIdentityRequest {
                    binders,
                    ..TypeIdentityRequest::ordinary(type_reference)
                }),
                requirement: carrier.requirement,
            },
        ))
    }

    pub(in crate::execution::terminal_unit) fn retain_transitive(
        &mut self,
        roots: &BTreeSet<typed_trees::type_identity::TypeIdentityId>,
    ) {
        let mut retained = roots.iter().copied().collect::<BTreeSet<_>>();
        loop {
            let old_len = retained.len();
            for identity in retained.clone() {
                let Some(plan) = self.types.get(&identity) else {
                    continue;
                };
                match &plan.shape {
                    CheckedUnitStructuralTypeShape::Reference {
                        referent_identity, ..
                    } => {
                        retained.insert(*referent_identity);
                    }
                    // The element row `add_type` registered beside a
                    // borrowed byte view survives with it: an element-view
                    // establishment of that view resolves its op's element
                    // through this row.
                    CheckedUnitStructuralTypeShape::ByteSequence(_) => {
                        retained.extend(self.byte_view_elements.iter().cloned());
                    }
                    CheckedUnitStructuralTypeShape::BindingCarrier
                    | CheckedUnitStructuralTypeShape::PrimitiveScalar(_) => {}
                    CheckedUnitStructuralTypeShape::Record { fields, storage } => {
                        if let checked_trees::CheckedUnitRecordStorage::TargetLayout {
                            schema_type_identity,
                            ..
                        } = storage
                        {
                            retained.insert(*schema_type_identity);
                        }
                        retained.extend(
                            fields
                                .iter()
                                .flat_map(CheckedUnitStructuralFieldPlan::retained_type_identities),
                        );
                    }
                    CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        ..
                    }
                    | CheckedUnitStructuralTypeShape::BorrowedSliceView {
                        element_type_identity,
                    } => {
                        retained.insert(*element_type_identity);
                    }
                    CheckedUnitStructuralTypeShape::Sum { cases } => {
                        retained.extend(
                            cases
                                .iter()
                                .flat_map(|case| &case.fields)
                                .flat_map(CheckedUnitStructuralFieldPlan::retained_type_identities),
                        );
                    }
                    CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                        retained.extend(
                            fields
                                .iter()
                                .chain(cases.iter().flat_map(|case| &case.fields))
                                .flat_map(CheckedUnitStructuralFieldPlan::retained_type_identities),
                        );
                    }
                }
            }
            if retained.len() == old_len {
                break;
            }
        }
        self.types.retain(|identity, _| retained.contains(identity));
        self.domains
            .retain(|domain| retained.contains(&domain.carrier_type_identity));
    }
}

/// Join the admitted plan to the exact synthesized field, not its spelling.
fn authored_field_storage(
    layout: &typed_trees::PlanLaidLayout,
    symbol: SymbolHandle,
    field_type: &CheckedUnitStructuralFieldType,
) -> Option<semantic_vocabulary::AuthoredFieldStorage> {
    let field_index = layout
        .field_symbols
        .iter()
        .position(|candidate| *candidate == symbol)?;
    let scalar_field = matches!(
        field_type,
        CheckedUnitStructuralFieldType::Scalar(_)
            | CheckedUnitStructuralFieldType::Atomic(_)
            | CheckedUnitStructuralFieldType::BoundedInteger(_)
            | CheckedUnitStructuralFieldType::RefinedScalar { .. }
    );
    layout
        .invariant_geometry()?
        .authored_field_storage(field_index, scalar_field)
}

/// A provider-backed field's logical receipt (when the carrier is an exact
/// `Binding<R>` requirement) plus the type node whose identity names the
/// provider — the carrier itself for a binding, the unwrapped
/// boundary-trait referent for a `&'a mut <boundary trait>` field.
pub(super) fn provider_backed_field(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<(
    Option<checked_trees::CheckedBindingFieldReceipt>,
    TypeReferenceHandle,
)> {
    if let Some(requirement) =
        typed_trees::service::service_requirement_declaration(program, type_reference)
    {
        // Logical classification never consults target provider selection.
        // Establishment joins this requirement to the selected provider later.
        return Some((
            Some(checked_trees::CheckedBindingFieldReceipt { requirement }),
            type_reference,
        ));
    }
    // Attached data stores a provider as `&'a mut <boundary trait>`: look
    // through the reference (and any qualifications wrapped around it) to the
    // trait symbol before deciding the field is not a provider handle.
    let mut unwrapped = type_reference;
    let provider_symbol = loop {
        match program.type_reference_table.type_reference(unwrapped) {
            TypeReferenceNode::Constrained {
                base_type: inner, ..
            } => unwrapped = *inner,
            TypeReferenceNode::Reference {
                referee: inner,
                access: language_core::ReferenceAccess::Mutable,
                ..
            } => unwrapped = *inner,
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::DynamicTrait { symbol, .. } => break *symbol,
            _ => return None,
        }
    };
    program
        .traits()
        .iter()
        .any(|definition| definition.symbol == provider_symbol && definition.is_boundary)
        .then_some((None, unwrapped))
}
