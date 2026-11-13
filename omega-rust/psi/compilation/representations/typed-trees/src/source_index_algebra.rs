//! Read algebra laws from the exact conformance bound used by an index operation.
//! These descriptions name the declared evidence obligations; they do not prove a body.
use crate::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    machine::GenericConformanceBound,
    signature::{SignatureContractKind, StateSignature},
    trait_definition::{AppliedTraitScope, TraitApplicationArguments},
    type_identity::{
        IndexIdentityArgument, NormalizedTypeIdentity, applied_index_identity_in_region_scope,
        applied_type_identity_in_scope,
    },
    types::TypeReferenceHandle,
};
use numerics::bignum::BigInt;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum IndexLawKind {
    Commutativity,
    Associativity,
    Distributivity,
    LeftIdentity,
    RightIdentity,
    LiteralConnection,
}

/// Ordinals rejoin a clause under its exact declaration after expression copying.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct IndexLawEvidence {
    pub bound: GenericConformanceBound,
    pub declaring_trait: SymbolHandle,
    pub law: SymbolHandle,
    pub contract: u32,
    pub fact: u32,
    pub conjunct: Vec<u32>,
    pub kind: IndexLawKind,
}

impl IndexLawEvidence {
    pub fn remap_symbols(&mut self, mut remap: impl FnMut(SymbolHandle) -> SymbolHandle) {
        self.bound.binder = remap(self.bound.binder);
        self.bound.trait_symbol = remap(self.bound.trait_symbol);
        self.declaring_trait = remap(self.declaring_trait);
        self.law = remap(self.law);
    }
}

#[derive(Clone, Copy)]
pub struct DeclaredIdentity {
    pub carrier: TypeReferenceHandle,
    pub subjects: [ExpressionHandle; 2],
}

