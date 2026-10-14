//! Proof-only classification (math roster N1). COMPUTED, never spelled:
//! recursive data (direct or mutual, through INLINE containment) is legal
//! and proof-only; containment of a proof-only type is contagious. The
//! classification is the single recognizer -- runtime consumption faces
//! (layout, machine data, state params, locals, wire, properties) consult
//! it and refuse with the classification named. References are
//! indirection, not containment: `next: &Node` breaks a cycle and keeps
//! `Node` runtime data.

use crate::TypedTrees;
use crate::data::DataDefinition;
use crate::name::Identifier;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use std::collections::HashMap;
use symbols::SymbolHandle;

/// Why a data definition is proof-only. `describe` renders the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofOnlyReason {
    /// The compiler's unbounded mathematical integer has no runtime layout.
    Integer,
    /// An N6 quotient is stored as its carrier's canonical representative, so
    /// it is proof-only exactly when the carrier is (the equivalence class
    /// cannot materialize a representative whose own type has no layout).
    Quotient { carrier: Identifier },
    /// The definition reaches itself through inline fields.
    Recursive,
    /// A field (or case payload field) holds a proof-only type inline.
    Contains { field: Identifier, held: Identifier },
}

#[derive(Debug, Default)]
pub struct ProofOnlyClassification {
    /// Keyed by `SymbolHandle::arena_index()` (handles carry no `Hash`).
    reasons: HashMap<u32, ProofOnlyReason>,
}

impl ProofOnlyClassification {
    pub fn is_proof_only(&self, symbol: SymbolHandle) -> bool {
        self.reasons.contains_key(&symbol.arena_index())
    }

    pub fn reason(&self, symbol: SymbolHandle) -> Option<&ProofOnlyReason> {
        self.reasons.get(&symbol.arena_index())
    }

    /// "`Nat` is proof-only: recursive data has no layout" /
    /// "`Wrapper` is proof-only: field `n` holds proof-only `Nat`".
    pub fn describe(&self, name: &str, symbol: SymbolHandle) -> Option<String> {
        Some(match self.reasons.get(&symbol.arena_index())? {
            ProofOnlyReason::Integer => {
                format!("`{name}` is proof-only: mathematical integers have no runtime layout")
            }
            ProofOnlyReason::Quotient { carrier } => {
                format!("`{name}` is proof-only: quotient carrier holds proof-only `{carrier}`")
            }
            ProofOnlyReason::Recursive => {
                format!("`{name}` is proof-only: recursive data has no layout")
            }
            ProofOnlyReason::Contains { field, held } => {
                format!("`{name}` is proof-only: field `{field}` holds proof-only `{held}`")
            }
        })
    }

    /// Is any data type this reference holds INLINE proof-only? Returns the
    /// held type's name. Walks through arrays/constraints/generic arguments
    /// AND through references -- a runtime face cannot use `&Nat` either
    /// (the pointee never materializes); containment edges during
    /// classification are the narrower `inline_data_edges`.
    pub fn proof_only_mention(
        &self,
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
    ) -> Option<Identifier> {
        proof_only_mention_in(&self.reasons, program, type_reference)
    }

    /// Whether an erased formal with this carrier can ride the contract term
    /// lane: either a proof-only carrier (`proof_only_mention`) or a closed
    /// checked-shape record/enum whose fields are all term-expressible — a
    /// scalar leaf, a proof-only field, a fixed array, or another admitted
    /// carrier. The second family keeps its runtime shape in the typed signature but owns
    /// no runtime storage at an erased binding, so the term lane retains its
    /// exact construction as proof evidence instead. References, slices,
    /// generics, quotients, boundary/ laid/ placed/ wired data stay
    /// out: none of them have a term form this lane can serialize.
    pub fn contract_term_carrier(
        &self,
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        if self.proof_only_mention(program, type_reference).is_some() {
            return true;
        }
        let mut visiting = Vec::new();
        self.closed_contract_carrier(program, type_reference, &mut visiting)
    }

    /// One closed carrier: a fixed array of term-admissible elements or a
    /// checked-shape data definition with term-admissible fields. The `visiting` set
    /// refuses carriers that reach themselves through erased fields — those
    /// fields leave the containment graph, so the recursion is the only place
    /// such a cycle can be seen, and an infinite proof term cannot exist.
    fn closed_contract_carrier(
        &self,
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
        visiting: &mut Vec<SymbolHandle>,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        if let TypeReferenceNode::FixedArray {
            element_type,
            length,
        } = program.type_reference_table.type_reference(type_reference)
        {
            return matches!(length, crate::types::FixedArrayLength::Literal(_))
                && self.contract_term_field(program, *element_type, visiting);
        }
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(type_reference)
        else {
            return false;
        };
        if visiting.contains(symbol) {
            return false;
        }
        let Some(definition) = program.data_definition_by_symbol(*symbol) else {
            return false;
        };
        let shape = DataDefinition::shape_kind_from_members(program.data_members(definition));
        if definition.supply_mode != language_core::DataSupplyMode::CheckedShape
            || definition.quotient.is_some()
            || !program.data_type_parameters(definition).is_empty()
            || !matches!(
                shape,
                crate::data::DataShapeKind::Empty
                    | crate::data::DataShapeKind::Record
                    | crate::data::DataShapeKind::Enum
                    | crate::data::DataShapeKind::Mixed
            )
            || program
                .plan_laid_layouts
                .iter()
                .any(|plan| plan.data_symbol == definition.symbol)
            || program
                .placed_view_plans
                .iter()
                .any(|plan| plan.data_symbol == definition.symbol)
            || program
                .wire_schemas()
                .iter()
                .any(|schema| schema.name == definition.name)
        {
            return false;
        }
        visiting.push(*symbol);
        let admitted = program
            .data_member_fields(program.data_members(definition))
            .all(|field| self.contract_term_field(program, field.type_reference, visiting));
        visiting.pop();
        admitted
    }

