//! Exact authored type custody, independently of executable projection and
//! prepared application sidecars. The shared traversal retains lexical scopes;
//! this operation selects only the requested source-owned occurrences.

use super::{
    DeclarationSelection, HashSet, StaticArgumentCall, TypeOccurrence, TypeOccurrences,
    TypedReferences, visit_declarations, visit_machine, visit_program,
};
use crate::{TypedTrees, expression::ExpressionHandle, types::TypeReferenceHandle};
use symbols::SymbolHandle;

#[cfg(test)]
use super::{ExpressionNode, TypeReferenceNode, conformance_occurrence_tests};

/// Discover authored type occurrences under the current declaration roots.
/// Unlike description replay, this never opens an omitted machine body; unlike
/// the complete dependency census, derived applications cannot create inputs.
pub fn collect_authored_program_type_occurrences(program: &TypedTrees) -> Vec<TypeOccurrence> {
    let mut source = AuthoredTypeOccurrences(TypeOccurrences::default());
    visit_program(program, &mut source);
    source.0.types
}

/// Re-derive source occurrences for exactly these lexical owners without
/// making their bodies executable. Current declaration roots supply ordinary
/// owners and nested signatures. Omitted machines require unique original
/// declarations; derived sidecars cannot attest their own source inputs.
pub fn authored_type_occurrences(
    program: &TypedTrees,
    owners: &[SymbolHandle],
) -> Vec<TypeOccurrence> {
    let mut requested = HashSet::default();
    let mut ancestors = HashSet::default();
    let mut machines = Vec::new();
    let mut machine_owners = HashSet::default();
    for &owner in owners {
        if !owner.is_valid() || requested.contains(&owner) {
            continue;
        }
        let mut scope = Vec::new();
        let mut current = owner;
        while current.is_valid() && !scope.contains(&current) {
            scope.push(current);
            current = program.symbols.get(current).parent;
        }
        let mut machine = None;
        let mut ambiguous = false;
        for &ancestor in &scope {
            if let Some(declaration) = program.machine_declaration_for_lexical_scope(ancestor) {
                machine = Some(declaration);
                break;
            }
            if program
                .tables
                .machines
                .iter()
                .any(|(_, declaration)| declaration.symbol == ancestor)
            {
                ambiguous = true;
                break;
            }
        }
        if ambiguous {
            continue;
        }
        requested.insert(owner);
        ancestors.extend(scope);
        if let Some(machine) = machine
            && machine_owners.insert(machine.symbol)
        {
            machines.push(machine);
        }
    }
    let mut source = AuthoredTypeOccurrences(TypeOccurrences {
        selection: DeclarationSelection::from_symbols(ancestors),
        ..TypeOccurrences::default()
    });
    // Traverse current non-machine roots once, then only the exact requested
    // machine descriptions. No body or occurrence is collected from unrelated
    // backing declarations, and no application row supplies its own evidence.
    visit_declarations(program, &mut source);
    for machine in machines {
        visit_machine(program, &mut source, machine);
    }
    source
        .0
        .types
        .into_iter()
        .filter(|occurrence| requested.contains(&occurrence.lexical_owner))
        .collect()
}

// The occurrence bookkeeping is shared with the ordinary census; only the
// graph's authored/derived boundary differs. Scope callbacks preserve nested
// signature owners rather than assigning every child to the outer declaration.
struct AuthoredTypeOccurrences(TypeOccurrences);

impl Extend<SymbolHandle> for AuthoredTypeOccurrences {
    fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, _: Symbols) {}
}

impl TypedReferences for AuthoredTypeOccurrences {
    fn derived_declaration_references(&self) -> bool {
        false
    }
    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        self.0.declaration(symbol)
    }
    fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
        self.0.enter_lexical_scope(owner);
    }
    fn leave_lexical_scope(&mut self) {
        self.0.leave_lexical_scope();
    }
    fn call_arguments(&mut self, program: &TypedTrees, call: StaticArgumentCall<'_>) -> bool {
        self.0.call_arguments(program, call)
    }
    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        self.0.type_reference(reference)
    }
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        self.0.expression(expression)
    }
    fn mathematical_type(
        &mut self,
        reference: crate::mathematical::MathematicalTypeHandle,
    ) -> bool {
        self.0.mathematical_type(reference)
    }
}

