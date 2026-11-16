//! Run custody uses ordinary prepared callable occurrences. This owner retains
//! their exact application; the derived index contains only boundary IDs and
//! grants no type, contract or live-cell authority.

use semantic_vocabulary::{BoundaryMachineId, IdTable};
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, static_applications::StaticMachineApplicationHandle};

use super::BoundarySource;

pub(crate) struct BoundarySources {
    rows: IdTable<BoundaryMachineId, BoundarySource>,
    applications: Vec<(StaticMachineApplicationHandle, Option<BoundaryMachineId>)>,
}

fn application_key(application: StaticMachineApplicationHandle) -> (u32, u32) {
    (application.arena_index(), application.generation())
}

impl BoundarySources {
    pub(super) fn new(typed: &TypedTrees) -> Self {
        // Only actual arena-owned occurrences determine allocation and keys;
        // a queried foreign or stale handle can never enlarge this index.
        let mut applications = typed
            .expression_table
            .static_machine_applications()
            .map(|(application, _)| (application, None))
            .collect::<Vec<_>>();
        applications.sort_unstable_by_key(|(application, _)| application_key(*application));
        Self {
            rows: IdTable::default(),
            applications,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.rows.len()
    }
    pub(super) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = (BoundaryMachineId, &BoundarySource)> {
        self.rows.iter()
    }
    pub(super) fn get(&self, id: BoundaryMachineId) -> Option<&BoundarySource> {
        self.rows.get(id)
    }
    pub(super) fn push(&mut self, symbol: SymbolHandle) -> BoundaryMachineId {
        self.rows.push(BoundarySource {
            symbol,
            application: None,
            element: None,
        })
    }
    pub(crate) fn symbols(&self) -> IdTable<BoundaryMachineId, SymbolHandle> {
        self.rows.map(|_, owner| owner.symbol)
    }
    fn index(
        &self,
        typed: &TypedTrees,
        application: StaticMachineApplicationHandle,
    ) -> Result<usize, &'static str> {
        if !typed
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return Err("run custody application has no live source occurrence");
        }
        self.applications
            .binary_search_by_key(&application_key(application), |(known, _)| {
                application_key(*known)
            })
            .map_err(|_| "run custody application is outside its prepared source owner")
    }
    pub(super) fn application(
        &self,
        typed: &TypedTrees,
        application: StaticMachineApplicationHandle,
        symbol: SymbolHandle,
    ) -> Result<Option<BoundaryMachineId>, &'static str> {
        let index = self.index(typed, application)?;
        let Some(id) = self.applications[index].1 else {
            return Ok(None);
        };
        if !self.rows.get(id).is_some_and(|owner| {
            owner.symbol == symbol
                && owner.application == Some(application)
                && owner.element.is_some()
        }) {
            return Err("run custody cached boundary belongs to another source application");
        }
        Ok(Some(id))
    }
    pub(super) fn insert_application(
        &mut self,
        typed: &TypedTrees,
        application: StaticMachineApplicationHandle,
        symbol: SymbolHandle,
        element: crate::body::ValueType,
    ) -> Result<BoundaryMachineId, &'static str> {
        let index = self.index(typed, application)?;
        if self.applications[index].1.is_some() {
            return Err("run custody source application already has a published boundary");
        }
        let id = self.rows.push(BoundarySource {
            symbol,
            application: Some(application),
            element: Some(element),
        });
        self.applications[index].1 = Some(id);
        Ok(id)
    }
}

use language_core::core_vocabulary::{CoreVocabulary, ExtentDomain, RunDeclaration};
use language_core::sealed_intrinsics::{CompilerPrimitive, RunCustodyOperation, SealedIntrinsic};
use semantic_vocabulary::{MachineId, TargetMachineCallable};
use terminal_check_tables::{Checked, Construct};
use typed_trees::data::{DataDefinition, DataMember, TypeParameterKind};
use typed_trees::references::StaticArgumentCall;
use typed_trees::signature::StateSignature;
use typed_trees::static_applications::StaticMachineBindingValue;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

