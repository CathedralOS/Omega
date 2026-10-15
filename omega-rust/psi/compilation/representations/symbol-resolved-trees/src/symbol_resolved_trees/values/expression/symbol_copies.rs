//! Copy a resolved expression together with its cross-table type payloads.
//! The expression table owns ordinary subtree copying and source provenance;
//! this program-level operation owns associations into the declaration arenas.
//! Every substitution uses exact symbols, never diagnostic spellings.

use crate::SymbolResolvedTrees;
use crate::domain::ProofFact;
use crate::expression::{ExpressionHandle, ExpressionNode, MatchPattern, StaticMachineArgument};
use crate::types::{FixedArrayLength, GenericApplicationOrigin, TypeConstraint, TypeReference};
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;

impl SymbolResolvedTrees {
    /// Copy an expression and alpha-rebase all retained symbol references.
    /// Reached type, constraint and static-argument payloads become independent
    /// arena graphs; original expressions and declarations remain unchanged.
    /// Source spans and authored-selection provenance follow the ordinary copy.
    /// Zero handles remain absent. Substitutions are simultaneous and the first
    /// matching source symbol wins, as in the typed expression remapper.
    /// Evidence terms/projections still have diagnostic spellings, not resolved
    /// symbols: they are copied unchanged, not guessed into a new binder scope.
    pub fn copy_expression_with_symbols(
        &mut self,
        source: ExpressionHandle,
        substitutions: &[(SymbolHandle, SymbolHandle)],
    ) -> ExpressionHandle {
        SymbolCopy {
            program: self,
            substitutions,
            types: Vec::new(),
        }
        .expression(source)
    }

    /// Copy a proof fact and its expression/type payloads with exact symbol
    /// substitutions. Authored domain-selection occurrences and source spans
    /// remain provenance of the original fact; diagnostic spellings are not
    /// binder identity and are not renamed.
    pub fn copy_proof_fact_with_symbols(
        &mut self,
        source: Handle<ProofFact>,
        substitutions: &[(SymbolHandle, SymbolHandle)],
    ) -> Handle<ProofFact> {
        SymbolCopy {
            program: self,
            substitutions,
            types: Vec::new(),
        }
        .proof_fact(source)
    }
}

struct SymbolCopy<'program, 'substitutions> {
    program: &'program mut SymbolResolvedTrees,
    substitutions: &'substitutions [(SymbolHandle, SymbolHandle)],
    types: Vec<(Handle<TypeReference>, Handle<TypeReference>)>,
}

impl SymbolCopy<'_, '_> {
    fn proof_fact(&mut self, source: Handle<ProofFact>) -> Handle<ProofFact> {
        if !source.is_valid() {
            return source;
        }
        let source_span = self.program.proof_fact_source_span(source);
        let fact = self
            .program
            .tables
            .declarations
            .proof_facts
            .get(source)
            .clone();
        let fact = match fact {
            ProofFact::Expression(expression) => ProofFact::Expression(self.expression(expression)),
            ProofFact::Membership(mut membership) => {
                membership.value = self.expression(membership.value);
                let names = self.program.domain_path_members(membership.domain).to_vec();
                membership.domain = self
                    .program
                    .tables
                    .declarations
                    .domain_path_members
                    .insert_many(names);
                membership.domain_symbol = self.symbol(membership.domain_symbol);
                membership.domain_arguments = self.type_span(membership.domain_arguments);
                ProofFact::Membership(membership)
            }
        };
        let copied = self.program.tables.declarations.proof_facts.insert(fact);
        if let Some(source_span) = source_span {
            self.program.set_proof_fact_source_span(copied, source_span);
        }
        copied
    }

    fn symbol(&self, source: SymbolHandle) -> SymbolHandle {
        if !source.is_valid() {
            return source;
        }
        self.substitutions
            .iter()
            .find(|(original, _)| *original == source)
            .map_or(source, |(_, replacement)| *replacement)
    }

    fn expression(&mut self, source: ExpressionHandle) -> ExpressionHandle {
        if !source.is_valid() {
            return source;
        }
        let copied = self
            .program
            .tables
            .bodies
            .expressions
            .copy_from_self(source);
        self.expression_payloads(copied);
        copied
    }

