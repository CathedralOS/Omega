//! The program-owned interner for named-callable identities.
//!
//! A named machine, a trait machine requirement and an explicitly named
//! operator are each selected by three parts: the declaration path, the
//! parameter signature, and the exact set of dispatch-bearing result
//! domains. Those parts used to be handed to every consult site as rendered,
//! escaped `String`s, and the reverse join — which machine carries this
//! identity — rendered every machine in the program and compared the text.
//! This module interns each structural key for the program's lifetime and
//! hands out handles: an overload comparison is an integer compare. Only the
//! declaration memo and reverse lookup belong to a particular revision.
//!
//! Boundary-operator provider slots retain a separate structural key containing
//! every operand type and the full result type. Their canonical `operator::`
//! encoding is distinct from named-callable result dispatch; rehydrating that
//! encoding must never depend on whether a direct consult already interned it.
//!
//! Text survives as a projection, not as the identity. A provider plan, a
//! package review record and a native artifact each carry an identity across
//! a serialization boundary and rejoin it in a later compilation, where no
//! handle minted here means anything; `callable_identity_text` renders the
//! retained record for exactly those encodings and for diagnostics.
//! `callable_identity_by_text` is the matching rehydration join and is the
//! only entry that accepts text.
//!
//! Freshness: durable keys never reset when monomorphization appends or root
//! spans change. The revision-guarded memo independently rebuilds declaration
//! addresses, active text joins and machine positions. Thus an old handle can
//! still render its key without claiming a retired declaration remains live.
//! Identity derivation walks type references and symbol paths outside both
//! locks; interning and memo publication use separate lock scopes.

use super::constraint_identity::{atom, compound};
use super::{NormalizedResultDispatchSet, TypeIdentityId};
use crate::TypedTrees;
use crate::machine::Machine;
use crate::operator::OperatorDefinition;
use crate::signature::StateSignature;
use crate::trait_definition::TraitDefinition;

/// Handle-keyed maps take the integer hasher; `usize` and tuple keys hash
/// through the same mixing as `symbols::SymbolKeyMap`, and none of these maps
/// is iterated for output order.
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

/// One interned named-callable identity. Zero is the absence state: a machine
/// whose declaration carries no entry state has no callable identity and no
/// slot is minted for it, so no consumer wraps this handle in an `Option`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
)]
pub struct CallableIdentityId(u32);

/// One interned declaration path, shared by every overload of a family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
pub struct CallablePathId(u32);

/// One interned parameter signature. Two identities share it exactly when
/// they are the same overload family under result-domain dispatch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
pub struct CallableParametersId(u32);

/// One interned dispatch-bearing result-domain set. Call sites intern the set
/// their destination requests through
/// [`TypedTrees::callable_result_dispatch_identity`] and then select an
/// overload by comparing handles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
pub struct CallableResultDispatchId(u32);

macro_rules! interned_handle {
    ($name:ident) => {
        impl $name {
            /// Whether this handle names a slot. The zero handle does not.
            pub fn is_valid(self) -> bool {
                self.0 != 0
            }

            fn slot(self) -> Option<usize> {
                (self.0 != 0).then(|| self.0 as usize - 1)
            }

            fn from_slot(slot: usize) -> Self {
                Self(slot as u32 + 1)
            }
        }
    };
}

interned_handle!(CallableIdentityId);
interned_handle!(CallablePathId);
interned_handle!(CallableParametersId);
interned_handle!(CallableResultDispatchId);

/// The derived parts of one named-callable identity, carried from the
/// deriving query to the interner so the derivation never runs under the
/// interner's lock.
pub(crate) struct CallableIdentityParts {
    pub(crate) path: String,
    pub(crate) parameters: String,
    pub(crate) result_dispatch: NormalizedResultDispatchSet,
}

/// A provider requirement identifies the complete operand and result types,
/// not merely the domains which select a named callable overload. Its existing
/// external slot rendering remains stable across the handle migration.
struct BoundaryOperatorIdentityParts {
    path: String,
    parameter_types: Vec<TypeIdentityId>,
    result_type: TypeIdentityId,
    parameters: String,
    result_dispatch: NormalizedResultDispatchSet,
    text: String,
}