use super::{Program, Slot};
use crate::body::{Expression, Termination, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

fn same_type(typed: &TypedTrees, left: TypeReferenceHandle, right: TypeReferenceHandle) -> bool {
    if !typed.type_reference_table.contains_type_reference(left)
        || !typed.type_reference_table.contains_type_reference(right)
    {
        return false;
    }
    let left = typed.normalized_type_identity(left);
    left.is_valid() && left == typed.normalized_type_identity(right)
}

fn qualified_base(
    typed: &TypedTrees,
    reference: TypeReferenceHandle,
    domains: &[CoreVocabulary],
    arguments: &[TypeReferenceHandle],
) -> Result<TypeReferenceHandle, &'static str> {
    if !typed
        .type_reference_table
        .contains_type_reference(reference)
    {
        return Err("run custody has a stale type occurrence");
    }
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = typed.type_reference_table.type_reference(reference)
    else {
        return if domains.is_empty() {
            Ok(reference)
        } else {
            Err("run custody lost its exact domain qualifications")
        };
    };
    let span = *constraints;
    let constraints = typed.type_reference_table.constraints(span);
    if constraints.len() != span.len()
        || (span.is_empty() && span != arena::HandleSpan::empty())
        || constraints.len() != domains.len()
    {
        return Err("run custody changed its domain qualification roster");
    }
    for expected in domains {
        let mut matching = constraints
            .iter()
            .filter_map(|constraint| match constraint {
                TypeConstraintNode::Domain(domain)
                    if typed.core_vocabulary(domain.symbol) == Some(*expected) =>
                {
                    Some(domain)
                }
                _ => None,
            });
        let domain = matching
            .next()
            .ok_or("run custody domain has no exact core declaration")?;
        if matching.next().is_some()
            || domain.names_no_visible_domain
            || domain.subject != typed_trees::types::DomainConstraintSubject::Declared
            || domain.arguments.len() != arguments.len()
            || !domain
                .arguments
                .iter()
                .zip(arguments)
                .all(|(left, right)| same_type(typed, *left, *right))
        {
            return Err("run custody domain changed its actual type arguments");
        }
    }
    Ok(*base_type)
}

fn record_role<'a>(
    typed: &'a TypedTrees,
    machine: MachineId,
    reference: TypeReferenceHandle,
    role: CoreVocabulary,
    arguments: &[TypeReferenceHandle],
) -> Checked<&'a DataDefinition> {
    let unavailable = |detail| refuse(machine, Construct::Machine, Unserved::Call(detail));
    if !typed
        .type_reference_table
        .contains_type_reference(reference)
    {
        return Err(unavailable(
            "run custody has a stale nominal data occurrence",
        ));
    }
    let data = typed
        .data_application_definition(reference)
        .or_else(
            || match typed.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } => {
                    typed.unique_data_definition_by_symbol(*symbol)
                }
                _ => None,
            },
        )
        .ok_or_else(|| unavailable("run custody needs its prepared nominal data application"))?;
    let origin = data.generic_instance.unwrap_or(reference);
    if !typed.type_reference_table.contains_type_reference(origin) {
        return Err(unavailable("run custody lost its original data occurrence"));
    }
    let (declaration, actuals, lifetimes) = match typed.type_reference_table.type_reference(origin)
    {
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            lifetime_arguments,
            ..
        } => (
            *base_symbol,
            typed
                .type_reference_table
                .type_reference_handles(*arguments),
            lifetime_arguments.as_slice(),
        ),
        TypeReferenceNode::Named { symbol, .. } => (*symbol, &[][..], &[][..]),
        _ => {
            return Err(unavailable(
                "run custody data lost its original nominal application",
            ));
        }
    };
    if typed.core_vocabulary(declaration) != Some(role)
        || actuals.len() != arguments.len()
        || !actuals
            .iter()
            .zip(arguments)
            .all(|(left, right)| same_type(typed, *left, *right))
        || !lifetimes.is_empty()
        || !data.lifetime_parameters.is_empty()
        || !data.type_parameters.is_empty()
        || !data.conformance_bounds.is_empty()
        || data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
        || data.properties.multiplicity != language_semantics::Multiplicity::Linear
        || data.properties.carry.is_some()
        || data.cleanup_selection.is_some()
        || data.quotient.is_some()
        || !data.requires_facts.is_empty()
    {
        return Err(unavailable(
            "run custody changed its exact linear nominal type roles",
        ));
    }
    super::source_records::source_application(typed, data, reference, machine)?;
    Ok(data)
}

