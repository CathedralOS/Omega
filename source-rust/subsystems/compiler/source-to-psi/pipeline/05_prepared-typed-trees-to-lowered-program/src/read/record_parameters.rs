//! Generic checking records the trait required of each const record. Calls
//! keep the public requirement separate from the selected implementation.

use super::Program;
use crate::lowering::static_contracts::SourceRecordBinding;
use crate::source_state::data::TypeParameter;
use crate::source_state::signature::StateSignature;
use crate::source_state::trait_definition::AppliedTraitScope;
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use terminal_check_tables::contracts::{
    StaticRecordBinding, StaticRecordMember, StaticRecordRequirementInstance,
};

impl Program<'_> {
    pub(super) fn record_parameter(&self, parameter: SymbolHandle) -> Option<&SourceRecordBinding> {
        self.static_contracts.record_binding(parameter)
    }

    /// Classify and form the complete declared schema once, including unused members.
    /// A false result leaves the carrier to the ordinary scalar-value reader.
    pub(super) fn prepare_record_parameter(
        &mut self,
        id: MachineId,
        owner: SymbolHandle,
        parameter: &TypeParameter,
    ) -> Checked<bool> {
        let typed = self.typed;
        if let Some(known) = self.record_parameter(parameter.symbol) {
            if typed
                .declaration_type_parameters(known.account.owner)
                .iter()
                .find(|declared| declared.symbol == parameter.symbol)
                != Some(parameter)
                || !typed
                    .declaration_type_parameters(owner)
                    .iter()
                    .any(|declared| declared == parameter)
            {
                return Err(super::services::unavailable(
                    id,
                    "static record profile changed its original binder declaration",
                ));
            }
            // Schema formation is cached; lexical visibility remains caller-specific.
            let mut scope = owner;
            while scope != known.account.owner {
                if !scope.is_valid() || !typed.symbols.symbols().nodes().is_valid(scope) {
                    return Err(super::services::unavailable(
                        id,
                        "static record profile is outside its actual lexical context",
                    ));
                }
                scope = typed.symbols.get(scope).parent;
            }
            return Ok(true);
        }
        let Some(root) = typed
            .static_record_parameter_schema(owner, parameter.symbol)
            .map_err(|reason| super::services::unavailable(id, reason))?
        else {
            return Ok(false);
        };
        if parameter.bounds != crate::source_state::data::DataProperties::default() {
            return Err(super::services::unavailable(
                id,
                "static record parameter needs its complete declared storage and multiplicity obligations",
            ));
        }
        self.read_trait_schema(id, root.declaration)?;
        let identity = |scope: &AppliedTraitScope<'_>| {
            scope
                .portable_application_identity(typed)
                .and_then(|identity| typed.type_identity_text(identity))
                .filter(|identity| !identity.is_empty())
                .ok_or("static record schema lost its complete application identity")
        };
        let application =
            identity(&root).map_err(|reason| super::services::unavailable(id, reason))?;
        let mut members = Vec::<StaticRecordMember>::new();
        let mut signatures = Vec::<StateSignature>::new();
        root.try_visit(typed, &mut |scope: &AppliedTraitScope<'_>| -> Result<(), &'static str> {
            // This representation owns callable requirement fields. An ordinary or erased
            // value/evidence field needs its own complete retained account, never omission.
            if typed.symbols.child_handles(scope.declaration.symbol).into_iter().flatten()
                .any(|field| typed.symbols.get(field).kind == symbols::SymbolKind::Field) {
                return Err("static record schema has a field without its complete value or evidence account");
            }
            let application = identity(scope)?;
            for original in typed.trait_machine_signatures(scope.declaration) {
                if let Some(known) = members.iter().find(|member| member.requirement == original.symbol) {
                    if known.declaring_trait != scope.declaration.symbol || known.application != application {
                        return Err("static record inherits competing requirement applications");
                    }
                    continue;
                }
                let signature = typed.applied_callable_signature(scope, original.symbol)
                    .or_else(|| scope.keeps_declared_signatures(typed).then_some(original))
                    .ok_or("static record has no complete prepared member signature")?;
                members.push(StaticRecordMember {
                    declaring_trait: scope.declaration.symbol, requirement: original.symbol,
                    signature: signature.symbol,
                    application: application.clone(), instances: Vec::new(),
                });
                signatures.push(signature.clone());
            }
            Ok(())
        }).map_err(|reason| super::services::unavailable(id, reason))?;
        self.static_contracts
            .insert_record_binding(
                root.declaration.symbol,
                SourceRecordBinding {
                    account: StaticRecordBinding {
                        parameter: parameter.symbol,
                        owner: typed.symbols.get(parameter.symbol).parent,
                        application,
                        members,
                    },
                    signatures,
                },
            )
            .map_err(|reason| super::services::unavailable(id, reason))?;
        Ok(true)
    }

    pub(super) fn record_requirement_signature(
        &self,
        id: MachineId,
        receiver: SymbolHandle,
        target: SymbolHandle,
    ) -> Checked<Option<(usize, StateSignature)>> {
        let Some(binding) = self.record_parameter(receiver) else {
            return Ok(None);
        };
        let origin = self
            .typed
            .closed_requirement_application(target)
            .map_or(target, |closed| closed.template);
        let position = binding
            .account
            .members
            .iter()
            .position(|member| member.requirement == origin)
            .ok_or_else(|| {
                super::services::unavailable(
                    id,
                    "const record call is outside its exact requirement roster",
                )
            })?;
        if !self
            .typed
            .declaration_type_parameters(*self.symbols.get(id).ok_or_else(|| {
                super::services::unavailable(id, "const record caller has no source declaration")
            })?)
            .iter()
            .any(|parameter| parameter.symbol == receiver)
        {
            return Err(super::services::unavailable(
                id,
                "const record receiver is outside the caller's retained telescope",
            ));
        }
        let signature = binding.signatures.get(position).ok_or_else(|| {
            super::services::unavailable(id, "const record requirement lost its prepared signature")
        })?;
        if signature.symbol != target {
            // A receiver can name the original requirement or its prepared
            // application. The receiver's declared schema must select this
            // member at the same trait arguments in either spelling.
            let exact = self
                .typed
                .static_record_bound_requirement(binding.account.owner, receiver, target)
                .map_err(|reason| super::services::unavailable(id, reason))?;
            let same_member = exact.is_some_and(|required| {
                required.signature.symbol == signature.symbol
                    || self
                        .typed
                        .closed_requirement_application(target)
                        .is_some_and(|closed| {
                            closed.template == origin
                                && closed.local_arguments.is_empty()
                                && self.typed.requirement_application_matches_scope(
                                    target,
                                    &required.application,
                                )
                        })
            });
            if !same_member {
                return Err(super::services::unavailable(
                    id,
                    "const record call has an unrepresented local requirement application",
                ));
            }
        }
        Ok(Some((position, signature.clone())))
    }

    pub(super) fn record_requirement(
        &mut self,
        receiver: SymbolHandle,
        position: usize,
        signature: &StateSignature,
        arguments: &[semantic_vocabulary::ScalarTerm],
    ) -> Checked<MachineId> {
        if let Some(instance) = self
            .record_parameter(receiver)
            .and_then(|binding| binding.account.members.get(position))
            .and_then(|member| {
                member
                    .instances
                    .iter()
                    .find(|instance| instance.arguments == arguments)
            })
        {
            return Ok(instance.machine);
        }
        let id = self.next_identity()?;
        let outer = self.clause_supplier.replace(receiver);
        let lowered = self.declared_requirement_signature(id, signature, arguments);
        self.clause_supplier = outer;
        let (lowered, names) = lowered?;
        self.retain_requirement_policy(id, signature, signature.symbol)?;
        self.symbols.push(signature.symbol);
        self.signatures.push(lowered);
        self.names.push(names);
        self.value_parameters.push(0);
        self.value_arguments.push(arguments.to_vec());
        self.scopes.push(Vec::new());
        self.source_scopes.push(self.source_scope.clone());
        self.const_scopes.push(Vec::new());
        self.requirements.push(id);
        let member = self
            .static_contracts
            .record_binding_mut(receiver)
            .and_then(|binding| binding.account.members.get_mut(position))
            .ok_or_else(|| {
                super::services::unavailable(id, "const record requirement lost its owning binder")
            })?;
        member.instances.push(StaticRecordRequirementInstance {
            machine: id,
            arguments: arguments.to_vec(),
        });
        Ok(id)
    }

    /// Source checking and final publication use the same retained argument.
    pub(super) fn selected_record_operand(
        &self,
        id: MachineId,
        call: crate::source_state::references::StaticArgumentCall<'_>,
        selected: &crate::source_state::expression::StaticMachineArgument,
        selected_supplier: SymbolHandle,
    ) -> Checked<
        Option<(
            crate::source_state::expression::StaticMachineArgument,
            SymbolHandle,
            terminal_check_tables::program::SelectedStaticRecordOperand,
        )>,
    > {
        let owner = *self.symbols.get(id).ok_or_else(|| {
            super::services::unavailable(id, "selected record has no actual source caller")
        })?;
        self.typed
            .source_call_record_operand(
                owner,
                call,
                selected,
                selected_supplier,
                self.shared
                    .exact_toolchain_sources(self.typed)
                    .map_err(|reason| super::services::unavailable(id, reason))?,
            )
            .map(|record| {
                record.map(|record| {
                    (
                        record.operand,
                        record.supplier,
                        terminal_check_tables::program::SelectedStaticRecordOperand {
                            parameter: record.parameter,
                            application: record.application,
                        },
                    )
                })
            })
            .map_err(|reason| super::services::unavailable(id, &reason))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_state::TypedTrees;
    use crate::source_state::data::{TypeParameter, TypeParameterKind};
    use crate::source_state::expression::{
        ExpressionHandle, StaticMachineArgument, TableCallExpression,
    };
    use crate::source_state::machine::{Machine, MachineBodyStatus};
    use crate::source_state::name::Identifier;
    use crate::source_state::state::State;
    use crate::source_state::static_applications::{
        StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
    };
    use crate::source_state::trait_definition::{
        Conformance, ConformanceRow, ConformanceRowSource, ImplementationValueIdentity,
        TraitDefinition,
    };
    use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
    use crate::source_symbols::SymbolTableBuilder;
    use std::sync::Arc;
    use symbols::{SymbolKind, SymbolNameRef};

    struct Fixture {
        typed: TypedTrees,
        caller: SymbolHandle,
        nested: SymbolHandle,
        outsider: SymbolHandle,
        supplier: SymbolHandle,
        parameters: [SymbolHandle; 2],
        requirement: SymbolHandle,
        foreign: SymbolHandle,
        records: [SymbolHandle; 3],
        instance: SymbolHandle,
        calls: [StaticMachineApplicationHandle; 2],
    }
    fn machine(
        typed: &mut TypedTrees,
        symbol: SymbolHandle,
        entry: SymbolHandle,
        unit: TypeReferenceHandle,
    ) {
        let states = typed.machine_states.insert_many([State {
            symbol: entry,
            name: Identifier::generated("entry"),
            return_type: unit,
            ..Default::default()
        }]);
        typed.push_machine(Machine {
            symbol,
            name: Identifier::generated(typed.symbols.name(symbol)),
            states,
            body_is_present: true,
            body_status: MachineBodyStatus::Complete,
            ..Default::default()
        });
    }
    fn fixture() -> Fixture {
        fixture_with_reach(false)
    }

    fn fixture_with_reach(authored_reach: bool) -> Fixture {
        let mut sources = crate::source_inputs::SourceMap::default();
        let source = sources
            .add_with_metadata(
                "record-binders/main.omg".into(),
                "root".into(),
                "record-binders".into(),
                semantic_vocabulary::PackageKeyIdentity::from_digest([19; 32]),
                source_admission::SourceOrigin::User,
            )
            .source_id;
        let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = builder.insert_root(
            SymbolKind::Root,
            SymbolNameRef::Source(source_text::SourceSpan::new(
                source,
                source_text::Span::new(0, 4),
            )),
        );
        let top = builder.insert_children(
            root,
            [
                (SymbolKind::Trait, SymbolNameRef::Static("Ops")),
                (SymbolKind::Trait, SymbolNameRef::Static("Other")),
                (SymbolKind::Machine, SymbolNameRef::Static("caller")),
                (SymbolKind::Machine, SymbolNameRef::Static("outsider")),
                (SymbolKind::Machine, SymbolNameRef::Static("supplier")),
                (SymbolKind::Machine, SymbolNameRef::Static("left_body")),
                (SymbolKind::Machine, SymbolNameRef::Static("right_body")),
                (SymbolKind::Conformance, SymbolNameRef::Static("Left")),
                (SymbolKind::Conformance, SymbolNameRef::Static("Right")),
                (SymbolKind::Conformance, SymbolNameRef::Static("LeftAlias")),
            ],
        );
        let [
            schema,
            other,
            caller,
            outsider,
            supplier,
            left_body,
            right_body,
            left,
            right,
            alias,
        ] = std::array::from_fn(|i| top.handle_at(i as u32));
        let child = |builder: &mut SymbolTableBuilder, owner, kind, name| {
            builder
                .insert_children(owner, [(kind, SymbolNameRef::Static(name))])
                .handle_at(0)
        };
        let requirement = child(&mut builder, schema, SymbolKind::State, "apply");
        let foreign = child(&mut builder, other, SymbolKind::State, "apply");
        let children = builder.insert_children(
            caller,
            [
                (SymbolKind::TypeParameter, SymbolNameRef::Static("A")),
                (SymbolKind::TypeParameter, SymbolNameRef::Static("B")),
                (SymbolKind::State, SymbolNameRef::Static("entry")),
                (SymbolKind::Machine, SymbolNameRef::Static("nested")),
            ],
        );
        let parameters = [children.handle_at(0), children.handle_at(1)];
        let caller_state = children.handle_at(2);
        let nested = children.handle_at(3);
        let nested_state = child(&mut builder, nested, SymbolKind::State, "entry");
        let outside_state = child(&mut builder, outsider, SymbolKind::State, "entry");
        let supply_state = child(&mut builder, supplier, SymbolKind::State, "entry");
        let left_state = child(&mut builder, left_body, SymbolKind::State, "entry");
        let right_state = child(&mut builder, right_body, SymbolKind::State, "entry");
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let unit = typed.type_reference_table.insert(TypeReferenceNode::Unit);
        let reach = if authored_reach {
            let service = typed.service_reaches.intern(other, "Other");
            typed.authored_service_reach_rows.push(
                crate::source_state::signature::AuthoredServiceReachRow {
                    owner: requirement,
                    keyword_source_spans: Vec::new(),
                    targets: vec![crate::source_state::signature::AuthoredServiceReachTarget {
                        service: crate::source_state::signature::ReachSubject::Static(other),
                        source_span: source_text::SourceSpan::new(
                            source,
                            source_text::Span::new(0, 4),
                        ),
                    }],
                    installation_bound: false,
                },
            );
            typed.service_reach_rows.intern(vec![service])
        } else {
            typed.service_reach_rows.intern(Vec::new())
        };
        for (symbol, req) in [(schema, requirement), (other, foreign)] {
            let machines = typed.trait_machine_signatures.insert_many([StateSignature {
                symbol: req,
                name: Identifier::generated("apply"),
                return_type: unit,
                service_reach_row: reach,
                suspends: authored_reach && symbol == schema,
                blocks: authored_reach && symbol == schema,
                declares_no_crash_route: true,
                ..Default::default()
            }]);
            typed.push_trait_definition(TraitDefinition {
                symbol,
                name: Identifier::generated(typed.symbols.name(symbol)),
                is_boundary: authored_reach && symbol == other,
                machines,
                ..Default::default()
            });
        }
        for (symbol, entry) in [
            (caller, caller_state),
            (nested, nested_state),
            (outsider, outside_state),
            (supplier, supply_state),
            (left_body, left_state),
            (right_body, right_state),
        ] {
            machine(&mut typed, symbol, entry, unit);
        }
        let reference = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: schema,
            name: Identifier::generated("Ops"),
        });
        let binder_rows = parameters
            .iter()
            .map(|symbol| TypeParameter {
                symbol: *symbol,
                name: Identifier::generated(typed.symbols.name(*symbol)),
                kind: TypeParameterKind::Const {
                    type_reference: reference,
                },
                ..Default::default()
            })
            .collect::<Vec<_>>();
        for owner in [caller, nested] {
            let rows = typed
                .data_type_parameters
                .insert_many(binder_rows.iter().cloned());
            let position = typed
                .machines()
                .iter()
                .position(|machine| machine.symbol == owner)
                .unwrap();
            let mut declaration = typed.machines().at(position).clone();
            declaration.type_parameters = rows;
            typed.set_machine(position, declaration);
        }
        for (symbol, body, entry, representative) in [
            (left, left_body, left_state, SymbolHandle::invalid()),
            (right, right_body, right_state, SymbolHandle::invalid()),
            (alias, left_body, left_state, left),
        ] {
            typed.push_conformance(Conformance {
                symbol,
                trait_symbol: schema,
                trait_name: Identifier::generated("Ops"),
                rows: vec![ConformanceRow {
                    declaring_trait: schema,
                    declaring_trait_name: Identifier::generated("Ops"),
                    requirement,
                    requirement_name: Identifier::generated("apply"),
                    realization_machine: body,
                    realization_state: entry,
                    realization_name: Identifier::generated(typed.symbols.name(body)),
                    source: ConformanceRowSource::Reference,
                }],
                value_identity: ImplementationValueIdentity {
                    representative,
                    ..Default::default()
                },
                ..Default::default()
            });
        }
        let instance = typed.symbols.insert_generated_root_from(
            caller,
            SymbolKind::Machine,
            "caller<Left,Right>",
        );
        let entry = typed
            .symbols
            .insert_generated_children(instance, [(SymbolKind::State, "entry")])
            .handle_at(0);
        machine(&mut typed, instance, entry, unit);
        let actuals = [left, right].map(|symbol| StaticMachineArgument {
            symbol,
            path: vec![Identifier::generated(typed.symbols.name(symbol))].into_boxed_slice(),
            ..Default::default()
        });
        let application = typed.expression_table.retain_static_machine_application(
            caller,
            Identifier::generated("caller"),
            &actuals,
        );
        let references = actuals
            .iter()
            .map(|actual| typed.static_operand_type_reference(actual).unwrap())
            .collect::<Vec<_>>();
        typed
            .expression_table
            .set_static_machine_application_bindings(
                application,
                &parameters
                    .iter()
                    .zip(references)
                    .map(|(parameter, reference)| StaticMachineBinding {
                        parameter: *parameter,
                        value: StaticMachineBindingValue::Const(reference),
                    })
                    .collect::<Vec<_>>(),
            );
        let retained = typed
            .expression_table
            .static_machine_application_mut(application);
        retained.lexical_owner = supplier;
        retained.binding_declaration = caller;
        let states = typed.captured_machine_states.insert_many([
            crate::source_state::CapturedMachineState {
                template: caller_state,
                instance: entry,
            },
        ]);
        let template_parameters = typed.machine_by_symbol(caller).unwrap().type_parameters;
        typed
            .machine_specializations
            .push(crate::source_state::MachineSpecialization {
                template: caller,
                template_parameters,
                binding_application: application,
                instance,
                states,
                ..Default::default()
            });
        let calls = parameters.map(|parameter| {
            let application = typed.expression_table.retain_static_machine_application(
                requirement,
                Identifier::generated("apply"),
                &[],
            );
            let source = typed
                .expression_table
                .static_machine_application_mut(application);
            source.lexical_owner = instance;
            source.binding_declaration = requirement;
            source.source_record_receiver = Some(parameter);
            application
        });
        Fixture {
            typed,
            caller,
            nested,
            outsider,
            supplier,
            parameters,
            requirement,
            foreign,
            records: [left, right, alias],
            instance,
            calls,
        }
    }
    fn call(
        application: StaticMachineApplicationHandle,
        target: SymbolHandle,
    ) -> TableCallExpression {
        TableCallExpression {
            runtime_call: None,
            authored_call_selection: None,
            authored_call_binding: None,
            build_selection: Default::default(),
            authored_machine_application: application,
            exact_coercion_target: Default::default(),
            requirement_application: Default::default(),
            receiver: Default::default(),
            target_symbol: target,
            static_machine_parameter: Default::default(),
            target: Identifier::generated("apply"),
            static_requirement_dispatch: None,
            machine_arguments: Box::new([]),
            quotient_operation: None,
            private_layout_operation: None,
            installed_interrupt_operation: None,
            layout_query: None,
            native_code_layout: None,
            arguments: Default::default(),
            evidence_arguments: Box::new([]),
            operational_acknowledgement: Default::default(),
        }
    }

    #[test]
    fn two_static_record_binders_keep_distinct_requirement_instances_and_full_schema_custody() {
        let fixture = fixture();
        for parameter in fixture.parameters {
            assert!(
                fixture
                    .typed
                    .static_record_bound_requirement(fixture.nested, parameter, fixture.requirement)
                    .unwrap()
                    .is_some()
            );
            assert!(
                fixture
                    .typed
                    .static_record_bound_requirement(fixture.caller, parameter, fixture.foreign)
                    .is_err()
            );
            assert!(
                fixture
                    .typed
                    .static_record_bound_requirement(
                        fixture.outsider,
                        parameter,
                        fixture.requirement
                    )
                    .is_err()
            );
        }
        let mut program = Program::new(&fixture.typed, true);
        let caller = program
            .machine(fixture.caller)
            .expect("form the complete static schema binder profiles");
        assert_eq!(
            program.value_parameters.get(caller),
            Some(&0),
            "static records are not fake scalar formals"
        );
        let mut instances = Vec::new();
        for parameter in fixture.parameters {
            let (position, signature) = program
                .record_requirement_signature(caller, parameter, fixture.requirement)
                .unwrap()
                .unwrap();
            let first = program
                .record_requirement(parameter, position, &signature, &[])
                .unwrap();
            assert_eq!(
                program
                    .record_requirement(parameter, position, &signature, &[])
                    .unwrap(),
                first
            );
            instances.push(first);
            let profile = program.record_parameter(parameter).unwrap();
            assert_eq!(profile.account.members.len(), 1);
            assert_eq!(profile.account.owner, fixture.caller);
        }
        assert_ne!(
            instances[0], instances[1],
            "the originating requirement does not identify its supplier binder"
        );
    }

    #[test]
    fn inherited_const_tuples_preserve_original_binder_positions_before_preparation() {
        let fixture = fixture();
        let captured = fixture.parameters.map(|symbol| StaticMachineArgument {
            symbol,
            path: Box::from([Identifier::generated(fixture.typed.symbols.name(symbol))]),
            ..Default::default()
        });
        for (arguments, valid) in [
            (captured.to_vec(), true),
            (vec![captured[1].clone(), captured[0].clone()], false),
            (vec![captured[0].clone()], false),
            (Vec::new(), false),
        ] {
            let mut typed = fixture.typed.clone();
            let application = typed.expression_table.retain_static_machine_application(
                fixture.nested,
                Identifier::generated("nested"),
                &arguments,
            );
            let source = typed
                .expression_table
                .static_machine_application_mut(application);
            source.lexical_owner = fixture.caller;
            source.binding_declaration = fixture.nested;
            assert!(
                typed
                    .expression_table
                    .static_machine_application_bindings(application)
                    .is_empty()
            );
            let mut program = Program::new(&typed, true);
            let caller = program.machine(fixture.caller).unwrap();
            let values = program.application_arguments(
                caller,
                fixture.caller,
                fixture.nested,
                application,
                &arguments,
            );
            if valid {
                let values = values.unwrap();
                assert!(
                    values.values.is_empty(),
                    "record captures are not scalar actuals"
                );
                assert_eq!(values.lexical_owner, fixture.caller);
                let outsider = program.machine(fixture.outsider).unwrap();
                let inherited = program
                    .application_arguments(
                        outsider,
                        fixture.caller,
                        fixture.nested,
                        application,
                        &arguments,
                    )
                    .unwrap();
                assert_eq!(inherited.lexical_owner, fixture.caller);
                let refused = program
                    .application_arguments(
                        outsider,
                        fixture.outsider,
                        fixture.nested,
                        application,
                        &arguments,
                    )
                    .unwrap_err();
                assert_eq!(
                    refused.detail,
                    "generic application has another source owner or declaration"
                );
            } else {
                assert_eq!(
                    values.unwrap_err().detail,
                    "generic application has no complete exact binder tuple"
                );
            }
        }
        let mut typed = fixture.typed.clone();
        let application = typed.expression_table.retain_static_machine_application(
            fixture.outsider,
            Identifier::generated("outsider"),
            &captured,
        );
        let source = typed
            .expression_table
            .static_machine_application_mut(application);
        source.lexical_owner = fixture.caller;
        source.binding_declaration = fixture.nested;
        let mut program = Program::new(&typed, true);
        let caller = program.machine(fixture.caller).unwrap();
        let refused = program
            .application_arguments(
                caller,
                fixture.caller,
                fixture.nested,
                application,
                &captured,
            )
            .unwrap_err();
        assert_eq!(
            refused.detail,
            "generic application has another source owner or declaration"
        );
    }

    #[test]
    fn record_projections_never_alias_declared_headers_in_either_allocation_order() {
        for record_first in [true, false] {
            let fixture = fixture_with_reach(true);
            let mut program = Program::new(&fixture.typed, true);
            let caller = program.machine(fixture.caller).unwrap();
            let (position, signature) = program
                .record_requirement_signature(caller, fixture.parameters[0], fixture.requirement)
                .unwrap()
                .unwrap();
            let source_scope = program.source_scope.clone();
            let (ordinary, record) = if record_first {
                let record = program
                    .record_requirement(fixture.parameters[0], position, &signature, &[])
                    .unwrap();
                let ordinary = program.requirement(&signature, &[]).unwrap();
                (ordinary, record)
            } else {
                let ordinary = program.requirement(&signature, &[]).unwrap();
                let record = program
                    .record_requirement(fixture.parameters[0], position, &signature, &[])
                    .unwrap();
                (ordinary, record)
            };
            assert_ne!(
                ordinary, record,
                "a declared header cannot acquire a receiver-bound projection identity"
            );
            assert_eq!(program.requirement(&signature, &[]).unwrap(), ordinary);
            assert_eq!(
                program
                    .record_requirement(fixture.parameters[0], position, &signature, &[])
                    .unwrap(),
                record
            );
            assert_eq!(
                program.source_scopes.len(),
                program.symbols.len(),
                "every allocated machine retains its own source scope row"
            );
            assert_eq!(program.source_scopes.get(record), Some(&source_scope));
            assert_eq!(program.source_scopes.get(ordinary), Some(&source_scope));
            assert_eq!(
                program.symbols.get(record),
                Some(&signature.symbol),
                "a record member retains its exact source requirement"
            );
            let second = program
                .record_requirement(fixture.parameters[1], position, &signature, &[])
                .unwrap();
            assert_ne!(
                record, second,
                "equal member signatures do not identify two supplier binders"
            );
            assert_eq!(
                program.requirement(&signature, &[]).unwrap(),
                ordinary,
                "later record allocation cannot redirect the declared header"
            );
            assert_eq!(program.source_scopes.len(), program.symbols.len());
        }
    }

    #[test]
    fn callable_witnesses_and_record_projections_keep_distinct_public_headers() {
        let fixture = fixture_with_reach(true);
        let mut program = Program::new(&fixture.typed, true);
        let caller = program.machine(fixture.caller).unwrap();
        let (position, signature) = program
            .record_requirement_signature(caller, fixture.parameters[0], fixture.requirement)
            .unwrap()
            .unwrap();
        let (_, declared) = fixture
            .typed
            .trait_machine_signature(fixture.requirement)
            .unwrap();
        let callable = program
            .callable_contract(crate::lowering::CallableDeclaration::Signature(declared))
            .unwrap();
        let record = program
            .record_requirement(fixture.parameters[0], position, &signature, &[])
            .unwrap();
        let public = program.requirement(&signature, &[]).unwrap();
        assert_ne!(
            public, callable,
            "a public header cannot reuse an independently owned callable witness"
        );
        assert_ne!(
            public, record,
            "a public header cannot reuse a receiver-bound record member"
        );
        assert_ne!(callable, record);
        assert_eq!(program.requirement(&signature, &[]).unwrap(), public);
        assert_eq!(program.symbols.get(callable), Some(&signature.symbol));
        assert_eq!(program.symbols.get(record), Some(&signature.symbol));
        assert_eq!(program.symbols.get(public), Some(&signature.symbol));
        assert_eq!(program.source_scopes.len(), program.symbols.len());
        for target in [callable, record, public] {
            assert_eq!(
                program.source_scopes.get(target),
                Some(&program.source_scope)
            );
        }
    }

    #[test]
    fn finite_requirement_policies_complete_generic_helpers_without_closing_omitted_reach() {
        use crate::lowering::body::{Body, Expression, ExpressionKind, Statement};
        use arena::IdTable;

        for authored_reach in [true, false] {
            let fixture = fixture_with_reach(authored_reach);
            let mut program = Program::new(&fixture.typed, true);
            let caller = program.machine(fixture.caller).unwrap();
            let helper = program
                .application(
                    caller,
                    fixture.nested,
                    super::super::machine::ApplicationTypes::Machine(Vec::new(), Vec::new()),
                )
                .unwrap();
            let (position, signature) = program
                .record_requirement_signature(caller, fixture.parameters[0], fixture.requirement)
                .unwrap()
                .unwrap();
            let ordinary = program.requirement(&signature, &[]).unwrap();
            // Ordinary and record-bound logical requirements use the same
            // declared policy and preserve distinct actual instance identities.
            let record = program
                .record_requirement(fixture.parameters[0], position, &signature, &[])
                .unwrap();
            assert_ne!(ordinary, record);
            let source = crate::lowering::SourceOrigin::Unpositioned(
                crate::lowering::Unpositioned::SpanAbsent,
            );
            let call = |callee: MachineId| {
                Statement::discard(Expression {
                    call_context: None,
                    kind: ExpressionKind::Call {
                        callee: callee.into(),
                        arguments: Vec::new(),
                        designated: None,
                        required_contract: None,
                        static_reach_binding: None,
                        static_reach_arguments: Vec::new(),
                    },
                    result: None,
                    source,
                })
            };
            let mut bodies: IdTable<MachineId, Body> = program.symbols.map(|_, _| Body {
                slots: Vec::new(),
                statements: Vec::new(),
            });
            bodies.get_mut(caller).unwrap().statements = vec![call(helper)];
            bodies.get_mut(helper).unwrap().statements = vec![call(ordinary), call(record)];
            let components = crate::lowering::components::discover(&bodies).unwrap();
            program
                .finish_operational_envelopes(&bodies, &components)
                .unwrap();
            if !authored_reach {
                assert!(matches!(
                    program.signatures.get(record).unwrap().reaches,
                    crate::lowering::Ceiling::Inferred
                ));
                assert!(!matches!(
                    program.callables.envelopes.get(record),
                    Some(Some(_))
                ));
                assert!(!matches!(
                    program.callables.envelopes.get(helper),
                    Some(Some(_))
                ));
                continue;
            }
            let expected = super::super::services::finite_reach(
                &fixture.typed,
                record,
                signature.service_reach_row,
            )
            .unwrap();
            assert!(!expected.is_empty());
            for machine in [ordinary, record, helper, caller] {
                let envelope = program
                    .callables
                    .envelopes
                    .get(machine)
                    .unwrap()
                    .as_ref()
                    .unwrap();
                assert_eq!(envelope.service_reach, expected);
                assert!(envelope.suspends && envelope.blocks);
                assert!(envelope.invocations.is_empty());
                assert_eq!(
                    envelope.termination,
                    terminal_psi::TerminalCallableTermination::NotGuaranteed
                );
            }
            let prepared =
                crate::lowering::finish_reading(program, Some(caller), bodies, None, None, None)
                    .unwrap();
            let entry = prepared
                .program
                .module
                .machines
                .iter()
                .find(|machine| machine.id == caller)
                .unwrap();
            assert!(
                entry.declaration.is_some(),
                "finite generic entry lost its declaration: {:?}",
                prepared.program.undeclared
            );
            assert!(
                prepared
                    .program
                    .module
                    .execution.callables
                    .machine_envelope(caller)
                    .is_some()
            );
            assert!(!entry.published_service_ceiling.is_empty());
            assert!(
                !prepared.program.module.execution.declaration_contracts.is_empty(),
                "the requirement must follow a nonempty declaration-contract prefix"
            );
            for requirement in [ordinary, record] {
                let machine = prepared
                    .program
                    .module
                    .machines
                    .iter()
                    .find(|machine| machine.id == requirement)
                    .unwrap();
                assert!(machine.declaration.is_none());
                assert_eq!(
                    machine.declared_service_reach.as_ref(),
                    Some(&entry.published_service_ceiling)
                );
                assert!(
                    prepared
                        .program
                        .module
                        .execution.callables
                        .machine_envelope(requirement)
                        .is_none(),
                    "retaining a source bound does not publish an executable requirement"
                );
                assert_eq!(
                    machine.published_service_ceiling,
                    entry.published_service_ceiling
                );
                assert_eq!(
                    prepared
                        .program
                        .requirement_ceilings
                        .get(requirement)
                        .unwrap()
                        .reaches,
                    crate::lowering::Ceiling::Authored(entry.published_service_ceiling.clone())
                );
            }
        }
    }

    #[test]
    fn selected_record_receivers_rejoin_actual_operands_allow_equal_aliases_and_refuse_crossed_suppliers()
     {
        let mut fixture = fixture();
        let left = StaticMachineArgument {
            symbol: fixture.records[0],
            ..Default::default()
        };
        let right = StaticMachineArgument {
            symbol: fixture.records[1],
            ..Default::default()
        };
        let alias = StaticMachineArgument {
            symbol: fixture.records[2],
            ..Default::default()
        };
        // Rejoin the actual retained instance, without lowering a selected body.
        {
            let mut program = Program::new(&fixture.typed, false);
            let caller = program.machine(fixture.instance).unwrap();
            let a = call(fixture.calls[0], fixture.requirement);
            let b = call(fixture.calls[1], fixture.requirement);
            let read = |call: &TableCallExpression, selected: &StaticMachineArgument| {
                program.selected_record_operand(
                    caller,
                    crate::source_state::references::StaticArgumentCall::Expression(
                        ExpressionHandle::invalid(),
                        call,
                    ),
                    selected,
                    fixture.instance,
                )
            };
            let (_, supplier, first) = read(&a, &left).unwrap().unwrap();
            let (_, other_supplier, second) = read(&b, &right).unwrap().unwrap();
            assert_eq!(supplier, fixture.supplier);
            assert_eq!(other_supplier, fixture.supplier);
            assert_ne!(first.parameter, second.parameter);
            assert_eq!(first.application, second.application);
            assert!(
                read(&a, &right).is_err(),
                "same schema is not the exact selected value"
            );
            assert!(
                read(&b, &left).is_err(),
                "one binder's row cannot supply another"
            );
            let (actual, _, binding) = read(&a, &alias).unwrap().unwrap();
            assert_eq!(
                actual.symbol, fixture.records[0],
                "an equal alias retains the actual supplying operand"
            );
            assert_eq!(binding, first);
        }
        fixture
            .typed
            .expression_table
            .static_machine_application_mut(fixture.calls[0])
            .registration_operation = fixture.foreign;
        assert!(
            fixture
                .typed
                .static_record_source_receiver(fixture.calls[0])
                .is_err(),
            "a forged requirement cannot change receiver scope"
        );
        let stale = SymbolHandle::from_parts(
            fixture.parameters[0].arena_index(),
            fixture.parameters[0].generation() + 1,
        );
        let source = fixture
            .typed
            .expression_table
            .static_machine_application_mut(fixture.calls[0]);
        source.registration_operation = fixture.requirement;
        source.source_record_receiver = Some(stale);
        assert!(
            fixture
                .typed
                .static_record_source_receiver(fixture.calls[0])
                .is_err()
        );
        let other = fixture.typed.symbols.get(fixture.foreign).parent;
        let reference = fixture
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: other,
                name: Identifier::generated("Other"),
            });
        let parameters = fixture
            .typed
            .machine_by_symbol(fixture.caller)
            .unwrap()
            .type_parameters;
        fixture
            .typed
            .data_type_parameters
            .get_mut(parameters.handle_at(1))
            .kind = TypeParameterKind::Const {
            type_reference: reference,
        };
        assert!(
            fixture
                .typed
                .static_record_source_receiver(fixture.calls[1])
                .is_err(),
            "the actual Ops record cannot supply an Other binder"
        );
    }
}
