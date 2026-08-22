//! §5b recast judgment, rung A (programmable-layouts brief): `&x as &T`
//! re-views a place's bytes under a second stated shape. Legality is a
//! STATIC judgment -- a bad relation is a compile error, never unsafety.
//!
//! The scalar rung serves the core end-to-end and fences the rest loudly:
//!
//! - **Served:** a recast between fixed-width scalar primitives of EQUAL byte
//!   size (`&i32 as &f32`, `&mut u32 as &mut i32`), or a scalar view into a
//!   proven in-bounds `[u8; N]` region, bound as the direct
//!   initializer of a reference-typed let whose stated type restates the
//!   target. Shared views may weaken source facts. Mutable scalar views admit
//!   fact-free types, normalized domain conjunctions that imply one another
//!   in BOTH directions, or integer ranges that denote the same normalized
//!   bit-pattern set. Same-carrier float ranges compose by numeric interval
//!   inclusion. A shared view may forget a float interval into an unconstrained
//!   equal-width bit carrier, but it never justifies cross-carrier mutable
//!   equivalence. Merely equal-looking cross-carrier predicates remain fenced.
//!   Byte-region aggregate views require recursively
//!   fact-free target shapes, including top-level and nested literal-length
//!   fixed arrays. Mutable typed aggregate aliases may retain facts when source
//!   and target have identical geometry and representation-equivalent leaves;
//!   shared aliases may weaken facts. The same repeated-leaf judgment serves
//!   unsized slices of aggregate elements over a complete typed fixed array;
//!   element stride includes layout padding rather than repacking the leaves.
//!   Lowering is address identity:
//!   native reads/writes the place through the stated type; the interpreter
//!   bit-reinterprets both sides of the alias or assembles/writes the complete
//!   little-endian byte-region footprint.
//! - **Fenced (deeper byte-view rung, L4/L5):** remaining dynamically-sized
//!   shapes beyond complete-source and proven interior unsized slices
//!   (byte-granular tiling over plan-laid layouts), and recasts in non-let
//!   positions. A runtime interior byte offset cannot establish multi-byte
//!   element tiling until its congruence is proved; an exact offset can.
//! - **Refused absolutely:** targets that would ESTABLISH a fact the bytes
//!   don't prove (`bool`'s 0/1, text encodings) -- establishing facts is a
//!   MINT's job (fallible, case-returning), never a recast's.
//!
//! The companion rule closes the accidental-pun hole this judgment would
//! otherwise be bypassed by: a reference-typed let whose initializer is a
//! BARE borrow of a differently-typed scalar place (`let v: &f32 =
//! &self.x` over an i64) used to compile unjudged and DIVERGE (native
//! bit-punned, the interpreter delivered the semantic value; pinned by
//! canaries/fail/recast/reference_let_pun_requires_recast). Re-viewing is
//! spelled `as`; the bare mismatch refuses.

use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use psi_typed_trees::statement::StatementNode;
use psi_typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};
use std::collections::HashSet;

mod offset_bounds;
mod record_representation;
mod scalar_representation;

use offset_bounds::incoming_guard_offset_bound;

use record_representation::{
    mutable_record_representations_equivalent, mutable_type_representation,
    record_representation_implies, repeat_representation, representation_is_exactly_tiled,
    shared_projection_type_representation,
};

use scalar_representation::{
    MutableScalarRepresentationFacts, ScalarRepresentationSet, full_scalar_bit_patterns,
    mutable_scalar_representation_facts, mutable_scalar_representation_facts_equivalent,
    scalar_representation_facts_imply,
};

pub(crate) fn validate_recasts(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    // The blessed positions: direct initializers of reference-typed lets
    // (mirrors the D14 literal gate's shape -- collect the legal roots,
    // then sweep the whole expression table for strays).
    let mut blessed: Vec<ExpressionHandle> = Vec::new();
    // Qualification casts judged WITH machine/state context (the declared-
    // range discharge needs the value's declared type); the positional
    // sweep below only judges strays (literal-only).
    let mut judged_qualifications: Vec<ExpressionHandle> = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                judge_statement_qualification_casts(
                    program,
                    machine,
                    state,
                    statement,
                    &mut judged_qualifications,
                    diagnostics,
                );
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                if !local.type_reference.is_valid() || !local.initial_value.is_valid() {
                    continue;
                }
                let TypeReferenceNode::Reference {
                    referee,
                    is_mutable: let_is_mutable,
                    ..
                } = program
                    .type_reference_table
                    .type_reference(local.type_reference)
                else {
                    continue;
                };
                // The `&mut x as &mut T` spelling parses as Mutable(Cast(..)):
                // the unary `&mut` wraps the postfix chain. Look through it
                // so the blessed root is the CAST node the sweep checks.
                let initializer = strip_mutable(program, local.initial_value);
                match program.expression_table.expression(initializer) {
                    ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                        blessed.push(initializer);
                        if crate::traits::dynamic_trait_symbol(program, cast.target_type).is_none()
                        {
                            judge_scalar_recast(
                                program,
                                machine,
                                state,
                                cast,
                                *referee,
                                *let_is_mutable,
                                diagnostics,
                            );
                        }
                    }
                    _ => {
                        report_unspelled_reference_pun(
                            program,
                            machine,
                            state,
                            local.initial_value,
                            *referee,
                            diagnostics,
                        );
                    }
                }
            }
        }
    }

    // Position sweep: a recast anywhere but a blessed root refuses. (The
    // parser builds recast nodes only from the `as &` spelling, and every
    // expression is reachable from some statement, so this catches guard /
    // argument / nested positions uniformly.)
    for (handle, node) in program.expression_table.expression_entries() {
        if let ExpressionNode::Cast(cast) = node
            && cast.form.is_recast()
            && !blessed.contains(&handle)
        {
            diagnostics.push(Diagnostic::error(
                "a recast binds to a reference-typed let (`let v: &T = &x as &T;`) in this \
                 rung; inline re-views land with the byte-view rung"
                    .to_string(),
            ));
        }
        // STR4 checked plans, slice 3 (decision 19): a NON-policy `in <Name>`
        // cast suffix is the semantic-domain QUALIFICATION spelling. It is
        // recognized here but its MINT rung (introduction authority +
        // predicate discharge) has not landed -- the staged fence names the
        // declared domain; an unmatched name gets the honest unknown error
        // the parser used to give (now with the declaration check the parser
        // could not perform).
        if let ExpressionNode::Cast(cast) = node
            && cast.semantic_domain.count() > 0
            && !judged_qualifications.contains(&handle)
        {
            judge_qualification_cast(program, None, cast, diagnostics);
        }
    }
}

