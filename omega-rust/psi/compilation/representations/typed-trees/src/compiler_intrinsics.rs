//! Compiler supply belongs to an exact bundled declaration and its entire
//! signature. Derive that signature from the ordinary machine owner: a token
//! view or a matching operand count cannot stand in for its contract.

use crate::TypedTrees;
use crate::data::TypeParameterKind;
use crate::domain::ProofFact;
use crate::machine::Machine;
use crate::signature::{SignatureContract, SignatureContractKind};
use crate::type_identity::constraint_identity::{atom, compound};
use crate::type_identity::identity_context::{
    TypeIdentityContext, TypeIdentityQualification, normalize_index_expression,
    normalize_type_reference,
};
use crate::type_identity::{LifetimeParameterIdentity, NormalizedTypeIdentity};
use diagnostics::Diagnostic;
use language_core::sealed_intrinsics::{CompilerPrimitive, SealedIntrinsic};
use language_semantics::type_identity::vocabulary::Spelled;
use std::cell::Cell;
use symbols::SymbolHandle;

impl TypedTrees {
    /// Source metadata only. This does not prove a requirement, select a
    /// provider, or authorize execution of an application. The caller must
    /// instantiate this declaration and check its whole contract at the use.
    pub fn compiler_primitive_schema(
        &self,
        symbol: SymbolHandle,
    ) -> Result<Option<(CompilerPrimitive, NormalizedTypeIdentity)>, Diagnostic> {
        let Some(machine) = self.machine_by_symbol(symbol) else {
            return Ok(None);
        };
        let custody = self
            .symbols
            .sealed_declaration(symbol, machine.name.as_str());
        let Some(SealedIntrinsic::CompilerPrimitive(operation)) = custody else {
            return if machine.supply_mode == language_semantics::MachineSupplyMode::CompilerProvided
            {
                Err(Diagnostic::error(
                    "compiler primitive has no exact sealed declaration custody",
                ))
            } else {
                Ok(None)
            };
        };
        if machine.supply_mode != language_semantics::MachineSupplyMode::CompilerProvided
            || machine.body_is_present
        {
            return Err(Diagnostic::error(
                "sealed compiler primitive has a different supply mode or an authored body",
            ));
        }
        normalized_schema(self, machine)
            .map(|schema| Some((operation, schema)))
            .ok_or_else(|| {
                Diagnostic::error(
                    "no product route yet: complete compiler primitive signature schema",
                )
            })
    }
}

// Schema coordinates use the existing canonical identity interner. They are
// derived descriptions of the one Machine, never another declaration carrier.
#[derive(Clone, Copy)]
enum Tag {
    Schema,
    Binders,
    Binder,
    Kind,
    Bounds,
    Lifetimes,
    Parameters,
    Parameter,
    Mode,
    Type,
    Result,
    Contracts,
    Contract,
    Requires,
    Ensures,
    Crash,
    Envelope,
}

impl Spelled for Tag {
    fn spelling(self) -> &'static str {
        match self {
            Self::Schema => "machine-schema",
            Self::Binders => "binders",
            Self::Binder => "binder",
            Self::Kind => "kind",
            Self::Bounds => "bounds",
            Self::Lifetimes => "lifetimes",
            Self::Parameters => "parameters",
            Self::Parameter => "parameter",
            Self::Mode => "mode",
            Self::Type => "type",
            Self::Result => "result",
            Self::Contracts => "contracts",
            Self::Contract => "contract",
            Self::Requires => "requires",
            Self::Ensures => "ensures",
            Self::Crash => "crash",
            Self::Envelope => "envelope",
        }
    }
}

// Complete signatures must not silently lose a malformed retained roster.
// Arena::span validates nonempty ownership; only the canonical empty handle
// represents a genuinely absent roster here.
fn schema_span<T: Default>(arena: &arena::Arena<T>, span: arena::HandleSpan<T>) -> Option<&[T]> {
    if span.is_empty() && span != arena::HandleSpan::empty() {
        return None;
    }
    arena.span(span)
}