    /// Children already belong to the copied expression table. Rebase them in
    /// place, copying only payloads the table deliberately leaves external.
    fn expression_payloads(&mut self, copied: ExpressionHandle) {
        if !copied.is_valid() {
            return;
        }
        let mut node = self
            .program
            .tables
            .bodies
            .expressions
            .expression(copied)
            .clone();
        match &mut node {
            ExpressionNode::Match(dispatch) => {
                self.expression_payloads(dispatch.subject);
                let arms = self
                    .program
                    .tables
                    .bodies
                    .expressions
                    .match_arms(dispatch.arms)
                    .to_vec();
                for arm in arms {
                    match arm.pattern {
                        MatchPattern::Value(pattern) => self.expression_payloads(pattern),
                        MatchPattern::Wildcard => {}
                    }
                    self.expression_payloads(arm.value);
                }
            }
            ExpressionNode::ArrayLiteral(elements) => self.expression_span_payloads(*elements),
            ExpressionNode::Atomic(atomic) => {
                self.expression_payloads(atomic.value);
                self.expression_payloads(atomic.result);
            }
            ExpressionNode::Binary(binary) => {
                self.expression_payloads(binary.left);
                self.expression_payloads(binary.right);
            }
            ExpressionNode::Cast(cast) => {
                self.expression_payloads(cast.value);
                cast.target_type = self.type_reference(cast.target_type);
                cast.semantic_domain_arguments = self.type_span(cast.semantic_domain_arguments);
                cast.semantic_domain_symbol = self.symbol(cast.semantic_domain_symbol);
            }
            ExpressionNode::Call(call) => {
                self.expression_payloads(call.receiver);
                self.expression_span_payloads(call.arguments);
                call.target_symbol = self.symbol(call.target_symbol);
                for argument in call.machine_arguments.iter_mut() {
                    self.static_argument(argument);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                self.expression_payloads(indexed.collection);
                self.expression_payloads(indexed.index);
            }
            ExpressionNode::Membership(membership) => {
                self.expression_payloads(membership.value);
                membership.domain_symbol = self.symbol(membership.domain_symbol);
                membership.case_type_symbol = self.symbol(membership.case_type_symbol);
                membership.case_symbol = self.symbol(membership.case_symbol);
            }
            ExpressionNode::Member(member) => {
                self.expression_payloads(member.receiver);
                member.member_symbol = self.symbol(member.member_symbol);
            }
            ExpressionNode::Borrow(borrow) => self.expression_payloads(borrow.target),
            ExpressionNode::Name(path) => {
                path.head_symbol = self.symbol(path.head_symbol);
                path.symbol = self.symbol(path.symbol);
                let symbols = self
                    .program
                    .tables
                    .bodies
                    .expressions
                    .name_path_member_symbols(path.member_symbols)
                    .to_vec();
                for (position, source) in (0..path.member_symbols.count()).zip(symbols) {
                    let replacement = self.symbol(source);
                    self.program
                        .tables
                        .bodies
                        .expressions
                        .set_name_path_member_symbol_at_offset(
                            path.member_symbols,
                            position,
                            replacement,
                        );
                }
            }
            ExpressionNode::Range(range) => {
                self.expression_payloads(range.start);
                self.expression_payloads(range.end);
            }
            ExpressionNode::StructLiteral(literal) => {
                literal.type_symbol = self.symbol(literal.type_symbol);
                literal.case_symbol = literal.case_symbol.map(|symbol| self.symbol(symbol));
                let fields = self
                    .program
                    .tables
                    .bodies
                    .expressions
                    .struct_fields(literal.fields)
                    .to_vec();
                for (position, mut field) in (0..literal.fields.count()).zip(fields) {
                    self.expression_payloads(field.value);
                    field.field_symbol = self.symbol(field.field_symbol);
                    self.program
                        .tables
                        .bodies
                        .expressions
                        .set_struct_field_at_offset(literal.fields, position, field);
                }
            }
            ExpressionNode::Unary(unary) => self.expression_payloads(unary.operand),
            ExpressionNode::ZeroValue(reference) => *reference = self.type_reference(*reference),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_) => {}
        }
        *self
            .program
            .tables
            .bodies
            .expressions
            .expression_mut(copied) = node;
    }

    fn expression_span_payloads(&mut self, expressions: HandleSpan<ExpressionHandle>) {
        let children = self
            .program
            .tables
            .bodies
            .expressions
            .expression_handles(expressions)
            .to_vec();
        for child in children {
            self.expression_payloads(child);
        }
    }

    fn static_argument(&mut self, argument: &mut StaticMachineArgument) {
        argument.type_reference = self.type_reference(argument.type_reference);
        argument.symbol = self.symbol(argument.symbol);
        if let Some(application) = &mut argument.application {
            for child in application.arguments.iter_mut() {
                self.static_argument(child);
            }
        }
    }

