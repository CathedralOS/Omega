//! Algebra-license machinery for the structural judge: REARRANGE-mode ring
//! and semiring licenses, their law-shape detectors, and the polynomial
//! normal form those licenses power. The judge keeps only the types it
//! stores; all candidate computation and shape checking lives here.
use super::super::{BinaryOperator, Machine, SymbolHandle, TraitDefinition, structural_term};
use super::StructuralTerm;
use crate::proof_contracts::contract_entailment::law_conformance::{
    TraitOperationSlots, collect_equality_conjuncts,
};
use std::collections::HashMap;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::machine::TraitConformance;
use typed_trees::types::PrimitiveType;
use typed_trees::types::TypeReferenceHandle;

/// REARRANGE-MODE license (settle 2026-07-18, rung C): a carrier EARNS ring
/// canonicalization over an op through EXPLICIT conformance, never
/// scope-sniffing. A license exists for op machine `add_machine` when some
/// trait declares an op slot with BOTH a commutativity law and an
/// associativity law over it (detected by SHAPE, not by name -- `R(x, y) ==
/// R(y, x)` and `R(R(x, y), z) == R(x, R(y, z))` with distinct requirement
/// params), the op slot is conformed by `add_machine`, and BOTH law slots
/// have satisfiers for the same carrier (whose proofs rung B already
/// machine-checked against the declared laws).
#[derive(Clone, Debug)]
pub(super) struct RingLicense {
    pub(super) add_machine: SymbolHandle,
}

/// Shape-detected commutativity, associativity and left-distributivity law
/// slots one trait declares over `requirement`. The names are the trait's own
/// law requirement slots; whether a carrier satisfies them is the supplied
/// conformance's checked business, not this judgment's. A slot that states a
/// premise is not collected: its equality is conditional.
#[derive(Clone, Debug, Default)]
pub(crate) struct DeclaredIndexAlgebraLaws {
    pub commutativity: Vec<String>,
    pub associativity: Vec<String>,
    /// The additive requirements the queried requirement distributes over,
    /// named by the left-distributivity law slots that mention both.
    pub distributivity: Vec<String>,
}

/// The trait-level half of the algebra license: `requirement`'s commutativity
/// (`R(x, y) == R(y, x)`) and associativity (`R(R(x, y), z) == R(x, R(y, z))`)
/// law slots, detected by shape in the trait's own signatures. A consuming
/// machine's `Binder: Subject satisfies Trait` conformance bound supplies the
/// checked evidence for these slots through the ordinary generic-conformance
/// machinery.
/// Each `ensures`-contract equality conjunct across a state signature's
/// contract roster, resolved to its structural-term pair.
fn for_each_ensures_identity(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    slots: &TraitOperationSlots,
    mut arm: impl FnMut(StructuralTerm, StructuralTerm),
) {
    // An unconditional rewrite has no premises to discharge. Reading a
    // conditional law's statement cannot make it an unconditional license.
    if program
        .state_signature_contracts(signature)
        .iter()
        .any(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Requires)
    {
        return;
    }
    let parameters = program.state_signature_parameters(signature);
    for expression in crate::proof_contracts::proof_facts::ensures_expression_facts(
        program,
        program.state_signature_contracts(signature),
    ) {
        let mut conjuncts = Vec::new();
        collect_equality_conjuncts(
            program,
            signature.symbol,
            parameters,
            expression,
            &mut conjuncts,
        );
        for conjunct in conjuncts {
            let Some(identity) = conjunct.identity() else {
                continue;
            };
            let [Some(left), Some(right)] = identity.subjects.map(|subject| {
                structural_term(
                    program,
                    None,
                    Some((signature.symbol, parameters)),
                    subject,
                    None,
                )
            }) else {
                continue;
            };
            if slots.covers_term(program, &left) && slots.covers_term(program, &right) {
                arm(left, right);
            }
        }
    }
}

pub(crate) fn declared_index_algebra_laws(
    program: &dyn crate::CheckingInput,
    trait_definition: &TraitDefinition,
    requirement: &str,
) -> DeclaredIndexAlgebraLaws {
    let mut laws = DeclaredIndexAlgebraLaws::default();
    let slots = TraitOperationSlots::new(program, trait_definition, None);
    for law in program.trait_machine_signatures(trait_definition) {
        let parameters = program
            .state_signature_parameters(law)
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect::<Vec<_>>();
        for_each_ensures_identity(program, law, &slots, |left, right| {
            if commutativity_shape(&left, &right, &parameters).as_deref() == Some(requirement) {
                laws.commutativity.push(law.name.as_str().to_owned());
            }
            if associativity_shape(&left, &right, &parameters).as_deref() == Some(requirement) {
                laws.associativity.push(law.name.as_str().to_owned());
            }
            if let Some((multiplication, addition)) =
                distributivity_shape(&left, &right, &parameters)
                && multiplication == requirement
                && !laws.distributivity.contains(&addition)
            {
                laws.distributivity.push(addition);
            }
        });
    }
    laws
}

