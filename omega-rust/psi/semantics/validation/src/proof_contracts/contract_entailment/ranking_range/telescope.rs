//! Entry-rooted parameter telescopes shared by the state-edge and the
//! call-component rank-range judgments. Each state's non-self formal carries
//! at most one authored entry role; both consumers must read the same
//! correspondence or a call hypothesis could name a different value than the
//! member's own termination witness proves.

use super::fields::{collect_record_paths, record_referent};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// One entry parameter symbol per non-self formal of every machine state
/// (`SymbolHandle::default()` marks a formal with no discovered entry role).
/// Returns `None` when any state cannot compose an exact correspondence:
/// identity arrivals anchor each telescope, computed arrivals may then reuse
/// an already-anchored role, and conflicting proposals remove the state.
/// Proposals join per slot: an actual that names no subject dependency --
/// a literal, auxiliary arithmetic, or a forward of a role-less slot --
/// abstains on that slot instead of contesting a role another arrival
/// claimed, while two different claimed entries for one slot conflict.
/// `required` names the entry symbols the caller's rank judgment holds equal
/// at every arrival; a duplicated claim on any other entry resolves to its
/// bare forward so one slot stays the unique carrier, and a duplicated
/// required claim resolves to the one slot the role ever reaches through a
/// strict `carrier +/- positive` step or the carrier-rebuild shapes that
/// step (`&R { f: carrier.f - 1 }` packs the scalar's moved copy inside the
/// record that keeps the role).
pub fn discover_state_entry_mappings(
    program: &TypedTrees,
    machine: &Machine,
    rank_subject: SymbolHandle,
    required: &[SymbolHandle],
) -> Option<Vec<Vec<SymbolHandle>>> {
    let preferred = if rank_subject.is_valid() {
        &[rank_subject][..]
    } else {
        &[][..]
    };
    discover_state_entry_mappings_preferring(program, machine, preferred, rank_subject, required)
}

/// The same discovery, with every entry role a multi-subject computed actual
/// may carry. A single-subject view has one preferred carrier;
/// `Nat::BoundedDistance` ranks BOTH its subjects, so a computed arrival
/// mentioning copies of each claims the entry its earliest dependency names —
/// the edge judgment then reproves equality, membership, and descent for that
/// exact reading instead of leaving the slot role-less. `record_subject`
/// remains the custom-view rank subject whose nominal record can claim a
/// dependency-free literal arrival through `fresh_record_carrier`; scalar-only
/// measures pass an invalid handle.
pub fn discover_state_entry_mappings_preferring(
    program: &TypedTrees,
    machine: &Machine,
    preferred: &[SymbolHandle],
    record_subject: SymbolHandle,
    required: &[SymbolHandle],
) -> Option<Vec<Vec<SymbolHandle>>> {
    let states = program.machine_states(machine);
    let root = states.first()?;
    let mut mappings = vec![None; states.len()];
    mappings[0] = Some(
        program
            .state_parameters(root)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>(),
    );
    let occurrences = state_call_occurrences(program, machine);
    for computed in [false, true] {
        let anchored = mappings.iter().map(Option::is_some).collect::<Vec<_>>();
        let mut pending_states = anchored
            .iter()
            .enumerate()
            .filter_map(|(position, known)| known.then_some(position))
            .collect::<Vec<_>>();
        while let Some(source_position) = pending_states.pop() {
            let source = states.get(source_position)?;
            let source_mapping = mappings[source_position].as_ref()?.clone();
            for target_position in 0..states.len() {
                if target_position == 0 {
                    continue;
                }
                let target = states.get(target_position)?;
                for (_, _, arguments) in occurrences.iter().filter(|(origin, target_state, _)| {
                    *origin == source_position && *target_state == target_position
                }) {
                    let identity = arguments.iter().all(|argument| {
                        matches!(
                            program.expression_table.expression(*argument),
                            ExpressionNode::Name(_)
                        )
                    });
                    if !identity && (!computed || anchored[target_position]) {
                        // Computed actuals use an identity-anchored target;
                        // they do not redefine its parameter correspondence.
                        continue;
                    }
                    let Some(incoming) = argument_mapping(
                        program,
                        machine,
                        source,
                        target,
                        &source_mapping,
                        arguments,
                        preferred,
                        record_subject,
                        required,
                    ) else {
                        if identity {
                            return None;
                        }
                        continue;
                    };
                    match &mut mappings[target_position] {
                        Some(existing) => {
                            // A proposal claims a slot's role only from the
                            // dependencies its actual names. An actual with no
                            // subject dependency -- a literal, auxiliary
                            // arithmetic, or a forward of a role-less slot --
                            // abstains rather than contesting the role another
                            // arrival established: the kept claim still reads
                            // this arrival's own actual through the edge
                            // judgment's substitution and equality checks.
                            // Two different valid claims for one slot remain a
                            // genuine conflict no correspondence can serve.
                            if existing.len() != incoming.len() {
                                return None;
                            }
                            let mut refined = false;
                            for (claim, proposal) in existing.iter_mut().zip(incoming.iter()) {
                                if !proposal.is_valid() {
                                    continue;
                                }
                                if claim.is_valid() {
                                    if *claim != *proposal {
                                        return None;
                                    }
                                    continue;
                                }
                                *claim = *proposal;
                                refined = true;
                            }
                            if refined {
                                // A slot that gains a role may let this state's
                                // own arrivals claim roles they could not name
                                // before; revisit its outgoing proposals.
                                pending_states.push(target_position);
                            }
                        }
                        None => {
                            mappings[target_position] = Some(incoming);
                            pending_states.push(target_position);
                        }
                    }
                }
            }
        }
    }
    demote_stale_copies(program, machine, &occurrences, &mut mappings, required);
    mappings.into_iter().collect()
}

