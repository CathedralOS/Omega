//! Reads the source-owned inputs of a logical invocation witness. Scalar leaves
//! here are mathematical snapshots, including fields of linear data, never an
//! executable representation of that data. Establishment-restricted domains
//! keep their nominal premise; every domain keeps its complete declared theory.
//! Unread theory refuses the witness before proof search, so failure under an
//! approximation cannot reject source.

use arena::{Arena, HandleSpan};
use semantic_vocabulary::{
    ContentPlaceVersion, IdTable, MachineId, Proposition, PropositionId, ScalarTerm, ScalarType,
    ValueId,
};
use symbols::{RetainedDeclaration, SymbolHandle, SymbolPath};
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use typed_trees::data::DataMember;
use typed_trees::domain::{ProofFact, ProofMembershipFact};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};
use typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use typed_trees::{
    TypedTrees,
    callable_signature_shapes::{CallableAddressModel, ParameterOccurrence},
};

use super::{
    Program,
    contracts::{ClauseScope, ResultTerm},
    span,
};
use crate::body::{
    Body, CrashCeiling, Expression, ExpressionKind, Qualification, Signature, Termination,
    ValueType,
};
use crate::{
    CallableArrivalWitness, CallableInputOrigin, MachineNames, NominalPremiseOrigin, SourceOrigin,
    Unserved, refuse,
};

fn identity(machine: MachineId, detail: &str) -> Refusal {
    Refusal {
        reason: RefusalReason::ForeignIdentity,
        machine,
        concerns: Construct::Machine,
        source_span: None,
        detail: detail.to_owned(),
    }
}

// Only a local prepared slice's ordering, as in typed_trees::references.
// The rows retain exact generational handles; coordinates never replace their
// identities, become a map key, or cross the proof/source boundary.
fn symbol_order(left: SymbolHandle, right: SymbolHandle) -> std::cmp::Ordering {
    (left.arena_index(), left.generation()).cmp(&(right.arena_index(), right.generation()))
}

fn field_order(left: &[SymbolHandle], right: &[SymbolHandle]) -> std::cmp::Ordering {
    left.iter()
        .map(|symbol| (symbol.arena_index(), symbol.generation()))
        .cmp(
            right
                .iter()
                .map(|symbol| (symbol.arena_index(), symbol.generation())),
        )
}

// These spans belong to this witness's explicit member arena, not to the
// borrowed SymbolTable. Every stored span is produced by insert_many below.
fn symbol_members(arena: &Arena<SymbolHandle>, path: SymbolPath) -> &[SymbolHandle] {
    arena
        .span(path.members)
        .expect("constructed witness symbol members")
}

fn path_order(
    arena: &Arena<SymbolHandle>,
    left: SymbolPath,
    right: SymbolPath,
) -> std::cmp::Ordering {
    symbol_order(left.root, right.root)
        .then_with(|| field_order(symbol_members(arena, left), symbol_members(arena, right)))
}

struct Leaf {
    path: SymbolPath,
    scalar: ScalarType,
    origin: usize,
    /// The first reference crossed by this source path. Retain its complete
    /// access/referee type instead of treating its contents as an owned value.
    reference: Option<TypeReferenceHandle>,
}

struct Theory {
    subject: SymbolPath,
    symbol: SymbolHandle,
    facts: HandleSpan<ProofFact>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NominalKey {
    formal: usize,
    fields: Vec<SymbolHandle>,
    domain: SymbolHandle,
    application: language_semantics::SemanticDomainId,
}

impl NominalKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.formal
            .cmp(&other.formal)
            .then_with(|| field_order(&self.fields, &other.fields))
            .then_with(|| symbol_order(self.domain, other.domain))
            .then_with(|| self.application.0.cmp(&other.application.0))
    }
}

struct Membership {
    nominal: bool,
    key: NominalKey,
    domain: DomainConstraint,
    subject: SymbolPath,
    reference: Option<TypeReferenceHandle>,
}

#[derive(Default)]
struct Inputs {
    leaves: Vec<Leaf>,
    lookup: Vec<usize>,
    theory: Vec<Theory>,
    memberships: Vec<Membership>,
    membership_lookup: Vec<usize>,
    origins: Vec<CallableInputOrigin>,
}

pub(super) struct MathematicalInputs {
    inputs: Inputs,
    members: Arena<SymbolHandle>,
}

