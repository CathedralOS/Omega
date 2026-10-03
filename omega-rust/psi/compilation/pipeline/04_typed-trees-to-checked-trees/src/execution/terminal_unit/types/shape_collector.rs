//! The structural type catalog a Unit plan family publishes, collected from the
//! types its signatures, locals, and results name.
//!
//! Collection describes storage, not permission to copy, construct, or discard
//! it. Nominal array elements use the same member plans as standalone data,
//! including provider receipts and stored reference shells. Their multiplicity
//! and cleanup remain obligations of the operations that use that storage.

use super::*;

pub(crate) struct ShapeCollector<'program> {
    pub(in crate::execution::terminal_unit) program: &'program TypedTrees,
    pub(in crate::execution::terminal_unit) types: BTreeMap<String, CheckedUnitStructuralTypePlan>,
    pub(in crate::execution::terminal_unit) domains: Vec<CheckedUnitStructuralDomainPlan>,
    in_progress: BTreeSet<String>,
    /// The element rows registered beside borrowed byte views. A byte
    /// carrier's plan keeps no element field, so `retain_transitive` follows
    /// this edge where it follows a `BorrowedSliceView`'s element identity.
    byte_view_elements: BTreeSet<String>,
}

impl<'program> ShapeCollector<'program> {
    pub(crate) fn new(program: &'program TypedTrees) -> Self {
        Self {
            program,
            types: BTreeMap::new(),
            domains: Vec::new(),
            in_progress: BTreeSet::new(),
            byte_view_elements: BTreeSet::new(),
        }
    }

    pub(in crate::execution::terminal_unit) fn add_domain(
        &mut self,
        domain: SemanticDomainId,
        carrier: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<()> {
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
    ) -> Option<()> {
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
    ) -> Option<()> {
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
            identity,
            carrier_type_identity,
            establishment_routes,
        };
        if let Some(existing) = self.domains.iter().find(|existing| {
            existing.domain == domain
                && existing.carrier_type_identity == plan.carrier_type_identity
        }) {
            return (existing == &plan).then_some(());
        }
        // A semantic domain names a family, not one storage carrier. Separate
        // bounded capacities retain that family under distinct exact carriers;
        // conflicting descriptions of the same pair still reject above.
        self.domains.push(plan);
        Some(())
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
    ) -> Option<String> {
        let attached = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == machine.attached_data_symbol)
            .or_else(|| {
                // A compiler-synthesized placed accessor attaches by a
                // generated name that carries no source-resolved symbol; the
                // signature builder joins that spelling to the exact shell
                // declaration in the same way.
                machine.attached_data.as_ref().and_then(|name| {
                    self.program
                        .data_definitions()
                        .iter()
                        .find(|data| data.name == *name)
                })
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
    ) -> Option<String> {
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
    ) -> Option<String> {
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
            identity: identity.clone(),
            shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
        };
        if self
            .types
            .get(&identity)
            .is_some_and(|existing| *existing != plan)
        {
            return None;
        }
        self.types.insert(identity.clone(), plan);
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

    pub(super) fn is_unrestricted_nonatomic_primitive(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        let types = &self.program.type_reference_table;
        if !types.contains_type_reference(type_reference) {
            return false;
        }
        // Arithmetic policy changes operations, not stored bits. Only this
        // exact shell may project to an array's primitive element shape; ranges
        // and other qualifications still need their own retained obligations.
        // The complete array identity and checked computations retain policy.
        let reference = match types.type_reference(type_reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                if !types.contains_type_reference(*base_type)
                    || !matches!(
                        types.constraint_span(*constraints),
                        Some([typed_trees::types::TypeConstraintNode::ArithmeticDomain(_)])
                    )
                {
                    return false;
                }
                let TypeReferenceNode::Named { symbol, name } = types.type_reference(*base_type)
                else {
                    return false;
                };
                let Some(atom) = self.program.symbols.builtin_type_atom(*symbol) else {
                    return false;
                };
                if name.as_str() != atom.symbol_name()
                    || !matches!(
                        atom,
                        symbols::BuiltinTypeAtom::I8
                            | symbols::BuiltinTypeAtom::I16
                            | symbols::BuiltinTypeAtom::I32
                            | symbols::BuiltinTypeAtom::I64
                            | symbols::BuiltinTypeAtom::U8
                            | symbols::BuiltinTypeAtom::U16
                            | symbols::BuiltinTypeAtom::U32
                            | symbols::BuiltinTypeAtom::U64
                    )
                {
                    return false;
                }
                *base_type
            }
            _ => type_reference,
        };
        matches!(types.type_reference(reference), TypeReferenceNode::Named { name, .. }
            if !name.as_str().starts_with("Atomic")
                && self.program.primitive_type_reference(reference).is_some()
                && crate::checks::type_multiplicity(self.program, type_reference)
                    == Multiplicity::Unrestricted)
    }