/// Which declaration a memoized identity was derived for. The address is the
/// exact key: a declaration's roster position is not unique under the forged
/// duplicate symbols fixtures can declare, and the memo's revision guard
/// bounds the address's lifetime to the declaration state it was taken from — the
/// same rule the isolation verdicts already key on. Cloning clears these
/// address keys because the clone owns different arena allocations.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum DeclarationKey {
    Machine(usize),
    TraitRequirement(usize, usize),
    Operator(usize),
    BoundaryOperatorRequirement(usize),
}

#[derive(Clone, compiler_storage::Record)]
struct CallableIdentityRecord {
    path: CallablePathId,
    parameters: CallableParametersId,
    result_dispatch: CallableResultDispatchId,
    /// The canonical named-callable or boundary-operator requirement
    /// rendering, retained so diagnostics and artifact encoding project it
    /// rather than re-deriving it from the declaration.
    text: Box<str>,
}

/// Append-only structural keys: every minted handle remains valid through
/// declaration revisions and is preserved when its owning program is cloned.
#[derive(Clone, Default, compiler_storage::Record)]
pub struct CallableIdentityTable {
    paths: Vec<Box<str>>,
    path_slots: HashMap<Box<str>, CallablePathId>,
    parameters: Vec<Box<str>>,
    parameter_slots: HashMap<Box<str>, CallableParametersId>,
    /// Dispatch sets are keyed structurally, not by their rendering, so two
    /// distinct sets can never collapse onto one handle through an escaping
    /// accident in their declared names.
    result_dispatches: Vec<NormalizedResultDispatchSet>,
    result_dispatch_slots:
        std::collections::HashMap<NormalizedResultDispatchSet, CallableResultDispatchId>,
    records: Vec<CallableIdentityRecord>,
    record_slots: HashMap<
        (
            CallablePathId,
            CallableParametersId,
            CallableResultDispatchId,
        ),
        CallableIdentityId,
    >,
    // Requirement slots retain the full result type, unlike named-callable
    // overload dispatch. Keep their structural namespace separate.
    boundary_operator_slots:
        HashMap<(CallablePathId, Vec<TypeIdentityId>, TypeIdentityId), CallableIdentityId>,
}

/// Revision-dependent declaration lookup, separate from durable key storage.
#[derive(Default)]
pub struct CallableIdentityMemo {
    by_text: HashMap<Box<str>, CallableIdentityId>,
    by_declaration: HashMap<DeclarationKey, CallableIdentityId>,
    /// Current non-target machine positions, indexed by durable identity slot.
    machines: Vec<Vec<u32>>,
    /// Only a completed current-revision sweep makes reverse lookup complete.
    swept: bool,
}

impl Clone for CallableIdentityMemo {
    fn clone(&self) -> Self {
        Self {
            // Arena clones preserve semantic coordinates, not allocation addresses.
            by_declaration: HashMap::default(),
            by_text: self.by_text.clone(),
            machines: self.machines.clone(),
            swept: self.swept,
        }
    }
}

impl CallableIdentityTable {
    fn intern_path(&mut self, path: &str) -> CallablePathId {
        super::intern_boxed_text(
            path,
            &mut self.paths,
            &mut self.path_slots,
            CallablePathId::from_slot,
        )
    }

    fn intern_parameters(&mut self, parameters: &str) -> CallableParametersId {
        super::intern_boxed_text(
            parameters,
            &mut self.parameters,
            &mut self.parameter_slots,
            CallableParametersId::from_slot,
        )
    }

    fn intern_result_dispatch(
        &mut self,
        result_dispatch: NormalizedResultDispatchSet,
    ) -> CallableResultDispatchId {
        super::intern_owned(
            result_dispatch,
            &mut self.result_dispatches,
            &mut self.result_dispatch_slots,
            CallableResultDispatchId::from_slot,
        )
    }