fn normalized_schema(program: &TypedTrees, machine: &Machine) -> Option<NormalizedTypeIdentity> {
    // Exhaust the header so a new Machine field requires a deliberate schema
    // decision. Source coordinates and body storage do not change a signature.
    let Machine {
        lexical_region_origin,
        assembly_target_requirement,
        symbol,
        name: _,
        attached_data,
        attached_data_symbol,
        attached_data_application,
        generic_data_template,
        spelling,
        is_public,
        interface_exposure,
        supply_mode: _,
        purpose,
        target,
        authored_target,
        target_family_interface,
        body_is_present: _,
        body_status: _,
        structural_type_equations_pending,
        termination_plan,
        service_reach_row,
        service_reach_is_installation_bound,
        admitted_same_stack_bytes,
        declares_no_crash_route,
        suspends_keyword_source_spans,
        blocks_keyword_source_spans,
        lifetime_parameters,
        type_parameters: _,
        declared_type_equations,
        owned_data,
        satisfies,
        conformance_bounds,
        invokes,
        suspends,
        blocks,
        contracts: _,
        states: _,
    } = machine;
    if lexical_region_origin.is_some()
        || attached_data.is_some()
        || attached_data_symbol.is_valid()
        || attached_data_application.is_valid()
        || generic_data_template.is_valid()
        || target.is_some()
        || authored_target.is_some()
        || *target_family_interface
        || *structural_type_equations_pending
        || *declared_type_equations != arena::HandleSpan::empty()
        || *owned_data != arena::HandleSpan::empty()
        || *satisfies != arena::HandleSpan::empty()
        || !conformance_bounds.is_empty()
        || *invokes != arena::HandleSpan::empty()
        || *service_reach_is_installation_bound
        || !symbol.is_valid()
        || termination_plan.implementation_witness.is_some()
        || termination_plan.checked_summary != language_semantics::TerminationGuarantee::NoGuarantee
    {
        return None;
    }
    let reach = if *service_reach_row == language_semantics::ServiceReachRowId::NULL {
        "inferred"
    } else if *service_reach_row == language_semantics::ServiceReachRowTable::EMPTY_ROW {
        "none"
    } else {
        // Nonempty/abstract service rows need their complete declaration-owned
        // normalization before this query can describe them.
        return None;
    };
    let termination = match &termination_plan.interface {
        language_semantics::TerminationInterface::InternalDerived => "inferred",
        language_semantics::TerminationInterface::Published(
            language_semantics::TerminationGuarantee::NoGuarantee,
        ) => "no-guarantee",
        _ => return None,
    };
    let [entry] = schema_span(&program.machine_states, machine.states)? else {
        return None;
    };
    if !entry.symbol.is_valid() {
        return None;
    }
    let types = schema_span(&program.data_type_parameters, machine.type_parameters)?;
    let parameters = schema_span(&program.state_parameters, entry.parameters)?;
    let binders = types
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| (parameter.symbol, format!("$static{ordinal}")))
        .chain(
            parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| (parameter.symbol, format!("$value{ordinal}"))),
        )
        .collect::<Vec<_>>();
    if binders.iter().any(|(symbol, _)| !symbol.is_valid()) {
        return None;
    }
    let lifetimes = lifetime_parameters
        .iter()
        .enumerate()
        .map(|(ordinal, _)| {
            (
                LifetimeParameterIdentity {
                    declaration: *symbol,
                    ordinal,
                },
                ordinal,
            )
        })
        .collect::<Vec<_>>();
    let invalid = Cell::new(false);
    let context = TypeIdentityContext {
        binders: &binders,
        lifetime_binders: &lifetimes,
        application_owner: *symbol,
        application_argument_owner: *symbol,
        signature_owners: Some((*symbol, entry.symbol)),
        qualification: TypeIdentityQualification::PackageQualified,
        invalid_application: Some(&invalid),
        invalid_index_argument: Some(&invalid),
        encountered_elided_regions: Some(&invalid),
        ..Default::default()
    };
    let type_identity = |reference| {
        program
            .contains_type_reference(reference)
            .then(|| normalize_type_reference(program, reference, &context))
    };
    let mut telescope = Vec::with_capacity(types.len());
    for parameter in types {
        let kind = match parameter.kind {
            TypeParameterKind::Type => atom(Tag::Kind, "type"),
            TypeParameterKind::Const { type_reference } => compound(
                Tag::Kind,
                [atom(Tag::Kind, "const"), type_identity(type_reference)?],
            ),
            // Refuse actual unserved binders; none is silently a type binder.
            _ => return None,
        };
        telescope.push(compound(
            Tag::Binder,
            [kind, atom(Tag::Bounds, &format!("{:?}", parameter.bounds))],
        ));
    }
    let parameters = parameters
        .iter()
        .map(|parameter| {
            Some(compound(
                Tag::Parameter,
                [
                    atom(
                        Tag::Mode,
                        &format!(
                            "const={};mutable={};relevance={:?}",
                            parameter.is_const, parameter.is_mutable, parameter.relevance
                        ),
                    ),
                    compound(Tag::Type, [type_identity(parameter.type_reference)?]),
                ],
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let contracts = contracts(
        program,
        schema_span(&program.signature_contracts, machine.contracts)?
            .iter()
            .chain(schema_span(&program.signature_contracts, entry.contracts)?),
        &context,
    )?;
    let identity = compound(
        Tag::Schema,
        [
            compound(Tag::Binders, telescope),
            atom(Tag::Lifetimes, &lifetimes.len().to_string()),
            compound(Tag::Parameters, parameters),
            compound(Tag::Result, [type_identity(entry.return_type)?]),
            compound(Tag::Contracts, contracts),
            atom(
                Tag::Envelope,
                &format!(
                    "public={is_public};exposure={interface_exposure:?};purpose={purpose:?};token={spelling:?};assembly={assembly_target_requirement:?};reach={reach};suspends={suspends};suspends-written={};blocks={blocks};blocks-written={};crashes-none={declares_no_crash_route};termination={termination};stack={admitted_same_stack_bytes}",
                    !suspends_keyword_source_spans.is_empty(),
                    !blocks_keyword_source_spans.is_empty()
                ),
            ),
        ],
    );
    (!invalid.get()).then(|| program.type_identities.intern(&identity))
}

fn contracts<'a>(
    program: &TypedTrees,
    contracts: impl Iterator<Item = &'a SignatureContract>,
    context: &TypeIdentityContext<'_>,
) -> Option<Vec<String>> {
    contracts
        .map(|contract| {
            if contract.binding.is_some() {
                return None;
            }
            let kind = match contract.kind {
                SignatureContractKind::Requires => atom(Tag::Requires, ""),
                SignatureContractKind::Ensures => atom(Tag::Ensures, ""),
                SignatureContractKind::Crashes { cause } => atom(Tag::Crash, &format!("{cause:?}")),
                SignatureContractKind::EnsuresForResultCase { .. } => return None,
            };
            let facts = schema_span(&program.proof_facts, contract.facts)?
                .iter()
                .map(|fact| {
                    let ProofFact::Expression(expression) = fact else {
                        return None;
                    };
                    expression
                        .is_valid()
                        .then(|| normalize_index_expression(program, *expression, context))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(compound(Tag::Contract, std::iter::once(kind).chain(facts)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::TypeParameter;
    use crate::expression::{
        BinaryOperator, CollectionMeasure, Expression, ExpressionHandle, ExpressionNode,
        MemberSelection, NamePath, TableBinaryExpression, TableMemberExpression, TableNamePath,
    };
    use crate::name::Identifier;
    use crate::signature::StateParameter;
    use crate::state::State;
    use crate::types::{FixedArrayLength, TypeReferenceNode};
    use symbols::{BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder};

    struct Fixture {
        program: TypedTrees,
        element: arena::Handle<TypeParameter>,
        extent: arena::Handle<TypeParameter>,
        result: ExpressionHandle,
        measure: ExpressionHandle,
    }

    impl Fixture {
        // The typed header of an array view, including a const extent and a
        // result loan tied to the first of two distinct declared lifetimes.
        fn new(names: [&'static str; 5]) -> Self {
            let [element_name, extent_name, life, other_life, value] = names;
            let mut builder = SymbolTableBuilder::default();
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let roots = builder.insert_children(
                root,
                symbols::builtin_type_symbols()
                    .into_iter()
                    .chain([(SymbolKind::Machine, SymbolNameRef::Static("view"))]),
            );
            let u64_symbol = roots.handle_at(BuiltinTypeAtom::U64.ordinal() as u32);
            let machine = roots.handle_at(BuiltinTypeAtom::ALL.len() as u32);
            let children = builder.insert_children(
                machine,
                [
                    (
                        SymbolKind::TypeParameter,
                        SymbolNameRef::Static(element_name),
                    ),
                    (
                        SymbolKind::TypeParameter,
                        SymbolNameRef::Static(extent_name),
                    ),
                    (SymbolKind::State, SymbolNameRef::Static("entry")),
                ],
            );
            let (element_symbol, extent_symbol, entry) = (
                children.handle_at(0),
                children.handle_at(1),
                children.handle_at(2),
            );
            let value_symbol = builder
                .insert_children(
                    entry,
                    [(SymbolKind::Parameter, SymbolNameRef::Static(value))],
                )
                .handle_at(0);
            let mut program = TypedTrees {
                symbols: builder.finish(),
                ..Default::default()
            };
            let u64_type = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: u64_symbol,
                    name: Identifier::generated("u64"),
                });
            let element_type = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: element_symbol,
                    name: Identifier::generated(element_name),
                });
            let type_parameters = program.data_type_parameters.insert_many([
                TypeParameter {
                    symbol: element_symbol,
                    name: Identifier::generated(element_name),
                    kind: TypeParameterKind::Type,
                    ..Default::default()
                },
                TypeParameter {
                    symbol: extent_symbol,
                    name: Identifier::generated(extent_name),
                    kind: TypeParameterKind::Const {
                        type_reference: u64_type,
                    },
                    ..Default::default()
                },
            ]);
            let array = program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::ConstParameter {
                        symbol: extent_symbol,
                        name: Identifier::generated(extent_name),
                    },
                });
            let input = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: array,
                    access: language_core::ReferenceAccess::Shared,
                    lifetime: Some(Identifier::generated(life)),
                });
            let slice = program
                .type_reference_table
                .insert(TypeReferenceNode::Slice { element_type });
            let output = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: slice,
                    access: language_core::ReferenceAccess::Shared,
                    lifetime: Some(Identifier::generated(life)),
                });
            let parameters = program.state_parameters.insert_many([StateParameter {
                symbol: value_symbol,
                name: Identifier::generated(value),
                type_reference: input,
                ..Default::default()
            }]);
            let result = program.expression_table.insert_tree(&Expression::Name(
                NamePath::unresolved(vec![Identifier::generated("result")])
                    .denoting_reserved_result_of(entry),
            ));
            let measure =
                program
                    .expression_table
                    .insert(ExpressionNode::Member(TableMemberExpression {
                        receiver: result,
                        selection: MemberSelection::CollectionMeasure(CollectionMeasure::Length),
                        member: Identifier::generated("len"),
                        case_variant: None,
                    }));
            let mut members = arena::HandleSpan::empty();
            program
                .expression_table
                .push_name_path_member(&mut members, Identifier::generated(extent_name));
            let extent = program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    symbol: extent_symbol,
                    head_symbol: extent_symbol,
                    ..Default::default()
                }));
            let equality =
                program
                    .expression_table
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        left: measure,
                        operator: BinaryOperator::Equal,
                        right: extent,
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    }));
            let facts = program
                .proof_facts
                .insert_many([ProofFact::Expression(equality)]);
            let contracts = program.signature_contracts.insert_many([SignatureContract {
                kind: SignatureContractKind::Ensures,
                facts,
                ..Default::default()
            }]);
            let states = program.machine_states.insert_many([State {
                symbol: entry,
                name: Identifier::generated("entry"),
                parameters,
                return_type: output,
                contracts,
                ..Default::default()
            }]);
            program.push_machine(Machine {
                symbol: machine,
                name: Identifier::generated("view"),
                lifetime_parameters: vec![
                    Identifier::generated(life),
                    Identifier::generated(other_life),
                ],
                type_parameters,
                states,
                supply_mode: language_semantics::MachineSupplyMode::CompilerProvided,
                ..Default::default()
            });
            Self {
                program,
                element: type_parameters.handle_at(0),
                extent: type_parameters.handle_at(1),
                result,
                measure,
            }
        }

        fn ordinary() -> Self {
            Self::new(["Element", "Count", "backing", "other", "items"])
        }

        fn text(&self) -> Option<String> {
            let schema = normalized_schema(&self.program, &self.program.machines()[0])?;
            self.program.type_identity_text(schema)
        }
    }

    #[test]
    fn complete_schema_renames_type_const_lifetime_and_value_binders() {
        let original = Fixture::ordinary()
            .text()
            .expect("the complete header must normalize");
        let renamed = Fixture::new(["T", "N", "scope", "unused", "source"])
            .text()
            .unwrap();
        assert_eq!(original, renamed);
    }

    #[test]
    fn binder_kinds_bounds_result_lifetimes_and_operational_clauses_change_schema() {
        let original = Fixture::ordinary().text().unwrap();
        let mut kind = Fixture::ordinary();
        kind.program.data_type_parameters.get_mut(kind.extent).kind = TypeParameterKind::Type;
        assert_ne!(Some(&original), kind.text().as_ref());
        let mut bound = Fixture::ordinary();
        bound
            .program
            .data_type_parameters
            .get_mut(bound.element)
            .bounds
            .multiplicity = language_semantics::Multiplicity::Unrestricted;
        assert_ne!(Some(&original), bound.text().as_ref());
        let mut lifetime = Fixture::ordinary();
        let output = lifetime
            .program
            .machine_states(lifetime.program.machines().first().unwrap())[0]
            .return_type;
        let mut replacement = lifetime
            .program
            .type_reference_table
            .type_reference(output)
            .clone();
        let TypeReferenceNode::Reference {
            lifetime: result_life,
            ..
        } = &mut replacement
        else {
            panic!("borrowed result")
        };
        *result_life = Some(Identifier::generated("other"));
        lifetime
            .program
            .type_reference_table
            .substitute_node(output, replacement);
        assert_ne!(Some(&original), lifetime.text().as_ref());
        let mut ceiling = Fixture::ordinary();
        ceiling.program.machines_mut()[0].declares_no_crash_route = true;
        assert_ne!(Some(&original), ceiling.text().as_ref());
    }

    #[test]
    fn every_added_requirement_is_part_of_the_same_schema() {
        let original = Fixture::ordinary().text().unwrap();
        let mut extra = Fixture::ordinary();
        let expression = extra
            .program
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let facts = extra
            .program
            .proof_facts
            .insert_many([ProofFact::Expression(expression)]);
        let contract = SignatureContract {
            kind: SignatureContractKind::Requires,
            facts,
            ..Default::default()
        };
        let contracts = extra.program.signature_contracts.insert_many([contract]);
        extra.program.machines_mut()[0].contracts = contracts;
        assert_ne!(Some(&original), extra.text().as_ref());
    }

    #[test]
    fn arithmetic_policy_is_part_of_the_complete_contract() {
        let mut exact = Fixture::ordinary();
        let expression = exact
            .program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: exact.measure,
                operator: BinaryOperator::Add,
                right: exact.measure,
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
            }));
        let equality = exact
            .program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: expression,
                operator: BinaryOperator::Equal,
                right: exact.measure,
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
            }));
        let facts = exact
            .program
            .proof_facts
            .insert_many([ProofFact::Expression(equality)]);
        let contracts = exact
            .program
            .signature_contracts
            .insert_many([SignatureContract {
                kind: SignatureContractKind::Ensures,
                facts,
                ..Default::default()
            }]);
        exact.program.machines_mut()[0].contracts = contracts;
        let original = exact.text().unwrap();
        let ExpressionNode::Binary(binary) =
            exact.program.expression_table.expression_mut(expression)
        else {
            panic!("arithmetic")
        };
        binary.policy = numerics::arithmetic::ArithmeticDomain::Wrapping;
        assert_ne!(Some(&original), exact.text().as_ref());
    }

    #[test]
    fn unresolved_members_foreign_results_and_unserved_binders_are_unavailable() {
        let mut member = Fixture::ordinary();
        let ExpressionNode::Member(selected) = member
            .program
            .expression_table
            .expression_mut(member.measure)
        else {
            panic!("measurement")
        };
        selected.selection = MemberSelection::Unresolved;
        assert!(member.text().is_none());
        let mut result = Fixture::ordinary();
        let ExpressionNode::Name(path) = result
            .program
            .expression_table
            .expression_mut(result.result)
        else {
            panic!("result")
        };
        path.reserved_result_of = SymbolHandle::from_arena_index(0);
        assert!(result.text().is_none());
        let mut binder = Fixture::ordinary();
        binder
            .program
            .data_type_parameters
            .get_mut(binder.extent)
            .kind = TypeParameterKind::Implementation;
        assert!(binder.text().is_none());
    }

    fn broken_span<T>(count: u32) -> arena::HandleSpan<T> {
        arena::HandleSpan::from_parts(arena::Handle::from_parts(u32::MAX, 1), count)
    }

    #[test]
    fn a_broken_retained_fact_span_cannot_become_an_empty_contract() {
        let mut broken = Fixture::ordinary();
        let entry = broken.program.machines()[0].states.start();
        let contract = broken.program.machine_states.get(entry).contracts.start();
        broken.program.signature_contracts.get_mut(contract).facts = broken_span(1);
        assert!(broken.text().is_none());

        // A genuinely empty fact roster is still a representable contract.
        broken.program.signature_contracts.get_mut(contract).facts = arena::HandleSpan::empty();
        assert!(broken.text().is_some());
        broken.program.signature_contracts.get_mut(contract).facts = broken_span(0);
        assert!(broken.text().is_none());
    }

    #[test]
    fn malformed_signature_rosters_are_unavailable_instead_of_disappearing() {
        for count in [0, 1] {
            let mut states = Fixture::ordinary();
            states.program.machines_mut()[0].states = broken_span(count);
            assert!(states.text().is_none(), "states, count={count}");

            let mut types = Fixture::ordinary();
            types.program.machines_mut()[0].type_parameters = broken_span(count);
            assert!(types.text().is_none(), "type parameters, count={count}");

            let mut parameters = Fixture::ordinary();
            let entry = parameters.program.machines()[0].states.start();
            parameters.program.machine_states.get_mut(entry).parameters = broken_span(count);
            assert!(parameters.text().is_none(), "parameters, count={count}");

            let mut machine_contracts = Fixture::ordinary();
            machine_contracts.program.machines_mut()[0].contracts = broken_span(count);
            assert!(
                machine_contracts.text().is_none(),
                "machine contracts, count={count}"
            );

            let mut entry_contracts = Fixture::ordinary();
            let entry = entry_contracts.program.machines()[0].states.start();
            entry_contracts
                .program
                .machine_states
                .get_mut(entry)
                .contracts = broken_span(count);
            assert!(
                entry_contracts.text().is_none(),
                "entry contracts, count={count}"
            );
        }
    }

    #[test]
    fn a_complete_header_does_not_grant_catalog_custody() {
        let fixture = Fixture::ordinary();
        assert!(fixture.text().is_some());
        assert!(
            fixture
                .program
                .compiler_primitive_schema(fixture.program.machines()[0].symbol)
                .is_err()
        );
    }
}