/// Resolve which copy of a duplicated required entry the rank reads. The edge
/// judgment holds every claim on a required entry equal at each arrival, so a
/// diverging transfer like `s(remaining, remaining)` followed by
/// `s(left - 1, right)` cannot be proved while both slots carry the role.
/// When exactly one claimant ever receives the entry through a strict
/// `carrier +/- positive` step -- a literal amount, or one whose declared
/// range proves it nonzero -- or through the rebuilt-literal and borrowed
/// carrier shapes `moved_copy_claim` judges, that moved copy is the
/// continuation the rank must read; a sibling whose arrivals are all bare
/// forwards still denotes the entry's own value -- a stale snapshot, not an
/// equal carrier -- so its claim demotes rather than forcing an equality the
/// step broke. Naming the moved copy is arrival-shape evidence, not a
/// positional guess: zero or several stepped claimants leave the set
/// contested, and a claimant with any non-step computed arrival keeps its
/// equality obligation, since the edge judgment may still prove it equal (a
/// `carrier + 0` spell changes nothing).
/// The same naming also resolves duplicated claims on entries outside
/// `required`: a slice or record carrier diverges through shapes
/// `carrier +/- positive` cannot spell, so a strict subslice or a rebuilt
/// literal marks the continuation while a stale forward's claim demotes. The
/// demotion runs even when `required` is empty -- an unranged member's slice
/// or record subjects are never required symbols, but their telescopes still
/// feed the call-component site judgment.
/// Slots never touched by the demotion keep the rules `argument_mapping`
/// already applied.
fn demote_stale_copies(
    program: &TypedTrees,
    machine: &Machine,
    occurrences: &[(usize, usize, &[ExpressionHandle])],
    mappings: &mut [Option<Vec<SymbolHandle>>],
    required: &[SymbolHandle],
) {
    let states = program.machine_states(machine);
    // Per destination slot: whether any arrival computes a non-forward actual
    // and whether any arrival is a strict step of a carrier of the slot's own
    // (pre-demotion) role. Raw merged claims answer both questions, since the
    // fixpoint already established that every arrival proposes the same role.
    let mut stepped = mappings
        .iter()
        .map(|mapping| vec![false; mapping.as_ref().map_or(0, Vec::len)])
        .collect::<Vec<_>>();
    let mut computed = stepped.clone();
    let mut moved = stepped.clone();
    for &(source_position, target_position, arguments) in occurrences {
        if target_position == 0 {
            continue;
        }
        let (Some(source), Some(source_mapping), Some(target_mapping)) = (
            states.get(source_position),
            mappings.get(source_position).and_then(Option::as_deref),
            mappings.get(target_position).and_then(Option::as_deref),
        ) else {
            continue;
        };
        for (position, (claimed, argument)) in
            target_mapping.iter().zip(arguments.iter()).enumerate()
        {
            if !matches!(
                program.expression_table.expression(*argument),
                ExpressionNode::Name(name)
                    if name.symbol.is_valid() && name.head_symbol == name.symbol
            ) {
                computed[target_position][position] = true;
            }
            if !claimed.is_valid() {
                continue;
            }
            if required.contains(claimed) {
                // A required entry's moved copy is a strict step of a formal
                // carrying it. `carrier +/- positive` spells that directly;
                // a role packed inside a record moves through the same shapes
                // an unrequired carrier uses -- a rebuilt literal or its
                // borrow -- since the slot itself is the carrier that steps.
                if strict_step_claim(program, source, source_mapping, *argument, *claimed)
                    || moved_copy_claim(
                        program,
                        machine,
                        source,
                        source_mapping,
                        *argument,
                        *claimed,
                    )
                {
                    stepped[target_position][position] = true;
                }
            } else if moved_copy_claim(
                program,
                machine,
                source,
                source_mapping,
                *argument,
                *claimed,
            ) {
                moved[target_position][position] = true;
            }
        }
    }
    for (state_position, mapping) in mappings.iter_mut().enumerate() {
        let Some(mapping) = mapping else {
            continue;
        };
        for entry in required {
            let claimants = mapping
                .iter()
                .enumerate()
                .filter_map(|(position, claimed)| (*claimed == *entry).then_some(position))
                .collect::<Vec<_>>();
            if claimants.len() < 2 {
                continue;
            }
            let moved = claimants
                .iter()
                .copied()
                .filter(|position| stepped[state_position][*position])
                .collect::<Vec<_>>();
            if moved.len() != 1 {
                continue;
            }
            for position in claimants {
                if position != moved[0] && !computed[state_position][position] {
                    mapping[position] = SymbolHandle::default();
                }
            }
        }
        // A duplicated non-required claim carries no equality obligation, so
        // demotion only ever needs the unique moved copy: `live[1..]` or a
        // rebuilt literal names the continuation the rank reads, and every
        // other claimant -- bare forward or other computation -- is a stale
        // sibling. Zero or several stepped claimants leave the set contested.
        let mut entries: Vec<SymbolHandle> = Vec::new();
        for claimed in mapping.iter() {
            if claimed.is_valid() && !required.contains(claimed) && !entries.contains(claimed) {
                entries.push(*claimed);
            }
        }
        for entry in entries {
            let claimants = mapping
                .iter()
                .enumerate()
                .filter_map(|(position, claimed)| (*claimed == entry).then_some(position))
                .collect::<Vec<_>>();
            if claimants.len() < 2 {
                continue;
            }
            let named = claimants
                .iter()
                .copied()
                .filter(|position| moved[state_position][*position])
                .collect::<Vec<_>>();
            if named.len() != 1 {
                continue;
            }
            for position in claimants {
                if position != named[0] {
                    mapping[position] = SymbolHandle::default();
                }
            }
        }
    }
}