/// Literal identities published by one selected conformance.
///
/// `left` and `right` are the decimal text of the integer a left or right
/// identity law may remove. Each is empty unless that side's identity law and
/// a nullary-to-literal connection (`zero() == 0`) name the same requirement
/// and the literal's carrier is the operation's operand type. A law written
/// directly over the literal does not count: the connection is a separate
/// checked slot.
#[derive(Clone, Debug, Default)]
pub(crate) struct LiteralIndexIdentities {
    pub left: String,
    pub right: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IdentitySide {
    Left,
    Right,
}

enum ConnectionOrientation {
    CallThenLiteral,
    LiteralThenCall,
}

struct LiteralConnection {
    requirement: SymbolHandle,
    literal: String,
    conflict: bool,
}

/// Collect identity and connection laws from the bound trait and every
/// inherited application. The operation's declaring trait may be a parent;
/// the literal connection often lives on the child that fixes the carrier.
pub(crate) fn literal_index_identities(
    program: &dyn crate::CheckingInput,
    bound: &typed_trees::machine::GenericConformanceBound,
    operation: SymbolHandle,
    operand_type: TypeReferenceHandle,
) -> LiteralIndexIdentities {
    let trees = program.typed_program();
    let Some(scope) = bound_identity_scope(trees, bound) else {
        return LiteralIndexIdentities::default();
    };
    let operand_identity = trees.normalized_type_identity(operand_type);
    let mut frames = Vec::new();
    scope.visit(trees, &mut |scope| {
        frames.push((
            scope.declaration.symbol,
            scope.substitutions.clone(),
            scope.substitution_scope_starts.clone(),
        ));
    });
    let mut connections = Vec::new();
    let mut identities = Vec::new();
    for (trait_symbol, substitutions, scope_starts) in &frames {
        let Some(declaration) = trees.trait_definition_by_symbol(*trait_symbol) else {
            continue;
        };
        let slots = TraitOperationSlots::new(program, declaration, None);
        for law in trees.trait_machine_signatures(declaration) {
            if trees.state_signature_contracts(law).iter().any(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            }) {
                continue;
            }
            let parameters = trees
                .state_signature_parameters(law)
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect::<Vec<_>>();
            for expression in crate::proof_contracts::proof_facts::ensures_expression_facts(
                program,
                trees.state_signature_contracts(law),
            ) {
                let mut conjuncts = Vec::new();
                collect_equality_conjuncts(
                    program,
                    law.symbol,
                    trees.state_signature_parameters(law),
                    expression,
                    &mut conjuncts,
                );
                for conjunct in conjuncts {
                    let Some(identity) = conjunct.identity() else {
                        continue;
                    };
                    let [Some(left), Some(right)] = identity.subjects.map(|subject| {
                        structural_term(
                            program,
                            None,
                            Some((law.symbol, trees.state_signature_parameters(law))),
                            subject,
                            None,
                        )
                    }) else {
                        continue;
                    };
                    if !slots.covers_term(program, &left)
                        || !slots.covers_term(program, &right)
                        || !consistent_operation_selections(&left, &right)
                        || trees.scoped_type_identity(
                            typed_trees::type_identity::TypeIdentityRequest {
                                substitutions,
                                ..typed_trees::type_identity::TypeIdentityRequest::ordinary(
                                    identity.carrier,
                                )
                            },
                            scope_starts,
                        ) != operand_identity
                    {
                        continue;
                    }
                    if let Some((requirement, side)) =
                        identity_law_shape(&left, &right, &parameters, operation)
                    {
                        identities.push((requirement, side));
                    }
                    if let Some((requirement, literal, orientation)) =
                        literal_connection_shape(&left, &right)
                    {
                        let (call, integer) = match orientation {
                            ConnectionOrientation::CallThenLiteral => {
                                (identity.subjects[0], identity.subjects[1])
                            }
                            ConnectionOrientation::LiteralThenCall => {
                                (identity.subjects[1], identity.subjects[0])
                            }
                        };
                        let carries = |expression| {
                            operands_type(program, law, expression).is_some_and(|reference| {
                                trees.scoped_type_identity(
                                    typed_trees::type_identity::TypeIdentityRequest {
                                        substitutions,
                                        ..typed_trees::type_identity::TypeIdentityRequest::ordinary(
                                            reference,
                                        )
                                    },
                                    scope_starts,
                                ) == operand_identity
                            })
                        };
                        let integer_type = operands_type(program, law, integer);
                        let integer_matches = match integer_type {
                            Some(_) => carries(integer),
                            None => carries(call),
                        };
                        if carries(call) && integer_matches {
                            record_connection(&mut connections, requirement, literal);
                        }
                    }
                }
            }
        }
    }
    LiteralIndexIdentities {
        left: licensed_identity_literal(&identities, &connections, IdentitySide::Left),
        right: licensed_identity_literal(&identities, &connections, IdentitySide::Right),
    }
}

fn bound_identity_scope<'program>(
    program: &'program typed_trees::TypedTrees,
    bound: &typed_trees::machine::GenericConformanceBound,
) -> Option<typed_trees::trait_definition::AppliedTraitScope<'program>> {
    let declaration = program.trait_definition_by_symbol(bound.trait_symbol)?;
    let parameters = program.trait_type_parameters(declaration);
    if parameters.len() != bound.arguments.len() {
        return None;
    }
    let mut scope = typed_trees::trait_definition::AppliedTraitScope::abstract_scope(declaration);
    scope.argument_owner = bound.binder;
    scope.substitutions = parameters
        .iter()
        .zip(&bound.arguments)
        .map(|(parameter, argument)| (parameter.symbol, *argument))
        .collect();
    Some(scope)
}

fn operands_type(
    program: &dyn crate::CheckingInput,
    law: &typed_trees::signature::StateSignature,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    crate::parameter_expression_result_type_reference(
        program,
        law.symbol,
        program.state_signature_parameters(law),
        expression,
    )
}

fn record_connection(
    connections: &mut Vec<LiteralConnection>,
    requirement: SymbolHandle,
    literal: String,
) {
    if let Some(existing) = connections
        .iter_mut()
        .find(|connection| connection.requirement == requirement)
    {
        if existing.literal != literal {
            existing.conflict = true;
        }
        return;
    }
    connections.push(LiteralConnection {
        requirement,
        literal,
        conflict: false,
    });
}

fn licensed_identity_literal(
    identities: &[(SymbolHandle, IdentitySide)],
    connections: &[LiteralConnection],
    side: IdentitySide,
) -> String {
    let mut chosen: Option<&str> = None;
    for (requirement, identity_side) in identities {
        if *identity_side != side {
            continue;
        }
        let Some(connection) = connections
            .iter()
            .find(|connection| connection.requirement == *requirement && !connection.conflict)
        else {
            continue;
        };
        match chosen {
            None => chosen = Some(connection.literal.as_str()),
            Some(existing) if existing == connection.literal => {}
            Some(_) => return String::new(),
        }
    }
    chosen.unwrap_or("").to_owned()
}

/// `R(x, Z()) == x` is a right identity and `R(Z(), x) == x` is a left
/// identity. `Z` is the nullary requirement's symbol, never its spelling.
fn identity_law_shape(
    left: &StructuralTerm,
    right: &StructuralTerm,
    parameters: &[String],
    operation: SymbolHandle,
) -> Option<(SymbolHandle, IdentitySide)> {
    for (operation_term, value_term) in [(left, right), (right, left)] {
        let StructuralTerm::Application {
            target,
            arguments,
            selections,
            ..
        } = operation_term.semantic_value()
        else {
            continue;
        };
        if *target != operation || !selections.is_empty() || arguments.len() != 2 {
            continue;
        }
        let Some(value) = parameter_variable(value_term, parameters) else {
            continue;
        };
        if let Some(variable) = parameter_variable(&arguments[0], parameters)
            && variable == value
            && let Some(requirement) = nullary_requirement(&arguments[1])
            && requirement != operation
        {
            return Some((requirement, IdentitySide::Right));
        }
        if let Some(requirement) = nullary_requirement(&arguments[0])
            && requirement != operation
            && let Some(variable) = parameter_variable(&arguments[1], parameters)
            && variable == value
        {
            return Some((requirement, IdentitySide::Left));
        }
    }
    None
}