fn run_role(
    typed: &TypedTrees,
    machine: MachineId,
    reference: TypeReferenceHandle,
    arguments: &[TypeReferenceHandle],
) -> Checked<()> {
    let unavailable = |detail| refuse(machine, Construct::Machine, Unserved::Call(detail));
    let base = qualified_base(
        typed,
        reference,
        &[CoreVocabulary::Run(RunDeclaration::Live)],
        arguments,
    )
    .map_err(unavailable)?;
    let data = record_role(
        typed,
        machine,
        base,
        CoreVocabulary::Run(RunDeclaration::Run),
        arguments,
    )?;
    let fields = typed.data_members(data);
    let (Some(DataMember::Field(storage)), Some(DataMember::Field(count)), 2) =
        (fields.get(0), fields.get(1), fields.len())
    else {
        return Err(unavailable(
            "run custody changed its storage or count fields",
        ));
    };
    if storage.relevance != language_core::BindingRelevance::Relevant
        || count.relevance != language_core::BindingRelevance::Relevant
    {
        return Err(unavailable(
            "run custody cannot erase its descriptor fields",
        ));
    }
    let storage = qualified_base(
        typed,
        storage.type_reference,
        &[CoreVocabulary::ExtentDomain(ExtentDomain::Granted)],
        &[],
    )
    .map_err(unavailable)?;
    record_role(typed, machine, storage, CoreVocabulary::Extent, &[])?;
    if !plain_u64(typed, count.type_reference) {
        return Err(unavailable(
            "run custody count is not the exact u64 carrier",
        ));
    }
    Ok(())
}

fn plain_u64(typed: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    typed
        .type_reference_table
        .contains_type_reference(reference)
        && matches!(typed.type_reference_table.type_reference(reference), TypeReferenceNode::Named { symbol, .. } if typed.symbols.builtin_type_atom(*symbol) == Some(symbols::BuiltinTypeAtom::U64))
}

fn check_roles(
    typed: &TypedTrees,
    machine: MachineId,
    application: StaticMachineApplicationHandle,
    target: SymbolHandle,
    operation: RunCustodyOperation,
    signature: &StateSignature,
) -> Checked<()> {
    let unavailable = |detail| refuse(machine, Construct::Machine, Unserved::Call(detail));
    let declared = typed
        .machine_by_symbol(target)
        .ok_or_else(|| unavailable("run custody lost its template machine"))?;
    let parameters = typed.machine_type_parameters(declared);
    let bindings = typed
        .expression_table
        .static_machine_application_bindings(application);
    let [p, t] = bindings else {
        return Err(unavailable(
            "run custody needs both actual P and T bindings",
        ));
    };
    if parameters.len() != 2
        || !parameters.iter().zip(bindings).all(|(parameter, binding)| {
            parameter.symbol == binding.parameter
                && parameter.kind == TypeParameterKind::Type
                && parameter.bounds == Default::default()
        })
    {
        return Err(unavailable(
            "run custody changed its ordinary P and T telescope",
        ));
    }
    let (StaticMachineBindingValue::Type(p), StaticMachineBindingValue::Type(t)) =
        (&p.value, &t.value)
    else {
        return Err(unavailable("run custody needs actual type operands"));
    };
    let arguments = [*p, *t];
    let formals = typed
        .state_parameters
        .span(signature.parameters)
        .ok_or_else(|| unavailable("run custody has a stale formal roster"))?;
    if !signature.type_parameters.is_empty()
        || !signature.conformance_bounds.is_empty()
        || !signature.lifetime_parameters.is_empty()
        || formals.iter().any(|formal| {
            formal.is_const
                || formal.is_mutable
                || formal.relevance != language_core::BindingRelevance::Relevant
        })
    {
        return Err(unavailable(
            "run custody changed its owned closed formal contract",
        ));
    }
    match (operation, formals) {
        (RunCustodyOperation::ReleaseEmpty, [run]) => {
            run_role(typed, machine, run.type_reference, &arguments)?;
            let result = qualified_base(
                typed,
                signature.return_type,
                &[
                    CoreVocabulary::ExtentDomain(ExtentDomain::Granted),
                    CoreVocabulary::ExtentDomain(ExtentDomain::Vacant),
                ],
                &[],
            )
            .map_err(unavailable)?;
            record_role(typed, machine, result, CoreVocabulary::Extent, &[])?;
        }
        (RunCustodyOperation::Split, [run, cut]) if plain_u64(typed, cut.type_reference) => {
            run_role(typed, machine, run.type_reference, &arguments)?;
            let base =
                qualified_base(typed, signature.return_type, &[], &[]).map_err(unavailable)?;
            let parts = record_role(
                typed,
                machine,
                base,
                CoreVocabulary::Run(RunDeclaration::Parts),
                &arguments,
            )?;
            let fields = typed.data_members(parts);
            let (Some(DataMember::Field(left)), Some(DataMember::Field(right)), 2) =
                (fields.get(0), fields.get(1), fields.len())
            else {
                return Err(unavailable(
                    "run split changed its exact prefix and suffix fields",
                ));
            };
            if left.relevance != language_core::BindingRelevance::Relevant
                || right.relevance != language_core::BindingRelevance::Relevant
            {
                return Err(unavailable("run split cannot erase an owned part"));
            }
            run_role(typed, machine, left.type_reference, &arguments)?;
            run_role(typed, machine, right.type_reference, &arguments)?;
        }
        (RunCustodyOperation::Merge, [left, right]) => {
            for reference in [
                left.type_reference,
                right.type_reference,
                signature.return_type,
            ] {
                run_role(typed, machine, reference, &arguments)?;
            }
        }
        _ => {
            return Err(unavailable(
                "run custody changed its complete parameter and result roles",
            ));
        }
    }
    Ok(())
}