/// Whether `argument` is `carrier +/- positive` where `carrier` names a source
/// formal that still carries `claimed`. This is the strict-step half of
/// `carrier_arrival_bound`'s arrival shapes in the call-component judgment:
/// a moved copy of the role, rather than a forward or a value the role never
/// described. The carrier is the formal the step reads -- including the
/// destination slot's own symbol on a self-edge -- and it must carry the same
/// entry the slot claims, or the step moved some other role's value.
fn strict_step_claim(
    program: &TypedTrees,
    source: &State,
    source_mapping: &[SymbolHandle],
    argument: ExpressionHandle,
    claimed: SymbolHandle,
) -> bool {
    let ExpressionNode::Binary(binary) = program
        .expression_table
        .expression(unwrapped(program, argument))
    else {
        return false;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Add | BinaryOperator::Subtract
    ) {
        return false;
    }
    if !positive_step_amount(program, source, binary.right) {
        return false;
    }
    let ExpressionNode::Name(name) = program
        .expression_table
        .expression(unwrapped(program, binary.left))
    else {
        return false;
    };
    if !name.symbol.is_valid() || name.head_symbol != name.symbol {
        return false;
    }
    program
        .state_parameters(source)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.symbol == name.symbol && !parameter.is_const)
        .is_some_and(|position| source_mapping.get(position).copied() == Some(claimed))
}