pub trait LawSource {
    fn identity(
        &self,
        law: &StateSignature,
        expression: ExpressionHandle,
    ) -> Result<Option<DeclaredIdentity>, &'static str>;
    fn conjunction(&self, law: &StateSignature, expression: ExpressionHandle) -> bool;
    fn expression_type(
        &self,
        law: &StateSignature,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle>;
    fn integer(&self, expression: ExpressionHandle) -> Option<BigInt>;
}

#[derive(Default)]
pub struct IndexAlgebraLaws {
    pub commutativity: bool,
    pub associativity: bool,
    pub distributivity: Vec<SymbolHandle>,
    pub left: Option<BigInt>,
    pub right: Option<BigInt>,
    pub evidence: Vec<IndexLawEvidence>,
}

#[derive(Clone, PartialEq, Eq)]
struct Slot {
    requirement: SymbolHandle,
    application: NormalizedTypeIdentity,
    local_arguments: Vec<NormalizedTypeIdentity>,
}
#[derive(Clone, PartialEq, Eq)]
enum Term {
    Formal(usize),
    Integer(BigInt),
    SourceValue(ExpressionHandle),
    Call {
        slot: Slot,
        selections: Vec<NormalizedTypeIdentity>,
        arguments: Vec<Term>,
    },
}
fn call(term: &Term) -> Option<(&Slot, &[NormalizedTypeIdentity], &[Term])> {
    match term {
        Term::Call {
            slot,
            selections,
            arguments,
        } => Some((slot, selections, arguments)),
        _ => None,
    }
}
fn formal(term: &Term) -> Option<usize> {
    match term {
        Term::Formal(position) => Some(*position),
        _ => None,
    }
}
fn same_call(left: &Term, right: &Term) -> bool {
    match (call(left), call(right)) {
        (Some((a, s, _)), Some((b, t, _))) => a == b && s == t,
        _ => false,
    }
}
fn commutative(left: &Term, right: &Term) -> bool {
    let (Some((_, _, a)), Some((_, _, b))) = (call(left), call(right)) else {
        return false;
    };
    let ([x, y], [ry, rx]) = (a, b) else {
        return false;
    };
    same_call(left, right)
        && formal(x).is_some()
        && formal(y).is_some()
        && x != y
        && x == rx
        && y == ry
}
fn associative(left: &Term, right: &Term) -> bool {
    for (first, second) in [(left, right), (right, left)] {
        let (Some((_, _, outer)), Some((_, _, other))) = (call(first), call(second)) else {
            continue;
        };
        let ([inner, z], [x, other_inner]) = (outer, other) else {
            continue;
        };
        let (Some((_, _, a)), Some((_, _, b))) = (call(inner), call(other_inner)) else {
            continue;
        };
        let ([ax, y], [by, bz]) = (a, b) else {
            continue;
        };
        if same_call(first, second)
            && same_call(first, inner)
            && same_call(first, other_inner)
            && [x, y, z].into_iter().all(|term| formal(term).is_some())
            && x != y
            && x != z
            && y != z
            && x == ax
            && y == by
            && z == bz
        {
            return true;
        }
    }
    false
}
fn distributes(left: &Term, right: &Term) -> Option<SymbolHandle> {
    for (first, second) in [(left, right), (right, left)] {
        let (Some((multiply, _, a)), Some((sum, _, b))) = (call(first), call(second)) else {
            continue;
        };
        let ([x, addition], [l, r]) = (a, b) else {
            continue;
        };
        let (Some((inner_sum, _, terms)), Some((_, _, ls)), Some((_, _, rs))) =
            (call(addition), call(l), call(r))
        else {
            continue;
        };
        let ([y, z], [lx, ly], [rx, rz]) = (terms, ls, rs) else {
            continue;
        };
        if multiply != sum
            && sum == inner_sum
            && same_call(second, addition)
            && same_call(first, l)
            && same_call(first, r)
            && [x, y, z].into_iter().all(|term| formal(term).is_some())
            && x != y
            && x != z
            && y != z
            && x == lx
            && x == rx
            && y == ly
            && z == rz
        {
            return Some(sum.requirement);
        }
    }
    None
}
fn nullary(term: &Term) -> Option<Slot> {
    let (slot, selections, arguments) = call(term)?;
    (arguments.is_empty() && selections.is_empty()).then(|| slot.clone())
}
fn identity(left: &Term, right: &Term, operation: &Slot) -> Option<(Slot, bool)> {
    for (computed, value) in [(left, right), (right, left)] {
        let Some((slot, _, arguments)) = call(computed) else {
            continue;
        };
        let [a, b] = arguments else {
            continue;
        };
        if slot != operation || formal(value).is_none() {
            continue;
        }
        if a == value
            && let Some(constant) = nullary(b)
            && constant != *operation
        {
            return Some((constant, false));
        }
        if b == value
            && let Some(constant) = nullary(a)
            && constant != *operation
        {
            return Some((constant, true));
        }
    }
    None
}
fn literal_connection(left: &Term, right: &Term) -> Option<(Slot, BigInt)> {
    for (computed, value) in [(left, right), (right, left)] {
        if let (Some(constant), Term::Integer(value)) = (nullary(computed), value) {
            return Some((constant, value.clone()));
        }
    }
    None
}

struct Terms<'a, 'scope> {
    program: &'a TypedTrees,
    scopes: &'scope [AppliedTraitScope<'a>],
    source: &'scope dyn LawSource,
    carrier: NormalizedTypeIdentity,
}
impl Terms<'_, '_> {
    fn slot(&self, target: SymbolHandle) -> Result<Slot, &'static str> {
        let (owner, template, closed, local_arguments) =
            if let Some(selected) = self.program.closed_requirement_application(target) {
                (
                    selected.declaring_trait,
                    selected.template,
                    Some(selected.trait_application_identity),
                    selected.portable_local_arguments.to_vec(),
                )
            } else {
                if !self.program.symbols.symbols().nodes().is_valid(target) {
                    return Err("index law names a stale operation declaration");
                }
                (
                    self.program.symbols.get(target).parent,
                    target,
                    None,
                    Vec::new(),
                )
            };
        let mut selected = None;
        for scope in self
            .scopes
            .iter()
            .filter(|scope| scope.declaration.symbol == owner)
        {
            if self
                .program
                .trait_machine_signature_by_symbol(scope.declaration, template)
                .is_none()
            {
                return Err("index law operation is outside the selected trait telescope");
            }
            let application = scope
                .application_identity(self.program)
                .ok_or("index law operation has an unresolved trait application")?;
            if closed.is_some_and(|closed| closed != application) {
                continue;
            }
            let slot = Slot {
                requirement: template,
                application,
                local_arguments: local_arguments.clone(),
            };
            if selected.as_ref().is_some_and(|previous| previous != &slot) {
                return Err("index law operation has ambiguous inherited applications");
            }
            selected = Some(slot);
        }
        selected.ok_or("index law operation has no exact application in its conformance bound")
    }
    fn carrier(
        &self,
        scope: &AppliedTraitScope<'_>,
        reference: TypeReferenceHandle,
    ) -> Result<NormalizedTypeIdentity, &'static str> {
        if !self.program.contains_type_reference(reference) {
            return Err("index law has a stale carrier type");
        }
        scope
            .with_identity_scopes(self.program, |scopes| {
                applied_type_identity_in_scope(
                    self.program,
                    reference,
                    scope.declaration.symbol,
                    scope.argument_owner,
                    scopes,
                )
            })
            .ok_or("index law carrier has no complete applied identity")
    }
    fn term(
        &self,
        scope: &AppliedTraitScope<'_>,
        law: &StateSignature,
        expression: ExpressionHandle,
        active: &mut Vec<ExpressionHandle>,
    ) -> Result<Option<Term>, &'static str> {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
            || active.contains(&expression)
        {
            return Err("index law has a stale or cyclic expression");
        }
        if let Some(value) = self.source.integer(expression) {
            // An anonymous literal can receive its carrier from the Id clause;
            // an explicitly typed literal must retain that exact applied carrier.
            if let Some(reference) = self.source.expression_type(law, expression)
                && self.carrier(scope, reference)? != self.carrier
            {
                return Err("index literal does not retain the declared identity carrier");
            }
            return Ok(Some(Term::Integer(value)));
        }
        active.push(expression);
        let result = match self.program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(saved) => {
                let source = self
                    .program
                    .saved_value_source(saved)
                    .ok_or("index law capture has no retained source value")?;
                self.term(scope, law, source, active)?
            }
            ExpressionNode::Name(path) => {
                if !self.program.symbols.symbols().nodes().is_valid(path.symbol) {
                    return Err("index law has an unresolved source value");
                }
                Some(
                    self.program
                        .state_signature_parameters(law)
                        .iter()
                        .position(|parameter| parameter.symbol == path.symbol)
                        .map_or(Term::SourceValue(expression), Term::Formal),
                )
            }
            ExpressionNode::Call(call) => {
                if call.runtime_call.is_some()
                    || !self.program.call_has_no_runtime_receiver_for_target(call)
                    || !call.evidence_arguments.is_empty()
                    || call.static_requirement_dispatch.is_some()
                    || call.operational_acknowledgement.acknowledges_suspend
                    || call.operational_acknowledgement.acknowledges_block
                {
                    active.pop();
                    return Ok(Some(Term::SourceValue(expression)));
                }
                if !self
                    .program
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(call.target_symbol)
                {
                    return Err("index law has a stale source call target");
                }
                let owner = self
                    .program
                    .closed_requirement_application(call.target_symbol)
                    .map_or_else(
                        || self.program.symbols.get(call.target_symbol).parent,
                        |closed| closed.declaring_trait,
                    );
                if !self
                    .scopes
                    .iter()
                    .any(|scope| scope.declaration.symbol == owner)
                {
                    active.pop();
                    return Ok(Some(Term::SourceValue(expression)));
                }
                let slot = self.slot(call.target_symbol)?;
                let mut arguments = Vec::new();
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                {
                    let Some(term) = self.term(scope, law, *argument, active)? else {
                        active.pop();
                        return Ok(None);
                    };
                    arguments.push(term);
                }
                let mut selections = Vec::new();
                for argument in call.machine_arguments.iter() {
                    let selected = scope
                        .with_identity_scopes(self.program, |scopes| {
                            applied_index_identity_in_region_scope(
                                self.program,
                                IndexIdentityArgument::StaticArgument(argument),
                                scope.argument_owner,
                                scopes,
                                &[],
                            )
                        })
                        .ok_or("index law operation has unresolved static actuals")?;
                    selections.push(selected);
                }
                Some(Term::Call {
                    slot,
                    selections,
                    arguments,
                })
            }
            ExpressionNode::Binary(binary) => {
                if let Some(spelling) =
                    crate::operator::arithmetic_operator_spelling(binary.operator)
                {
                    let operands = [binary.left, binary.right]
                        .map(|operand| self.source.expression_type(law, operand));
                    if crate::operator::has_builtin_spelled_expression_meaning(
                        self.program,
                        law.symbol,
                        expression,
                        spelling,
                        &operands,
                    ) {
                        Some(Term::SourceValue(expression))
                    } else {
                        let candidates = crate::operator::trait_scope_operator_meanings(
                            self.program,
                            scope.declaration,
                            spelling,
                            &operands,
                        );
                        match candidates.as_slice() {
                            [] => Some(Term::SourceValue(expression)),
                            [meaning] => {
                                if !self
                                    .program
                                    .state_signature_type_parameters(meaning.requirement)
                                    .is_empty()
                                {
                                    return Err(
                                        "index law operation lacks its exact local static actuals",
                                    );
                                }
                                let slot = self.slot(meaning.requirement.symbol)?;
                                let mut arguments = Vec::new();
                                for operand in [binary.left, binary.right] {
                                    arguments.push(
                                        self.term(scope, law, operand, active)?
                                            .ok_or("index law operation lost a source operand")?,
                                    );
                                }
                                Some(Term::Call {
                                    slot,
                                    selections: Vec::new(),
                                    arguments,
                                })
                            }
                            _ => {
                                return Err("index law operator has ambiguous source requirements");
                            }
                        }
                    }
                } else {
                    Some(Term::SourceValue(expression))
                }
            }
            _ => Some(Term::SourceValue(expression)),
        };
        active.pop();
        if result
            .as_ref()
            .is_some_and(|term| !matches!(term, Term::SourceValue(_)))
        {
            let reference = self
                .source
                .expression_type(law, expression)
                .ok_or("index law operand has no declared type")?;
            if self.carrier(scope, reference)? != self.carrier {
                return Err("index law operands do not retain the exact index carrier");
            }
        }
        Ok(result)
    }
}