/// Judge one qualification cast (`x as T in <DeclaredDomain>`, decision 19).
/// Predicate domains discharge their propositions at this exact site. An
/// empty domain qualifies vacuously. A routed domain cannot be fabricated by
/// `as`, even when it has no predicate propositions.
fn judge_qualification_cast(
    program: &TypedTrees,
    context: Option<(
        &psi_typed_trees::machine::Machine,
        &psi_typed_trees::state::State,
    )>,
    cast: &TableCastExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let members = program
        .expression_table
        .name_path_members(cast.semantic_domain);
    let base_name = members
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let mut name = base_name.clone();
    if !cast.semantic_domain_arguments.is_empty() {
        let arguments = program
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments)
            .iter()
            .map(|argument| program.display_type_reference(*argument))
            .collect::<Vec<_>>()
            .join(", ");
        name = format!("{name}<{arguments}>");
    }
    let declared = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == cast.semantic_domain_symbol);
    match declared {
        Some(domain) => {
            if !domain.predicate_body.is_present() {
                if !domain_is_vacuous(program, domain.symbol, &mut Vec::new()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "`as ... in {name}` cannot establish a non-vacuous domain; every \
                         predicate must be proved and routed provenance must come from an exact \
                         requirement authorized by the domain"
                    )));
                }
                return;
            }
            let mut judgment = literal_mint_discharges(program, domain, cast.value);
            if matches!(judgment, MintJudgment::NotLiteral)
                && let Some((machine, state)) = context
                && let Some(judged) =
                    range_mint_discharges(program, machine, state, domain, cast.value)
            {
                judgment = judged;
            }
            // The GUARD chain's landing: the machine's own REQUIRES facts
            // about the cast value bound it one-sidedly; callers prove the
            // requires at their call sites (incoming guards already serve
            // there), so `transition raw >= 0 { true -> use(raw) }` +
            // `machine use(..) requires raw >= 0` mints inside `use`.
            if matches!(judgment, MintJudgment::NotLiteral)
                && let Some((machine, _)) = context
                && let Some(judged) = requires_mint_discharges(program, machine, domain, cast.value)
            {
                judgment = judged;
            }
            match judgment {
                MintJudgment::Discharged => {}
                MintJudgment::FactFalse => {
                    diagnostics.push(Diagnostic::error(format!(
                        "`as ... in {name}` cannot mint: a `{name}` domain fact is \
                         FALSE at this value -- the predicate obligation is owed \
                         (decision 19's \"predicate obligation not discharged\" \
                         class)",
                    )));
                }
                MintJudgment::NotLiteral => {
                    diagnostics.push(Diagnostic::error(format!(
                        "`as ... in {name}` mints LITERAL values, or names whose \
                         DECLARED RANGE entails the domain facts, in this rung; \
                         other values route through a validating call or guard \
                         until full flow integration lands",
                    )));
                }
            }
        }
        None => {
            let matching_names = program
                .domain_definitions()
                .iter()
                .filter(|domain| {
                    domain.name.as_str() == base_name
                        || domain.name.as_str().ends_with(&format!("::{base_name}"))
                })
                .count();
            diagnostics.push(Diagnostic::error(format!(
                "{} cast domain `{name}` for target `{}`",
                if matching_names > 1 {
                    "ambiguous"
                } else {
                    "unknown"
                },
                program.display_type_reference(cast.target_type),
            )));
        }
    }
}

fn domain_is_vacuous(
    program: &TypedTrees,
    domain_symbol: SymbolHandle,
    stack: &mut Vec<SymbolHandle>,
) -> bool {
    if !domain_symbol.is_valid() || stack.contains(&domain_symbol) {
        return false;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|candidate| candidate.symbol == domain_symbol)
    else {
        return false;
    };
    if let Some(alias) = domain.alias.as_ref() {
        if alias.constituents.is_empty() {
            return false;
        }
        stack.push(domain_symbol);
        let vacuous = alias
            .constituents
            .iter()
            .all(|constituent| domain_is_vacuous(program, constituent.domain_symbol, stack));
        stack.pop();
        return vacuous;
    }
    !domain.predicate_body.is_present() && domain.establishment_routes.is_empty()
}