impl MathematicalInputs {
    pub(super) fn new(
        typed: &TypedTrees,
        machine: MachineId,
        parameters: &[typed_trees::mathematical::MathematicalParameter],
        model: Option<&CallableAddressModel>,
    ) -> Checked<Self> {
        let mut inputs = Inputs::default();
        let member_arena = Arena::new();
        for (ordinal, parameter) in parameters.iter().enumerate() {
            if !typed.symbols.symbols().nodes().is_valid(parameter.symbol)
                || !typed.mathematical_types.is_valid(parameter.ty)
            {
                return Err(identity(
                    machine,
                    "mathematical formal has no exact declaration/type",
                ));
            }
            if parameter.relevance != language_core::BindingRelevance::Relevant {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Parameter(
                        "erased mathematical formal without its evidence interpretation",
                    ),
                ));
            }
            let typed_trees::mathematical::MathematicalType::Ordinary(reference) =
                typed.mathematical_type(parameter.ty)
            else {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Contract("higher-order mathematical premise input"),
                ));
            };
            inputs.leaves.push(Leaf {
                path: SymbolPath {
                    root: parameter.symbol,
                    members: HandleSpan::empty(),
                },
                scalar: scalar_type(typed, machine, *reference, model)?,
                origin: ordinal,
                reference: None,
            });
        }
        inputs.lookup = (0..inputs.leaves.len()).collect();
        inputs.lookup.sort_unstable_by(|left, right| {
            path_order(
                &member_arena,
                inputs.leaves[*left].path,
                inputs.leaves[*right].path,
            )
        });
        if inputs.lookup.windows(2).any(|pair| {
            path_order(
                &member_arena,
                inputs.leaves[pair[0]].path,
                inputs.leaves[pair[1]].path,
            )
            .is_eq()
        }) {
            return Err(identity(
                machine,
                "mathematical declaration repeats a formal identity",
            ));
        }
        Ok(Self {
            inputs,
            members: member_arena,
        })
    }

    pub(super) fn empty() -> Self {
        Self {
            inputs: Inputs::default(),
            members: Arena::new(),
        }
    }

    pub(super) fn scope<'a>(
        &'a self,
        machine: MachineId,
        model: Option<&'a CallableAddressModel>,
        expanding: &'a [SymbolHandle],
    ) -> LogicalScope<'a> {
        LogicalScope {
            inputs: &self.inputs,
            members: &self.members,
            subject: None,
            model,
            expanding,
            machine,
            observation: ContentPlaceVersion::Entry,
            atoms: &[],
        }
    }

    pub(super) fn validate(&self, machine: MachineId, proposition: &Proposition) -> Checked<()> {
        let context = semantic_vocabulary::PropositionContext::from_value_types(
            self.inputs
                .leaves
                .iter()
                .enumerate()
                .map(|(position, leaf)| {
                    (
                        ValueId::new(position as u64 + 1).expect("allocated formal count"),
                        leaf.scalar,
                    )
                }),
        )
        .map_err(|error| identity(machine, &format!("mathematical formal context: {error:?}")))?;
        context.validate(proposition).map_err(|error| {
            identity(
                machine,
                &format!("mathematical predicate formation: {error:?}"),
            )
        })
    }
}

#[derive(Clone, Copy)]
pub(super) struct LogicalScope<'a> {
    inputs: &'a Inputs,
    members: &'a Arena<SymbolHandle>,
    subject: Option<(SymbolHandle, &'a SymbolPath)>,
    pub(super) model: Option<&'a CallableAddressModel>,
    pub(super) expanding: &'a [SymbolHandle],
    machine: MachineId,
    observation: ContentPlaceVersion,
    atoms: &'a [NominalKey],
}

// The scratch is a query over an AST occurrence, never a retained place key.
fn source_fields(
    typed: &TypedTrees,
    expression: ExpressionHandle,
    fields: &mut Vec<SymbolHandle>,
) -> Option<SymbolHandle> {
    if !typed.expression_table.expression_is_valid(expression) {
        return None;
    }
    match typed.expression_table.expression(expression) {
        ExpressionNode::Name(name)
            if name.symbol.is_valid()
                && !name.reserved_result_of.is_valid()
                && !name.static_application.is_valid() =>
        {
            Some(name.symbol)
        }
        ExpressionNode::Member(member) if member.selection.field_symbol().is_valid() => {
            let root = source_fields(typed, member.receiver, fields)?;
            fields.push(member.selection.field_symbol());
            Some(root)
        }
        _ => None,
    }
}