    fn intern(&mut self, parts: CallableIdentityParts) -> CallableIdentityId {
        let dispatch_text = parts.result_dispatch.identity();
        let path = self.intern_path(&parts.path);
        let parameters = self.intern_parameters(&parts.parameters);
        let result_dispatch = self.intern_result_dispatch(parts.result_dispatch);
        let slot = (path, parameters, result_dispatch);
        if let Some(interned) = self.record_slots.get(&slot) {
            return *interned;
        }
        let text: Box<str> = compound(
            "named-callable",
            [
                atom("path", &parts.path),
                atom("parameters", &parts.parameters),
                atom("result-dispatch", &dispatch_text),
            ],
        )
        .into();
        let interned = CallableIdentityId::from_slot(self.records.len());
        self.records.push(CallableIdentityRecord {
            path,
            parameters,
            result_dispatch,
            text,
        });
        self.record_slots.insert(slot, interned);
        interned
    }

    fn intern_boundary_operator(
        &mut self,
        parts: BoundaryOperatorIdentityParts,
    ) -> CallableIdentityId {
        let path = self.intern_path(&parts.path);
        let key = (path, parts.parameter_types, parts.result_type);
        if let Some(identity) = self.boundary_operator_slots.get(&key) {
            return *identity;
        }
        let parameters = self.intern_parameters(&parts.parameters);
        let result_dispatch = self.intern_result_dispatch(parts.result_dispatch);
        let identity = CallableIdentityId::from_slot(self.records.len());
        let text: Box<str> = parts.text.into();
        self.records.push(CallableIdentityRecord {
            path,
            parameters,
            result_dispatch,
            text,
        });
        self.boundary_operator_slots.insert(key, identity);
        identity
    }

    fn record(&self, identity: CallableIdentityId) -> Option<&CallableIdentityRecord> {
        self.records.get(identity.slot()?)
    }
}

impl TypedTrees {
    /// The interned identity of one top-level named machine overload, or the
    /// zero handle when the declaration carries no entry state. A machine's
    /// first state is its callable entry signature; explicit sibling states
    /// do not create additional top-level overloads.
    pub fn machine_callable_identity(&self, machine: &Machine) -> CallableIdentityId {
        self.interned_callable_identity(
            DeclarationKey::Machine(machine as *const Machine as usize),
            || self.machine_callable_identity_parts(machine),
        )
    }

    /// The interned identity of one trait machine requirement overload.
    pub fn trait_requirement_callable_identity(
        &self,
        trait_definition: &TraitDefinition,
        requirement: &StateSignature,
    ) -> CallableIdentityId {
        self.interned_callable_identity(
            DeclarationKey::TraitRequirement(
                trait_definition as *const TraitDefinition as usize,
                requirement as *const StateSignature as usize,
            ),
            || Some(self.trait_requirement_callable_identity_parts(trait_definition, requirement)),
        )
    }

    /// The interned identity of one explicitly named operator requirement.
    /// Unspelled boundary operators participate in the same result-domain
    /// lookup as other named requirements; fixed spellings remain
    /// operand-directed and use this only for declaration diagnostics.
    pub fn operator_callable_identity(&self, operator: &OperatorDefinition) -> CallableIdentityId {
        self.interned_callable_identity(
            DeclarationKey::Operator(operator as *const OperatorDefinition as usize),
            || Some(self.operator_callable_identity_parts(operator)),
        )
    }

    /// The exact provider slot of a boundary operator. Ordinary operator
    /// callable identity is a different judgment: it retains result dispatch,
    /// while provider satisfaction also depends on the complete result type.
    /// Token views of requirement machines retain the machine's own identity.
    pub fn boundary_operator_requirement_callable_identity(
        &self,
        operator: &OperatorDefinition,
    ) -> CallableIdentityId {
        if !operator.is_boundary {
            return Default::default();
        }
        if let Some(requirement) = self.machine(operator.symbol) {
            return self.machine_callable_identity(requirement);
        }
        let declaration = DeclarationKey::BoundaryOperatorRequirement(
            operator as *const OperatorDefinition as usize,
        );
        let revision = self.structural_fingerprint();
        if let Some(identity) =
            self.read_callable_memo(|memo| memo.by_declaration.get(&declaration).copied())
        {
            return identity;
        }
        let parts = self.boundary_operator_identity_parts(operator);
        let identity = self.callable_identities.with_mut(|table| {
            parts
                .map(|parts| table.intern_boundary_operator(parts))
                .unwrap_or_default()
        });
        self.memoize_callable_identity(revision, declaration, identity);
        identity
    }