/// Flow-integration v1: when the cast VALUE is a Name whose declared type
/// carries a range constraint, every domain fact must hold over the WHOLE
/// interval (`self >= K` iff low >= K; `self <= K` iff high <= K; strict/
/// equality forms accordingly). `None` when the value has no usable range
/// (the caller falls back to the staged refusal).
fn range_mint_discharges(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    domain: &psi_typed_trees::domain::DomainDefinition,
    value: ExpressionHandle,
) -> Option<MintJudgment> {
    // The RAW declared type keeps the Constrained shell the range lives in
    // (declared_place_type strips it -- the R2 slice-9 gotcha).
    let declared = crate::places::declared_place_type_raw(program, machine, Some(state), value)?;
    let interval = crate::arithmetic_domains::range_constraint_interval(program, declared)?;
    let (low, high) = (interval.low?, interval.high?);
    for fact in program.proof_facts.span_or_empty(domain.facts) {
        let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
            continue;
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
        else {
            continue;
        };
        // Normalize to `self OP literal`.
        let is_self = |handle: ExpressionHandle| {
            matches!(
                program.expression_table.expression(handle),
                ExpressionNode::Name(path)
                    if matches!(
                        program.expression_table.name_path_members(path.members),
                        [only] if only.as_str() == "self"
                    )
            )
        };
        let literal_of = |handle: ExpressionHandle| -> Option<i64> {
            match program.expression_table.expression(handle) {
                ExpressionNode::Integer(value) => value.text().parse::<i64>().ok(),
                _ => None,
            }
        };
        use psi_typed_trees::expression::BinaryOperator;
        let (operator, bound) = if is_self(binary.left) {
            (binary.operator, literal_of(binary.right)?)
        } else if is_self(binary.right) {
            let flipped = match binary.operator {
                BinaryOperator::Less => BinaryOperator::Greater,
                BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
                BinaryOperator::Greater => BinaryOperator::Less,
                BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
                other => other,
            };
            (flipped, literal_of(binary.left)?)
        } else {
            return None;
        };
        let holds = match operator {
            BinaryOperator::GreaterOrEqual => low >= bound,
            BinaryOperator::Greater => low > bound,
            BinaryOperator::LessOrEqual => high <= bound,
            BinaryOperator::Less => high < bound,
            BinaryOperator::Equal => low == bound && high == bound,
            BinaryOperator::NotEqual => high < bound || low > bound,
            _ => return None,
        };
        if !holds {
            // The interval does not ENTAIL the fact -- it may still hold at
            // runtime, so this is the undischarged (not FALSE) class.
            return Some(MintJudgment::NotLiteral);
        }
    }
    Some(MintJudgment::Discharged)
}