/// Whether `argument` is a strict step of a source formal carrying
/// `claimed`, spelled for the carriers `carrier +/- positive` cannot reach:
/// a slice carrier moves through a builtin subslice whose front bound is
/// proved positive (`carrier[1..]`, of a bare formal or a member chain
/// rooted at one), and a record carrier moves through a literal rebuild
/// whose some field value is itself a strict leaf step of that carrier's
/// chain (`&R { f: carrier.f - 1 }`). A bare `carrier +/- positive` actual
/// is the integer shape `strict_step_claim` judges. Required claims use the
/// same evidence when the entry's value packs inside the record carrier a
/// literal rebuild steps; a slice spelling cannot carry a scalar entry, so
/// only the literal and borrowed shapes newly apply there.
fn moved_copy_claim(
    program: &TypedTrees,
    machine: &Machine,
    source: &State,
    source_mapping: &[SymbolHandle],
    argument: ExpressionHandle,
    claimed: SymbolHandle,
) -> bool {
    let argument = unwrapped(program, argument);
    match program.expression_table.expression(argument) {
        ExpressionNode::Borrow(borrow) => moved_copy_claim(
            program,
            machine,
            source,
            source_mapping,
            borrow.target,
            claimed,
        ),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| {
                moved_leaf_step(
                    program,
                    machine,
                    source,
                    source_mapping,
                    field.value,
                    claimed,
                )
            }),
        ExpressionNode::Indexed(_) => {
            moved_leaf_step(program, machine, source, source_mapping, argument, claimed)
        }
        _ => false,
    }
}

/// One field value's -- or a slice argument's own -- strict step:
/// `carrier.path +/- positive` or `carrier.path[positive..]`, where the
/// chain's root formal still carries `claimed` at the source. A subslice
/// must drop a proved-positive front segment; `carrier[..end]` or a zero
/// start keeps the window's length unbounded below and is not divergence
/// evidence.
fn moved_leaf_step(
    program: &TypedTrees,
    machine: &Machine,
    source: &State,
    source_mapping: &[SymbolHandle],
    expression: ExpressionHandle,
    claimed: SymbolHandle,
) -> bool {
    let expression = unwrapped(program, expression);
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add | BinaryOperator::Subtract
            ) =>
        {
            positive_step_amount(program, source, binary.right)
                && carrier_root_carries(program, source, source_mapping, binary.left, claimed)
        }
        ExpressionNode::Indexed(indexed)
            if crate::value_custody::places::has_builtin_subslice_meaning(
                program,
                machine,
                Some(source),
                expression,
            ) =>
        {
            let ExpressionNode::Range(range) = program
                .expression_table
                .expression(unwrapped(program, indexed.index))
            else {
                return false;
            };
            range.start.is_valid()
                && positive_step_amount(program, source, range.start)
                && carrier_root_carries(
                    program,
                    source,
                    source_mapping,
                    indexed.collection,
                    claimed,
                )
        }
        _ => false,
    }
}