    fn boundary_operator_identity_parts(
        &self,
        operator: &OperatorDefinition,
    ) -> Option<BoundaryOperatorIdentityParts> {
        let parameter_types = self
            .operator_parameters(operator)
            .iter()
            .map(|parameter| self.normalized_type_identity(parameter.type_reference))
            .collect::<Vec<_>>();
        let parameters = parameter_types
            .iter()
            .map(|identity| self.type_identity_text(*identity))
            .collect::<Option<Vec<_>>>()?
            .join(",");
        let result_type = if operator.return_type.is_valid() {
            let identity = self.normalized_type_identity(operator.return_type);
            identity.is_valid().then_some(identity)?
        } else {
            Default::default()
        };
        Some(BoundaryOperatorIdentityParts {
            path: self.operator_path_string(operator.name),
            parameter_types,
            result_type,
            parameters,
            result_dispatch: self.normalized_result_dispatch_set(operator.return_type),
            text: crate::operator::boundary_operator_requirement_identity(self, operator),
        })
    }

    fn interned_callable_identity(
        &self,
        declaration: DeclarationKey,
        derive: impl FnOnce() -> Option<CallableIdentityParts>,
    ) -> CallableIdentityId {
        let revision = self.structural_fingerprint();
        // A memoized absence is stored as the zero handle, so a hit answers
        // both questions and only a miss pays for the derivation.
        if let Some(interned) =
            self.read_callable_memo(|memo| memo.by_declaration.get(&declaration).copied())
        {
            return interned;
        }
        let parts = derive();
        let interned = self
            .callable_identities
            .with_mut(|table| parts.map(|parts| table.intern(parts)).unwrap_or_default());
        self.memoize_callable_identity(revision, declaration, interned);
        interned
    }

    fn memoize_callable_identity(
        &self,
        revision: u64,
        declaration: DeclarationKey,
        identity: CallableIdentityId,
    ) {
        let text = self.read_callable_identities(|table| {
            table.record(identity).map(|record| record.text.clone())
        });
        self.callable_identity_memo
            .with_mut(revision, CallableIdentityMemo::default, |memo| {
                memo.by_declaration.insert(declaration, identity);
                if let Some(text) = text {
                    memo.by_text.insert(text, identity);
                }
            });
    }

    /// The declaration path shared by one identity's overload family.
    pub fn callable_identity_path(&self, identity: CallableIdentityId) -> CallablePathId {
        self.read_callable_identities(|table| {
            table
                .record(identity)
                .map(|record| record.path)
                .unwrap_or_default()
        })
    }

    /// The parameter signature one identity dispatches a result domain under.
    pub fn callable_identity_parameters(
        &self,
        identity: CallableIdentityId,
    ) -> CallableParametersId {
        self.read_callable_identities(|table| {
            table
                .record(identity)
                .map(|record| record.parameters)
                .unwrap_or_default()
        })
    }

    /// Whether two identities name the same overload family: one declaration
    /// path under one parameter signature, differing only in the
    /// dispatch-bearing result domains that select between them. The zero
    /// handle shares a family with nothing, including itself.
    pub fn callable_identities_share_family(
        &self,
        left: CallableIdentityId,
        right: CallableIdentityId,
    ) -> bool {
        self.read_callable_identities(|table| match (table.record(left), table.record(right)) {
            (Some(left), Some(right)) => {
                left.path == right.path && left.parameters == right.parameters
            }
            _ => false,
        })
    }

    /// Whether two identities share a declaration path, whatever their
    /// parameter signatures. The zero handle shares a path with nothing.
    pub fn callable_identities_share_path(
        &self,
        left: CallableIdentityId,
        right: CallableIdentityId,
    ) -> bool {
        self.read_callable_identities(|table| match (table.record(left), table.record(right)) {
            (Some(left), Some(right)) => left.path == right.path,
            _ => false,
        })
    }

    /// The dispatch-bearing result-domain set one identity declares.
    pub fn callable_identity_result_dispatch(
        &self,
        identity: CallableIdentityId,
    ) -> CallableResultDispatchId {
        self.read_callable_identities(|table| {
            table
                .record(identity)
                .map(|record| record.result_dispatch)
                .unwrap_or_default()
        })
    }