impl LogicalScope<'_> {
    pub(super) fn normal_return(self) -> Self {
        Self {
            observation: ContentPlaceVersion::Current,
            ..self
        }
    }

    pub(super) fn membership_clause(
        &self,
        program: &mut Program<'_>,
        scope: &ClauseScope<'_>,
        fact: &ProofMembershipFact,
    ) -> Checked<(Proposition, crate::ClauseText)> {
        self.inputs
            .membership_clause(program, self.machine, scope, fact, self.atoms)
    }

    fn subject_root(&self, root: SymbolHandle) -> SymbolPath {
        self.subject.filter(|(subject, _)| *subject == root).map_or(
            SymbolPath {
                root,
                members: HandleSpan::empty(),
            },
            |(_, actual)| *actual,
        )
    }

    pub(super) fn term(
        &self,
        typed: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Checked<Option<(ScalarTerm, ScalarType)>> {
        let Some(position) = self.source_leaf(typed, expression)? else {
            return Ok(None);
        };
        let leaf = &self.inputs.leaves[position];
        if self.observation == ContentPlaceVersion::Current && leaf.reference.is_some() {
            return Err(refuse(
                self.machine,
                Construct::Machine,
                Unserved::Contract(
                    "logical guarantee needs its normal-return reference observation",
                ),
            ));
        }
        self.input_term(position).map(Some)
    }

    pub(super) fn old_term(
        &self,
        typed: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Checked<Option<(ScalarTerm, ScalarType)>> {
        let Some(position) = self.source_leaf(typed, expression)? else {
            return Ok(None);
        };
        if self.inputs.leaves[position].reference.is_some() {
            // Selecting the entry version cannot establish that referenced
            // storage had a formed, readable value. No such receipt is present
            // in this mathematical projection; the ordinary owner must supply it.
            return Err(refuse(
                self.machine,
                Construct::Machine,
                Unserved::Contract("logical old needs ordinary reference observation formation"),
            ));
        }
        self.input_term(position).map(Some)
    }

    fn source_leaf(
        &self,
        typed: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Checked<Option<usize>> {
        let mut suffix = Vec::new();
        let Some(root) = source_fields(typed, expression, &mut suffix) else {
            return Ok(None);
        };
        let root = self.subject_root(root);
        let prefix = span(
            self.members,
            root.members,
            self.machine,
            "logical source subject members",
        )?;
        let Ok(found) = self.inputs.lookup.binary_search_by(|position| {
            let stored = self.inputs.leaves[*position].path;
            symbol_order(stored.root, root.root).then_with(|| {
                symbol_members(self.members, stored)
                    .iter()
                    .map(|s| (s.arena_index(), s.generation()))
                    .cmp(
                        prefix
                            .iter()
                            .chain(&suffix)
                            .map(|s| (s.arena_index(), s.generation())),
                    )
            })
        }) else {
            return Ok(None);
        };
        Ok(Some(self.inputs.lookup[found]))
    }

    fn input_term(&self, position: usize) -> Checked<(ScalarTerm, ScalarType)> {
        let leaf = &self.inputs.leaves[position];
        let value = ValueId::new(position as u64 + 1).ok_or_else(|| {
            refuse(
                self.machine,
                Construct::Machine,
                Unserved::Contract("logical value identity overflow"),
            )
        })?;
        Ok((ScalarTerm::value(value, leaf.scalar), leaf.scalar))
    }

    pub(super) fn signature(
        self,
        program: &mut Program<'_>,
        owner: SymbolHandle,
        contracts: &[&typed_trees::signature::SignatureContract],
    ) -> Checked<(Signature, MachineNames)> {
        let logical = self;
        let atoms = logical.atoms;
        let inputs = logical.inputs;
        let machine = logical.machine;
        // Validate every retained span at the proof boundary. Content-based
        // comparisons never treat equal span coordinates as semantic identity.
        for path in inputs
            .leaves
            .iter()
            .map(|leaf| leaf.path)
            .chain(inputs.theory.iter().map(|theory| theory.subject))
            .chain(
                inputs
                    .memberships
                    .iter()
                    .map(|membership| membership.subject),
            )
        {
            span(
                logical.members,
                path.members,
                machine,
                "logical input source members",
            )?;
        }
        let scope = ClauseScope {
            slots: &[],
            values: &[],
            bound: &[],
            owner,
            result: ResultTerm::Absent("result of a Unit logical callable"),
            subject: None,
            formal_fields: true,
            mathematical: super::contracts::MathematicalTerms::Served,
            logical: Some(logical),
        };
        let mut requires = Vec::new();
        for membership in inputs
            .memberships
            .iter()
            .filter(|membership| membership.nominal)
        {
            let position = atoms
                .binary_search_by(|key| key.cmp(&membership.key))
                .map_err(|_| {
                    refuse(
                        machine,
                        Construct::Machine,
                        Unserved::Contract("logical membership correspondence"),
                    )
                })?;
            let atom = PropositionId::new(position as u64 + 1).ok_or_else(|| {
                refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Contract("logical membership identity overflow"),
                )
            })?;
            requires.push((
                Proposition::Atom(atom),
                crate::ClauseText(vec![crate::Piece::Text(format!(
                    "{} in {}",
                    program.typed.symbols.name(membership.subject.root),
                    membership.domain.name.as_str()
                ))]),
            ));
        }
        for theory in &inputs.theory {
            let facts = span(
                &program.typed.proof_facts,
                theory.facts,
                machine,
                "logical premise theory span",
            )?;
            let scope = ClauseScope {
                logical: Some(LogicalScope {
                    subject: Some((theory.symbol, &theory.subject)),
                    ..logical
                }),
                ..scope
            };
            for fact in facts {
                let expression = program.fact(machine, fact)?;
                requires.push((
                    program.clause(machine, &scope, expression)?,
                    program.clause_text(&[], expression),
                ));
            }
        }
        let exit = ClauseScope {
            logical: Some(logical.normal_return()),
            ..scope
        };
        let mut authored = Vec::new();
        let mut ensures = Vec::new();
        for contract in contracts {
            let (scope, clauses) = match contract.kind {
                SignatureContractKind::Requires => (&scope, &mut authored),
                SignatureContractKind::Ensures => (&exit, &mut ensures),
                SignatureContractKind::EnsuresForResultCase { .. } => {
                    return Err(refuse(
                        machine,
                        Construct::Machine,
                        Unserved::Contract("outcome-specific logical guarantee"),
                    ));
                }
                // Operational admission is retained by the executable closure.
                SignatureContractKind::Crashes { .. } => continue,
            };
            if contract.binding.is_some() {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Contract("unsupplied logical evidence"),
                ));
            }
            for fact in span(
                &program.typed.proof_facts,
                contract.facts,
                machine,
                "logical contract fact span",
            )? {
                let requirement = contract.kind == SignatureContractKind::Requires;
                if let Some(clause) = program.contract_clause(machine, scope, fact, requirement)? {
                    clauses.push(clause);
                }
            }
        }
        requires.extend(crate::canonical::associated_clauses(
            authored,
            crate::canonical::Repeated::Refuse,
            machine,
        )?);
        let (requires, requires_text) = crate::canonical::associated_clauses(
            requires,
            crate::canonical::Repeated::Merge,
            machine,
        )?
        .into_iter()
        .unzip();
        let (ensures, ensures_text) = crate::canonical::associated_clauses(
            ensures,
            crate::canonical::Repeated::Refuse,
            machine,
        )?
        .into_iter()
        .unzip();
        let names = MachineNames {
            machine: program.typed.symbols.display_path(owner, "::"),
            formals: inputs
                .leaves
                .iter()
                .map(|leaf| program.typed.symbols.display_path(leaf.path.root, "::"))
                .collect(),
            requires: requires_text,
            ensures: ensures_text,
            ..MachineNames::default()
        };
        Ok((
            Signature {
                // These are mathematical subjects, never executable storage.
                // Reference poststate reads refuse at their source occurrence;
                // the executable closure retains operational admission.
                operational: crate::body::OperationalCeilings::default(),
                parameters: inputs
                    .leaves
                    .iter()
                    .map(|leaf| ValueType::Scalar(leaf.scalar, Qualification::NONE))
                    .collect(),
                erased: Vec::new(),
                borrowed: Vec::new(),
                result: None,
                requires,
                ensures,
                crashes: CrashCeiling::Inferred,
                termination: Termination::Silent,
                ranking: None,
                reaches: crate::Ceiling::Inferred,
                invokes: crate::Ceiling::Inferred,
            },
            names,
        ))
    }
}