pub fn bound_laws(
    program: &TypedTrees,
    owner: SymbolHandle,
    bound: &GenericConformanceBound,
    operation: SymbolHandle,
    operand_type: TypeReferenceHandle,
    source: &dyn LawSource,
) -> Result<IndexAlgebraLaws, &'static str> {
    let machine = program
        .machine_by_symbol(owner)
        .ok_or("index law query has no source machine")?;
    if !machine
        .conformance_bounds
        .iter()
        .any(|candidate| candidate == bound)
    {
        return Err("index operation does not own the supplied conformance bound");
    }
    let declaration = program
        .trait_definition_by_symbol(bound.trait_symbol)
        .ok_or("index bound has no trait declaration")?;
    let root = AppliedTraitScope::new(
        program,
        declaration,
        TraitApplicationArguments {
            lexical_owner: owner,
            enclosing_data_applications: &[],
            arguments: &bound.arguments,
            lifetimes: &bound.lifetime_arguments,
        },
    )
    .ok_or("index bound has no complete trait application")?;
    let mut scopes = Vec::new();
    root.try_visit(program, &mut |scope| {
        scopes.push(scope.clone());
        Ok(())
    })?;
    let carrier = applied_type_identity_in_scope(program, operand_type, owner, owner, &[])
        .ok_or("index operation has no complete operand identity")?;
    let terms = Terms {
        program,
        scopes: &scopes,
        source,
        carrier,
    };
    if program.closed_requirement_application(operation).is_none()
        && let Some((_, signature)) = program.trait_machine_signature(operation)
        && !program
            .state_signature_type_parameters(signature)
            .is_empty()
    {
        return Err("index operation needs its exact callable-local static actuals");
    }
    let operation = terms.slot(operation)?;
    let mut result = IndexAlgebraLaws::default();
    let mut identities: Vec<(Slot, bool, IndexLawEvidence)> = Vec::new();
    let mut connections: Vec<(Slot, BigInt, IndexLawEvidence)> = Vec::new();
    for scope in &scopes {
        for law in program.trait_machine_signatures(scope.declaration) {
            let contracts = program.state_signature_contracts(law);
            if contracts
                .iter()
                .any(|contract| contract.kind == SignatureContractKind::Requires)
            {
                continue;
            }
            for (contract_ordinal, contract) in
                contracts.iter().enumerate().filter(|(_, contract)| {
                    contract.kind == SignatureContractKind::Ensures && contract.binding.is_none()
                })
            {
                for (fact_ordinal, fact) in program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .enumerate()
                {
                    let Some(expression) = fact.as_expression().copied() else {
                        continue;
                    };
                    let mut pending = vec![(expression, Vec::new())];
                    while let Some((expression, path)) = pending.pop() {
                        if source.conjunction(law, expression) {
                            let binary = program
                                .expression_table
                                .binary_node(expression)
                                .ok_or("index law conjunction has no binary source")?;
                            let mut right = path.clone();
                            right.push(1);
                            pending.push((binary.right, right));
                            let mut left = path;
                            left.push(0);
                            pending.push((binary.left, left));
                            continue;
                        }
                        let Some(identity_source) = source.identity(law, expression)? else {
                            continue;
                        };
                        if terms.carrier(scope, identity_source.carrier)? != carrier {
                            continue;
                        }
                        let [left, right] = identity_source.subjects;
                        let (Some(left), Some(right)) = (
                            terms.term(scope, law, left, &mut Vec::new())?,
                            terms.term(scope, law, right, &mut Vec::new())?,
                        ) else {
                            return Err(
                                "index law identity has unresolved symbolic source operands",
                            );
                        };
                        let evidence = |kind| -> Result<IndexLawEvidence, &'static str> {
                            Ok(IndexLawEvidence {
                                bound: bound.clone(),
                                declaring_trait: scope.declaration.symbol,
                                law: law.symbol,
                                contract: u32::try_from(contract_ordinal).map_err(
                                    |_| "index law contract ordinal exceeds its representation",
                                )?,
                                fact: u32::try_from(fact_ordinal).map_err(
                                    |_| "index law fact ordinal exceeds its representation",
                                )?,
                                conjunct: path.clone(),
                                kind,
                            })
                        };
                        if call(&left).is_some_and(|(slot, _, _)| *slot == operation) {
                            if commutative(&left, &right) {
                                result.commutativity = true;
                                result.evidence.push(evidence(IndexLawKind::Commutativity)?);
                            }
                            if associative(&left, &right) {
                                result.associativity = true;
                                result.evidence.push(evidence(IndexLawKind::Associativity)?);
                            }
                            if let Some(sum) = distributes(&left, &right) {
                                if !result.distributivity.contains(&sum) {
                                    result.distributivity.push(sum);
                                }
                                result
                                    .evidence
                                    .push(evidence(IndexLawKind::Distributivity)?);
                            }
                        }
                        if let Some((constant, left)) = identity(&left, &right, &operation) {
                            identities.push((
                                constant,
                                left,
                                evidence(if left {
                                    IndexLawKind::LeftIdentity
                                } else {
                                    IndexLawKind::RightIdentity
                                })?,
                            ));
                        }
                        if let Some((constant, value)) = literal_connection(&left, &right) {
                            if connections.iter().any(|(previous, literal, _)| {
                                previous == &constant && literal != &value
                            }) {
                                return Err(
                                    "index evidence gives one exact constant conflicting literal identities",
                                );
                            }
                            connections.push((
                                constant,
                                value,
                                evidence(IndexLawKind::LiteralConnection)?,
                            ));
                        }
                    }
                }
            }
        }
    }
    for (constant, left, evidence) in identities {
        for (_, value, connection) in connections
            .iter()
            .filter(|(candidate, _, _)| candidate == &constant)
        {
            let selected = if left {
                &mut result.left
            } else {
                &mut result.right
            };
            if selected.as_ref().is_some_and(|previous| previous != value) {
                return Err("index evidence provides conflicting neutral elements");
            }
            *selected = Some(value.clone());
            result.evidence.push(evidence.clone());
            result.evidence.push(connection.clone());
        }
    }
    Ok(result)
}