    /// The canonical rendering of one identity: for diagnostics and for the
    /// artifact, plan and review encodings that carry an identity across a
    /// compilation boundary. The zero handle renders as the empty string.
    pub fn callable_identity_text(&self, identity: CallableIdentityId) -> String {
        self.read_callable_identities(|table| {
            table
                .record(identity)
                .map(|record| record.text.as_ref().to_owned())
                .unwrap_or_default()
        })
    }

    /// The rendered identity of one machine overload, absent exactly when the
    /// declaration carries no entry state. Rendering serves diagnostics and
    /// the plan, review and artifact encodings that carry an identity into a
    /// later compilation; an in-program comparison uses the handle from
    /// [`TypedTrees::machine_callable_identity`] instead.
    pub fn machine_callable_identity_text(&self, machine: &Machine) -> Option<String> {
        let identity = self.machine_callable_identity(machine);
        identity
            .is_valid()
            .then(|| self.callable_identity_text(identity))
    }

    /// The rendered identity of one trait machine requirement overload, for
    /// diagnostics and the encodings that carry it across a compilation.
    pub fn trait_requirement_callable_identity_text(
        &self,
        trait_definition: &TraitDefinition,
        requirement: &StateSignature,
    ) -> String {
        self.callable_identity_text(
            self.trait_requirement_callable_identity(trait_definition, requirement),
        )
    }

    /// The declaration path's text, for diagnostics and export payload.
    pub fn callable_path_text(&self, path: CallablePathId) -> String {
        self.read_callable_identities(|table| {
            path.slot()
                .and_then(|slot| table.paths.get(slot))
                .map(|path| path.as_ref().to_owned())
                .unwrap_or_default()
        })
    }

    /// The parameter signature's text, for diagnostics and export payload.
    pub fn callable_parameters_text(&self, parameters: CallableParametersId) -> String {
        self.read_callable_identities(|table| {
            parameters
                .slot()
                .and_then(|slot| table.parameters.get(slot))
                .map(|parameters| parameters.as_ref().to_owned())
                .unwrap_or_default()
        })
    }

    /// The dispatch set's text, for diagnostics. An empty set renders empty;
    /// callers that distinguish it consult
    /// [`TypedTrees::callable_result_dispatch_is_empty`].
    pub fn callable_result_dispatch_text(&self, dispatch: CallableResultDispatchId) -> String {
        self.read_callable_identities(|table| {
            dispatch
                .slot()
                .and_then(|slot| table.result_dispatches.get(slot))
                .map(NormalizedResultDispatchSet::identity)
                .unwrap_or_default()
        })
    }

    /// Whether one interned dispatch set selects no domains. The zero handle
    /// names no set and is also empty.
    pub fn callable_result_dispatch_is_empty(&self, dispatch: CallableResultDispatchId) -> bool {
        self.read_callable_identities(|table| {
            dispatch
                .slot()
                .and_then(|slot| table.result_dispatches.get(slot))
                .is_none_or(NormalizedResultDispatchSet::is_empty)
        })
    }

    /// Intern one declaration path spelled in source, so a call target's
    /// authored name compares against declared paths by handle. Interning on
    /// demand keeps the comparison independent of which declarations have
    /// been consulted so far.
    pub fn callable_path_by_text(&self, path: &str) -> CallablePathId {
        self.callable_identities
            .with_mut(|table| table.intern_path(path))
    }

    /// Intern the dispatch-bearing result set a call site requests, so
    /// overload selection compares handles rather than domain-term vectors.
    pub fn callable_result_dispatch_identity(
        &self,
        result_dispatch: &NormalizedResultDispatchSet,
    ) -> CallableResultDispatchId {
        self.callable_identities
            .with_mut(|table| table.intern_result_dispatch(result_dispatch.clone()))
    }

    /// Rejoin one identity carried in as text by a plan, review record or
    /// artifact. The zero handle means no declaration in this program renders
    /// that identity.
    pub fn callable_identity_by_text(&self, text: &str) -> CallableIdentityId {
        self.sweep_callable_identities();
        self.read_callable_memo(|memo| memo.by_text.get(text).copied().unwrap_or_default())
    }