pub(super) fn scalar_type(
    typed: &TypedTrees,
    machine: MachineId,
    reference: TypeReferenceHandle,
    model: Option<&CallableAddressModel>,
) -> Checked<ScalarType> {
    if !typed
        .type_reference_table
        .contains_type_reference(reference)
    {
        return Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("logical input without an exact type"),
        ));
    }
    let TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(reference)
    else {
        return Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("non-scalar mathematical formal"),
        ));
    };
    if !typed.symbols.symbols().nodes().is_valid(*symbol) {
        return Err(identity(
            machine,
            "mathematical input carrier has no exact declaration",
        ));
    }
    let atom = typed.symbols.builtin_type_atom(*symbol).ok_or_else(|| {
        refuse(
            machine,
            Construct::Machine,
            Unserved::Type("non-builtin mathematical formal"),
        )
    })?;
    if atom == symbols::BuiltinTypeAtom::Address {
        let model = model
            .filter(|model| {
                model.carrier.is_address()
                    && model.semantics_version != 0
                    && model.commitment != [0; 32]
            })
            .ok_or_else(|| {
                refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("address without its binding's mathematical model"),
                )
            })?;
        return Ok(ScalarType::Integer(model.carrier.as_checking()));
    }
    super::types::atom_type(atom, machine)
}