#[cfg(test)]
mod record_tests {
    use super::*;
    use crate::expression::OpenIndexOperationSelection;
    fn selection() -> OpenIndexOperationSelection {
        OpenIndexOperationSelection {
            expression: ExpressionHandle::from_parts(3, 0),
            spelling: language_core::OperatorSpelling::Add,
            operator: SymbolHandle::from_parts(4, 0),
            operand_type: TypeReferenceHandle::from_parts(5, 0),
            provider: SymbolHandle::from_parts(6, 0),
            application: None,
            algebra_trait: SymbolHandle::from_parts(7, 0),
            algebra_requirement: "combine".to_owned(),
            algebra_alias: None,
            commutativity_licensed: true,
            associativity_licensed: false,
            distributes_over: Vec::new(),
            left_identity_literal: String::new(),
            right_identity_literal: String::new(),
            law_evidence: vec![IndexLawEvidence {
                bound: GenericConformanceBound {
                    binder: SymbolHandle::from_parts(6, 0),
                    binder_name: "Evidence".into(),
                    trait_symbol: SymbolHandle::from_parts(7, 0),
                    trait_name: "Algebra".into(),
                    lifetime_arguments: Vec::new(),
                    arguments: vec![TypeReferenceHandle::from_parts(5, 0)],
                },
                declaring_trait: SymbolHandle::from_parts(7, 0),
                law: SymbolHandle::from_parts(8, 0),
                contract: 1,
                fact: 2,
                conjunct: vec![1, 0],
                kind: IndexLawKind::Commutativity,
            }],
        }
    }
    fn encode(value: &OpenIndexOperationSelection) -> Vec<u8> {
        compiler_storage::encode(value, compiler_storage::RecordLimits::default()).unwrap()
    }
    #[test]
    fn index_operation_record_preserves_exact_declared_law_coordinates() {
        let source = selection();
        let restored: OpenIndexOperationSelection =
            compiler_storage::decode(&encode(&source), compiler_storage::RecordLimits::default())
                .unwrap();
        assert_eq!(restored, source);
    }
    #[test]
    fn index_operation_record_refuses_old_source_evidence_marker() {
        let mut bytes = encode(&selection());
        bytes[..8].copy_from_slice(b"OMGIDX01");
        let result = compiler_storage::decode::<OpenIndexOperationSelection>(
            &bytes,
            compiler_storage::RecordLimits::default(),
        );
        assert!(result.is_err_and(|error| {
            error
                .to_string()
                .contains("unsupported source-evidence format")
        }));
    }
    #[test]
    fn index_operation_record_refuses_missing_or_malformed_law_evidence() {
        let mut source = selection();
        source.law_evidence[0].conjunct.push(2);
        assert!(
            compiler_storage::decode::<OpenIndexOperationSelection>(
                &encode(&source),
                compiler_storage::RecordLimits::default()
            )
            .is_err()
        );
        source.law_evidence.clear();
        assert!(
            compiler_storage::decode::<OpenIndexOperationSelection>(
                &encode(&source),
                compiler_storage::RecordLimits::default()
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod application_tests {
    use super::*;
    #[test]
    fn a_declared_law_cannot_join_another_requirement_or_application() {
        let mut program = TypedTrees::default();
        let unit = program
            .type_reference_table
            .insert(crate::types::TypeReferenceNode::Unit);
        let array =
            program
                .type_reference_table
                .insert(crate::types::TypeReferenceNode::FixedArray {
                    element_type: unit,
                    length: crate::types::FixedArrayLength::Literal(1),
                });
        let application = program.normalized_type_identity(unit);
        let other_application = program.normalized_type_identity(array);
        let operation = Slot {
            requirement: SymbolHandle::from_parts(1, 0),
            application,
            local_arguments: Vec::new(),
        };
        let term = |slot: Slot, a, b| Term::Call {
            slot,
            selections: Vec::new(),
            arguments: vec![Term::Formal(a), Term::Formal(b)],
        };
        let left = term(operation.clone(), 0, 1);
        assert!(commutative(&left, &term(operation.clone(), 1, 0)));
        assert!(!commutative(
            &left,
            &term(
                Slot {
                    requirement: SymbolHandle::from_parts(2, 0),
                    ..operation.clone()
                },
                1,
                0
            )
        ));
        assert!(!commutative(
            &left,
            &term(
                Slot {
                    application: other_application,
                    ..operation
                },
                1,
                0
            )
        ));
    }
}