    /// Resolve one identity within an explicit declaration frontier. Build and
    /// Product copies can share an identity without being interchangeable.
    /// Missing or repeated matches inside this frontier resolve to nothing.
    pub fn machine_by_callable_identity(
        &self,
        identity: CallableIdentityId,
        declarations: &crate::references::DeclarationSelection,
    ) -> Option<&Machine> {
        if !identity.is_valid() {
            return None;
        }
        self.sweep_callable_identities();
        let position = self.read_callable_memo(|memo| {
            let mut positions =
                memo.machines
                    .get(identity.slot()?)?
                    .iter()
                    .copied()
                    .filter(|position| {
                        self.machines()
                            .get(*position as usize)
                            .is_some_and(|machine| declarations.contains(machine.symbol))
                    });
            let position = positions.next()?;
            positions.next().is_none().then_some(position)
        })?;
        self.machines().get(position as usize)
    }

    fn read_callable_identities<R>(&self, read: impl FnOnce(&CallableIdentityTable) -> R) -> R {
        self.callable_identities.with(read)
    }

    fn read_callable_memo<R>(&self, read: impl FnOnce(&CallableIdentityMemo) -> R) -> R {
        self.callable_identity_memo.with(
            self.structural_fingerprint(),
            CallableIdentityMemo::default,
            read,
        )
    }

    /// Intern every non-target machine and boundary-operator requirement, so
    /// reverse joins answer for the current program. Text rehydration and
    /// machine lookup need this roster; ordinary queries intern one key at a time.
    fn sweep_callable_identities(&self) {
        let revision = self.structural_fingerprint();
        if self.read_callable_memo(|memo| memo.swept) {
            return;
        }
        // Derivation walks type references and symbol paths and therefore
        // runs before the lock is taken.
        let mut derived = Vec::with_capacity(self.machines().len());
        for (position, machine) in self.machines().iter().enumerate() {
            if machine.target.is_some() {
                continue;
            }
            let Some(parts) = self.machine_callable_identity_parts(machine) else {
                continue;
            };
            derived.push((
                position as u32,
                DeclarationKey::Machine(machine as *const Machine as usize),
                parts,
            ));
        }
        let boundary_operators = self
            .all_operators()
            .filter(|operator| operator.is_boundary && self.machine(operator.symbol).is_none())
            .filter_map(|operator| {
                Some((
                    DeclarationKey::BoundaryOperatorRequirement(
                        operator as *const OperatorDefinition as usize,
                    ),
                    self.boundary_operator_identity_parts(operator)?,
                ))
            })
            .collect::<Vec<_>>();
        let (machines, boundary_operators) = self.callable_identities.with_mut(|table| {
            let machines = derived
                .into_iter()
                .map(|(position, declaration, parts)| {
                    let identity = table.intern(parts);
                    let text = table
                        .record(identity)
                        .expect("interned callable record")
                        .text
                        .clone();
                    (position, declaration, identity, text)
                })
                .collect::<Vec<_>>();
            let boundary_operators = boundary_operators
                .into_iter()
                .map(|(declaration, parts)| {
                    let identity = table.intern_boundary_operator(parts);
                    let text = table
                        .record(identity)
                        .expect("interned callable record")
                        .text
                        .clone();
                    (declaration, identity, text)
                })
                .collect::<Vec<_>>();
            (machines, boundary_operators)
        });
        self.callable_identity_memo
            .with_mut(revision, CallableIdentityMemo::default, |memo| {
                if memo.swept {
                    return;
                }
                for (position, declaration, identity, text) in machines {
                    memo.by_declaration.insert(declaration, identity);
                    memo.by_text.insert(text, identity);
                    let Some(slot) = identity.slot() else {
                        continue;
                    };
                    memo.machines
                        .resize_with(memo.machines.len().max(slot + 1), Vec::new);
                    let positions = &mut memo.machines[slot];
                    if !positions.contains(&position) {
                        positions.push(position);
                    }
                }
                for (declaration, identity, text) in boundary_operators {
                    memo.by_declaration.insert(declaration, identity);
                    memo.by_text.insert(text, identity);
                }
                memo.swept = true;
            });
    }
}