/// The rung-A judgment for one blessed `&x as &T` (§5b rules 1-4 over the
/// scalar fragment).
fn judge_scalar_recast(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    cast: &TableCastExpression,
    let_referee: TypeReferenceHandle,
    let_is_mutable: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let context = format!(
        "machine `{}` state `{}`",
        machine.name.as_str(),
        state.name.as_str()
    );

    let mutable_recast = cast.form == psi_language_core::cast_form::CastForm::RecastMutable;
    if mutable_recast != let_is_mutable {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: recast borrow polarity must agree -- use `&x as &T` for a shared \
             view or `&mut x as &mut T` for a writable view"
        )));
        return;
    }

    let source = strip_mutable(program, cast.value);
    let source_type = crate::places::declared_place_type_raw(program, machine, Some(state), source);
    let source_placed = source_type
        .and_then(|type_reference| program.placed_view_plan_for_type_reference(type_reference));
    let target_placed = program.placed_view_plan_for_type_reference(cast.target_type);
    if source_placed.is_some() || target_placed.is_some() {
        let source_name = source_placed
            .map(|view| view.data_name.as_str())
            .unwrap_or("non-placed storage");
        let target_name = target_placed
            .map(|view| view.data_name.as_str())
            .unwrap_or("non-placed storage");
        diagnostics.push(Diagnostic::error(format!(
            "{context}: placed-view recast from `{source_name}` to `{target_name}` is unavailable; retain the underlying qualified extent borrow and explicitly admit the intended placement"
        )));
        return;
    }

    // Target: a fixed-width scalar or recursively fixed aggregate, restated
    // exactly by the let. Structural targets are semantic type references;
    // their cached display spelling never participates in the judgment.
    let target_name = program
        .named_type_reference(cast.target_type)
        .map(|name| name.as_str().to_string())
        .unwrap_or_else(|| program.display_type_reference(cast.target_type));
    if let TypeReferenceNode::Slice { element_type } = program
        .type_reference_table
        .type_reference(cast.target_type)
    {
        judge_slice_recast(
            program,
            machine,
            state,
            cast,
            *element_type,
            let_referee,
            mutable_recast,
            diagnostics,
            &context,
        );
        return;
    }
    if program.primitive_type_reference(cast.target_type).is_none() {
        // RUNG C2/C3: a recursively fixed RECORD or literal-length ARRAY
        // target. The same normalized representation supplies size/alignment
        // and scalar-leaf facts; this is the top-level-array continuation of
        // the array fields records already admit.
        // Stored-width integer leaves are admissible in a mutable BYTE-REGION
        // view because every concrete assignment remains a proved-fit lowering
        // obligation. Typed aggregate aliases still reject them below: those
        // require one representation valid for arbitrary writes in both
        // directions, not per-write encoding evidence.
        let target_representation =
            shared_projection_type_representation(program, cast.target_type);
        if let Some(target_representation) = target_representation {
            if program.normalized_type_identity(let_referee)
                != program.normalized_type_identity(cast.target_type)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{context}: the let's declared type must restate the recast target \
                     `&{}{target_name}`",
                    if mutable_recast { "mut " } else { "" },
                )));
                return;
            }
            let interior = interior_byte_region_source(program, machine, state, source);
            if let InteriorByteRegion::OffsetUnproven {
                offset_display,
                region_length,
            } = &interior
            {
                push_offset_unproven(diagnostics, &context, offset_display, *region_length);
                return;
            }
            if let InteriorByteRegion::Bounded {
                offset,
                region_length,
            } = interior
            {
                if !record_view_type_is_fact_free(program, cast.target_type, &mut HashSet::new()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{context}: byte-region recast target `{target_name}` must be recursively \
                         fact-free; unchecked bytes cannot establish constrained fields, bool, \
                         or record invariants{}",
                        if mutable_recast {
                            "; mutable views require fact implication in BOTH directions"
                        } else {
                            ""
                        },
                    )));
                    return;
                }
                let Some(end) = offset.checked_add(target_representation.size as i64) else {
                    return;
                };
                if end > region_length {
                    diagnostics.push(Diagnostic::error(format!(
                        "{context}: the recast target `{target_name}` needs {} bytes at offset \
                         {offset}, but the region holds {region_length} -- the view would read \
                         past the buffer (§5b rule 1 is byte-granular)",
                        target_representation.size,
                    )));
                }
                return;
            }
            let source_type =
                crate::places::declared_place_type_raw(program, machine, Some(state), source);
            if let Some(source_type) = source_type
                && let Some(source_representation) =
                    mutable_type_representation(program, source_type)
            {
                if target_representation.has_stored_integer_projection
                    || source_representation.has_stored_integer_projection
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "{context}: stored-width integer decoding is admitted only for a shared \
                         view over a proven byte region; typed aggregate aliases require identical \
                         storage representations"
                    )));
                    return;
                }
                let compatible = if mutable_recast {
                    mutable_record_representations_equivalent(
                        program,
                        &source_representation,
                        &target_representation,
                    )
                } else {
                    record_representation_implies(
                        program,
                        &source_representation,
                        &target_representation,
                    )
                };
                if compatible {
                    return;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "{context}: {} aggregate aliases require identical layout geometry and {}; \
                     the source and target `{target_name}` are not representation-compatible",
                    if mutable_recast { "mutable" } else { "shared" },
                    if mutable_recast {
                        "leaf fact implication in BOTH directions"
                    } else {
                        "source leaf facts implying every target leaf fact"
                    },
                )));
                return;
            }
        }
        diagnostics.push(Diagnostic::error(format!(
            "{context}: recast target `{target_name}` is not a scalar primitive or an \
             eligible fixed aggregate over a byte region or typed aggregate place; deeper shapes \
             land with the byte-view rung"
        )));
        return;
    }
    let Some(target) = program.primitive_type_reference(cast.target_type) else {
        return;
    };
    let let_primitive = crate::places::unwrapped_type_reference(program, let_referee)
        .and_then(|unwrapped| program.primitive_type_reference(unwrapped));
    if let_primitive != Some(target) {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: the let's declared type must restate the recast target `&{target_name}` \
             (the stated shape is the single source of truth for reads through the view)"
        )));
        return;
    }

    // Source: a scalar place of the SAME byte width (§5b rule 1: same total
    // size; scalar alignment follows from size). Facts on the source are
    // fine under a shared view (weakening).
    // RUNG B: an INTERIOR recast into a `[u8; N]` region at a STATIC offset
    // (`&self.buf[4] as &u32`): the target's footprint must fit the
    // remaining bytes (`k + size(T) <= N`). Byte buffers carry no facts and
    // align to 1; both ISAs' scalar loads tolerate the resulting unaligned
    // addresses on normal memory. The stated-type-restated check above
    // already ran; the same-width rule below does NOT apply (the source
    // region is byte-granular).
    let interior = interior_byte_region_source(program, machine, state, source);
    if let InteriorByteRegion::OffsetUnproven {
        offset_display,
        region_length,
    } = &interior
    {
        push_offset_unproven(diagnostics, &context, offset_display, *region_length);
        return;
    }
    if let InteriorByteRegion::Bounded {
        offset,
        region_length,
    } = interior
    {
        let target_facts = mutable_scalar_representation_facts(program, let_referee);
        let raw_facts = MutableScalarRepresentationFacts {
            domains: Vec::new(),
            values: ScalarRepresentationSet::ExactBitPatterns(full_scalar_bit_patterns(target)),
        };
        let compatible = target_facts.as_ref().is_some_and(|target_facts| {
            if mutable_recast {
                mutable_scalar_representation_facts_equivalent(program, &raw_facts, target_facts)
            } else {
                scalar_representation_facts_imply(program, &raw_facts, target_facts)
            }
        });
        if !compatible {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: a {} recast {}; a raw byte region cannot establish the target's \
                 representation facts",
                if mutable_recast { "mutable" } else { "shared" },
                if mutable_recast {
                    "must prove fact implication in BOTH directions"
                } else {
                    "may weaken established facts but cannot strengthen them"
                },
            )));
            return;
        }
        let Some(target_size) = target.scalar_byte_size() else {
            return;
        };
        let Some(end) = offset.checked_add(target_size as i64) else {
            return;
        };
        if end > region_length {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: the recast target `{target_name}` needs {target_size} bytes at offset {offset}, but the region holds {region_length} -- the view would read past the buffer (§5b rule 1 is byte-granular)",
            )));
        }
        return;
    }
    let source_type = crate::places::declared_place_type_raw(program, machine, Some(state), source);
    let source_primitive =
        source_type.and_then(|type_reference| program.primitive_type_reference(type_reference));
    let Some(source_primitive) = source_primitive else {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: a recast re-views a PLACE's bytes -- the source must be a borrowed \
             scalar place (`&x as &{target_name}`); record sources and temporaries land \
             with the byte-view rung"
        )));
        return;
    };
    let (Some(source_size), Some(target_size)) = (
        source_primitive.scalar_byte_size(),
        target.scalar_byte_size(),
    ) else {
        return;
    };
    if source_size != target_size {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: a recast re-views the SAME bytes, so the shapes must agree on \
             size (§5b rule 1) -- source `{}` is {source_size} bytes, target \
             `{target_name}` is {target_size} bytes",
            source_primitive.name()
        )));
        return;
    }

    let source_facts = source_type
        .and_then(|type_reference| mutable_scalar_representation_facts(program, type_reference));
    let target_facts = mutable_scalar_representation_facts(program, let_referee);
    let target_is_fact_free = target_facts.as_ref().is_some_and(|target_facts| {
        target_facts.domains.is_empty()
            && target_facts.values
                == ScalarRepresentationSet::ExactBitPatterns(full_scalar_bit_patterns(target))
    });
    let compatible = if !mutable_recast && target_is_fact_free {
        // A shared view may always forget source facts. This remains safe even
        // when the source uses a fact family (such as a float interval) whose
        // precise representation set is not yet modeled.
        true
    } else {
        source_facts
            .as_ref()
            .zip(target_facts.as_ref())
            .is_some_and(|(source_facts, target_facts)| {
                if mutable_recast {
                    mutable_scalar_representation_facts_equivalent(
                        program,
                        source_facts,
                        target_facts,
                    )
                } else {
                    scalar_representation_facts_imply(program, source_facts, target_facts)
                }
            })
    };
    if !compatible {
        diagnostics.push(Diagnostic::error(if mutable_recast {
            format!(
                "{context}: a mutable recast must prove fact implication in BOTH directions; \
                 source and target constraints are not proven representation-equivalent"
            )
        } else {
            format!(
                "{context}: a shared recast may weaken established facts but cannot strengthen \
                 them; source facts do not establish the target representation"
            )
        }));
    }
}

