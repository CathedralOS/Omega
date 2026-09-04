//! Proof-only classification (math roster N1). COMPUTED, never spelled:
//! recursive data (direct or mutual, through INLINE containment) is legal
//! and proof-only; containment of a proof-only type is contagious. The
//! classification is the single recognizer -- runtime consumption faces
//! (layout, machine data, state params, locals, wire, properties) consult
//! it and refuse with the classification named. References are
//! indirection, not containment: `next: &Node` breaks a cycle and keeps
//! `Node` runtime data.

use crate::TypedTrees;
use crate::data::DataMember;
use crate::name::Identifier;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use psi_symbols::SymbolHandle;
use std::collections::HashMap;

/// Why a data definition is proof-only. `describe` renders the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofOnlyReason {
    /// An N6 quotient is an equivalence class with no runtime representative.
    Quotient,
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
            ProofOnlyReason::Quotient => {
                format!("`{name}` is proof-only: quotient data has no representative layout")
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
        if !type_reference.is_valid() {
            return None;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Named { symbol, name } => {
                self.is_proof_only(*symbol).then(|| name.clone())
            }
            TypeReferenceNode::Reference { referee, .. } => {
                self.proof_only_mention(program, *referee)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.proof_only_mention(program, *base_type)
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                self.proof_only_mention(program, *element_type)
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } => {
                if self.is_proof_only(*base_symbol) {
                    return Some(base_name.clone());
                }
                program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .find_map(|argument| self.proof_only_mention(program, *argument))
            }
            TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Unit => None,
        }
    }
}

impl ProofOnlyClassification {
    /// Machine-stratum contagion (math roster N2d gateway): a free machine
    /// whose signature mentions proof-only data, or a by-value operation
    /// attached to proof-only data itself, is a PROOF MACHINE. The latter is a
    /// proof-side receiver operation, not storage-backed runtime dispatch.
    /// Borrowed or mutable receivers remain runtime consumption attempts, as
    /// do operations attached to runtime data even when another signature
    /// position mentions proof-only data.
    pub fn is_proof_machine(
        &self,
        program: &TypedTrees,
        machine: &crate::machine::Machine,
    ) -> bool {
        if let Some(attached) = machine.attached_data.as_ref() {
            let attached_is_proof_only = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == attached.as_str())
                .is_some_and(|definition| self.is_proof_only(definition.symbol));
            return attached_is_proof_only
                && program.machine_states(machine).iter().all(|state| {
                    program
                        .state_parameters(state)
                        .iter()
                        .find(|parameter| parameter.is_self)
                        .is_some_and(|receiver| {
                            !receiver.is_mutable
                                && receiver.type_reference.is_valid()
                                && !matches!(
                                    program
                                        .type_reference_table
                                        .type_reference(receiver.type_reference),
                                    TypeReferenceNode::Reference { .. }
                                )
                        })
                });
        }
        program.machine_states(machine).iter().any(|state| {
            program.state_parameters(state).iter().any(|parameter| {
                self.proof_only_mention(program, parameter.type_reference)
                    .is_some()
            }) || (state.return_type.is_valid()
                && self
                    .proof_only_mention(program, state.return_type)
                    .is_some())
                || program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .any(|statement| match statement {
                        crate::statement::StatementNode::LocalData(local_data) => self
                            .proof_only_mention(program, local_data.type_reference)
                            .is_some(),
                        _ => false,
                    })
        })
    }
}

/// Classify every data definition: recursion seeds (a definition on an
/// inline-containment cycle), then contagion to fixpoint.
pub fn classify(program: &TypedTrees) -> ProofOnlyClassification {
    let definitions = program.data_definitions();
    let index_by_symbol: HashMap<u32, usize> = definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| (definition.symbol.arena_index(), index))
        .collect();

    // Inline containment edges, with the field that carries each edge (for
    // the contagion message).
    let mut edges: Vec<Vec<(usize, Identifier, Identifier)>> = vec![Vec::new(); definitions.len()];
    for (index, definition) in definitions.iter().enumerate() {
        for member in program.data_members(definition) {
            let fields = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => program.data_payload_fields(variant),
            };
            for field in fields {
                // Occurrence-level erasure removes this containment edge from
                // the runtime representation graph. The field remains in the
                // semantic tree and in proof/ownership frontiers; it simply
                // cannot make its containing runtime record proof-only.
                if field.relevance == psi_language_core::BindingRelevance::Erased {
                    continue;
                }
                collect_inline_data_edges(
                    program,
                    field.type_reference,
                    &index_by_symbol,
                    &mut |target, held| {
                        edges[index].push((target, field.name.clone(), held));
                    },
                );
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

    // Quotients are proof-only by construction: an equivalence class does not
    // expose or store a chosen representative.
    for definition in definitions {
        if definition.quotient.is_some() {
            reasons.insert(definition.symbol.arena_index(), ProofOnlyReason::Quotient);
        }
    }

    // Recursion seeds: definitions that can reach themselves.
    for (start, definition) in definitions.iter().enumerate() {
        if reaches(start, start, &edges) {
            reasons.insert(definition.symbol.arena_index(), ProofOnlyReason::Recursive);
        }
    }

    // Contagion fixpoint: holding a proof-only type inline makes the holder
    // proof-only.
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
    index_by_symbol: &HashMap<u32, usize>,
    edge: &mut impl FnMut(usize, Identifier),
) {
    if !type_reference.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, name } => {
            if let Some(target) = index_by_symbol.get(&symbol.arena_index()) {
                edge(*target, name.clone());
            }
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            collect_inline_data_edges(program, *base_type, index_by_symbol, edge)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_inline_data_edges(program, *element_type, index_by_symbol, edge)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            if let Some(target) = index_by_symbol.get(&base_symbol.arena_index()) {
                edge(*target, base_name.clone());
            }
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_inline_data_edges(program, *argument, index_by_symbol, edge);
            }
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => {}
    }
}