    fn type_reference(&mut self, source: Handle<TypeReference>) -> Handle<TypeReference> {
        if !source.is_valid() {
            return source;
        }
        if let Some((_, copied)) = self.types.iter().find(|(original, _)| *original == source) {
            return *copied;
        }
        let reference = self
            .program
            .tables
            .declarations
            .child_type_references
            .get(source)
            .clone();
        // Reserve the association before following generic-origin edges, which
        // can lead back to the instance whose application they describe.
        let copied = self
            .program
            .tables
            .declarations
            .child_type_references
            .insert(TypeReference::Unit);
        self.types.push((source, copied));
        let reference = self.type_payload(reference);
        *self
            .program
            .tables
            .declarations
            .child_type_references
            .get_mut(copied) = reference;
        self.type_origins(source, copied);
        copied
    }

    fn type_origins(&mut self, source: Handle<TypeReference>, copied: Handle<TypeReference>) {
        let origins = self
            .program
            .tables
            .types
            .generic_application_origins
            .iter()
            .filter_map(|(_, origin)| (origin.instance == source).then_some(origin.clone()))
            .collect::<Vec<_>>();
        for origin in origins {
            let application = self.type_reference(origin.application);
            self.program
                .tables
                .types
                .generic_application_origins
                .insert(GenericApplicationOrigin {
                    instance: copied,
                    application,
                });
        }
    }

    fn type_span(&mut self, source: HandleSpan<TypeReference>) -> HandleSpan<TypeReference> {
        let references = self
            .program
            .tables
            .declarations
            .child_type_references
            .span_or_empty(source)
            .to_vec();
        let copied = self
            .program
            .tables
            .declarations
            .child_type_references
            .insert_many(references.iter().map(|_| TypeReference::Unit));
        // Span rows need the same exact-handle associations as standalone
        // nodes. Register the complete span before following origin edges.
        for position in 0..source.count() {
            let original = source.handle_at(position);
            if !self
                .types
                .iter()
                .any(|(candidate, _)| *candidate == original)
            {
                self.types.push((original, copied.handle_at(position)));
            }
        }
        for (position, reference) in (0..source.count()).zip(references) {
            let destination = copied.handle_at(position);
            let reference = self.type_payload(reference);
            *self
                .program
                .tables
                .declarations
                .child_type_references
                .get_mut(destination) = reference;
            self.type_origins(source.handle_at(position), destination);
        }
        copied
    }

    fn type_payload(&mut self, mut reference: TypeReference) -> TypeReference {
        match &mut reference {
            TypeReference::Reference(borrow) => {
                borrow.referee = self.type_reference(borrow.referee)
            }
            TypeReference::Constrained(constrained) => {
                constrained.base_type = self.type_reference(constrained.base_type);
                let constraints = self
                    .program
                    .tables
                    .types
                    .constraints
                    .span_or_empty(constrained.constraints)
                    .to_vec();
                let copied = constraints
                    .into_iter()
                    .map(|constraint| self.constraint(constraint))
                    .collect::<Vec<_>>();
                constrained.constraints = self.program.tables.types.constraints.insert_many(copied);
            }
            TypeReference::FixedArray(array) => {
                array.element_type = self.type_reference(array.element_type);
                match &mut array.length {
                    FixedArrayLength::ConstParameter { symbol, .. } => {
                        *symbol = self.symbol(*symbol)
                    }
                    FixedArrayLength::ConstCall { machine, .. } => *machine = self.symbol(*machine),
                    FixedArrayLength::Expression(value) => *value = self.type_reference(*value),
                    FixedArrayLength::Literal(_) => {}
                }
            }
            TypeReference::Slice(slice) => {
                slice.element_type = self.type_reference(slice.element_type)
            }
            TypeReference::Generic(application) => {
                application.base_symbol = self.symbol(application.base_symbol);
                application.arguments = self.type_span(application.arguments);
            }
            TypeReference::ConstExpression(expression) => {
                *expression = self.expression(*expression)
            }
            TypeReference::DynamicTrait {
                symbol, arguments, ..
            } => {
                *symbol = self.symbol(*symbol);
                *arguments = self.type_span(*arguments);
            }
            TypeReference::Named { symbol, .. } => *symbol = self.symbol(*symbol),
            TypeReference::Unit => {}
        }
        reference
    }