/// `Z() == 0`, either orientation. The integer is decimal text. The
/// orientation says which source operand is the call so its carrier can be
/// checked against the operation's operand type.
fn literal_connection_shape(
    left: &StructuralTerm,
    right: &StructuralTerm,
) -> Option<(SymbolHandle, String, ConnectionOrientation)> {
    if let (Some(requirement), Some(literal)) =
        (nullary_requirement(left), integer_literal_text(right))
    {
        return Some((requirement, literal, ConnectionOrientation::CallThenLiteral));
    }
    if let (Some(requirement), Some(literal)) =
        (nullary_requirement(right), integer_literal_text(left))
    {
        return Some((requirement, literal, ConnectionOrientation::LiteralThenCall));
    }
    None
}

fn integer_literal_text(term: &StructuralTerm) -> Option<String> {
    match term.semantic_value() {
        StructuralTerm::Integer(value) => Some(value.to_string()),
        _ => None,
    }
}

fn nullary_requirement(term: &StructuralTerm) -> Option<SymbolHandle> {
    let StructuralTerm::Application {
        target,
        arguments,
        selections,
        ..
    } = term.semantic_value()
    else {
        return None;
    };
    (arguments.is_empty() && selections.is_empty() && target.is_valid()).then_some(*target)
}

fn parameter_variable<'term>(
    term: &'term StructuralTerm,
    parameters: &[String],
) -> Option<&'term str> {
    let StructuralTerm::Variable(name) = term.semantic_value() else {
        return None;
    };
    parameters
        .iter()
        .any(|parameter| parameter == name)
        .then_some(name.as_str())
}
/// each carrying comm+assoc, connected by a conformed DISTRIBUTIVITY law.
#[derive(Clone)]
pub(super) struct SemiringLicense {
    pub(super) add_machine: SymbolHandle,
    pub(super) mul_machine: SymbolHandle,
}

/// Each machine's entry-state symbol with the machine's position, sorted by
/// symbol; an entry state shared by two machines selects neither. Unfolding
/// looks up one machine per application, and a whole-table scan per unfold
/// dominated entailment over the core Rat and Nat proofs. Built once per root
/// judge; arm and site clones share it.
pub(crate) struct EntryMachines(Vec<((u32, u32), u32)>);

impl EntryMachines {
    pub(crate) fn of(program: &dyn crate::CheckingInput) -> Self {
        let key = |symbol: SymbolHandle| (symbol.arena_index(), symbol.generation());
        let mut entries = program
            .machines()
            .iter()
            .enumerate()
            .filter_map(|(position, machine)| {
                let entry = program.machine_states(machine).first()?;
                Some((key(entry.symbol), u32::try_from(position).ok()?))
            })
            .collect::<Vec<_>>();
        entries.sort_unstable();
        let mut unique = Vec::with_capacity(entries.len());
        let mut index = 0;
        while index < entries.len() {
            let run = entries[index..]
                .iter()
                .take_while(|(symbol, _)| *symbol == entries[index].0)
                .count();
            if run == 1 {
                unique.push(entries[index]);
            }
            index += run;
        }
        Self(unique)
    }

    /// The machine whose entry state is `target`, as
    /// `structural_terms::selected_application_machine` selects it.
    pub(super) fn machine<'program>(
        &self,
        program: &'program dyn crate::CheckingInput,
        target: SymbolHandle,
    ) -> Option<&'program Machine> {
        if !target.is_valid() {
            return None;
        }
        let key = (target.arena_index(), target.generation());
        let found = self
            .0
            .binary_search_by_key(&key, |(symbol, _)| *symbol)
            .ok()?;
        program.machines().get(self.0[found].1 as usize)
    }
}

/// Distribute the licensed mul through the licensed add: a term becomes a
/// list of MONOMIALS (sorted factor lists). `None` past the size cap (the
/// cross product is quadratic; a runaway form refuses into the ordinary
/// path rather than stalling).
pub(super) fn polynomial_normal_form(
    term: &StructuralTerm,
    license: &SemiringLicense,
) -> Option<Vec<Vec<StructuralTerm>>> {
    const MONOMIAL_CAP: usize = 64;
    if let StructuralTerm::Application {
        result_type: _,
        target,
        selections,
        machine: _,
        arguments,
    } = term
        && arguments.len() == 2
    {
        if *target == license.add_machine && selections.is_empty() {
            let mut left = polynomial_normal_form(&arguments[0], license)?;
            let right = polynomial_normal_form(&arguments[1], license)?;
            left.extend(right);
            return (left.len() <= MONOMIAL_CAP).then_some(left);
        }
        if *target == license.mul_machine && selections.is_empty() {
            let left = polynomial_normal_form(&arguments[0], license)?;
            let right = polynomial_normal_form(&arguments[1], license)?;
            let mut product = Vec::new();
            for left_monomial in &left {
                for right_monomial in &right {
                    let mut monomial = left_monomial.clone();
                    monomial.extend(right_monomial.iter().cloned());
                    monomial.sort_by_key(|term| format!("{term:?}"));
                    product.push(monomial);
                }
            }
            return (product.len() <= MONOMIAL_CAP).then_some(product);
        }
    }
    Some(vec![vec![term.clone()]])
}

/// `left - from` as multisets; `None` when `from` is not a sub-multiset of
/// `left` (the exchange does not apply). Generic over the element (tier-1
/// addend displays, tier-2 monomial factor lists).
pub(super) fn sorted_multiset_subtract<T: Clone + PartialEq>(
    left: &[T],
    from: &[T],
) -> Option<Vec<T>> {
    let mut remaining = left.to_vec();
    for item in from {
        let index = remaining.iter().position(|candidate| candidate == item)?;
        remaining.remove(index);
    }
    Some(remaining)
}