/// The step's carrier resolves through borrows and member chains to a
/// non-const source formal whose discovered role is `claimed` -- the step
/// moved this copy of the entry, not an unrelated value. This is the
/// member-chain generalization of the bare-name carrier `strict_step_claim`
/// requires: `live`, `live.power`, and `&holder.items` all root at the
/// formal whose role decides.
fn carrier_root_carries(
    program: &TypedTrees,
    source: &State,
    source_mapping: &[SymbolHandle],
    mut expression: ExpressionHandle,
    claimed: SymbolHandle,
) -> bool {
    loop {
        match program
            .expression_table
            .expression(unwrapped(program, expression))
        {
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Name(name)
                if name.symbol.is_valid() && name.head_symbol == name.symbol =>
            {
                return program
                    .state_parameters(source)
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .position(|parameter| parameter.symbol == name.symbol && !parameter.is_const)
                    .is_some_and(|position| {
                        source_mapping.get(position).copied() == Some(claimed)
                    });
            }
            _ => return false,
        }
    }
}

/// Whether `expression` is a proved-positive step amount evaluated in
/// `state`: a positive integer literal, or a formal whose declared ranges
/// include a literal floor of at least 1. The declared bound constrains every
/// stored value of the formal -- mutable or not -- so the amount needs no
/// hypothesis machinery at discovery time; the edge judgment still proves the
/// descent and landing the step claims. An unbounded or zero-able operand is
/// not divergence evidence: the copies may still hold equal values, so the
/// claimant keeps its equality obligation.
pub(crate) fn positive_step_amount(
    program: &TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    match program
        .expression_table
        .expression(unwrapped(program, expression))
    {
        ExpressionNode::Integer(literal) => literal.value_i64().is_some_and(|amount| amount > 0),
        ExpressionNode::Name(name) if name.symbol.is_valid() && name.head_symbol == name.symbol => {
            program.state_parameters(state).iter().any(|parameter| {
                !parameter.is_self
                    && parameter.symbol == name.symbol
                    && declared_positive_floor(program, parameter.type_reference)
            })
        }
        _ => false,
    }
}

/// The formal's declared type carries a literal range floor of at least 1:
/// some constrained shell bounds the value above zero outright, whatever the
/// other constraints or the base primitive say. A non-literal minimum proves
/// nothing here -- the arrival judgment keeps its own accounting of it.
fn declared_positive_floor(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        let positive_floor = |constraint: &TypeConstraintNode| match constraint {
            TypeConstraintNode::Range { minimum, .. } => matches!(
                program.expression_table.expression(*minimum),
                ExpressionNode::Integer(literal)
                    if literal.value_i64().is_some_and(|floor| floor >= 1)
            ),
            _ => false,
        };
        if program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .any(positive_floor)
        {
            return true;
        }
        reference = *base_type;
    }
    false
}

/// Strip `Atomic` wrappers, matching the call-component judgment's arrival
/// reading: an atomically-evaluated step is still a step.
fn unwrapped(program: &TypedTrees, mut expression: ExpressionHandle) -> ExpressionHandle {
    while let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) {
        expression = atomic.value;
    }
    expression
}