#[allow(clippy::too_many_arguments)]
fn judge_slice_recast(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    cast: &TableCastExpression,
    element_type: TypeReferenceHandle,
    let_referee: TypeReferenceHandle,
    mutable_recast: bool,
    diagnostics: &mut Vec<Diagnostic>,
    context: &str,
) {
    let target_label = program.display_type_reference(cast.target_type);
    if program.normalized_type_identity(let_referee)
        != program.normalized_type_identity(cast.target_type)
    {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: the let's declared type must restate the recast target \
             `&{}{target_label}`",
            if mutable_recast { "mut " } else { "" },
        )));
        return;
    }

    let source = strip_mutable(program, cast.value);
    let Some(element_representation) = mutable_type_representation(program, element_type) else {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: slice target `{target_label}` needs a fixed-layout element type"
        )));
        return;
    };

    // An interior slice starts at one byte of a proven `[u8; N]` region and
    // consumes every remaining byte. This is the dynamically-sized companion
    // to the fixed aggregate/scalar interior rungs: raw bytes may establish
    // only recursively fact-free, exactly tiled element representations.
    let interior = interior_byte_region_source(program, machine, state, source);
    if let InteriorByteRegion::OffsetUnproven {
        offset_display,
        region_length,
    } = &interior
    {
        push_offset_unproven(diagnostics, context, offset_display, *region_length);
        return;
    }
    if let InteriorByteRegion::Bounded {
        offset,
        region_length,
    } = interior
    {
        let target_tiled = representation_is_exactly_tiled(&element_representation);
        let target_fact_free =
            record_view_type_is_fact_free(program, element_type, &mut HashSet::new());
        if !target_tiled || !target_fact_free {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: interior slice recast `{target_label}` requires a recursively \
                 fact-free element whose scalar leaves exactly tile its byte stride; raw \
                 storage cannot establish element facts or implicit padding"
            )));
            return;
        }
        if offset > region_length {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: interior slice recast starts at byte {offset}, past the \
                 {region_length}-byte source region"
            )));
            return;
        }
        let exact_offset = exact_interior_byte_region_offset(program, machine, state, source);
        if let Some(exact_offset) = exact_offset {
            let remaining = region_length - exact_offset;
            if remaining < 0
                || element_representation.size == 0
                || remaining as usize % element_representation.size != 0
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{context}: interior slice target `{target_label}` does not exactly tile \
                     the {remaining} bytes remaining at offset {exact_offset} with {}-byte \
                     elements",
                    element_representation.size,
                )));
            }
        } else if element_representation.size != 1 {
            diagnostics.push(Diagnostic::error(format!(
                "{context}: cannot prove exact tiling for interior slice `{target_label}`: \
                 the runtime byte offset may leave a remainder for {}-byte elements; use a \
                 statically exact offset or validate the dynamic region before establishing \
                 the typed slice",
                element_representation.size,
            )));
        }
        return;
    }

    let Some(source_type) =
        crate::places::declared_place_type_raw(program, machine, Some(state), source)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: a slice recast re-views a fixed-layout PLACE's complete bytes"
        )));
        return;
    };
    let Some(source_representation) = mutable_type_representation(program, source_type) else {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: slice recast source `{}` has no fixed representation",
            program.display_type_reference(source_type)
        )));
        return;
    };
    if element_representation.size == 0
        || source_representation.size % element_representation.size != 0
    {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: slice target `{target_label}` does not exactly tile the source's {} \
             bytes with {}-byte elements",
            source_representation.size, element_representation.size,
        )));
        return;
    }

    let element_count = source_representation.size / element_representation.size;
    let Some(target_representation) = repeat_representation(&element_representation, element_count)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: slice target `{target_label}` representation overflows"
        )));
        return;
    };

    let source_tiled = representation_is_exactly_tiled(&source_representation);
    let target_tiled = representation_is_exactly_tiled(&target_representation);
    let source_fact_free = record_view_type_is_fact_free(program, source_type, &mut HashSet::new());
    let target_fact_free =
        record_view_type_is_fact_free(program, element_type, &mut HashSet::new());
    let compatible = if source_tiled && target_tiled && target_fact_free {
        !mutable_recast || source_fact_free
    } else if mutable_recast {
        mutable_record_representations_equivalent(
            program,
            &source_representation,
            &target_representation,
        )
    } else {
        record_representation_implies(program, &source_representation, &target_representation)
    };
    if !compatible {
        diagnostics.push(Diagnostic::error(format!(
            "{context}: slice recast `{target_label}` does not preserve exact byte tiling and {}; \
             raw storage cannot establish element facts",
            if mutable_recast {
                "fact implication in BOTH directions"
            } else {
                "source-to-target fact implication"
            },
        )));
    }
}