/// Flatten nested applications of the licensed op into its addend list; any
/// other term is one addend, compared by complete structural identity.
pub(super) fn additive_addends(
    term: &StructuralTerm,
    op: &SymbolHandle,
    out: &mut Vec<StructuralTerm>,
) {
    if let StructuralTerm::Application {
        result_type: _,
        target,
        selections,
        machine: _,
        arguments,
    } = term
        && target == op
        && selections.is_empty()
        && arguments.len() == 2
    {
        additive_addends(&arguments[0], op, out);
        additive_addends(&arguments[1], op, out);
        return;
    }
    out.push(term.clone());
}

pub(super) fn term_uses_application(term: &StructuralTerm, op: &SymbolHandle) -> bool {
    let term = term.semantic_value();
    matches!(term, StructuralTerm::Application { target, selections, .. }
        if *target == *op && selections.is_empty())
        || term.any_child(|child| term_uses_application(child, op))
}

/// The (trait symbol, bound requirement name) -> conforming (carrier, entry
/// symbol) index: conformances are fixed for the build, so both the per-law
/// slot membership test and each candidate scan read this instead of
/// re-walking every machine and conformance per license.
fn conformance_slot_carriers_uncached(
    program: &dyn crate::CheckingInput,
) -> HashMap<(SymbolHandle, String), Vec<(TypeReferenceHandle, SymbolHandle)>> {
    let mut slot_carriers = HashMap::new();
    for candidate in program.machines() {
        for conformance in program.machine_trait_conformances(candidate) {
            let Some(requirement_name) = bound_requirement_name(conformance, candidate) else {
                continue;
            };
            let Some((candidate_entry, carrier)) = entry_carrier(program, candidate) else {
                continue;
            };
            slot_carriers
                .entry((conformance.symbol, requirement_name))
                .or_insert_with(Vec::new)
                .push((carrier, candidate_entry.symbol));
        }
    }
    slot_carriers
}

/// Compute the program's REARRANGE licenses (settle 2026-07-18): for every
/// trait, find op slots carrying BOTH a commutativity law and an
/// associativity law (matched by SHAPE over the trait's own requirement
/// names), then license each conforming op machine whose carrier also has
/// satisfiers for both law slots. Conformance is the license -- rung B
/// machine-checked those satisfiers against the declared laws, so the
/// closure the canonicalizer assumes is exactly what the carrier proved.
///
/// NO CIRCULAR LICENSING: a machine that itself binds a comm/assoc LAW slot
/// of a trait gets NO licenses from that trait -- the axiom base always
/// proves ring-free. This kills self-licensing (add_comm rearranging its own
/// goal into triviality) AND multi-machine cycles (two comm satisfiers each
/// licensed by the other's conformance, none carrying a real proof).
/// A ring license before any judged machine's circularity break.
pub(super) struct RingCandidate {
    trait_symbol: SymbolHandle,
    comm_law: String,
    assoc_law: String,
    add_machine: SymbolHandle,
    carrier: TypeReferenceHandle,
}

/// A paired license before any judged machine's circularity break;
/// `law_slots` are the add and mul commutativity and associativity slots
/// and the distributivity slot.
pub(super) struct SemiringCandidate {
    trait_symbol: SymbolHandle,
    law_slots: [String; 5],
    license: SemiringLicense,
    carrier: TypeReferenceHandle,
}

/// Every ring and semiring license the program's conformances earn. The
/// trait-law scan and the carrier search are program-pure, so a frozen
/// program derives them once; each judge only drops the licenses its own
/// machine underpins.
pub(crate) struct LicenseCandidates {
    ring: Vec<RingCandidate>,
    semiring: Vec<SemiringCandidate>,
}

impl LicenseCandidates {
    pub(crate) fn of(program: &dyn crate::CheckingInput) -> Self {
        // LicenseCandidates is already invocation-memoized. A second cache
        // for this intermediate index would retain duplicate owned storage.
        let slot_carriers = conformance_slot_carriers_uncached(program);
        Self {
            ring: ring_license_candidates(&slot_carriers, program),
            semiring: semiring_license_candidates(&slot_carriers, program),
        }
    }

    /// PER-LICENSE circularity break (refined 2026-07-16 from the old
    /// trait-wide skip): the judged machine is excluded only from
    /// licenses it ITSELF underpins -- the ones whose comm/assoc law
    /// slots it binds FOR THE SAME CARRIER. A law lemma's goal is
    /// exactly the law shape over its own op, so no other carrier's
    /// license can rearrange it -- per-carrier exclusion breaks every
    /// cycle while letting IntPair's mul_comm keep using NAT's earned
    /// licenses (the trait-wide skip wrongly stripped those).
    pub(super) fn ring_licenses(
        &self,
        program: &dyn crate::CheckingInput,
        judged_machine: &Machine,
    ) -> Vec<RingLicense> {
        let judged_carrier = judged_carrier(program, judged_machine);
        self.ring
            .iter()
            .filter(|candidate| {
                let judged_underpins_this_license =
                    judged_bound_laws(program, judged_machine, candidate.trait_symbol)
                        .any(|law| law == candidate.comm_law || law == candidate.assoc_law)
                        && judged_carrier.is_some_and(|judged| {
                            crate::value_custody::type_references::type_references_match(
                                program,
                                judged,
                                candidate.carrier,
                            )
                        });
                !judged_underpins_this_license
            })
            .map(|candidate| RingLicense {
                add_machine: candidate.add_machine,
            })
            .collect()
    }

    /// PER-LICENSE circularity break (refined 2026-07-16, mirroring
    /// `ring_licenses`): the judged machine is excluded only
    /// from paired licenses it underpins -- binding one of the five
    /// law slots FOR THE SAME CARRIER. A law lemma's goal is the law
    /// shape over its own carrier's ops, so other carriers' licenses
    /// cannot rearrange it.
    pub(super) fn semiring_licenses(
        &self,
        program: &dyn crate::CheckingInput,
        judged_machine: &Machine,
    ) -> Vec<SemiringLicense> {
        let judged_carrier = judged_carrier(program, judged_machine);
        self.semiring
            .iter()
            .filter(|candidate| {
                let judged_underpins_this_license =
                    judged_bound_laws(program, judged_machine, candidate.trait_symbol)
                        .any(|name| candidate.law_slots.contains(&name))
                        && judged_carrier.is_some_and(|judged| {
                            crate::value_custody::type_references::type_references_match(
                                program,
                                judged,
                                candidate.carrier,
                            )
                        });
                !judged_underpins_this_license
            })
            .map(|candidate| candidate.license.clone())
            .collect()
    }
}