    /// One field of a candidate carrier: a primitive scalar lowers to a scalar
    /// leaf, a proof-only mention stays a nested proof term, and another
    /// admitted carrier nests as a construction. Anything else — references,
    /// slices, generics, dynamic traits — has no term form.
    fn contract_term_field(
        &self,
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
        visiting: &mut Vec<SymbolHandle>,
    ) -> bool {
        program.primitive_type_reference(type_reference).is_some()
            || self.proof_only_mention(program, type_reference).is_some()
            || self.closed_contract_carrier(program, type_reference, visiting)
    }
}

/// The `proof_only_mention` walk against a reason table still being computed
/// -- `classify` resolves quotient carriers through it inside the contagion
/// fixpoint, before a `ProofOnlyClassification` exists to call the method on.
fn proof_only_mention_in(
    reasons: &HashMap<u32, ProofOnlyReason>,
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<Identifier> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, name } => reasons
            .contains_key(&symbol.arena_index())
            .then(|| name.clone()),
        TypeReferenceNode::Reference { referee, .. } => {
            proof_only_mention_in(reasons, program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            proof_only_mention_in(reasons, program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            proof_only_mention_in(reasons, program, *element_type)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            if reasons.contains_key(&base_symbol.arena_index()) {
                return Some(base_name.clone());
            }
            program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .find_map(|argument| proof_only_mention_in(reasons, program, *argument))
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => None,
    }
}

impl ProofOnlyClassification {
    /// Explicit non-erased mathematical values select the proof-computation
    /// stratum. Namespace qualification contributes neither a subject nor
    /// storage. Erased occurrences alone leave a runtime machine executable.
    ///
    /// Borrowing proof-only data requires storage that cannot exist; it must
    /// remain a runtime-consumption error, not make the containing machine
    /// proof-side and exempt it from consumption checking.
    pub fn is_proof_machine(
        &self,
        program: &TypedTrees,
        machine: &crate::machine::Machine,
    ) -> bool {
        let mut has_proof_value = false;
        for state in program.machine_states(machine) {
            let parameters = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| !parameter.relevance.is_erased())
                .map(|parameter| (parameter.type_reference, parameter.is_mutable));
            let result = std::iter::once((state.return_type, false));
            let locals = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .filter_map(|statement| match statement {
                    crate::statement::StatementNode::LocalData(local)
                        if !local.relevance.is_erased() =>
                    {
                        Some((local.type_reference, false))
                    }
                    _ => None,
                });
            for (type_reference, is_mutable_parameter) in parameters.chain(result).chain(locals) {
                if self.proof_only_mention(program, type_reference).is_none() {
                    continue;
                }
                if is_mutable_parameter || self.borrows_proof_only_data(program, type_reference) {
                    return false;
                }
                has_proof_value = true;
            }
        }
        has_proof_value
    }

    /// Follow type constructors, not the spelling or position of an argument.
    /// An array or generic argument cannot hide a borrow of storage-free data.
    fn borrows_proof_only_data(
        &self,
        program: &TypedTrees,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                self.proof_only_mention(program, *referee).is_some()
            }
            TypeReferenceNode::Slice { element_type } => {
                self.proof_only_mention(program, *element_type).is_some()
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.borrows_proof_only_data(program, *base_type)
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                self.borrows_proof_only_data(program, *element_type)
            }
            TypeReferenceNode::Generic { arguments, .. } => program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .any(|argument| self.borrows_proof_only_data(program, *argument)),
            TypeReferenceNode::Named { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Unit => false,
        }
    }
}