#[cfg(test)]
mod authored_type_occurrence_tests {
    use super::*;

    fn contains(program: &TypedTrees, owner: SymbolHandle, reference: TypeReferenceHandle) -> bool {
        authored_type_occurrences(program, &[owner]).contains(&TypeOccurrence {
            lexical_owner: owner,
            reference,
        })
    }

    #[test]
    fn authored_call_actuals_and_cast_targets_exclude_computed_expression_types() {
        use crate::expression::{
            BinaryOperator, OpenIndexOperationSelection, StaticMachineArgument,
            TableBinaryExpression, TableCastExpression,
        };
        let mut program = TypedTrees::default();
        let owner = SymbolHandle::from_parts(41, 1);
        let mut ty = |name: &str| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: crate::name::Identifier::from(name),
                })
        };
        let authored_call = ty("AuthoredCallArgument");
        let authored_cast = ty("AuthoredCastTarget");
        let computed = ty("ComputedTypeOnly");
        let (call, _) = super::conformance_occurrence_tests::occurrence_call(&mut program, owner);
        let application = program.expression_table.retain_static_machine_application(
            SymbolHandle::invalid(),
            Default::default(),
            &[StaticMachineArgument {
                type_reference: authored_call,
                ..Default::default()
            }],
        );
        program
            .expression_table
            .static_machine_application_mut(application)
            .lexical_owner = owner;
        if let ExpressionNode::Call(call) = program.expression_table.expression_mut(call) {
            // Executable retargeting erased the ordinary static arguments;
            // the retained source telescope is still independent source input.
            call.machine_arguments = Box::new([]);
            call.authored_machine_application = application;
        }
        let leaf = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let binary =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: leaf,
                    right: leaf,
                    operator: BinaryOperator::Add,
                }));
        program
            .expression_table
            .index_expression_authority_mut(binary)
            .operation = Some(OpenIndexOperationSelection {
            expression: binary,
            spelling: language_core::OperatorSpelling::Add,
            operator: SymbolHandle::invalid(),
            operand_type: computed,
            provider: SymbolHandle::invalid(),
            application: Some(StaticMachineArgument {
                type_reference: computed,
                ..Default::default()
            }),
            algebra_trait: SymbolHandle::invalid(),
            algebra_requirement: String::new(),
            algebra_alias: None,
            commutativity_licensed: false,
            associativity_licensed: false,
        });
        let cast = program
            .expression_table
            .insert(ExpressionNode::Cast(TableCastExpression {
                value: leaf,
                target_type: authored_cast,
                result_type: computed,
                target_label: Default::default(),
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
                semantic_domain: Default::default(),
                semantic_domain_arguments: Default::default(),
                semantic_domain_symbol: SymbolHandle::invalid(),
                semantic_domain_id: language_semantics::SemanticDomainId::NULL,
                form: language_core::cast_form::CastForm::Value,
            }));
        let elements = program
            .expression_table
            .insert_expression_handles([call, binary, cast]);
        let expression = program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(elements));
        let mut machine = crate::machine::Machine {
            symbol: owner,
            ..Default::default()
        };
        program.push_machine_owned_data(
            &mut machine,
            crate::machine::OwnedData {
                initial_value: expression,
                ..Default::default()
            },
        );
        program.push_machine(machine);
        for projected in [false, true] {
            if projected {
                program.roots.machines = Default::default();
            }
            assert!(contains(&program, owner, authored_call));
            assert!(contains(&program, owner, authored_cast));
            assert!(!contains(&program, owner, computed));
            let current = collect_authored_program_type_occurrences(&program);
            assert_eq!(
                current.contains(&TypeOccurrence {
                    lexical_owner: owner,
                    reference: authored_call
                }),
                !projected,
            );
            assert_eq!(
                current.contains(&TypeOccurrence {
                    lexical_owner: owner,
                    reference: authored_cast
                }),
                !projected,
            );
            assert!(
                !current
                    .iter()
                    .any(|occurrence| occurrence.reference == computed)
            );
        }
    }

    #[test]
    fn source_occurrences_survive_projection_without_derived_self_attestation() {
        let mut symbols = symbols::SymbolTableBuilder::default();
        let owner = symbols.insert_root(
            symbols::SymbolKind::Machine,
            symbols::SymbolNameRef::Static("source"),
        );
        let signature = symbols
            .insert_children(
                owner,
                [(
                    symbols::SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("callback"),
                )],
            )
            .handle_at(0);
        let constant = symbols.insert_root(
            symbols::SymbolKind::Const,
            symbols::SymbolNameRef::Static("constant"),
        );
        let missing = symbols.insert_root(
            symbols::SymbolKind::Machine,
            symbols::SymbolNameRef::Static("missing"),
        );
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let forged = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: crate::name::Identifier::from("sidecar_only"),
            });
        let mut machine = crate::machine::Machine {
            symbol: owner,
            ..Default::default()
        };
        program.push_machine_owned_data(
            &mut machine,
            crate::machine::OwnedData {
                type_reference: reference,
                ..Default::default()
            },
        );
        machine.type_parameters =
            program
                .data_type_parameters
                .insert_many([crate::data::TypeParameter {
                    kind: crate::data::TypeParameterKind::Machine {
                        contract: crate::data::MachineParameterContract::Structural(
                            crate::signature::StateSignature {
                                symbol: signature,
                                return_type: reference,
                                ..Default::default()
                            },
                        ),
                    },
                    ..Default::default()
                }]);
        program.push_machine(machine.clone());
        program.push_const_declaration(crate::constant::ConstDeclaration {
            symbol: constant,
            declared_type: reference,
            ..Default::default()
        });
        // Derived rows can name any backing type handle. Neither a live owner
        // nor a retained capture makes such a row its own source occurrence.
        program.data_implementation_applications.insert(
            crate::typed_trees::DataImplementationApplication {
                lexical_owner: owner,
                source_application: forged,
                ..Default::default()
            },
        );
        assert!(contains(&program, owner, reference));
        assert!(contains(&program, signature, reference));
        assert!(contains(&program, constant, reference));
        let batch = authored_type_occurrences(&program, &[owner, signature, owner]);
        assert_eq!(batch.len(), 2);
        assert!(batch.contains(&TypeOccurrence {
            lexical_owner: owner,
            reference
        }));
        assert!(batch.contains(&TypeOccurrence {
            lexical_owner: signature,
            reference
        }));
        assert!(!contains(&program, owner, forged));
        let current = collect_authored_program_type_occurrences(&program);
        for lexical_owner in [owner, signature, constant] {
            assert!(current.contains(&TypeOccurrence {
                lexical_owner,
                reference
            }));
        }
        assert!(
            !current
                .iter()
                .any(|occurrence| occurrence.reference == forged)
        );
        assert!(!contains(&program, missing, reference));
        assert!(!contains(
            &program,
            owner,
            TypeReferenceHandle::from_parts(reference.arena_index(), reference.generation() + 1),
        ));

        program.roots.machines = Default::default();
        assert!(program.machine_by_symbol(owner).is_none());
        assert!(
            collect_authored_program_type_occurrences(&program)
                == vec![TypeOccurrence {
                    lexical_owner: constant,
                    reference,
                }]
        );
        assert!(contains(&program, owner, reference));
        assert!(contains(&program, signature, reference));
        assert!(!contains(&program, owner, forged));
        assert!(program.machine_by_symbol(owner).is_none());
        // Identical relocated copies are one description; conflicting copies
        // cannot supply lexical custody, even if one contains the requested type.
        let duplicate = program.tables.machines.insert(machine);
        assert!(contains(&program, owner, reference));
        program
            .tables
            .machines
            .get_mut(duplicate)
            .lifetime_parameters
            .push(crate::name::Identifier::from("conflicting"));
        assert!(!contains(&program, owner, reference));
        assert!(!contains(&program, signature, reference));
        assert!(
            authored_type_occurrences(&program, &[owner, signature, constant])
                == vec![TypeOccurrence {
                    lexical_owner: constant,
                    reference
                }]
        );
    }
}