/// The law requirements `judged_machine` binds on one trait.
fn judged_bound_laws<'program>(
    program: &'program dyn crate::CheckingInput,
    judged_machine: &'program Machine,
    trait_symbol: SymbolHandle,
) -> impl Iterator<Item = String> + 'program {
    program
        .machine_trait_conformances(judged_machine)
        .iter()
        .filter(move |conformance| conformance.symbol == trait_symbol)
        .filter_map(|conformance| bound_requirement_name(conformance, judged_machine))
}

/// The entry state a machine's laws quantify over, with the carrier those
/// laws speak about: its first entry parameter's type, else its result type.
/// The conformance index, both candidate scans and the judged-machine
/// circularity break each reconstructed this row; it is derived here once.
fn entry_carrier<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
) -> Option<(&'program typed_trees::state::State, TypeReferenceHandle)> {
    let entry = program.machine_states(machine).first()?;
    Some((
        entry,
        program
            .state_parameters(entry)
            .first()
            .map(|parameter| parameter.type_reference)
            .unwrap_or(entry.return_type),
    ))
}

/// The carrier alone, for a judged machine whose entry state is not needed.
fn judged_carrier(
    program: &dyn crate::CheckingInput,
    judged_machine: &Machine,
) -> Option<TypeReferenceHandle> {
    entry_carrier(program, judged_machine).map(|(_, carrier)| carrier)
}

/// The algebraic law inventory of every trait: (op, law-requirement) pairs
/// for commutativity and associativity, plus (mul, add, law) triples for
/// distributivity. Ring and semiring licensing both build the inventory
/// before applying their own carrier-closure tests.
#[allow(clippy::type_complexity)]
fn algebraic_law_slots(
    program: &dyn crate::CheckingInput,
) -> Vec<(
    &TraitDefinition,
    Vec<(String, String)>,
    Vec<(String, String)>,
    Vec<(String, String, String)>,
)> {
    program
        .traits()
        .iter()
        .map(|trait_definition| {
            let slots = TraitOperationSlots::new(program, trait_definition, None);
            let mut comm_laws: Vec<(String, String)> = Vec::new();
            let mut assoc_laws: Vec<(String, String)> = Vec::new();
            let mut dist_laws: Vec<(String, String, String)> = Vec::new();
            for requirement in program.trait_machine_signatures(trait_definition) {
                let parameters: Vec<String> = program
                    .state_signature_parameters(requirement)
                    .iter()
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .collect();
                for_each_ensures_identity(program, requirement, &slots, |left, right| {
                    if let Some(op) = commutativity_shape(&left, &right, &parameters) {
                        comm_laws.push((op, requirement.name.as_str().to_owned()));
                    }
                    if let Some(op) = associativity_shape(&left, &right, &parameters) {
                        assoc_laws.push((op, requirement.name.as_str().to_owned()));
                    }
                    if let Some((mul_op, add_op)) = distributivity_shape(&left, &right, &parameters)
                    {
                        dist_laws.push((mul_op, add_op, requirement.name.as_str().to_owned()));
                    }
                });
            }
            (trait_definition, comm_laws, assoc_laws, dist_laws)
        })
        .collect()
}

fn ring_license_candidates(
    slot_carriers: &HashMap<(SymbolHandle, String), Vec<(TypeReferenceHandle, SymbolHandle)>>,
    program: &dyn crate::CheckingInput,
) -> Vec<RingCandidate> {
    let mut candidates = Vec::new();

    // Op slot name -> (has commutativity law named, has associativity law
    // named): the LAW requirement names matter later (their satisfiers
    // must exist for the carrier).
    for (trait_definition, comm_laws, assoc_laws, _dist_laws) in algebraic_law_slots(program) {
        for (op_slot, comm_law) in &comm_laws {
            let Some((_, assoc_law)) = assoc_laws.iter().find(|(op, _)| op == op_slot) else {
                continue;
            };
            // Every machine the slot index lists under the op slot is a
            // candidate license -- the index already resolved each machine's
            // carrier and entry state, which this scan used to rebuild.
            let Some(rows) = slot_carriers.get(&(trait_definition.symbol, op_slot.clone())) else {
                continue;
            };
            for (carrier, add_machine) in rows {
                if slot_satisfier(
                    slot_carriers,
                    program,
                    trait_definition.symbol,
                    comm_law,
                    *carrier,
                )
                .is_some()
                    && slot_satisfier(
                        slot_carriers,
                        program,
                        trait_definition.symbol,
                        assoc_law,
                        *carrier,
                    )
                    .is_some()
                {
                    candidates.push(RingCandidate {
                        trait_symbol: trait_definition.symbol,
                        comm_law: comm_law.clone(),
                        assoc_law: assoc_law.clone(),
                        add_machine: *add_machine,
                        carrier: *carrier,
                    });
                }
            }
        }
    }

    candidates
}

/// The requirement name a conformance binds: the authored `as Name` alias
/// when present, else the standalone machine's own name. Attached machines
/// bind nothing implicitly.
fn bound_requirement_name(conformance: &TraitConformance, machine: &Machine) -> Option<String> {
    conformance
        .requirement
        .as_ref()
        .map(|name| name.as_str().to_owned())
        .or_else(|| {
            machine
                .attached_data
                .is_none()
                .then(|| machine.name.as_str().to_owned())
        })
}

/// The first machine the slot index lists under `(trait, requirement_name)`
/// whose carrier matches; a caller that only needs membership reads
/// `.is_some()`. Law-slot membership and op-slot lookups share this one
/// traversal.
fn slot_satisfier(
    slot_carriers: &HashMap<(SymbolHandle, String), Vec<(TypeReferenceHandle, SymbolHandle)>>,
    program: &dyn crate::CheckingInput,
    trait_symbol: SymbolHandle,
    requirement_name: &str,
    carrier: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    slot_carriers
        .get(&(trait_symbol, requirement_name.to_owned()))
        .and_then(|rows| {
            rows.iter().find_map(|(candidate_carrier, entry_symbol)| {
                crate::value_custody::type_references::type_references_match(
                    program,
                    *candidate_carrier,
                    carrier,
                )
                .then_some(*entry_symbol)
            })
        })
}