impl Inputs {
    fn prepare(
        typed: &TypedTrees,
        machine: MachineId,
        parameters: &[StateParameter],
        origins: &[usize],
        model: Option<&CallableAddressModel>,
        member_arena: &mut Arena<SymbolHandle>,
    ) -> Checked<Self> {
        if parameters.len() != origins.len() {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Contract("logical input projection arity"),
            ));
        }
        let mut result = Self::default();
        for (ordinal, (parameter, origin)) in parameters.iter().zip(origins).enumerate() {
            if !typed.symbols.symbols().nodes().is_valid(parameter.symbol) {
                return Err(identity(
                    machine,
                    "logical parameter without an exact declaration",
                ));
            }
            if parameter.relevance != language_core::BindingRelevance::Relevant {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Parameter("erased logical input without supplied evidence"),
                ));
            }
            result.origins.push(CallableInputOrigin {
                occurrence: ParameterOccurrence {
                    ordinal,
                    symbol: parameter.symbol,
                },
                projected_from: *origin,
            });
            let _ = result.input_type(
                typed,
                machine,
                parameter.type_reference,
                SymbolPath {
                    root: parameter.symbol,
                    members: HandleSpan::empty(),
                },
                *origin,
                None,
                model,
                member_arena,
                &mut Vec::new(),
            )?;
        }
        result.membership_lookup = (0..result.memberships.len()).collect();
        result.membership_lookup.sort_unstable_by(|left, right| {
            let left = &result.memberships[*left];
            let right = &result.memberships[*right];
            path_order(member_arena, left.subject, right.subject)
                .then_with(|| symbol_order(left.domain.symbol, right.domain.symbol))
                .then_with(|| left.domain.semantic_id.0.cmp(&right.domain.semantic_id.0))
        });
        result.lookup = (0..result.leaves.len()).collect();
        result.lookup.sort_unstable_by(|left, right| {
            path_order(
                member_arena,
                result.leaves[*left].path,
                result.leaves[*right].path,
            )
        });
        if result.lookup.windows(2).any(|pair| {
            path_order(
                member_arena,
                result.leaves[pair[0]].path,
                result.leaves[pair[1]].path,
            )
            .is_eq()
        }) {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Contract("duplicate logical input occurrence"),
            ));
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn input_type(
        &mut self,
        typed: &TypedTrees,
        machine: MachineId,
        reference: TypeReferenceHandle,
        subject: SymbolPath,
        origin: usize,
        observed_through: Option<TypeReferenceHandle>,
        model: Option<&CallableAddressModel>,
        member_arena: &mut Arena<SymbolHandle>,
        expanding: &mut Vec<SymbolHandle>,
    ) -> Checked<Option<TypeReferenceHandle>> {
        if !typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("logical input without an exact type"),
            ));
        }
        if let TypeReferenceNode::Named { symbol, .. } =
            typed.type_reference_table.type_reference(reference)
            && !typed.symbols.symbols().nodes().is_valid(*symbol)
        {
            return Err(identity(
                machine,
                "logical input names an unresolved declaration",
            ));
        }
        let observed_through = match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let observed_through = self.input_type(
                    typed,
                    machine,
                    *base_type,
                    subject,
                    origin,
                    observed_through,
                    model,
                    member_arena,
                    expanding,
                )?;
                let declared = typed.type_reference_table.constraints(*constraints);
                if declared.len() != constraints.len() {
                    return Err(refuse(
                        machine,
                        Construct::Machine,
                        Unserved::Type("logical constraint span"),
                    ));
                }
                for constraint in declared {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        return Err(refuse(
                            machine,
                            Construct::Machine,
                            Unserved::Type("logical constraint theory"),
                        ));
                    };
                    if domain.subject != typed_trees::types::DomainConstraintSubject::Declared {
                        return Err(refuse(
                            machine,
                            Construct::Machine,
                            Unserved::Contract(
                                "non-declaration input qualification needs its own logical theory",
                            ),
                        ));
                    }
                    let definition = typed
                        .domain_definition_for_instance(domain.symbol, domain.semantic_id)
                        .ok_or_else(|| {
                            refuse(
                                machine,
                                Construct::Machine,
                                Unserved::Contract("logical domain without its declaration"),
                            )
                        })?;
                    if definition.alias.is_some()
                        || !definition.type_parameters.is_empty()
                        || !definition.index_arguments.is_empty()
                    {
                        return Err(refuse(
                            machine,
                            Construct::Machine,
                            Unserved::Contract("applied or aliased logical domain theory"),
                        ));
                    }
                    if definition.semantic_id == language_semantics::SemanticDomainId::NULL
                        || definition.semantic_id != domain.semantic_id
                    {
                        return Err(identity(
                            machine,
                            "logical domain application differs from its exact declaration",
                        ));
                    }
                    let nominal = match definition.issuance() {
                        typed_trees::domain::DomainIssuance::Unrestricted => false,
                        typed_trees::domain::DomainIssuance::Only(_) => true,
                        typed_trees::domain::DomainIssuance::Planned(_) => {
                            return Err(refuse(
                                machine,
                                Construct::Machine,
                                Unserved::Contract(
                                    "logical premise needs its complete domain access plan",
                                ),
                            ));
                        }
                    };
                    // Pure predicate domains have no additional nominal fact.
                    // Extra classification, operator or cleanup theory cannot
                    // be silently approximated by their predicate clauses.
                    if !nominal
                        && (definition.classification.is_some()
                            || !definition.semantic_roles.is_empty()
                            || definition.cleanup_selection.is_some()
                            || !definition.operators.is_empty())
                    {
                        return Err(refuse(
                            machine,
                            Construct::Machine,
                            Unserved::Contract(
                                "logical domain membership has unavailable non-predicate theory",
                            ),
                        ));
                    }
                    self.memberships.push(Membership {
                        nominal,
                        key: NominalKey {
                            formal: origin,
                            fields: symbol_members(member_arena, subject).to_vec(),
                            domain: definition.symbol,
                            application: domain.semantic_id,
                        },
                        domain: domain.clone(),
                        subject,
                        reference: observed_through,
                    });
                    let TypeReferenceNode::Named { symbol, .. } = typed
                        .type_reference_table
                        .type_reference(definition.target_type)
                    else {
                        return Err(refuse(
                            machine,
                            Construct::Machine,
                            Unserved::Contract(
                                "logical domain carrier has unavailable inherited constraints",
                            ),
                        ));
                    };
                    if typed.symbols.builtin_type_atom(*symbol).is_none()
                        && typed.data_definition_by_symbol(*symbol).is_none()
                    {
                        return Err(refuse(
                            machine,
                            Construct::Machine,
                            Unserved::Contract(
                                "logical domain carrier needs its alias constraints",
                            ),
                        ));
                    }
                    self.theory.push(Theory {
                        subject,
                        symbol: definition.subject_symbol,
                        facts: definition.facts,
                    });
                }
                observed_through
            }
            TypeReferenceNode::Reference {
                access: language_core::ReferenceAccess::WriteOnly,
                ..
            } => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Parameter("write-only input has no readable entry snapshot"),
                ));
            }
            TypeReferenceNode::Reference { referee, .. } => self.input_type(
                typed,
                machine,
                *referee,
                subject,
                origin,
                observed_through.or(Some(reference)),
                model,
                member_arena,
                expanding,
            )?,
            TypeReferenceNode::Named { symbol, .. }
                if typed.symbols.builtin_type_atom(*symbol).is_some() =>
            {
                self.leaves.push(Leaf {
                    path: subject,
                    scalar: scalar_type(typed, machine, reference, model)?,
                    origin,
                    reference: observed_through,
                });
                observed_through
            }
            TypeReferenceNode::Named { symbol, .. } => {
                let declaration = typed.data_definition_by_symbol(*symbol).ok_or_else(|| {
                    refuse(
                        machine,
                        Construct::Machine,
                        Unserved::Type("logical input without a data declaration"),
                    )
                })?;
                if expanding.contains(symbol)
                    || declaration.quotient.is_some()
                    || !declaration.type_parameters.is_empty()
                    || !declaration.lifetime_parameters.is_empty()
                    || !declaration.conformance_bounds.is_empty()
                    || declaration.supply_mode != language_semantics::DataSupplyMode::CheckedShape
                {
                    return Err(refuse(
                        machine,
                        Construct::Machine,
                        Unserved::Type("recursive, open or quotient logical input"),
                    ));
                }
                let members = typed.data_members(declaration);
                if members.len() != declaration.members.len() {
                    return Err(refuse(
                        machine,
                        Construct::Machine,
                        Unserved::Type("logical data member span"),
                    ));
                }
                expanding.push(*symbol);
                for member in members {
                    let DataMember::Field(field) = member else {
                        return Err(refuse(
                            machine,
                            Construct::Machine,
                            Unserved::Type("logical case input"),
                        ));
                    };
                    if field.relevance != language_core::BindingRelevance::Relevant {
                        return Err(refuse(
                            machine,
                            Construct::Machine,
                            Unserved::Type("erased logical field"),
                        ));
                    }
                    if !typed.symbols.symbols().nodes().is_valid(field.symbol) {
                        return Err(identity(
                            machine,
                            "logical field without an exact declaration",
                        ));
                    }
                    let mut fields = symbol_members(member_arena, subject).to_vec();
                    fields.push(field.symbol);
                    let path = SymbolPath {
                        root: subject.root,
                        members: member_arena.insert_many(fields),
                    };
                    let _ = self.input_type(
                        typed,
                        machine,
                        field.type_reference,
                        path,
                        origin,
                        observed_through,
                        model,
                        member_arena,
                        expanding,
                    )?;
                }
                expanding.pop();
                self.theory.push(Theory {
                    subject,
                    symbol: declaration.predicate_subject_symbol,
                    facts: declaration.requires_facts,
                });
                observed_through
            }
            _ => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("logical input shape"),
                ));
            }
        };
        Ok(observed_through)
    }

    /// Membership facts emitted from parameter qualification retain the same
    /// exact input occurrence and semantic domain as the input type. Rejoin
    /// that source qualification rather than treating the membership spelling
    /// as an independent premise or dropping the generated contract clause.
    fn membership_clause(
        &self,
        program: &mut Program<'_>,
        machine: MachineId,
        scope: &ClauseScope<'_>,
        fact: &ProofMembershipFact,
        atoms: &[NominalKey],
    ) -> Checked<(Proposition, crate::ClauseText)> {
        let unserved = |what| refuse(machine, Construct::Machine, Unserved::Contract(what));
        let logical = scope
            .logical
            .ok_or_else(|| unserved("membership without logical inputs"))?;
        let mut suffix = Vec::new();
        let root = source_fields(program.typed, fact.value, &mut suffix)
            .ok_or_else(|| unserved("logical membership without an exact input path"))?;
        let root = logical.subject_root(root);
        let prefix = span(
            logical.members,
            root.members,
            machine,
            "logical membership subject members",
        )?;
        let fact_identity = if fact.semantic_domain.is_valid() {
            fact.semantic_domain
        } else {
            program
                .typed
                .domain_definition(fact.domain_symbol)
                .filter(|definition| program.typed.domain_type_parameters(definition).is_empty())
                .map(|definition| definition.semantic_id)
                .ok_or_else(|| unserved("logical membership lost its applied domain identity"))?
        };
        let position = self
            .membership_lookup
            .binary_search_by(|index| {
                let membership = &self.memberships[*index];
                symbol_order(membership.subject.root, root.root)
                    .then_with(|| {
                        symbol_members(logical.members, membership.subject)
                            .iter()
                            .map(|s| (s.arena_index(), s.generation()))
                            .cmp(
                                prefix
                                    .iter()
                                    .chain(&suffix)
                                    .map(|s| (s.arena_index(), s.generation())),
                            )
                    })
                    .then_with(|| symbol_order(membership.domain.symbol, fact.domain_symbol))
                    .then_with(|| membership.domain.semantic_id.0.cmp(&fact_identity.0))
            })
            .map_err(|_| {
                unserved("logical membership not formed by an exact input qualification")
            })?;
        let membership = &self.memberships[self.membership_lookup[position]];
        if logical.observation == ContentPlaceVersion::Current && membership.reference.is_some() {
            return Err(unserved(
                "logical guarantee needs its normal-return reference qualification",
            ));
        }
        let path = membership.subject;
        // Only legacy unindexed facts may omit their semantic coordinate.
        // Every family use rejoins its complete retained application.
        if membership.domain.semantic_id != fact_identity {
            return Err(identity(
                machine,
                "logical membership differs from its exact input domain",
            ));
        }
        let mut clauses = Vec::new();
        if membership.nominal {
            let position = atoms
                .binary_search_by(|key| key.cmp(&membership.key))
                .map_err(|_| {
                    identity(
                        machine,
                        "logical membership lost its nominal correspondence",
                    )
                })?;
            clauses.push(Proposition::Atom(
                PropositionId::new(position as u64 + 1)
                    .expect("bounded by allocated membership rows"),
            ));
        }
        let definition = program
            .typed
            .domain_definition_for_instance(membership.domain.symbol, membership.domain.semantic_id)
            .ok_or_else(|| identity(machine, "logical membership lost its source domain"))?;
        let scope = ClauseScope {
            logical: Some(LogicalScope {
                subject: Some((definition.subject_symbol, &path)),
                ..logical
            }),
            ..*scope
        };
        for fact in span(
            &program.typed.proof_facts,
            definition.facts,
            machine,
            "logical membership theory span",
        )? {
            let expression = program.fact(machine, fact)?;
            clauses.push(program.clause(machine, &scope, expression)?);
        }
        let proposition = match clauses.len() {
            0 => Proposition::Truth,
            1 => clauses.pop().expect("one clause"),
            _ => Proposition::Conjunction(clauses),
        };
        Ok((
            proposition,
            crate::ClauseText(vec![crate::Piece::Text(format!(
                "{} in {}",
                program.typed.symbols.name(path.root),
                membership.domain.name.as_str()
            ))]),
        ))
    }
}