impl Program<'_> {
    pub(super) fn run_custody_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: StaticArgumentCall<'_>,
        source: SourceOrigin,
        expected: Option<ValueType>,
    ) -> Checked<Option<Expression>> {
        let typed = self.typed;
        let authored = match call {
            StaticArgumentCall::Expression(_, call) => call.target_symbol,
            StaticArgumentCall::Statement(call) => call.target_symbol,
        };
        let Some(machine) = self
            .typed
            .machine_by_symbol(authored)
            .or_else(|| typed.machine_holding_state(authored))
        else {
            return Ok(None);
        };
        let Some(SealedIntrinsic::CompilerPrimitive(CompilerPrimitive::RunCustody(operation))) =
            typed
                .symbols
                .sealed_declaration(machine.symbol, machine.name.as_str())
        else {
            return Ok(None);
        };
        let target = self.call_target_symbol(id, authored)?;
        let (selection, binding, arguments, unsupported) = match call {
            StaticArgumentCall::Expression(_, call) => (
                call.authored_call_selection,
                call.authored_call_binding,
                typed.expression_table.expression_handles(call.arguments),
                (call.receiver.is_valid() && !typed.call_has_no_runtime_receiver_for_target(call))
                    || !call.evidence_arguments.is_empty()
                    || call.requirement_application.is_valid()
                    || call.exact_coercion_target.is_valid()
                    || call.build_selection.is_valid()
                    || call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
                    || call.installed_interrupt_operation.is_some()
                    || call.layout_query.is_some()
                    || call.native_code_layout.is_some()
                    || call.operational_acknowledgement.acknowledges_suspend
                    || call.operational_acknowledgement.acknowledges_block,
            ),
            StaticArgumentCall::Statement(call) => (
                call.authored_call_selection,
                None,
                typed.statement_table.expression_handles(call.arguments),
                call.receiver_root_symbol.is_valid()
                    || call.receiver_symbol.is_valid()
                    || !call.receiver.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || call.installed_interrupt_operation.is_some()
                    || call.reflected_field_origin.is_some()
                    || call.operational_acknowledgement.acknowledges_suspend
                    || call.operational_acknowledgement.acknowledges_block,
            ),
        };
        let unavailable = |detail| refuse(id, Construct::Machine, Unserved::Call(detail));
        self.visible_to(id, target)?;
        self.direct_selection(id, authored, selection, binding)?;
        if unsupported {
            return Err(unavailable(
                "run custody has another call feature or runtime receiver",
            ));
        }
        let application = call.application();
        let (actual_operation, prepared) = self
            .typed
            .compiler_primitive_callable_signature(application, target)
            .map_err(unavailable)?
            .ok_or_else(|| unavailable("run custody has no prepared closed source contract"))?;
        if actual_operation != CompilerPrimitive::RunCustody(operation) {
            return Err(unavailable(
                "run custody changed its exact compiler operation",
            ));
        }
        let occurrence = typed
            .expression_table
            .static_machine_application(application);
        if self.symbols.get(id).copied() != Some(occurrence.lexical_owner)
            || occurrence.binding_declaration != target
        {
            return Err(unavailable(
                "run custody belongs to another source call owner",
            ));
        }
        let authored_arguments = match call {
            StaticArgumentCall::Expression(_, call) => &call.machine_arguments,
            StaticArgumentCall::Statement(call) => &call.machine_arguments,
        };
        if !authored_arguments.is_empty()
            && authored_arguments.as_ref()
                != typed
                    .expression_table
                    .static_machine_application_arguments(application)
        {
            return Err(unavailable(
                "run custody changed its authored static arguments",
            ));
        }
        self.run_custody_header(id, application, target, operation, prepared)?;
        let boundary = match self
            .boundary_symbols
            .application(typed, application, target)
            .map_err(unavailable)?
        {
            Some(boundary) => boundary,
            None => {
                let (signature, _) = self.declared_requirement_signature(id, prepared, &[])?;
                if !signature.borrowed.is_empty()
                    || !signature.erased.is_empty()
                    || signature.crashes != crate::CrashCeiling::DeclaredNone
                    || signature.reaches != crate::Ceiling::Authored(Vec::new())
                    || signature.invokes != crate::Ceiling::Authored(Vec::new())
                    || signature.termination != Termination::Promised
                    || signature.ranking.is_some()
                {
                    return Err(unavailable(
                        "run custody lost its total owned source contract",
                    ));
                }
                let typed_trees::static_applications::StaticMachineBindingValue::Type(element) =
                    typed
                        .expression_table
                        .static_machine_application_bindings(application)[1]
                        .value
                else {
                    return Err(unavailable("run custody lost its actual element operand"));
                };
                let element = self.value_type(id, element)?;
                if matches!(element, ValueType::Rigid(_)) {
                    return Err(unavailable(
                        "run custody element still has an open type role",
                    ));
                }
                let boundary = self
                    .boundary_symbols
                    .insert_application(typed, application, target, element)
                    .map_err(unavailable)?;
                if self.boundary_signatures.push(signature) != boundary
                    || self.boundary_services.push(None) != boundary
                    || self
                        .boundary_completions
                        .push(semantic_vocabulary::BoundaryCompletion::NormalReturn)
                        != boundary
                {
                    return Err(unavailable(
                        "run custody lost its exact boundary coordinate",
                    ));
                }
                boundary
            }
        };
        self.call_of(
            id,
            slots,
            (TargetMachineCallable::Boundary(boundary), None),
            arguments,
            source,
            expected,
        )
        .map(Some)
    }

    fn run_custody_header(
        &self,
        id: MachineId,
        application: StaticMachineApplicationHandle,
        target: SymbolHandle,
        operation: RunCustodyOperation,
        prepared: &StateSignature,
    ) -> Checked<()> {
        let unavailable = |detail| refuse(id, Construct::Machine, Unserved::Call(detail));
        let machine = self
            .typed
            .machine_by_symbol(target)
            .ok_or_else(|| unavailable("run custody lost its source machine"))?;
        let public = operation != RunCustodyOperation::ReleaseEmpty;
        let exposure = if public {
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
        } else {
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
        };
        if machine.is_public != public
            || machine.interface_exposure != exposure
            || machine.purpose != typed_trees::machine::MachinePurpose::Runtime
            || machine.spelling.is_some()
            || machine.admitted_same_stack_bytes != 0
            || !machine.lifetime_parameters.is_empty()
            || machine.assembly_target_requirement
                != language_core::inline_assembly::AsmTargetApplicability::Any
            || prepared.suspends
            || prepared.blocks
            || !prepared.invokes.is_empty()
            || prepared.service_reach_is_installation_bound
            || prepared.service_reach_row != language_semantics::ServiceReachRowTable::EMPTY_ROW
            || !prepared.declares_no_crash_route
            || prepared.termination_guarantee
                != (language_semantics::TerminationGuarantee::Terminates {
                    premises: Vec::new(),
                })
        {
            return Err(unavailable(
                "run custody changed its complete provider-free source header",
            ));
        }
        check_roles(self.typed, id, application, target, operation, prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::{BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{data::DataProperties, expression::StaticMachineArgument, name::Identifier};

    #[test]
    fn closed_boundary_applications_cannot_reuse_a_template_or_another_occurrence() {
        let mut typed = TypedTrees::default();
        let target = SymbolHandle::from_parts(7, 1);
        let p = typed.type_reference_table.insert(TypeReferenceNode::Unit);
        let t = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::from_parts(8, 1),
            name: Identifier::generated("T"),
        });
        let application = |typed: &mut TypedTrees, types: [TypeReferenceHandle; 2]| {
            typed.expression_table.retain_static_machine_application(
                target,
                Identifier::generated("split"),
                &types.map(|type_reference| StaticMachineArgument {
                    type_reference,
                    ..Default::default()
                }),
            )
        };
        let first = application(&mut typed, [p, t]);
        let second = application(&mut typed, [t, p]);
        let unprepared = application(&mut typed, [p, t]);
        let mut boundaries = BoundarySources::new(&typed);
        let template = boundaries.push(target);
        assert_eq!(boundaries.application(&typed, first, target).unwrap(), None);
        let first_id = boundaries
            .insert_application(
                &typed,
                first,
                target,
                ValueType::plain(semantic_vocabulary::ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                )),
            )
            .unwrap();
        let second_id = boundaries
            .insert_application(
                &typed,
                second,
                target,
                ValueType::plain(semantic_vocabulary::ScalarType::Boolean),
            )
            .unwrap();
        assert_ne!(template, first_id);
        assert_ne!(first_id, second_id);
        assert_eq!(
            boundaries.application(&typed, first, target).unwrap(),
            Some(first_id)
        );
        assert_eq!(
            boundaries.application(&typed, second, target).unwrap(),
            Some(second_id)
        );
        assert_eq!(
            boundaries.application(&typed, unprepared, target).unwrap(),
            None
        );
        assert!(
            boundaries
                .application(&typed, first, SymbolHandle::from_parts(7, 2))
                .is_err()
        );
        let stale =
            StaticMachineApplicationHandle::from_parts(first.arena_index(), first.generation() + 1);
        assert!(boundaries.application(&typed, stale, target).is_err());
        assert!(
            boundaries
                .insert_application(
                    &typed,
                    first,
                    target,
                    ValueType::plain(semantic_vocabulary::ScalarType::Boolean)
                )
                .is_err()
        );
        assert_eq!(
            boundaries.get(first_id).unwrap().element,
            Some(ValueType::plain(semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64
                )
                .unwrap()
            )))
        );
    }

    #[test]
    fn run_role_checks_refuse_lookalike_nominals_domains_and_borrowed_headers() {
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("user"));
        let declarations = symbols.insert_children(
            root,
            symbols::builtin_type_symbols().into_iter().chain([
                (SymbolKind::Data, SymbolNameRef::Static("Run")),
                (SymbolKind::Domain, SymbolNameRef::Static("Live")),
            ]),
        );
        let run = declarations.handle_at(BuiltinTypeAtom::ALL.len() as u32);
        let live = declarations.handle_at(BuiltinTypeAtom::ALL.len() as u32 + 1);
        let mut typed = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let run_type = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: run,
            name: Identifier::generated("Run"),
        });
        typed.push_data_definition(DataDefinition {
            symbol: run,
            name: Identifier::generated("Run"),
            properties: DataProperties {
                multiplicity: language_semantics::Multiplicity::Linear,
                ..Default::default()
            },
            ..Default::default()
        });
        let machine = MachineId::new(1).unwrap();
        assert!(
            record_role(
                &typed,
                machine,
                run_type,
                CoreVocabulary::Run(RunDeclaration::Run),
                &[]
            )
            .unwrap_err()
            .detail
            .contains("exact linear nominal type roles")
        );
        let constraints =
            typed
                .type_reference_table
                .insert_constraints([TypeConstraintNode::Domain(
                    typed_trees::types::DomainConstraint {
                        symbol: live,
                        name: Identifier::generated("Live"),
                        ..Default::default()
                    },
                )]);
        let qualified = typed
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: run_type,
                constraints,
            });
        assert_eq!(
            qualified_base(
                &typed,
                qualified,
                &[CoreVocabulary::Run(RunDeclaration::Live)],
                &[]
            )
            .unwrap_err(),
            "run custody domain has no exact core declaration"
        );
        let borrowed = typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: run_type,
                access: language_core::ReferenceAccess::Mutable,
                lifetime: None,
            });
        assert!(
            run_role(&typed, machine, borrowed, &[])
                .unwrap_err()
                .detail
                .contains("exact domain qualifications")
        );
    }
}