/// `R(x, y) == R(y, x)` with `x`/`y` DISTINCT requirement parameters -> the
/// op slot `R` is declared commutative by this law.
/// Tier-2: recognize `mul(a, add(b, c)) == add(mul(a, b), mul(a, c))` up
/// to parameter naming -- returns (mul_op, add_op).
fn distributivity_shape(
    left: &StructuralTerm,
    right: &StructuralTerm,
    parameters: &[String],
) -> Option<(String, String)> {
    if !consistent_operation_selections(left, right) {
        return None;
    }

    // left = mul(a, add(b, c))
    let StructuralTerm::Application {
        machine: mul_op,
        arguments: mul_args,
        ..
    } = left
    else {
        return None;
    };
    let [
        StructuralTerm::Variable(a),
        StructuralTerm::Application {
            machine: add_op,
            arguments: add_args,
            ..
        },
    ] = mul_args.as_slice()
    else {
        return None;
    };
    let [StructuralTerm::Variable(b), StructuralTerm::Variable(c)] = add_args.as_slice() else {
        return None;
    };
    if mul_op == add_op {
        return None;
    }
    // Like the other shapes, a law over repeated operands states only a
    // special case; `R(a, S(a, a)) == S(R(a, a), R(a, a))` is not distributivity.
    if a == b || b == c || a == c {
        return None;
    }
    for name in [a, b, c] {
        if !parameters.contains(name) {
            return None;
        }
    }
    // right = add(mul(a, b), mul(a, c))
    let StructuralTerm::Application {
        machine: outer_add,
        arguments: outer_args,
        ..
    } = right
    else {
        return None;
    };
    if outer_add != add_op {
        return None;
    }
    let [
        StructuralTerm::Application {
            machine: left_mul,
            arguments: left_args,
            ..
        },
        StructuralTerm::Application {
            machine: right_mul,
            arguments: right_args,
            ..
        },
    ] = outer_args.as_slice()
    else {
        return None;
    };
    if left_mul != mul_op || right_mul != mul_op {
        return None;
    }
    let (
        [StructuralTerm::Variable(la), StructuralTerm::Variable(lb)],
        [StructuralTerm::Variable(ra), StructuralTerm::Variable(rc)],
    ) = (left_args.as_slice(), right_args.as_slice())
    else {
        return None;
    };
    (la == a && lb == b && ra == a && rc == c).then(|| (mul_op.clone(), add_op.clone()))
}

/// Tier-2 licensing: a trait carrying comm+assoc for BOTH an add op and a
/// mul op, plus a DISTRIBUTIVITY law connecting them, licenses each carrier
/// that conformed ALL FIVE law slots. Same no-circularity rule: the judged
/// machine binding ANY involved law slot gets nothing from this trait.
fn semiring_license_candidates(
    slot_carriers: &HashMap<(SymbolHandle, String), Vec<(TypeReferenceHandle, SymbolHandle)>>,
    program: &dyn crate::CheckingInput,
) -> Vec<SemiringCandidate> {
    let mut candidates = Vec::new();
    for (trait_definition, comm_laws, assoc_laws, dist_laws) in algebraic_law_slots(program) {
        for (mul_op, add_op, dist_law) in &dist_laws {
            let Some((_, add_comm)) = comm_laws.iter().find(|(op, _)| op == add_op) else {
                continue;
            };
            let Some((_, add_assoc)) = assoc_laws.iter().find(|(op, _)| op == add_op) else {
                continue;
            };
            let Some((_, mul_comm)) = comm_laws.iter().find(|(op, _)| op == mul_op) else {
                continue;
            };
            let Some((_, mul_assoc)) = assoc_laws.iter().find(|(op, _)| op == mul_op) else {
                continue;
            };
            let law_slots = [add_comm, add_assoc, mul_comm, mul_assoc, dist_law];
            // Each carrier the slot index lists under the add op earns the
            // paired license once all five law slots are satisfied and a
            // paired mul satisfier exists. The index already resolved each
            // machine's carrier and entry state, which this scan used to
            // rebuild.
            let Some(rows) = slot_carriers.get(&(trait_definition.symbol, add_op.clone())) else {
                continue;
            };
            for (carrier, add_machine) in rows {
                if !law_slots.iter().all(|law| {
                    slot_satisfier(
                        slot_carriers,
                        program,
                        trait_definition.symbol,
                        law,
                        *carrier,
                    )
                    .is_some()
                }) {
                    continue;
                }
                if let Some(mul_machine) = slot_satisfier(
                    slot_carriers,
                    program,
                    trait_definition.symbol,
                    mul_op,
                    *carrier,
                ) {
                    candidates.push(SemiringCandidate {
                        trait_symbol: trait_definition.symbol,
                        law_slots: law_slots.map(|law| law.clone()),
                        license: SemiringLicense {
                            add_machine: *add_machine,
                            mul_machine,
                        },
                        carrier: *carrier,
                    });
                }
            }
        }
    }
    candidates
}

fn commutativity_shape(
    left: &StructuralTerm,
    right: &StructuralTerm,
    parameters: &[String],
) -> Option<String> {
    if !consistent_operation_selections(left, right) {
        return None;
    }

    let StructuralTerm::Application {
        machine: op_l,
        arguments: args_l,
        ..
    } = left
    else {
        return None;
    };
    let StructuralTerm::Application {
        machine: op_r,
        arguments: args_r,
        ..
    } = right
    else {
        return None;
    };
    if op_l != op_r || args_l.len() != 2 || args_r.len() != 2 {
        return None;
    }
    let [StructuralTerm::Variable(x), StructuralTerm::Variable(y)] = args_l.as_slice() else {
        return None;
    };
    let [StructuralTerm::Variable(rx), StructuralTerm::Variable(ry)] = args_r.as_slice() else {
        return None;
    };
    let is_parameter = |name: &String| parameters.iter().any(|parameter| parameter == name);
    (x != y && rx == y && ry == x && is_parameter(x) && is_parameter(y)).then(|| op_l.clone())
}