/// Classify every data definition: recursion seeds (a definition on an
/// inline-containment cycle), then contagion to fixpoint.
pub fn classify(program: &TypedTrees) -> ProofOnlyClassification {
    let definitions = program.data_definitions();
    let proof_integer = program
        .symbols
        .builtin_type_symbol(symbols::BuiltinType::Int);
    let index_by_symbol: HashMap<u32, usize> = definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| (definition.symbol.arena_index(), index))
        .collect();

    // Inline containment edges, with the field that carries each edge (for
    // the contagion message).
    let mut edges: Vec<Vec<(usize, Identifier, Identifier)>> = vec![Vec::new(); definitions.len()];
    let mut integer_containment = vec![None; definitions.len()];
    for (index, definition) in definitions.iter().enumerate() {
        for member in program.data_members(definition) {
            let fields = program.data_member_fields(std::slice::from_ref(member));
            for field in fields {
                // Occurrence-level erasure removes this containment edge from
                // the runtime representation graph. The field remains in the
                // semantic tree and in proof/ownership frontiers; it simply
                // cannot make its containing runtime record proof-only.
                if field.relevance == language_core::BindingRelevance::Erased {
                    continue;
                }
                collect_inline_data_edges(program, field.type_reference, &mut |target, held| {
                    if Some(target) == proof_integer {
                        integer_containment[index] = Some((field.name.clone(), held.clone()));
                    }
                    if let Some(target) = index_by_symbol.get(&target.arena_index()) {
                        edges[index].push((*target, field.name.clone(), held));
                    }
                });
            }
        }
    }

    if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
        for (index, definition) in definitions.iter().enumerate() {
            eprintln!(
                "CLASSIFY def[{index}] {} symbol={} edges={:?}",
                definition.name,
                definition.symbol.arena_index(),
                edges[index]
                    .iter()
                    .map(|(target, field, held)| (
                        definitions[*target].name.as_str(),
                        field.as_str(),
                        held.as_str()
                    ))
                    .collect::<Vec<_>>()
            );
        }
    }

    let mut reasons: HashMap<u32, ProofOnlyReason> = HashMap::new();
    if let Some(symbol) = proof_integer {
        reasons.insert(symbol.arena_index(), ProofOnlyReason::Integer);
    }
    // The builtin is not an authored data-definition node in the recursion
    // graph. Seed its inline holders before the ordinary containment fixpoint
    // so wrappers of those holders inherit the same proof-only classification.
    for (index, contained) in integer_containment.into_iter().enumerate() {
        if let Some((field, held)) = contained {
            reasons.insert(
                definitions[index].symbol.arena_index(),
                ProofOnlyReason::Contains { field, held },
            );
        }
    }

    // Recursion seeds: definitions that can reach themselves.
    for (start, definition) in definitions.iter().enumerate() {
        if reaches(start, start, &edges) {
            reasons.insert(definition.symbol.arena_index(), ProofOnlyReason::Recursive);
        }
    }

    // Contagion fixpoint: holding a proof-only type inline makes the holder
    // proof-only. Quotients realize through their carrier's representative, so
    // a quotient is proof-only exactly when its carrier is -- resolved in the
    // same loop because the carrier may itself be a quotient (chained
    // equivalence classes) or hold proof-only content.
    loop {
        let mut changed = false;
        for (index, definition) in definitions.iter().enumerate() {
            if reasons.contains_key(&definition.symbol.arena_index()) {
                continue;
            }
            let contained = edges[index].iter().find(|(target, _, _)| {
                reasons.contains_key(&definitions[*target].symbol.arena_index())
            });
            if let Some((_, field, held)) = contained {
                reasons.insert(
                    definition.symbol.arena_index(),
                    ProofOnlyReason::Contains {
                        field: field.clone(),
                        held: held.clone(),
                    },
                );
                changed = true;
                continue;
            }
            if let Some(quotient) = definition.quotient.as_ref()
                && let Some(carrier) = proof_only_mention_in(&reasons, program, quotient.carrier)
            {
                reasons.insert(
                    definition.symbol.arena_index(),
                    ProofOnlyReason::Quotient { carrier },
                );
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
        for definition in definitions {
            eprintln!(
                "CLASSIFY reason {} symbol={} -> {:?}",
                definition.name,
                definition.symbol.arena_index(),
                reasons.get(&definition.symbol.arena_index()),
            );
        }
    }

    ProofOnlyClassification { reasons }
}

/// Can `from` reach `goal` through one or more inline edges?
fn reaches(goal: usize, from: usize, edges: &[Vec<(usize, Identifier, Identifier)>]) -> bool {
    let mut visited = vec![false; edges.len()];
    let mut stack: Vec<usize> = edges[from].iter().map(|(target, _, _)| *target).collect();
    while let Some(node) = stack.pop() {
        if node == goal {
            return true;
        }
        if std::mem::replace(&mut visited[node], true) {
            continue;
        }
        stack.extend(edges[node].iter().map(|(target, _, _)| *target));
    }
    false
}

/// Inline containment: named data, fixed arrays of it, constrained shells,
/// generic bases and their arguments. References and slices are
/// indirection -- they stop the walk (a `&Node` field is the sanctioned
/// cycle-breaker and stays runtime-legal).
fn collect_inline_data_edges(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    edge: &mut impl FnMut(SymbolHandle, Identifier),
) {
    if !type_reference.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, name } => {
            if symbol.is_valid() {
                edge(*symbol, name.clone());
            }
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            collect_inline_data_edges(program, *base_type, edge)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_inline_data_edges(program, *element_type, edge)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            if base_symbol.is_valid() {
                edge(*base_symbol, base_name.clone());
            }
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_inline_data_edges(program, *argument, edge);
            }
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => {}
    }
}