/// Every authored internal arrival: `(source index, destination index,
/// actuals)` for each named transition target and each implicit self
/// transition. The state-edge judgment's guarded-edge reader collects the
/// same occurrence set; discovery must not enumerate a different one.
fn state_call_occurrences<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
) -> Vec<(usize, usize, &'program [ExpressionHandle])> {
    let states = program.machine_states(machine);
    let mut occurrences = Vec::new();
    for (source_position, source) in states.iter().enumerate() {
        for statement in program.statement_table.statements(source.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        if let Some(target_position) =
                            named_target_state_index(program, machine, path.symbol)
                        {
                            occurrences.push((
                                source_position,
                                target_position,
                                program.statement_table.expression_handles(*arguments),
                            ));
                        }
                    }
                    TransitionTargetNode::SelfTarget => {
                        occurrences.push((source_position, source_position, &[]));
                    }
                    TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
    occurrences
}

/// Resolve one named transition target to its state index inside `machine`.
/// A transition back to the machine's declared entry names the machine symbol,
/// while transitions to subordinate states name the state symbol. This
/// normalization must match the state-graph owner so an entry back-edge is
/// not mistaken for a nested machine invocation.
fn named_target_state_index(
    program: &TypedTrees,
    machine: &Machine,
    target_symbol: SymbolHandle,
) -> Option<usize> {
    if target_symbol == machine.symbol {
        let entry_name = machine
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or_default();
        return program
            .machine_states(machine)
            .iter()
            .position(|state| state.name.as_str() == entry_name)
            .or_else(|| (!program.machine_states(machine).is_empty()).then_some(0));
    }
    program
        .machine_states(machine)
        .iter()
        .position(|state| state.symbol == target_symbol)
}

/// Compose destination formal ordinal -> exact source parameter -> entry
/// subject. States may drop or repeat unrelated parameters. The arithmetic
/// query independently proves equality among required scalar copies on every
/// arrival; discovering shared ancestry does not establish equal values.
fn argument_mapping(
    program: &TypedTrees,
    machine: &Machine,
    source: &State,
    target: &State,
    source_mapping: &[SymbolHandle],
    arguments: &[ExpressionHandle],
    preferred: &[SymbolHandle],
    record_subject: SymbolHandle,
    required: &[SymbolHandle],
) -> Option<Vec<SymbolHandle>> {
    let source_parameters = program
        .state_parameters(source)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if source_mapping.len() != source_parameters.len()
        || arguments.len()
            != program
                .state_parameters(target)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count()
    {
        return None;
    }
    let mut parameters = Vec::with_capacity(arguments.len());
    let mut bare_forwards = Vec::with_capacity(arguments.len());
    let mut subjects = Vec::new();
    for (position, argument) in arguments.iter().enumerate() {
        subjects.clear();
        argument_subjects(program, machine, source, *argument, &mut subjects, 0)?;
        // A bare forward names the exact formal -- including under one
        // borrow, the only way a record carrier forwards its role: `&card`
        // denotes `card`'s record, while `&pair.left` denotes a projection
        // and stays computed.
        bare_forwards.push(
            match program
                .expression_table
                .expression(unwrapped(program, *argument))
            {
                ExpressionNode::Name(name) => {
                    name.symbol.is_valid() && name.head_symbol == name.symbol
                }
                ExpressionNode::Borrow(borrow) => matches!(
                    program
                        .expression_table
                        .expression(unwrapped(program, borrow.target)),
                    ExpressionNode::Name(name)
                        if name.symbol.is_valid() && name.head_symbol == name.symbol
                ),
                _ => false,
            },
        );
        let mut selected_entry = None;
        for subject in &subjects {
            let source_position = source_parameters
                .iter()
                .position(|parameter| parameter.symbol == *subject && !parameter.is_const)?;
            let entry_symbol = source_mapping[source_position];
            if subjects.len() == 1 || (entry_symbol.is_valid() && preferred.contains(&entry_symbol))
            {
                // Discover the authored role, not equality of current values.
                // The edge judgment independently establishes equality of all
                // required rank copies on every arrival before using it as an
                // invariant. Diverging copies still fail that judgment.
                // A computed actual can mention several preferred entries —
                // both subjects of a bounded distance rank the pair — so the
                // earliest dependency in argument order names the carrier and
                // the rest keep the actual's substitution rather than
                // conflicting the slot out of its telescope.
                if let Some(selected) = selected_entry
                    && selected != entry_symbol
                {
                    if subjects.len() == 1 {
                        return None;
                    }
                    continue;
                }
                selected_entry = Some(entry_symbol);
            }
        }
        if let Some(owner) =
            fresh_record_carrier(program, machine, target, position, record_subject)
        {
            let discovered_record = selected_entry
                .is_some_and(|entry| entry_is_record_of(program, machine, entry, owner));
            if !discovered_record && !parameters.contains(&record_subject) {
                // A fresh record literal carries no subject dependency, but
                // its destination slot is still the record the selected view
                // reads. A discovered scalar role can never serve this slot —
                // the mapping validator rejects it — while a discovered record
                // lineage stays authoritative. The claim only locates the
                // record; the edge judgment extracts the literal's fields and
                // proves membership, pinning and descent independently.
                selected_entry = Some(record_subject);
            }
        }
        // Auxiliary-only arithmetic over several inputs names no single role.
        // Choosing an operand would let its entry constraints reach a value
        // they never described; an absent role keeps the slot premise-free.
        parameters.push(selected_entry.unwrap_or_default());
    }
    // Several slots claiming one entry cannot each be its unique carrier.
    // Entries the rank obligation reads keep every claim: the edge judgment
    // holds those copies equal at every arrival, which is what rejects a
    // diverging transfer of a ranked subject or a pinned endpoint. Any other
    // claimed entry has no such equality evidence to preserve, so the moved
    // copy alone names the carrier when exactly one claimant is a strict
    // step of a role carrier -- `live[1..]` or a rebuilt record literal
    // against bare forwards -- and otherwise the bare forward names it while
    // a computed claimant becomes a premise-free payload — letting an edge
    // that computes a copy agree with sibling arrivals that leave the slot
    // unmapped, and leaving one carrier for records whose field coordinate
    // cannot be read through two slots.
    for position in 0..parameters.len() {
        let entry = parameters[position];
        if !entry.is_valid() || required.contains(&entry) {
            continue;
        }
        let claimants = (0..parameters.len())
            .filter(|&slot| parameters[slot] == entry)
            .collect::<Vec<_>>();
        if claimants.len() < 2 {
            continue;
        }
        let moved = claimants
            .iter()
            .copied()
            .filter(|&slot| {
                moved_copy_claim(
                    program,
                    machine,
                    source,
                    source_mapping,
                    arguments[slot],
                    entry,
                )
            })
            .collect::<Vec<_>>();
        for slot in claimants {
            let keeps = if moved.len() == 1 {
                slot == moved[0]
            } else {
                bare_forwards[slot]
            };
            if !keeps {
                parameters[slot] = SymbolHandle::default();
            }
        }
    }
    Some(parameters)
}

/// A dependency-free record actual can still carry the ranked role: when the
/// destination has exactly one formal whose own record can hold the rank
/// subject's lineage -- the subject's record itself, a record containing it
/// at a unique nested path, or a record its coordinate chain reaches partway
/// -- that slot is the only candidate the field view can read. The role
/// claims nothing about value ancestry — the arrival's field values are
/// substituted and proved independently. Multiple candidate formals keep the
/// slot role-less rather than guessing between records. Returns the slot's
/// own referent record when this position is the unique carrier, so a
/// discovered lineage through that exact record still outranks the claim.
fn fresh_record_carrier(
    program: &TypedTrees,
    machine: &Machine,
    target: &State,
    position: usize,
    rank_subject: SymbolHandle,
) -> Option<SymbolHandle> {
    if !rank_subject.is_valid() {
        return None;
    }
    let root = program.machine_states(machine).first()?;
    let subject = program
        .state_parameters(root)
        .iter()
        .find(|parameter| !parameter.is_self && parameter.symbol == rank_subject)?;
    // The same referent the field coordinate resolves: a subject or formal
    // reached through one reference is still this record, and its literal
    // actual arrives under one borrow.
    let (owner, _) = record_referent(program, subject.type_reference)?;
    let target_parameters = program
        .state_parameters(target)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let mut carriers = target_parameters.iter().filter(|parameter| {
        let Some((referent, _)) = record_referent(program, parameter.type_reference) else {
            return false;
        };
        if referent == owner {
            return true;
        }
        // The formal may instead carry the role at a nested path -- its own
        // record contains the subject's -- or be the record the subject's
        // coordinate chain reaches partway. Either carriage demands the one
        // unique path the arrival judgment resolves; two readings stay a
        // guess and a reversed direction still names this slot's record.
        let mut nested = Vec::new();
        collect_record_paths(program, referent, owner, &mut nested);
        if !nested.is_empty() {
            return nested.len() == 1;
        }
        let mut boundary = Vec::new();
        collect_record_paths(program, owner, referent, &mut boundary);
        boundary.len() == 1
    });
    let formal = carriers.next()?;
    if carriers.next().is_none()
        && target_parameters
            .get(position)
            .is_some_and(|parameter| parameter.symbol == formal.symbol)
    {
        record_referent(program, formal.type_reference).map(|(referent, _)| referent)
    } else {
        None
    }
}

/// Whether the discovered entry symbol is a root parameter of this exact
/// record type — a genuine record lineage that outranks the fresh-literal
/// claim. Scalar or role-less selections cannot serve a record slot and do
/// not block the claim.
fn entry_is_record_of(
    program: &TypedTrees,
    machine: &Machine,
    entry: SymbolHandle,
    owner: SymbolHandle,
) -> bool {
    let Some(root) = program.machine_states(machine).first() else {
        return false;
    };
    program.state_parameters(root).iter().any(|parameter| {
        !parameter.is_self
            && parameter.symbol == entry
            && record_referent(program, parameter.type_reference)
                .is_some_and(|(referent, _)| referent == owner)
    })
}

/// Discover a dependency, not a value equality or an arithmetic theorem. The
/// ordinary edge query still checks selected builtin meaning and every rank
/// obligation before this provisional correspondence can authorize anything.
/// Retain all distinct current dependencies before selecting the authored role;
/// nested auxiliary arithmetic must not select an operand merely by position.
/// A literal subtree contributes no subject of its own.
fn argument_subjects(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    subjects: &mut Vec<SymbolHandle>,
    depth: usize,
) -> Option<()> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
        ExpressionNode::Name(name) if name.symbol.is_valid() && name.head_symbol == name.symbol => {
            if !subjects.contains(&name.symbol) {
                subjects.push(name.symbol);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            argument_subjects(program, machine, state, atomic.value, subjects, depth + 1)?;
        }
        ExpressionNode::Borrow(borrow) => {
            // A constructed borrow carries the lineage of its target: `&x`
            // names `x`'s role and `&R { field: value }` names the field
            // values' roles, exactly as the unborrowed record arrival does.
            // The edge judgment still requires the literal or projection the
            // field coordinate can read before this dependency means anything.
            argument_subjects(program, machine, state, borrow.target, subjects, depth + 1)?;
        }
        ExpressionNode::Member(member) => {
            // A projection identifies a candidate input role, not its field's
            // value or type. The edge owner separately resolves the exact owned
            // declaration before binding any arithmetic coordinate.
            argument_subjects(
                program,
                machine,
                state,
                member.receiver,
                subjects,
                depth + 1,
            )?;
        }
        ExpressionNode::StructLiteral(literal) => {
            // Rebuilding a record is an ordinary computed arrival. Collect all
            // dependencies so field order cannot choose its role, and let the
            // simultaneous field substitution prove the actual arrival. This
            // does not infer equality, endpoint pinning, or strict descent.
            for field in program.expression_table.struct_fields(literal.fields) {
                argument_subjects(program, machine, state, field.value, subjects, depth + 1)?;
            }
        }
        ExpressionNode::Indexed(indexed)
            if crate::value_custody::places::has_builtin_subslice_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) =>
        {
            // A window retains its collection's lineage, not the lineage of
            // the scalar bounds. The edge query separately proves its length
            // and bounds before this mapping can support a ranking fact.
            argument_subjects(
                program,
                machine,
                state,
                indexed.collection,
                subjects,
                depth + 1,
            )?;
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Modulo
            ) =>
        {
            argument_subjects(program, machine, state, binary.left, subjects, depth + 1)?;
            argument_subjects(program, machine, state, binary.right, subjects, depth + 1)?;
        }
        _ => return None,
    }
    Some(())
}