pub(crate) fn projected_arrival(
    typed: &TypedTrees,
    purpose: source::DependencyScope,
    required: &StateSignature,
    supplied: SymbolHandle,
    projection: &[ParameterOccurrence],
    model: Option<&CallableAddressModel>,
    source: SourceOrigin,
    captures: crate::NamedCaptures,
) -> Checked<CallableArrivalWitness> {
    let entry = MachineId::new(1).expect("nonzero identity");
    let callee = MachineId::new(2).expect("nonzero identity");
    let required_parameters = typed.state_signature_parameters(required);
    if required_parameters.len() != required.parameters.len() {
        return Err(refuse(
            entry,
            Construct::Machine,
            Unserved::Contract("incomplete source logical parameter inventory"),
        ));
    }
    let mut previous = None;
    for occurrence in projection {
        if required_parameters
            .get(occurrence.ordinal)
            .is_none_or(|parameter| parameter.symbol != occurrence.symbol)
            || previous.is_some_and(|previous| occurrence.ordinal <= previous)
        {
            return Err(refuse(
                entry,
                Construct::Machine,
                Unserved::Contract("logical projection lost its exact source occurrence"),
            ));
        }
        previous = Some(occurrence.ordinal);
    }
    let mut member_arena = Arena::new();
    let required_inputs = Inputs::prepare(
        typed,
        entry,
        required_parameters,
        &(0..required_parameters.len()).collect::<Vec<_>>(),
        model,
        &mut member_arena,
    )?;
    let machine = typed.machine(supplied).ok_or_else(|| {
        refuse(
            entry,
            Construct::Machine,
            Unserved::Machine("logical callee without a declaration"),
        )
    })?;
    let state = typed.machine_states(machine).first().ok_or_else(|| {
        refuse(
            entry,
            Construct::Machine,
            Unserved::Machine("logical callee without an entry"),
        )
    })?;
    let actual_parameters = typed.state_parameters(state);
    if actual_parameters.len() != state.parameters.len()
        || typed.machine_contracts(machine).len() != machine.contracts.len()
        || typed.state_contracts(state).len() != state.contracts.len()
        || typed.state_signature_contracts(required).len() != required.contracts.len()
    {
        return Err(refuse(
            entry,
            Construct::Machine,
            Unserved::Contract("incomplete logical callable inventory"),
        ));
    }
    let mut program = Program::with_captures(typed, false, purpose, captures);
    if program.result_type(entry, required.return_type)?.is_some()
        || program.result_type(callee, state.return_type)?.is_some()
    {
        return Err(refuse(
            entry,
            Construct::Machine,
            Unserved::Contract("projected refinement needs its complete result correspondence"),
        ));
    }
    // A declared conformer has one routed entry, even when its body writes a
    // different precondition. Use the ordinary caller-facing interface owner;
    // the executable closure still owes its independent member refinement.
    let interface = program.conformer_requirement(callee, machine)?;
    let (parameters, supplied_contracts, supplied_owner, callee_requirement) = match interface {
        Some((_, signature)) => {
            let parameters = typed.state_signature_parameters(signature);
            let contracts = typed.state_signature_contracts(signature);
            if parameters.len() != signature.parameters.len()
                || contracts.len() != signature.contracts.len()
            {
                return Err(identity(
                    callee,
                    "logical interface lost its source inventory",
                ));
            }
            if !signature.type_parameters.is_empty()
                || !signature.lifetime_parameters.is_empty()
                || !signature.conformance_bounds.is_empty()
                || !signature.native_callback_parameters.is_empty()
            {
                return Err(refuse(
                    callee,
                    Construct::Machine,
                    Unserved::Contract("logical interface needs its complete invocation telescope"),
                ));
            }
            if program
                .result_type(callee, signature.return_type)?
                .is_some()
            {
                return Err(refuse(
                    callee,
                    Construct::Machine,
                    Unserved::Contract(
                        "projected interface needs its complete result correspondence",
                    ),
                ));
            }
            (
                parameters,
                contracts.iter().collect::<Vec<_>>(),
                signature.symbol,
                Some(
                    RetainedDeclaration::capture(&typed.symbols, signature.symbol)
                        .map_err(|detail| identity(callee, detail))?,
                ),
            )
        }
        None => (
            actual_parameters,
            typed
                .machine_contracts(machine)
                .iter()
                .chain(typed.state_contracts(state))
                .collect(),
            state.symbol,
            None,
        ),
    };
    if parameters.iter().any(|parameter| {
        typed
            .access_view_plan_for_input_type_reference(parameter.type_reference)
            .is_some()
    }) {
        return Err(refuse(
            entry,
            Construct::Machine,
            Unserved::Parameter("logical arrival without installed access-view establishment"),
        ));
    }
    let supplied_inputs = Inputs::prepare(
        typed,
        callee,
        parameters,
        &projection
            .iter()
            .map(|occurrence| occurrence.ordinal)
            .collect::<Vec<_>>(),
        model,
        &mut member_arena,
    )?;
    let mut memberships: Vec<_> = required_inputs
        .memberships
        .iter()
        .chain(&supplied_inputs.memberships)
        .filter(|membership| membership.nominal)
        .collect();
    memberships.sort_unstable_by(|left, right| left.key.cmp(&right.key));
    memberships.dedup_by(|left, right| left.key == right.key);
    let atoms: Vec<_> = memberships
        .iter()
        .map(|membership| membership.key.clone())
        .collect();
    let mut nominal = Vec::with_capacity(atoms.len());
    for (position, declaration) in memberships.iter().enumerate() {
        let key = &declaration.key;
        let occurrence = required_parameters.get(key.formal).ok_or_else(|| {
            refuse(
                entry,
                Construct::Machine,
                Unserved::Contract("logical nominal subject outside the source projection"),
            )
        })?;
        let domain =
            RetainedDeclaration::capture(&typed.symbols, key.domain).map_err(|detail| {
                identity(
                    entry,
                    &format!("logical nominal domain without source custody: {detail}"),
                )
            })?;
        nominal.push(NominalPremiseOrigin {
            atom: PropositionId::new(position as u64 + 1).expect("bounded by allocated rows"),
            occurrence: ParameterOccurrence {
                ordinal: key.formal,
                symbol: occurrence.symbol,
            },
            path: SymbolPath {
                root: occurrence.symbol,
                members: member_arena.insert_many(key.fields.iter().copied()),
            },
            domain,
        });
    }
    let required_contracts: Vec<_> = typed.state_signature_contracts(required).iter().collect();
    let (entry_signature, mut entry_names) = LogicalScope {
        inputs: &required_inputs,
        members: &member_arena,
        subject: None,
        model,
        expanding: &[],
        machine: entry,
        observation: ContentPlaceVersion::Entry,
        atoms: &atoms,
    }
    .signature(&mut program, required.symbol, &required_contracts)?;
    let (callee_signature, mut callee_names) = LogicalScope {
        inputs: &supplied_inputs,
        members: &member_arena,
        subject: None,
        model,
        expanding: &[],
        machine: callee,
        observation: ContentPlaceVersion::Entry,
        atoms: &atoms,
    }
    .signature(&mut program, supplied_owner, &supplied_contracts)?;
    entry_names.machine = required.name.as_str().to_owned();
    entry_names.state = required.name.as_str().to_owned();
    callee_names.machine = machine.name.as_str().to_owned();
    callee_names.state = state.name.as_str().to_owned();
    let mut arguments = Vec::with_capacity(supplied_inputs.leaves.len());
    for leaf in &supplied_inputs.leaves {
        let root = required_parameters[leaf.origin].symbol;
        let path = SymbolPath {
            root,
            members: leaf.path.members,
        };
        let position = required_inputs
            .lookup
            .binary_search_by(|position| {
                path_order(&member_arena, required_inputs.leaves[*position].path, path)
            })
            .map_err(|_| {
                refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Contract("logical argument lost its exact source leaf"),
                )
            })?;
        let position = required_inputs.lookup[position];
        if required_inputs.leaves[position].scalar != leaf.scalar {
            return Err(refuse(
                entry,
                Construct::Machine,
                Unserved::Typing("logical argument carrier differs"),
            ));
        }
        arguments.push(Expression {
            call_context: None,
            kind: ExpressionKind::Slot(position),
            result: Some(ValueType::Scalar(leaf.scalar, Qualification::NONE)),
            source,
        });
    }
    let mut signatures = IdTable::default();
    signatures.push(entry_signature.clone());
    signatures.push(callee_signature.clone());
    let mut bodies = IdTable::default();
    bodies.push(crate::callable_refinement_body(
        &entry_signature,
        Expression {
            call_context: None,
            kind: ExpressionKind::Call {
                callee: callee.into(),
                // This abstract callee already carries the authenticated public
                // requirement. There is no source call or selected member whose
                // separate contract must be substituted into this proof call.
                required_contract: None,
                static_reach_binding: None,
                source_call: None,
                arguments,
                designated: None,
            },
            result: None,
            source,
        },
    ));
    bodies.push(Body {
        slots: callee_signature.parameters,
        statements: Vec::new(),
    });
    let mut elaboration =
        crate::lower_generic(entry, &bodies, &signatures, &IdTable::default(), &[callee])?;
    elaboration.names.push(entry_names);
    elaboration.names.push(callee_names);
    elaboration.machines.push(required.symbol);
    elaboration.machines.push(supplied);
    let (schema, _) = typed
        .trait_machine_signature(required.symbol)
        .ok_or_else(|| identity(entry, "logical requirement lost its exact declaring schema"))?;
    Ok(CallableArrivalWitness {
        slot: String::new(),
        semantic_schema: RetainedDeclaration::capture(&typed.symbols, schema.symbol)
            .map_err(|detail| identity(entry, detail))?,
        requirement: RetainedDeclaration::capture(&typed.symbols, required.symbol)
            .map_err(|detail| identity(entry, detail))?,
        implementation: RetainedDeclaration::capture(&typed.symbols, supplied)
            .map_err(|detail| identity(entry, detail))?,
        callee_requirement,
        elaboration,
        required_inputs: required_inputs.origins,
        supplied_inputs: supplied_inputs.origins,
        nominal_premises: nominal,
        address_model: model.cloned(),
        members: member_arena,
    })
}