    fn constraint(&mut self, mut constraint: TypeConstraint) -> TypeConstraint {
        match &mut constraint {
            TypeConstraint::Range {
                minimum, maximum, ..
            } => {
                *minimum = self.expression(*minimum);
                *maximum = self.expression(*maximum);
            }
            TypeConstraint::Domain(domain) => domain.arguments = self.type_span(domain.arguments),
            TypeConstraint::Named(_) | TypeConstraint::ArithmeticDomain(_) => {}
        }
        constraint
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::{StaticSymbolApplication, TableCallExpression, TableNamePath};
    use crate::name::DiagnosticName;
    use crate::types::{GenericTypeReference, GenericTypeReferenceStorage};
    use source::{SourceId, SourceSpan, Span};

    fn generic(base: SymbolHandle, arguments: HandleSpan<TypeReference>) -> TypeReference {
        TypeReference::Generic(GenericTypeReference {
            storage: GenericTypeReferenceStorage {
                supplied_operand_owner: Default::default(),
                base_symbol: base,
                base_name: DiagnosticName::generated("Carrier"),
                lifetime_arguments: Vec::new(),
                arguments,
            },
        })
    }

    #[test]
    fn proof_membership_copies_domain_arguments_and_preserves_provenance() {
        let mut program = SymbolResolvedTrees::default();
        let original = SymbolHandle::from_arena_index(3);
        let replacement = SymbolHandle::from_arena_index(4);
        let domain = program
            .tables
            .declarations
            .domain_path_members
            .insert_many([
                DiagnosticName::generated("contracts"),
                DiagnosticName::generated("Resident"),
            ]);
        let arguments = program
            .tables
            .declarations
            .child_type_references
            .insert_many([TypeReference::Named {
                supplied_operand_owner: Default::default(),
                symbol: original,
                name: DiagnosticName::generated("T"),
            }]);
        let fact = program
            .tables
            .declarations
            .proof_facts
            .insert(ProofFact::Membership(crate::domain::ProofMembershipFact {
                domain,
                domain_symbol: original,
                domain_arguments: arguments,
                ..Default::default()
            }));
        let source_span = SourceSpan::new(SourceId(7), Span::new(11, 19));
        program.set_proof_fact_source_span(fact, source_span);
        let copied = program.copy_proof_fact_with_symbols(fact, &[(original, replacement)]);
        assert_eq!(program.proof_fact_source_span(copied), Some(source_span));
        let ProofFact::Membership(membership) = program.tables.declarations.proof_facts.get(copied)
        else {
            panic!("copy remains membership");
        };
        assert_eq!(membership.domain_symbol, replacement);
        assert_ne!(membership.domain, domain);
        assert_eq!(
            program.domain_path_members(membership.domain),
            program.domain_path_members(domain)
        );
        assert_ne!(membership.domain_arguments, arguments);
        assert_eq!(membership.authored_domain_selection, None);
        let TypeReference::Named { symbol, .. } = program
            .tables
            .declarations
            .child_type_references
            .get(membership.domain_arguments.handle_at(0))
        else {
            panic!("domain argument remains named");
        };
        assert_eq!(*symbol, replacement);
        let ProofFact::Membership(original_fact) =
            program.tables.declarations.proof_facts.get(fact)
        else {
            panic!("original remains membership");
        };
        assert_eq!(original_fact.domain_symbol, original);
        assert_eq!(original_fact.domain, domain);
        assert_eq!(original_fact.domain_arguments, arguments);
    }

    #[test]
    fn copies_span_origins_and_expression_payloads_without_mutating_originals() {
        let mut program = SymbolResolvedTrees::default();
        let original = SymbolHandle::from_arena_index(3);
        let replacement = SymbolHandle::from_arena_index(4);
        let member_symbols = program
            .tables
            .bodies
            .expressions
            .reserve_name_path_member_symbols(1);
        program
            .tables
            .bodies
            .expressions
            .set_name_path_member_symbol_at_offset(member_symbols, 0, original);
        let name = program
            .tables
            .bodies
            .expressions
            .insert(ExpressionNode::Name(TableNamePath {
                members: HandleSpan::empty(),
                member_symbols,
                head_symbol: original,
                symbol: original,
            }));
        let arguments = program
            .tables
            .declarations
            .child_type_references
            .insert_many([TypeReference::ConstExpression(name)]);
        let application = program
            .tables
            .declarations
            .child_type_references
            .insert(generic(original, HandleSpan::empty()));
        program
            .tables
            .types
            .generic_application_origins
            .insert(GenericApplicationOrigin {
                instance: arguments.handle_at(0),
                application,
            });
        let carrier = program
            .tables
            .declarations
            .child_type_references
            .insert(generic(original, arguments));
        let source = program
            .tables
            .bodies
            .expressions
            .insert(ExpressionNode::ZeroValue(carrier));
        let source_span = SourceSpan::new(SourceId(7), Span::new(11, 19));
        program
            .tables
            .bodies
            .expressions
            .set_source_span(source, source_span);
        let copied = program.copy_expression_with_symbols(source, &[(original, replacement)]);
        assert_eq!(
            program.tables.bodies.expressions.source_span(copied),
            source_span
        );
        let ExpressionNode::ZeroValue(copied_carrier) =
            *program.tables.bodies.expressions.expression(copied)
        else {
            panic!("copy must retain zero construction");
        };
        assert_ne!(copied_carrier, carrier);
        let TypeReference::Generic(copied_type) = program
            .tables
            .declarations
            .child_type_references
            .get(copied_carrier)
        else {
            panic!("copy must retain generic application");
        };
        assert_eq!(copied_type.base_symbol, replacement);
        let copied_argument = copied_type.arguments.handle_at(0);
        let TypeReference::ConstExpression(copied_name) = program
            .tables
            .declarations
            .child_type_references
            .get(copied_argument)
        else {
            panic!("copy must retain static expression");
        };
        let ExpressionNode::Name(copied_path) =
            program.tables.bodies.expressions.expression(*copied_name)
        else {
            panic!("copy must retain name");
        };
        assert_eq!(copied_path.symbol, replacement);
        assert_eq!(
            program
                .tables
                .bodies
                .expressions
                .name_path_member_symbols(copied_path.member_symbols),
            [replacement]
        );
        let origin = program
            .tables
            .types
            .generic_application_origins
            .iter()
            .find_map(|(_, origin)| (origin.instance == copied_argument).then_some(origin))
            .expect("copied span origin");
        let TypeReference::Generic(origin_application) = program
            .tables
            .declarations
            .child_type_references
            .get(origin.application)
        else {
            panic!("copy must retain origin application");
        };
        assert_eq!(origin_application.base_symbol, replacement);
        let ExpressionNode::Name(original_path) =
            program.tables.bodies.expressions.expression(name)
        else {
            panic!("original remains a name");
        };
        assert_eq!(original_path.symbol, original);
        let TypeReference::Generic(original_type) = program
            .tables
            .declarations
            .child_type_references
            .get(carrier)
        else {
            panic!("original remains generic");
        };
        assert_eq!(original_type.base_symbol, original);
        assert_eq!(original_type.arguments, arguments);
    }

    #[test]
    fn rebases_nested_static_arguments_simultaneously_and_preserves_absence() {
        let mut program = SymbolResolvedTrees::default();
        let original = SymbolHandle::from_arena_index(3);
        let replacement = SymbolHandle::from_arena_index(4);
        let argument = StaticMachineArgument {
            type_reference: Handle::invalid(),
            path: Box::new([]),
            application: None,
            const_literal: None,
            evidence_projection: None,
            symbol: original,
        };
        let source = program
            .tables
            .bodies
            .expressions
            .insert(ExpressionNode::Call(TableCallExpression {
                receiver: ExpressionHandle::invalid(),
                target_symbol: original,
                target: DiagnosticName::generated("run"),
                requirement_application: arena::Handle::invalid(),
                machine_arguments: vec![StaticMachineArgument {
                    application: Some(Box::new(StaticSymbolApplication {
                        lifetime_arguments: Box::new([]),
                        arguments: vec![argument.clone()].into_boxed_slice(),
                    })),
                    ..argument
                }]
                .into_boxed_slice(),
                arguments: HandleSpan::empty(),
                evidence_arguments: Box::new([]),
                operational_acknowledgement: Default::default(),
            }));
        let copied = program.copy_expression_with_symbols(
            source,
            &[
                (original, replacement),
                (replacement, SymbolHandle::from_arena_index(5)),
                (SymbolHandle::invalid(), original),
            ],
        );
        let ExpressionNode::Call(call) = program.tables.bodies.expressions.expression(copied)
        else {
            panic!("copy remains a call");
        };
        assert_eq!(call.target_symbol, replacement);
        assert!(!call.receiver.is_valid());
        assert!(!call.machine_arguments[0].type_reference.is_valid());
        assert_eq!(call.machine_arguments[0].symbol, replacement);
        assert_eq!(
            call.machine_arguments[0]
                .application
                .as_ref()
                .expect("nested application")
                .arguments[0]
                .symbol,
            replacement
        );
        assert!(
            !program
                .copy_expression_with_symbols(ExpressionHandle::invalid(), &[])
                .is_valid()
        );
    }
}