/// Exact value of an interior byte offset when its syntax or declared range
/// pins one value. A mere upper bound is sufficient for fixed-footprint views,
/// but an unsized slice with multi-byte elements additionally owes divisibility
/// of the complete remaining region; an interval cannot prove that congruence.
fn exact_interior_byte_region_offset(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    source: ExpressionHandle,
) -> Option<i64> {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(source) else {
        return None;
    };
    if let ExpressionNode::Integer(literal) = program.expression_table.expression(indexed.index) {
        return literal.value_i64().filter(|offset| *offset >= 0);
    }
    let type_reference =
        crate::places::declared_place_type_raw(program, machine, Some(state), indexed.index)?;
    let interval = crate::arithmetic_domains::range_constraint_interval(program, type_reference)?;
    let (Some(low), Some(high)) = (interval.low(), interval.high()) else {
        return None;
    };
    (low == high && low >= 0).then_some(low)
}

/// The companion hole-closer: a reference-typed let over a BARE borrow of a
/// scalar place whose type disagrees with the stated referee. Without this,
/// `let v: &f32 = &self.x;` (x: i64) is an accidental, unjudged recast --
/// native bit-puns while the interpreter delivers the semantic value.
fn report_unspelled_reference_pun(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    initializer: ExpressionHandle,
    let_referee: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = strip_mutable(program, initializer);
    let Some(source_primitive) =
        crate::places::declared_place_type(program, machine, Some(state), source)
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
    else {
        return;
    };
    let Some(referee_primitive) = crate::places::unwrapped_type_reference(program, let_referee)
        .and_then(|unwrapped| program.primitive_type_reference(unwrapped))
    else {
        return;
    };
    if source_primitive != referee_primitive {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}`: reference initializer type `{}` must match the \
             stated `&{}`; re-viewing a place's bytes under another shape is spelled \
             `&x as &{}` (§5b recast)",
            machine.name.as_str(),
            state.name.as_str(),
            source_primitive.name(),
            referee_primitive.name(),
            referee_primitive.name(),
        )));
    }
}

fn strip_mutable(program: &TypedTrees, expression: ExpressionHandle) -> ExpressionHandle {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => strip_mutable(program, *inner),
        _ => expression,
    }
}

/// The interior byte-region judgment's three-way answer (owner-measured
/// diagnostic split 2026-07-11: a recognized shape whose OFFSET cannot be
/// bounded must say so -- it used to fall through to the form errors
/// ("not a scalar primitive or an eligible fixed record" / "source must be a
/// borrowed scalar place"), which misled: the real failure was the unproven
/// bound).
enum InteriorByteRegion {
    /// Not `<[u8; N] place>[k]` at all -- fall through to the other source
    /// classes and their form messages.
    NotInteriorShape,
    /// The shape is right, but no route bounds the runtime offset.
    OffsetUnproven {
        offset_display: String,
        region_length: i64,
    },
    /// `k` (or its proven upper bound) and `N`.
    Bounded { offset: i64, region_length: i64 },
}

/// Rungs B/C1's interior source: `<[u8; N] place>[k]`. Shape is recognized
/// FIRST (byte-element fixed array, literal length); the offset bound then
/// comes from a literal, the declared range, the dominating incoming
/// guard, or the boundary-ensures witness.
fn interior_byte_region_source(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    source: ExpressionHandle,
) -> InteriorByteRegion {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(source) else {
        return InteriorByteRegion::NotInteriorShape;
    };
    let Some(collection_type) =
        crate::places::declared_place_type(program, machine, Some(state), indexed.collection)
    else {
        return InteriorByteRegion::NotInteriorShape;
    };
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
        ..
    } = program.type_reference_table.type_reference(collection_type)
    else {
        return InteriorByteRegion::NotInteriorShape;
    };
    let element_is_byte = crate::places::unwrapped_type_reference(program, *element_type)
        .and_then(|unwrapped| program.primitive_type_reference(unwrapped))
        == Some(PrimitiveType::U8);
    if !element_is_byte {
        return InteriorByteRegion::NotInteriorShape;
    }
    let psi_typed_trees::types::FixedArrayLength::Literal(length) = length else {
        return InteriorByteRegion::NotInteriorShape;
    };
    let region_length = *length as i64;

    // RUNG C1: a RUNTIME offset (`&self.buf[k] as &u32`) discharges through
    // the index place's enforced interval -- its declared range (dependent
    // maxima substitute through the field's own range) bounds the offset,
    // so `high(k) + size(T) <= N` is the footprint check. The interval is
    // store-enforced/caller-proved by the R1 machinery, so it is a true
    // bound at every read. Gap #4 routes: the dominating incoming-arm
    // guard, and the R4 boundary-ensures witness.
    let offset = match program.expression_table.expression(indexed.index) {
        ExpressionNode::Integer(literal) => literal.value_i64().filter(|offset| *offset >= 0),
        _ => {
            let declared_high = crate::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                indexed.index,
            )
            .and_then(|raw| {
                let interval = crate::arithmetic_domains::range_constraint_interval(program, raw)?;
                let high = interval.high()?;
                (!interval.low().is_some_and(|low| low < 0) && high >= 0).then_some(high)
            });
            declared_high
                .or_else(|| incoming_guard_offset_bound(program, machine, state, indexed.index))
        }
    };
    match offset {
        Some(offset) => InteriorByteRegion::Bounded {
            offset,
            region_length,
        },
        None => InteriorByteRegion::OffsetUnproven {
            offset_display: program.expression_table.display_name(indexed.index),
            region_length,
        },
    }
}

fn push_offset_unproven(
    diagnostics: &mut Vec<Diagnostic>,
    context: &str,
    offset_display: &str,
    region_length: i64,
) {
    diagnostics.push(Diagnostic::error(format!(
        "{context}: cannot bound the recast offset `{offset_display}` -- the region holds \
         {region_length} bytes, but no declared range, dominating incoming guard, or \
         boundary-ensures witness bounds the offset below the footprint. Bound it: declare \
         a range on the offset param, guard the transition arm (`transition \
         {offset_display} <= K {{ true -> ... }}`), or `ensures`-bound the boundary \
         out-param that feeds it",
    )));
}

/// Mutable byte views must preserve every target fact after arbitrary writes.
/// Until the general bidirectional entailment solver lands, the complete
/// decidable subset is a raw named record whose transitive fields are raw
/// fact-free scalar primitives or records with no default-domain facts.
fn record_view_is_fact_free(
    program: &TypedTrees,
    name: &str,
    visiting: &mut HashSet<String>,
) -> bool {
    if !visiting.insert(name.to_owned()) {
        return false;
    }
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == name)
    else {
        visiting.remove(name);
        return false;
    };
    if !data.where_facts.is_empty() || data.zero_gated {
        visiting.remove(name);
        return false;
    }
    for member in program.data_members(data) {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            visiting.remove(name);
            return false;
        };
        if !record_view_type_is_fact_free(program, field.type_reference, visiting) {
            visiting.remove(name);
            return false;
        }
    }
    visiting.remove(name);
    true
}

fn record_view_type_is_fact_free(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut HashSet<String>,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => {
            PrimitiveType::from_name(name.as_str())
                .is_some_and(|primitive| primitive != PrimitiveType::Bool)
                || record_view_is_fact_free(program, name.as_str(), visiting)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => record_view_type_is_fact_free(program, *element_type, visiting),
        _ => false,
    }
}

/// Walk one statement's expressions for qualification casts and judge each
/// WITH machine/state context: the literal fold first, then the value's
/// DECLARED RANGE (flow-integration v1 -- a Name whose declared type
/// carries `[lo..=hi]` discharges facts the whole interval satisfies).
fn judge_statement_qualification_casts(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statement: &StatementNode,
    judged: &mut Vec<ExpressionHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut roots: Vec<ExpressionHandle> = Vec::new();
    match statement {
        StatementNode::AssemblyFact(fact) => roots.push(fact.expression),
        StatementNode::Assignment(assignment) => {
            roots.push(assignment.target);
            roots.push(assignment.value);
        }
        StatementNode::Expression(expression) => roots.push(*expression),
        StatementNode::LocalData(local) => roots.push(local.initial_value),
        StatementNode::Call(call) => roots.extend(
            program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .copied(),
        ),
        StatementNode::Transition(transition) => {
            if let psi_typed_trees::statement::TransitionGuardNode::When(guard) = &transition.guard
            {
                roots.push(*guard);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    psi_typed_trees::statement::TransitionTargetNode::Value(value) => {
                        roots.push(*value);
                    }
                    psi_typed_trees::statement::TransitionTargetNode::Named {
                        arguments, ..
                    } => {
                        roots.extend(
                            program
                                .statement_table
                                .expression_handles(*arguments)
                                .iter()
                                .copied(),
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    for root in roots {
        judge_expression_qualification_casts(program, machine, state, root, judged, diagnostics);
    }
}

fn judge_expression_qualification_casts(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    expression: ExpressionHandle,
    judged: &mut Vec<ExpressionHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Cast(cast) => {
            if cast.semantic_domain.count() > 0 {
                judged.push(expression);
                judge_qualification_cast(program, Some((machine, state)), cast, diagnostics);
            }
            judge_expression_qualification_casts(
                program,
                machine,
                state,
                cast.value,
                judged,
                diagnostics,
            );
        }
        ExpressionNode::Binary(binary) => {
            judge_expression_qualification_casts(
                program,
                machine,
                state,
                binary.left,
                judged,
                diagnostics,
            );
            judge_expression_qualification_casts(
                program,
                machine,
                state,
                binary.right,
                judged,
                diagnostics,
            );
        }
        ExpressionNode::Unary(unary) => {
            judge_expression_qualification_casts(
                program,
                machine,
                state,
                unary.operand,
                judged,
                diagnostics,
            );
        }
        ExpressionNode::Mutable(inner) => {
            judge_expression_qualification_casts(
                program,
                machine,
                state,
                *inner,
                judged,
                diagnostics,
            );
        }
        ExpressionNode::Call(call) => {
            for argument in program.expression_table.expression_handles(call.arguments) {
                judge_expression_qualification_casts(
                    program,
                    machine,
                    state,
                    *argument,
                    judged,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

/// The mint's tri-state judgment for one qualification cast.
enum MintJudgment {
    Discharged,
    FactFalse,
    NotLiteral,
}

/// Fold every domain fact at the cast's literal value (`self := literal`).
/// Only integer literals and `self <op> literal` / `literal <op> self`
/// comparison facts fold; anything else is conservatively NotLiteral.
/// `introduction`-clause pseudo-facts (non-Binary) are skipped -- they are
/// policy, not predicate.
fn literal_mint_discharges(
    program: &TypedTrees,
    domain: &psi_typed_trees::domain::DomainDefinition,
    value: ExpressionHandle,
) -> MintJudgment {
    let ExpressionNode::Integer(literal) = program.expression_table.expression(value) else {
        return MintJudgment::NotLiteral;
    };
    let Ok(minted) = literal.text().parse::<i128>() else {
        return MintJudgment::NotLiteral;
    };
    for fact in program.proof_facts.span_or_empty(domain.facts) {
        let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
            continue;
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
        else {
            continue;
        };
        let side_value = |handle: ExpressionHandle| -> Option<i128> {
            match program.expression_table.expression(handle) {
                ExpressionNode::Name(path)
                    if matches!(
                        program.expression_table.name_path_members(path.members),
                        [only] if only.as_str() == "self"
                    ) =>
                {
                    Some(minted)
                }
                ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
                _ => None,
            }
        };
        let (Some(left), Some(right)) = (side_value(binary.left), side_value(binary.right)) else {
            return MintJudgment::NotLiteral;
        };
        use psi_typed_trees::expression::BinaryOperator;
        let holds = match binary.operator {
            BinaryOperator::Less => left < right,
            BinaryOperator::LessOrEqual => left <= right,
            BinaryOperator::Greater => left > right,
            BinaryOperator::GreaterOrEqual => left >= right,
            BinaryOperator::Equal => left == right,
            BinaryOperator::NotEqual => left != right,
            _ => return MintJudgment::NotLiteral,
        };
        if !holds {
            return MintJudgment::FactFalse;
        }
    }
    MintJudgment::Discharged
}

/// Requires-route discharge: the machine's REQUIRES facts about the cast
/// value's NAME accumulate one-sided bounds (`raw >= 0` -> low = 0); the
/// domain facts must be entailed by those bounds. `None` when the value is
/// not a bare name or no requires fact speaks about it.
fn requires_mint_discharges(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    domain: &psi_typed_trees::domain::DomainDefinition,
    value: ExpressionHandle,
) -> Option<MintJudgment> {
    use psi_typed_trees::expression::BinaryOperator;
    use psi_typed_trees::signature::SignatureContractKind;

    let ExpressionNode::Name(path) = program.expression_table.expression(value) else {
        return None;
    };
    let [value_name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };

    let mut low: Option<i64> = None;
    let mut high: Option<i64> = None;
    let mut spoke = false;
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
            else {
                continue;
            };
            let names_value = |handle: ExpressionHandle| {
                matches!(
                    program.expression_table.expression(handle),
                    ExpressionNode::Name(fact_path)
                        if matches!(
                            program.expression_table.name_path_members(fact_path.members),
                            [only] if only.as_str() == value_name.as_str()
                        )
                )
            };
            let literal_of = |handle: ExpressionHandle| -> Option<i64> {
                match program.expression_table.expression(handle) {
                    ExpressionNode::Integer(value) => value.text().parse::<i64>().ok(),
                    _ => None,
                }
            };
            let (operator, bound) = if names_value(binary.left) {
                let Some(bound) = literal_of(binary.right) else {
                    continue;
                };
                (binary.operator, bound)
            } else if names_value(binary.right) {
                let Some(bound) = literal_of(binary.left) else {
                    continue;
                };
                let flipped = match binary.operator {
                    BinaryOperator::Less => BinaryOperator::Greater,
                    BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
                    BinaryOperator::Greater => BinaryOperator::Less,
                    BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
                    other => other,
                };
                (flipped, bound)
            } else {
                continue;
            };
            spoke = true;
            match operator {
                BinaryOperator::GreaterOrEqual => low = Some(low.map_or(bound, |l| l.max(bound))),
                BinaryOperator::Greater => {
                    let floor = bound.saturating_add(1);
                    low = Some(low.map_or(floor, |l| l.max(floor)));
                }
                BinaryOperator::LessOrEqual => high = Some(high.map_or(bound, |h| h.min(bound))),
                BinaryOperator::Less => {
                    let ceiling = bound.saturating_sub(1);
                    high = Some(high.map_or(ceiling, |h| h.min(ceiling)));
                }
                BinaryOperator::Equal => {
                    low = Some(low.map_or(bound, |l| l.max(bound)));
                    high = Some(high.map_or(bound, |h| h.min(bound)));
                }
                _ => {}
            }
        }
    }
    if !spoke {
        return None;
    }

    for fact in program.proof_facts.span_or_empty(domain.facts) {
        let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
            continue;
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
        else {
            continue;
        };
        let is_self = |handle: ExpressionHandle| {
            matches!(
                program.expression_table.expression(handle),
                ExpressionNode::Name(fact_path)
                    if matches!(
                        program.expression_table.name_path_members(fact_path.members),
                        [only] if only.as_str() == "self"
                    )
            )
        };
        let literal_of = |handle: ExpressionHandle| -> Option<i64> {
            match program.expression_table.expression(handle) {
                ExpressionNode::Integer(value) => value.text().parse::<i64>().ok(),
                _ => None,
            }
        };
        let (operator, bound) = if is_self(binary.left) {
            (binary.operator, literal_of(binary.right)?)
        } else if is_self(binary.right) {
            let flipped = match binary.operator {
                BinaryOperator::Less => BinaryOperator::Greater,
                BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
                BinaryOperator::Greater => BinaryOperator::Less,
                BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
                other => other,
            };
            (flipped, literal_of(binary.left)?)
        } else {
            return None;
        };
        let holds = match operator {
            BinaryOperator::GreaterOrEqual => low.is_some_and(|l| l >= bound),
            BinaryOperator::Greater => low.is_some_and(|l| l > bound),
            BinaryOperator::LessOrEqual => high.is_some_and(|h| h <= bound),
            BinaryOperator::Less => high.is_some_and(|h| h < bound),
            BinaryOperator::Equal => {
                low.is_some_and(|l| l == bound) && high.is_some_and(|h| h == bound)
            }
            BinaryOperator::NotEqual => {
                high.is_some_and(|h| h < bound) || low.is_some_and(|l| l > bound)
            }
            _ => return None,
        };
        if !holds {
            return Some(MintJudgment::NotLiteral);
        }
    }
    Some(MintJudgment::Discharged)
}