/// `R(R(x, y), z) == R(x, R(y, z))` (either orientation) with distinct
/// requirement parameters -> the op slot `R` is declared associative.
fn associativity_shape(
    left: &StructuralTerm,
    right: &StructuralTerm,
    parameters: &[String],
) -> Option<String> {
    if !consistent_operation_selections(left, right) {
        return None;
    }

    for (first, second) in [(left, right), (right, left)] {
        let StructuralTerm::Application {
            machine: op_outer,
            arguments: outer_args,
            ..
        } = first
        else {
            continue;
        };
        if outer_args.len() != 2 {
            continue;
        }
        let StructuralTerm::Application {
            machine: op_inner,
            arguments: inner_args,
            ..
        } = &outer_args[0]
        else {
            continue;
        };
        if op_inner != op_outer || inner_args.len() != 2 {
            continue;
        }
        let (StructuralTerm::Variable(x), StructuralTerm::Variable(y), StructuralTerm::Variable(z)) =
            (&inner_args[0], &inner_args[1], &outer_args[1])
        else {
            continue;
        };
        let StructuralTerm::Application {
            machine: op_right,
            arguments: right_args,
            ..
        } = second
        else {
            continue;
        };
        if op_right != op_outer || right_args.len() != 2 {
            continue;
        }
        let StructuralTerm::Variable(rx) = &right_args[0] else {
            continue;
        };
        let StructuralTerm::Application {
            machine: op_right_inner,
            arguments: right_inner_args,
            ..
        } = &right_args[1]
        else {
            continue;
        };
        if op_right_inner != op_outer || right_inner_args.len() != 2 {
            continue;
        }
        let (StructuralTerm::Variable(ry), StructuralTerm::Variable(rz)) =
            (&right_inner_args[0], &right_inner_args[1])
        else {
            continue;
        };
        let is_parameter = |name: &String| parameters.iter().any(|parameter| parameter == name);
        let distinct = x != y && y != z && x != z;
        if distinct
            && rx == x
            && ry == y
            && rz == z
            && is_parameter(x)
            && is_parameter(y)
            && is_parameter(z)
        {
            return Some(op_outer.clone());
        }
    }
    None
}

/// Law schema names identify slots, but repeated occurrences must retain the
/// same resolved operation and complete static application before a shape can
/// license a rewrite.
fn consistent_operation_selections(left: &StructuralTerm, right: &StructuralTerm) -> bool {
    fn visit<'term>(
        term: &'term StructuralTerm,
        seen: &mut Vec<(&'term str, SymbolHandle, &'term [StaticMachineArgument])>,
    ) -> bool {
        let term = term.semantic_value();
        if let StructuralTerm::Application {
            target,
            selections,
            machine,
            ..
        } = term
        {
            if let Some((_, previous, previous_selections)) =
                seen.iter().find(|(name, _, _)| *name == machine)
            {
                if previous != target || *previous_selections != selections.as_slice() {
                    return false;
                }
            } else {
                seen.push((machine, *target, selections));
            }
        }
        !term.any_child(|child| !visit(child, seen))
    }
    let mut seen = Vec::new();
    visit(left, &mut seen) && visit(right, &mut seen)
}

/// Scalar evaluation after the substituted operands retain their observation
/// authority. Exact integer comparisons also consume the existing polynomial
/// algebra over completed values: only a constant difference decides truth.
/// Other policies remain whole value atoms, so wrapping or saturation cannot
/// acquire integer reassociation. IEEE and selected callable meanings stay in
/// their own operation owners.
///
/// Once both operands reduce to `Integer`, the operation is a constant whose
/// value the declared primitive and operand domain already fix. Exact/Trapping fold only inside the
/// representable range (out-of-range keeps the term -- the operation's own
/// trap/proof obligation owns that edge); Wrapping reduces modulo 2^bits and
/// reinterprets the residue under the result signedness; Saturating clamps.
/// Comparisons fold to the bool constructor. Division/remainder refuse a zero
/// divisor outright rather than manufacture a value; mixed operand primitives
/// and non-integer arithmetic never fold. Address equality also folds when
/// both operands name the same completed observation.
pub(super) fn fold_scalar_binary(
    operator: BinaryOperator,
    meaning: [(PrimitiveType, numerics::arithmetic::ArithmeticDomain); 2],
    left: &StructuralTerm,
    right: &StructuralTerm,
) -> Option<StructuralTerm> {
    let left_observation = left;
    let right_observation = right;
    let left = left.semantic_value();
    let right = right.semantic_value();
    // Addresses have equality without integer-polynomial semantics. Comparing
    // the same completed address observation needs no width or backing fact;
    // it grants no access to the addressed storage. Floats deliberately do not
    // share this reflexivity rule because their observation may be NaN.
    if meaning
        .iter()
        .all(|(primitive, _)| *primitive == PrimitiveType::Addr)
        && left == right
    {
        match operator {
            BinaryOperator::Equal => return Some(super::value_predicates::boolean_term(true)),
            BinaryOperator::NotEqual => return Some(super::value_predicates::boolean_term(false)),
            _ => {}
        }
    }
    if meaning
        .iter()
        .all(|(primitive, _)| *primitive == PrimitiveType::Bool)
    {
        let truth = |term: &StructuralTerm| term.boolean_constructor();
        // Operands here are already formed Boolean observations. Preserve
        // the evaluated operand when an identity removes only a fixed value;
        // a fixed left operand also determines ordinary short-circuiting.
        // A fixed right absorbing value cannot erase evaluation of the left.
        match (operator, truth(left), truth(right)) {
            (BinaryOperator::And, Some(true), _) | (BinaryOperator::Or, Some(false), _) => {
                return Some(right_observation.clone());
            }
            (BinaryOperator::And, _, Some(true)) | (BinaryOperator::Or, _, Some(false)) => {
                return Some(left_observation.clone());
            }
            (BinaryOperator::And, Some(false), _) | (BinaryOperator::Or, Some(true), _) => {
                return Some(left_observation.clone());
            }
            _ => {}
        }
        let left = truth(left)?;
        let right = truth(right)?;
        let value = match operator {
            BinaryOperator::Equal => left == right,
            BinaryOperator::NotEqual => left != right,
            BinaryOperator::And => left && right,
            BinaryOperator::Or => left || right,
            _ => return None,
        };
        return Some(super::value_predicates::boolean_term(value));
    }
    if let Some(verdict) = completed_integer_comparison(operator, meaning, left, right) {
        return Some(super::value_predicates::boolean_term(verdict));
    }
    let (StructuralTerm::Integer(left), StructuralTerm::Integer(right)) = (left, right) else {
        return None;
    };
    let (primitive, _) = meaning[0];
    if meaning[1].0 != primitive || !primitive.accepts_integer_literal() {
        return None;
    }
    match operator {
        operator if operator.is_comparison() => {
            let verdict = operator
                .evaluate_comparison(&left, &right)
                .expect("comparison operator");
            return Some(StructuralTerm::boolean(verdict));
        }
        _ => {}
    }
    let exact = match operator {
        BinaryOperator::Add => Some(left.add(right)),
        BinaryOperator::Subtract => Some(left.sub(right)),
        BinaryOperator::Multiply => Some(left.mul(right)),
        BinaryOperator::Divide => left.div_rem(right).map(|(quotient, _)| quotient),
        BinaryOperator::Modulo => left.div_rem(right).map(|(_, remainder)| remainder),
        _ => None,
    }?;
    let bits = primitive.scalar_byte_size()? * 8;
    let domain = meaning[0].1.combine(meaning[1].1);
    let modulus = numerics::bignum::BigInt::from_u64(1).shl_bits(bits);
    let half = numerics::bignum::BigInt::from_u64(1).shl_bits(bits - 1);
    let one = numerics::bignum::BigInt::from_u64(1);
    let (minimum, maximum) = if primitive.is_signed_integer() {
        (half.negate(), half.sub(&one))
    } else {
        (numerics::bignum::BigInt::zero(), modulus.sub(&one))
    };
    // Literals substituted into an operation still owe operand landing. In
    // particular MIN % -1 cannot fold to zero under Exact/Trapping: the common
    // quotient is unrepresentable even though the remainder fits. Keep the
    // operation visible so later formation checks cannot lose it to folding.
    if left < &minimum || left > &maximum || right < &minimum || right > &maximum {
        return None;
    }
    if matches!(
        domain,
        numerics::arithmetic::ArithmeticDomain::Exact
            | numerics::arithmetic::ArithmeticDomain::Trapping
    ) && matches!(operator, BinaryOperator::Divide | BinaryOperator::Modulo)
    {
        let (quotient, _) = left.div_rem(right)?;
        if quotient < minimum || quotient > maximum {
            return None;
        }
    }
    match domain {
        numerics::arithmetic::ArithmeticDomain::Exact
        | numerics::arithmetic::ArithmeticDomain::Trapping => {
            (minimum <= exact && exact <= maximum).then_some(StructuralTerm::Integer(exact))
        }
        numerics::arithmetic::ArithmeticDomain::Wrapping => {
            let (_, residue) = exact.div_rem(&modulus)?;
            let residue = if residue.is_negative() {
                residue.add(&modulus)
            } else {
                residue
            };
            let value = if primitive.is_signed_integer() && residue > maximum {
                residue.sub(&modulus)
            } else {
                residue
            };
            Some(StructuralTerm::Integer(value))
        }
        numerics::arithmetic::ArithmeticDomain::Saturating => {
            let value = if exact < minimum {
                minimum
            } else if exact > maximum {
                maximum
            } else {
                exact
            };
            Some(StructuralTerm::Integer(value))
        }
    }
}

