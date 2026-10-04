//! The program-owned interner for named-callable identities.
//!
//! A named machine, a trait machine requirement and an explicitly named
//! operator are each selected by three parts: the declaration path, the
//! parameter signature, and the exact set of dispatch-bearing result
//! domains. Those parts used to be handed to every consult site as rendered,
//! escaped `String`s, and the reverse join — which machine carries this
//! identity — rendered every machine in the program and compared the text.
//! This module interns each part once per program revision and hands out
//! handles instead: an overload comparison is an integer compare, and the
//! reverse join is one map read.
//!
//! Text survives as a projection, not as the identity. A provider plan, a
//! package review record and a native artifact each carry an identity across
//! a serialization boundary and rejoin it in a later compilation, where no
//! handle minted here means anything; `callable_identity_text` renders the
//! retained record for exactly those encodings and for diagnostics.
//! `callable_identity_by_text` is the matching rehydration join and is the
//! only entry that accepts text.
//!
//! Freshness: the interner lives inside the program it describes and is
//! guarded by `TypedTrees::structural_fingerprint()`, so a monomorphization
//! append or a re-pointed root span drops every memoized identity instead of
//! answering from a shape the program no longer has. Identity derivation
//! walks type references and symbol paths, which consult other program-owned
//! indexes, so every rendering happens outside this interner's lock.

use super::NormalizedResultDispatchSet;
use super::constraint_identity::{atom, compound};
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CallableIdentityId(u32);

/// One interned declaration path, shared by every overload of a family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CallablePathId(u32);

/// One interned parameter signature. Two identities share it exactly when
/// they are the same overload family under result-domain dispatch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CallableParametersId(u32);

/// One interned dispatch-bearing result-domain set. Call sites intern the set
/// their destination requests through
/// [`TypedTrees::callable_result_dispatch_identity`] and then select an
/// overload by comparing handles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
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

/// Which declaration a memoized identity was derived for. The address is the
/// exact key: a declaration's roster position is not unique under the forged
/// duplicate symbols fixtures can declare, and the interner's revision guard
/// bounds the address's lifetime to the table state it was taken from — the
/// same rule the isolation verdicts already key on.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum DeclarationKey {
    Machine(usize),
    TraitRequirement(usize, usize),
    Operator(usize),
}

#[derive(Clone)]
struct CallableIdentityRecord {
    path: CallablePathId,
    parameters: CallableParametersId,
    result_dispatch: CallableResultDispatchId,
    /// The escaped `named-callable(path(..),parameters(..),result-dispatch(..))`
    /// rendering, retained so diagnostics and artifact encoding project it
    /// rather than re-deriving it from the declaration.
    text: Box<str>,
    /// Machine roster positions declaring this identity, excluding target
    /// siblings: a target body shares its family's identity by design and is
    /// never the realized declaration. Filled only by the complete sweep, so
    /// `swept` says whether this list is the whole program's answer.
    machines: Vec<u32>,
}

/// The interned identities of one program. Records are append-only within a
/// revision, so a handle stays valid for as long as the table it came from.
#[derive(Clone, Default)]
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
    by_text: HashMap<Box<str>, CallableIdentityId>,
    by_declaration: HashMap<DeclarationKey, CallableIdentityId>,
    /// Whether every non-target machine in the program has been interned.
    /// The reverse joins (`by_text`, each record's `machines`) answer for the
    /// whole program only after that sweep.
    swept: bool,
}

impl CallableIdentityTable {
    fn intern_path(&mut self, path: &str) -> CallablePathId {
        if let Some(interned) = self.path_slots.get(path) {
            return *interned;
        }
        let interned = CallablePathId::from_slot(self.paths.len());
        let path: Box<str> = path.into();
        self.paths.push(path.clone());
        self.path_slots.insert(path, interned);
        interned
    }

    fn intern_parameters(&mut self, parameters: &str) -> CallableParametersId {
        if let Some(interned) = self.parameter_slots.get(parameters) {
            return *interned;
        }
        let interned = CallableParametersId::from_slot(self.parameters.len());
        let parameters: Box<str> = parameters.into();
        self.parameters.push(parameters.clone());
        self.parameter_slots.insert(parameters, interned);
        interned
    }