    /// A checked-shape data declaration whose every stored
    /// member is material: a record of fields, a pure sum of cases, or a
    /// mixed declaration of both, with no erased field or payload field.
    /// Each member type still shapes itself through `add_type`; this only
    /// decides that the declaration is an ordinary owned array element. The
    /// three kinds share one Terminal vocabulary (`Record`, `Sum`, `Mixed`
    /// beneath `FixedArray`). Multiplicity is not a layout distinction: an
    /// affine service-bearing payload needs its exact member shape too.
    /// `add_type` resolves generic arguments and rejects unrepresentable members;
    /// this predicate grants neither trivial disposal nor construction authority.
    fn is_material_data(&self, type_reference: TypeReferenceHandle) -> bool {
        let symbol = match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::Generic {
                base_symbol: symbol,
                ..
            } => *symbol,
            _ => return false,
        };
        self.program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == symbol)
            .is_some_and(|data| {
                let members = self.program.data_members(data);
                data.supply_mode == language_semantics::DataSupplyMode::CheckedShape
                    && data.lifetime_parameters.is_empty()
                    && data.quotient.is_none()
                    && (data.where_facts.is_empty()
                        || validation::data_where_field_intervals(self.program, data).is_some())
                    && !data.zero_gated
                    && matches!(
                        typed_trees::data::DataDefinition::shape_kind_from_members(members),
                        DataShapeKind::Record | DataShapeKind::Enum | DataShapeKind::Mixed
                    )
                    && members.iter().all(|member| match member {
                        DataMember::Field(field) => !field.relevance.is_erased(),
                        DataMember::Variant(variant) => self
                            .program
                            .data_payload_fields(variant)
                            .iter()
                            .all(|field| !field.relevance.is_erased()),
                    })
            })
    }

    /// A reference type whose shell loans its referent: `parts`' `&mut
    /// <primitive>` and every other `&mut` referent, or a shared `&'a V`
    /// named view. `parts` needs no arm of its own — a bare `&mut
    /// <primitive>` is one of the `&mut` shells — and the two accesses
    /// never classify the same shell.
    pub(in crate::execution::terminal_unit) fn add_reference_type(
        &mut self,
        reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<String> {
        self.add_view_type(
            reference,
            binders,
            language_semantics::ReferenceAccess::Mutable,
        )
        .or_else(|| {
            self.add_view_type(
                reference,
                binders,
                language_semantics::ReferenceAccess::Shared,
            )
        })
    }

    /// A `&'a V` shared view or a `&mut R`/`&mut [T]` exclusive view spells
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
    ) -> Option<String> {
        let referee = match access {
            language_semantics::ReferenceAccess::Mutable => {
                validation::reference_result_custody::mutable_borrowed_parts(
                    self.program,
                    reference,
                )?
                .0
            }
            language_semantics::ReferenceAccess::Shared => {
                borrowed_view_referent(self.program, reference, access)?
            }
            language_semantics::ReferenceAccess::WriteOnly => return None,
        };
        let referent_identity = self.add_type(referee, binders, &[])?;
        self.add_reference_shape(reference, binders, referent_identity, access.into())
    }

    /// A `&'a [T]` borrowed view also spells its own type reference — the same
    /// `Reference` shell `add_view_type` keeps for `&'a V`. Its referent is
    /// the view carrier `add_type` mints from the whole reference, so `&[u8]`
    /// registers the `slice(u8)` byte view and `&[T]` its element view.
    pub(in crate::execution::terminal_unit) fn add_slice_view_type(
        &mut self,
        reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<String> {
        if !borrowed_slice_view(self.program, reference) {
            return None;
        }
        let referent_identity = self.add_type(reference, binders, &[])?;
        self.add_reference_shape(
            reference,
            binders,
            referent_identity,
            CheckedStructuralAccess::SharedBorrow,
        )
    }

    /// Register `reference` as a `Reference` shape over an already registered
    /// referent. One identity names one shape: a conflicting earlier plan
    /// under the same identity declines the registration.
    fn add_reference_shape(
        &mut self,
        reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        referent_identity: String,
        access: CheckedStructuralAccess,
    ) -> Option<String> {
        let identity = self
            .program
            .type_identity(TypeIdentityRequest {
                binders,
                ..TypeIdentityRequest::ordinary(reference)
            })
            .into_string();
        let plan = CheckedUnitStructuralTypePlan {
            identity: identity.clone(),
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
        self.types.insert(identity.clone(), plan);
        Some(identity)
    }

    pub(in crate::execution::terminal_unit) fn add_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<String> {
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
                let mut referee = type_reference;
                loop {
                    match self.program.type_reference_table.type_reference(referee) {
                        TypeReferenceNode::Reference { referee: inner, .. }
                        | TypeReferenceNode::Constrained {
                            base_type: inner, ..
                        } => referee = *inner,
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
            let identity =
                byte_sequence_type_identity(self.program, type_reference, binders, substitutions)?;
            let plan = CheckedUnitStructuralTypePlan {
                identity: identity.clone(),
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
            self.types.insert(identity.clone(), plan);
            return Some(identity);
        }
        if let Some(element) =
            borrowed_slice_view_element(self.program, type_reference, substitutions)
        {
            let identity = borrowed_slice_view_type_identity(
                self.program,
                type_reference,
                binders,
                substitutions,
            );
            if !self.in_progress.insert(identity.clone()) {
                return None;
            }
            let element_type_identity = self.add_type(element, binders, substitutions);
            self.in_progress.remove(&identity);
            let plan = CheckedUnitStructuralTypePlan {
                identity: identity.clone(),
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
            self.types.insert(identity.clone(), plan);
            return Some(identity);
        }
        let mut type_reference = type_reference;
        loop {
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => type_reference = *referee,
                TypeReferenceNode::Named { symbol, .. } => {
                    if let Some((_, replacement)) = substitutions
                        .iter()
                        .rev()
                        .find(|(parameter, _)| parameter == symbol)
                    {
                        // A substitution can bind a parameter to its own
                        // reference; rewriting it again only loops.
                        if *replacement == type_reference {
                            break;
                        }
                        type_reference = *replacement;
                        continue;
                    }
                    break;
                }
                _ => break,
            }
        }
        let identity = self
            .program
            .type_identity(TypeIdentityRequest {
                binders,
                substitutions,
                ..TypeIdentityRequest::ordinary(type_reference)
            })
            .into_string();
        if self.types.contains_key(&identity) {
            return Some(identity);
        }
        if let Some(primitive) = self.program.primitive_type_reference(type_reference) {
            self.types.insert(
                identity.clone(),
                CheckedUnitStructuralTypePlan {
                    identity: identity.clone(),
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
            let Some(length) = (match length {
                typed_trees::types::FixedArrayLength::Literal(value) => Some(*value),
                typed_trees::types::FixedArrayLength::ConstParameter { symbol, .. } => {
                    substitutions
                        .iter()
                        .rev()
                        .find(|(parameter, _)| parameter == symbol)
                        .and_then(|(_, replacement)| {
                            self.const_length_reference_value(*replacement)
                        })
                }
                typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
            }) else {
                return None;
            };
            let plain_owned_array = validation::has_plain_owned_contents_with_substitutions(
                self.program,
                type_reference,
                substitutions,
            );
            // The plain-owned classifier deliberately excludes qualified
            // fields. Bounded byte fields have their own retained carrier,
            // already supported in affine records; wrapping that record in an
            // array must not erase its shape. Keep the existing owned-content
            // and no-nominal-drop checks rather than treating borrows as data.
            let owned_affine_array = substitutions.is_empty()
                && crate::checks::type_multiplicity(self.program, *element_type)
                    == Multiplicity::Affine
                && partial_affine_source_contents_are_owned(
                    self.program,
                    *element_type,
                    &mut Vec::new(),
                )
                && !type_graph_requires_nominal_drop(self.program, type_reference);
            let unrestricted_primitive_element =
                self.is_unrestricted_nonatomic_primitive(*element_type);
            let material_data_element = self.is_material_data(*element_type);
            // The recursive collector validates the inner array's own extent
            // and element shell. Looking through it here to demand primitive
            // leaves duplicates collection and rejects valid aggregate nesting.
            let nested_array_element = matches!(
                self.program
                    .type_reference_table
                    .type_reference(*element_type),
                TypeReferenceNode::FixedArray { .. }
            );
            // Domain membership is carried separately at each indexed place.
            // Peel only constraints for this owned-carrier classification;
            // a reference still cannot become an owned linear element.
            let mut element_carrier = *element_type;
            while let TypeReferenceNode::Constrained { base_type, .. } = self
                .program
                .type_reference_table
                .type_reference(element_carrier)
            {
                element_carrier = *base_type;
            }
            // An empty extent removes elements, not their declared type.
            // Collect the same owned element shell at every length; the
            // recursive collector still rejects unknown and recursive types.
            if (!plain_owned_array
                && !owned_affine_array
                && (substitutions.iter().any(|(parameter, _)| {
                    crate::monomorphization::runtime_bound_occurrence_in(
                        self.program,
                        &[*element_type],
                        *parameter,
                    )
                }) || (!matches!(
                    self.program
                        .type_reference_table
                        .type_reference(element_carrier),
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
                ) && !unrestricted_primitive_element
                    && !nested_array_element)
                    || (crate::checks::type_multiplicity(self.program, *element_type)
                        != Multiplicity::Linear
                        && !unrestricted_primitive_element
                        && !material_data_element
                        && !nested_array_element)))
                || !self.in_progress.insert(identity.clone())
            {
                return None;
            }
            let Some(element_type_identity) = self.add_type(*element_type, binders, substitutions)
            else {
                self.in_progress.remove(&identity);
                return None;
            };
            let length = u64::try_from(length).ok()?;
            self.types.insert(
                identity.clone(),
                CheckedUnitStructuralTypePlan {
                    identity: identity.clone(),
                    shape: CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        length,
                    },
                },
            );
            self.in_progress.remove(&identity);
            return Some(identity);
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
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == data_symbol)?
            .clone();
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
    ) -> Option<String> {
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
    ) -> Option<String> {
        // Construction-prefix admission remains owned by its separate caller.
        self.add_type(type_reference, binders, &[])
    }

    pub(in crate::execution::terminal_unit) fn add_data_shape(
        &mut self,
        identity: String,
        data: typed_trees::data::DataDefinition,
        binders: &[(SymbolHandle, String)],
        substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    ) -> Option<String> {
        if self.types.contains_key(&identity) {
            return Some(identity);
        }
        if !self.in_progress.insert(identity.clone()) {
            return None;
        }
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
                            &identity,
                            stated(field),
                        ) else {
                            self.in_progress.remove(&identity);
                            return None;
                        };
                        fields.push(field);
                    }
                    DataMember::Variant(variant) => {
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
                                &identity,
                                bounds,
                            ) else {
                                self.in_progress.remove(&identity);
                                return None;
                            };
                            payload_fields.push(field);
                        }
                        cases.push(checked_trees::CheckedUnitStructuralCasePlan {
                            identity: variant.path_identity(),
                            fields: payload_fields,
                        });
                    }
                }
            }
            self.types.insert(
                identity.clone(),
                CheckedUnitStructuralTypePlan {
                    identity: identity.clone(),
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

        let mut fields = Vec::new();
        for member in &members {
            let DataMember::Field(field) = member else {
                self.in_progress.remove(&identity);
                return None;
            };
            let bounds = stated(field);
            let Some(field) =
                self.structural_field_plan(field, binders, &substitutions, &identity, bounds)
            else {
                self.in_progress.remove(&identity);
                return None;
            };
            fields.push(field);
        }
        self.types.insert(
            identity.clone(),
            CheckedUnitStructuralTypePlan {
                identity: identity.clone(),
                shape: CheckedUnitStructuralTypeShape::Record { fields },
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
        owner_identity: &str,
        stated_bounds: Option<(Option<i64>, Option<i64>)>,
    ) -> Option<CheckedUnitStructuralFieldPlan> {
        // Classify the actual member before choosing its storage route. A
        // formal T instantiated with a reference or Binding must retain that
        // shell, not reach add_type's referent-oriented fallback as plain T.
        let field_type_reference =
            substituted_formal_type(self.program, field.type_reference, substitutions);
        let field_type = if field.relevance.is_erased() {
            CheckedUnitStructuralFieldType::Erased {
                type_identity: self
                    .program
                    .type_identity(TypeIdentityRequest {
                        binders,
                        substitutions,
                        ..TypeIdentityRequest::ordinary(field_type_reference)
                    })
                    .into_string(),
            }
        } else if let Some((fused_service_erasure, provider_node)) =
            provider_backed_field(self.program, field_type_reference)
        {
            let provider_type_identity = self
                .program
                .type_identity(TypeIdentityRequest {
                    binders,
                    substitutions,
                    ..TypeIdentityRequest::ordinary(provider_node)
                })
                .into_string();
            match fused_service_erasure {
                Some(erasure) => {
                    self.add_fused_service_carrier_type(provider_node, binders)?;
                    CheckedUnitStructuralFieldType::FusedServiceBacked {
                        provider_type_identity,
                        erasure,
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
        } else if is_reference(self.program, field_type_reference)
            && element_view_shell(self.program, field_type_reference, substitutions).is_none()
        {
            // Provider and sequence fields have their own carriers above or
            // in add_type. Every other stored loan needs an exact Reference
            // shape: an unsupported access must fail here, never fall through
            // to add_type and turn borrowed storage into inline owned data.
            structural_access_for_type_reference(self.program, field_type_reference)?;
            let shell = validation::reference_shell(self.program, field_type_reference)?;
            if matches!(
                self.program
                    .type_reference_table
                    .type_reference(shell.referee),
                TypeReferenceNode::Constrained { .. }
            ) {
                // Reference shapes have no referent qualification slot.
                return None;
            }
            CheckedUnitStructuralFieldType::Structural {
                type_identity: self.add_reference_type(field_type_reference, binders)?,
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
                    let nested = self.add_type(field_type_reference, binders, substitutions)?;
                    if nested == owner_identity {
                        return None;
                    }
                    CheckedUnitStructuralFieldType::Structural {
                        type_identity: nested,
                    }
                }
            }
        };
        Some(CheckedUnitStructuralFieldPlan {
            identity: field.path_identity(),
            relevance: field.relevance,
            field_type,
        })
    }

    /// Register the semantic binding independently from its occurrence receipt.
    /// Keeping a nonconstructible type lets fields and parameters share ordinary
    /// loan paths without turning zero-sized layout into establishment authority.
    fn add_fused_service_carrier_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<String> {
        let carrier = typed_trees::service::classify_exact_bound_service_carrier(
            self.program,
            type_reference,
        )
        .ok()??;
        self.program.fused_service_erasure(carrier.requirement)?;
        let identity = self
            .program
            .type_identity(TypeIdentityRequest {
                binders,
                ..TypeIdentityRequest::ordinary(type_reference)
            })
            .into_string();
        let plan = CheckedUnitStructuralTypePlan {
            identity: identity.clone(),
            shape: CheckedUnitStructuralTypeShape::FusedServiceBinding,
        };
        if self
            .types
            .get(&identity)
            .is_some_and(|existing| existing != &plan)
        {
            return None;
        }
        self.types.insert(identity.clone(), plan);
        Some(identity)
    }

    /// Retain the exact source parameter, including any borrow shell, beside
    /// its zero-payload semantic carrier. The receipt grants no new binding.
    pub(in crate::execution::terminal_unit) fn add_fused_service_parameter_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        source_parameter: SymbolHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<(String, checked_trees::CheckedFusedServiceParameterReceipt)> {
        let parameter_carrier = typed_trees::service::classify_exact_service_parameter_carrier(
            self.program,
            type_reference,
        )
        .ok()??;
        let carrier = parameter_carrier.carrier;
        self.program.fused_service_erasure(carrier.requirement)?;
        if !source_parameter.is_valid() {
            return None;
        }

        let type_identity =
            self.add_fused_service_carrier_type(parameter_carrier.carrier_type, binders)?;

        Some((
            type_identity,
            checked_trees::CheckedFusedServiceParameterReceipt {
                source_parameter,
                carrier_type_identity: self
                    .program
                    .type_identity(TypeIdentityRequest {
                        binders,
                        ..TypeIdentityRequest::ordinary(type_reference)
                    })
                    .into_string(),
                requirement: carrier.requirement,
            },
        ))
    }

    pub(in crate::execution::terminal_unit) fn retain_transitive(
        &mut self,
        roots: &BTreeSet<&str>,
    ) {
        let mut retained = roots
            .iter()
            .map(|root| (*root).to_owned())
            .collect::<BTreeSet<_>>();
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
                        retained.insert(referent_identity.clone());
                    }
                    // The element row `add_type` registered beside a
                    // borrowed byte view survives with it: an element-view
                    // establishment of that view resolves its op's element
                    // through this row.
                    CheckedUnitStructuralTypeShape::ByteSequence(_) => {
                        retained.extend(self.byte_view_elements.iter().cloned());
                    }
                    CheckedUnitStructuralTypeShape::FusedServiceBinding
                    | CheckedUnitStructuralTypeShape::PrimitiveScalar(_) => {}
                    CheckedUnitStructuralTypeShape::Record { fields } => {
                        for field in fields {
                            if let CheckedUnitStructuralFieldType::Structural { type_identity }
                            | CheckedUnitStructuralFieldType::FusedServiceBacked {
                                provider_type_identity: type_identity,
                                ..
                            } = &field.field_type
                            {
                                retained.insert(type_identity.clone());
                            }
                        }
                    }
                    CheckedUnitStructuralTypeShape::FixedArray {
                        element_type_identity,
                        ..
                    }
                    | CheckedUnitStructuralTypeShape::BorrowedSliceView {
                        element_type_identity,
                    } => {
                        retained.insert(element_type_identity.clone());
                    }
                    CheckedUnitStructuralTypeShape::Sum { cases } => {
                        for field in cases.iter().flat_map(|case| &case.fields) {
                            if let CheckedUnitStructuralFieldType::Structural { type_identity }
                            | CheckedUnitStructuralFieldType::FusedServiceBacked {
                                provider_type_identity: type_identity,
                                ..
                            } = &field.field_type
                            {
                                retained.insert(type_identity.clone());
                            }
                        }
                    }
                    CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
                        for field in fields
                            .iter()
                            .chain(cases.iter().flat_map(|case| &case.fields))
                        {
                            if let CheckedUnitStructuralFieldType::Structural { type_identity }
                            | CheckedUnitStructuralFieldType::FusedServiceBacked {
                                provider_type_identity: type_identity,
                                ..
                            } = &field.field_type
                            {
                                retained.insert(type_identity.clone());
                            }
                        }
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

/// A provider-backed field's erasure receipt (when the carrier is an exact
/// `Binding<R>` requirement) plus the type node whose identity names the
/// provider — the carrier itself for a fused service, the unwrapped
/// boundary-trait referent for a `&'a mut <boundary trait>` field.
pub(super) fn provider_backed_field(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(
    Option<checked_trees::CheckedFusedServiceErasureReceipt>,
    TypeReferenceHandle,
)> {
    if let Some(requirement) =
        typed_trees::service::exact_bound_service_requirement(program, type_reference)
    {
        // A `Binding<boundary>` field with no settled provider selection still
        // carries a provider handle: mint the unerased ProviderBacked shape so
        // the record keeps its plan. Establishment — not shape collection —
        // owns rejecting the missing selection, which lets the review's
        // discovery pass mint this shape before its nomination exists.
        let authorization = program.fused_service_erasure(requirement);
        return Some((
            authorization.map(|_| checked_trees::CheckedFusedServiceErasureReceipt { requirement }),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::front_end::typed_program_with_core_service;

    fn carrier_field(program: &TypedTrees) -> TypeReferenceHandle {
        let carrier = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == "Carrier")
            .expect("carrier");
        let [DataMember::Field(field)] = program.data_members(carrier) else {
            panic!("one carrier field");
        };
        field.type_reference
    }

    #[test]
    fn array_shapes_do_not_erase_unrepresented_element_shells() {
        for member in [
            "[&mut u64; 2]",
            "[u64 [0..=7]; 2]",
            "[[&mut u64; 2]; 3]",
            "[Token; 0]",
            "[Wrapper<&write u64>; 2]",
            "[Wrapper<&mut &mut u64>; 2]",
            "[Wrapper<&mut u64 [0..=7]>; 2]",
        ] {
            let program = typed_program_with_core_service(&format!(
                "data Token {{ value: u64; }} data Wrapper<T> {{ value: T; }} data Carrier {{ values: {member}; }}"
            ));
            assert!(
                ShapeCollector::new(&program)
                    .add_type(carrier_field(&program), &[], &[])
                    .is_none(),
                "{member} has no faithful array element representation"
            );
        }
    }

    #[test]
    fn provider_array_shape_does_not_grant_partial_affine_disposal() {
        let program = typed_program_with_core_service(
            r#"
            pub boundary trait Host { machine exit(code: i32); }
            data Task { host: Binding<Host>; }
            data Slot { case Empty; case Held(task: Task); }
            data Carrier { values: [Slot; 2]; }
        "#,
        );
        let reference = carrier_field(&program);
        let mut collector = ShapeCollector::new(&program);
        assert!(collector.add_type(reference, &[], &[]).is_some());
        assert!(
            collector.add_partial_affine_type(reference, &[]).is_none(),
            "a layout cannot authorize disposal of provider-bearing contents"
        );
    }

    #[test]
    fn generic_array_members_retain_substituted_reference_shells() {
        for referee in ["u64", "Token"] {
            let program = typed_program_with_core_service(&format!(
                "data Token {{ value: u64; }} data Wrapper<T> {{ value: T; }} data Carrier {{ values: [Wrapper<&mut {referee}>; 2]; }}",
            ));
            let mut collector = ShapeCollector::new(&program);
            let identity = collector
                .add_type(carrier_field(&program), &[], &[])
                .expect("array layout");
            let CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                ..
            } = &collector.types[&identity].shape
            else {
                panic!("array");
            };
            let CheckedUnitStructuralTypeShape::Record { fields } =
                &collector.types[element_type_identity].shape
            else {
                panic!("wrapper record");
            };
            let CheckedUnitStructuralFieldType::Structural { type_identity } =
                &fields[0].field_type
            else {
                panic!("stored reference");
            };
            assert!(
                matches!(
                    collector.types[type_identity].shape,
                    CheckedUnitStructuralTypeShape::Reference {
                        access: CheckedStructuralAccess::MutableBorrow,
                        ..
                    }
                ),
                "a substituted reference must not become inline referent storage"
            );
        }
    }
}