/// This is value normalization, not expression evaluation: the scalar term
/// already retains builtin selection, carrier/policy and completed occurrences.
/// The caller checks recursive observation authority before entering this owner.
fn completed_integer_comparison(
    operator: BinaryOperator,
    meaning: [(PrimitiveType, numerics::arithmetic::ArithmeticDomain); 2],
    left: &StructuralTerm,
    right: &StructuralTerm,
) -> Option<bool> {
    use super::super::arithmetic_judgment::Polynomial;
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::bignum::BigInt;

    let primitive = meaning[0].0;
    if !primitive.is_integer()
        || meaning != [(primitive, ArithmeticDomain::Exact); 2]
        || !matches!(
            operator,
            BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
        )
    {
        return None;
    }
    fn polynomial(term: &StructuralTerm, primitive: PrimitiveType) -> Option<Polynomial> {
        match term.semantic_value() {
            StructuralTerm::Integer(value) => Some(Polynomial::constant(value.clone())),
            StructuralTerm::ScalarBinary {
                operator,
                meaning,
                left,
                right,
                ..
            } if *meaning == [(primitive, ArithmeticDomain::Exact); 2] => match operator {
                BinaryOperator::Add => {
                    Some(polynomial(left, primitive)?.add(&polynomial(right, primitive)?))
                }
                BinaryOperator::Subtract => {
                    Some(polynomial(left, primitive)?.sub(&polynomial(right, primitive)?))
                }
                BinaryOperator::Multiply => {
                    polynomial(left, primitive)?.checked_mul(&polynomial(right, primitive)?)
                }
                _ => Some(Polynomial::atom(format!("{term:?}"))),
            },
            _ => Some(Polynomial::atom(format!("{term:?}"))),
        }
    }
    let difference = polynomial(left, primitive)?
        .sub(&polynomial(right, primitive)?)
        .constant_value()?;
    operator.evaluate_comparison(&difference, &BigInt::zero())
}

#[cfg(test)]
mod tests {
    use super::{BinaryOperator, PrimitiveType, StructuralTerm, fold_scalar_binary};
    use numerics::arithmetic::ArithmeticDomain;

    #[test]
    fn completed_address_equality_does_not_assume_distinct_addresses_or_float_reflexivity() {
        let address = StructuralTerm::Variable("address".to_owned());
        let other_address = StructuralTerm::Variable("other_address".to_owned());
        let address_meaning = [(PrimitiveType::Addr, ArithmeticDomain::Exact); 2];
        assert_eq!(
            fold_scalar_binary(BinaryOperator::Equal, address_meaning, &address, &address)
                .and_then(|value| value.boolean_constructor()),
            Some(true),
        );
        assert_eq!(
            fold_scalar_binary(
                BinaryOperator::NotEqual,
                address_meaning,
                &address,
                &address
            )
            .and_then(|value| value.boolean_constructor()),
            Some(false),
        );
        assert!(
            fold_scalar_binary(
                BinaryOperator::Equal,
                address_meaning,
                &address,
                &other_address,
            )
            .is_none()
        );
        for other in [PrimitiveType::F32, PrimitiveType::F64, PrimitiveType::U64] {
            let meaning = [
                (PrimitiveType::Addr, ArithmeticDomain::Exact),
                (other, ArithmeticDomain::Exact),
            ];
            assert!(
                fold_scalar_binary(BinaryOperator::Equal, meaning, &address, &address).is_none()
            );
        }
        for primitive in [PrimitiveType::F32, PrimitiveType::F64] {
            let meaning = [(primitive, ArithmeticDomain::Exact); 2];
            assert!(
                fold_scalar_binary(BinaryOperator::Equal, meaning, &address, &address).is_none()
            );
        }
    }
}