    fn intern_result_dispatch(
        &mut self,
        result_dispatch: NormalizedResultDispatchSet,
    ) -> CallableResultDispatchId {
        if let Some(interned) = self.result_dispatch_slots.get(&result_dispatch) {
            return *interned;
        }
        let interned = CallableResultDispatchId::from_slot(self.result_dispatches.len());
        self.result_dispatches.push(result_dispatch.clone());
        self.result_dispatch_slots.insert(result_dispatch, interned);
        interned
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
            text: text.clone(),
            machines: Vec::new(),
        });
        self.record_slots.insert(slot, interned);
        self.by_text.insert(text, interned);
        interned
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

    fn interned_callable_identity(
        &self,
        declaration: DeclarationKey,
        derive: impl FnOnce() -> Option<CallableIdentityParts>,
    ) -> CallableIdentityId {
        let revision = self.structural_fingerprint();
        // A memoized absence is stored as the zero handle, so a hit answers
        // both questions and only a miss pays for the derivation.
        if let Some(interned) =
            self.callable_identities
                .with_mut(revision, CallableIdentityTable::default, |table| {
                    table.by_declaration.get(&declaration).copied()
                })
        {
            return interned;
        }
        let parts = derive();
        self.callable_identities
            .with_mut(revision, CallableIdentityTable::default, |table| {
                let interned = parts.map(|parts| table.intern(parts)).unwrap_or_default();
                table.by_declaration.insert(declaration, interned);
                interned
            })
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
        self.callable_identities.with_mut(
            self.structural_fingerprint(),
            CallableIdentityTable::default,
            |table| table.intern_path(path),
        )
    }

    /// Intern the dispatch-bearing result set a call site requests, so
    /// overload selection compares handles rather than domain-term vectors.
    pub fn callable_result_dispatch_identity(
        &self,
        result_dispatch: &NormalizedResultDispatchSet,
    ) -> CallableResultDispatchId {
        self.callable_identities.with_mut(
            self.structural_fingerprint(),
            CallableIdentityTable::default,
            |table| table.intern_result_dispatch(result_dispatch.clone()),
        )
    }

    /// Rejoin one identity carried in as text by a plan, review record or
    /// artifact. The zero handle means no declaration in this program renders
    /// that identity.
    pub fn callable_identity_by_text(&self, text: &str) -> CallableIdentityId {
        self.sweep_callable_identities();
        self.read_callable_identities(|table| table.by_text.get(text).copied().unwrap_or_default())
    }

    /// Resolve one identity to its uniquely declaring machine. A missing or
    /// duplicated identity resolves to nothing; callers must not fall back to
    /// a short machine spelling.
    pub fn machine_by_callable_identity(&self, identity: CallableIdentityId) -> Option<&Machine> {
        if !identity.is_valid() {
            return None;
        }
        self.sweep_callable_identities();
        let position = self.read_callable_identities(|table| {
            match table
                .record(identity)
                .map(|record| record.machines.as_slice())
            {
                Some([position]) => Some(*position),
                _ => None,
            }
        })?;
        self.machines().get(position as usize)
    }

    fn read_callable_identities<R>(&self, read: impl FnOnce(&CallableIdentityTable) -> R) -> R {
        self.callable_identities.with(
            self.structural_fingerprint(),
            CallableIdentityTable::default,
            read,
        )
    }

    /// Intern every non-target machine's identity, so the reverse joins
    /// answer for the whole program. Only the text rehydration path needs
    /// it: ordinary consults mint one record at a time.
    fn sweep_callable_identities(&self) {
        let revision = self.structural_fingerprint();
        if self.read_callable_identities(|table| table.swept) {
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
        self.callable_identities
            .with_mut(revision, CallableIdentityTable::default, |table| {
                if table.swept {
                    return;
                }
                for (position, declaration, parts) in derived {
                    let interned = table.intern(parts);
                    table.by_declaration.insert(declaration, interned);
                    let Some(slot) = interned.slot() else {
                        continue;
                    };
                    let positions = &mut table.records[slot].machines;
                    if !positions.contains(&position) {
                        positions.push(position);
                    }
                }
                table.swept = true;
            });
    }
}
