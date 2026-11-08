//! Temporary observations of the remaining Rat close proof. Reads existing query
//! values and decisions only; no proof, resolution or intake is performed here.

use super::{StructuralJudge, StructuralTerm};
use std::sync::{
    OnceLock,
    atomic::{AtomicUsize, Ordering},
};

impl StructuralJudge<'_> {
    fn le_case_trace_owner(&self) -> Option<usize> {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        if !*ENABLED.get_or_init(|| std::env::var_os("OMEGA_LE_CASE_TRACE").is_some()) {
            return None;
        }
        let machine = self
            .program
            .machine_by_symbol(self.machine_symbol)
            .or_else(|| self.program.machine_holding_state(self.machine_symbol))?;
        match machine.name.as_str() {
            "rat_close_triangle_split" => Some(1),
            _ => None,
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn le_case_trace_enabled(&self) -> bool {
        self.le_case_trace_owner().is_some()
    }

    pub(in crate::proof_contracts::contract_entailment) fn trace_le_case(
        &self,
        stage: &str,
        terms: &[(&str, &StructuralTerm)],
        detail: std::fmt::Arguments<'_>,
    ) -> bool {
        static MAIN: [AtomicUsize; 25] = [const { AtomicUsize::new(0) }; 25];
        static STEP: [AtomicUsize; 25] = [const { AtomicUsize::new(0) }; 25];
        let Some(owner) = self.le_case_trace_owner() else {
            return false;
        };
        let (name, counters) = match owner {
            0 => ("rat_gap_triangle_scaled", &MAIN),
            _ => ("rat_close_triangle_split", &STEP),
        };
        let (index, limit) = match stage {
            "query" => (0, 4),
            "result" => (1, 4),
            "typed-row" => (2, 192),
            "directed-row" => (3, 192),
            "intake-original" => (4, 12),
            "intake-normalized" => (5, 12),
            "intake-owned" => (6, 12),
            "constructor-clash" => (7, 4),
            "normalized-goal" => (8, 64),
            "case-fact" => (9, 16),
            "substitution" => (10, 16),
            "case-substitution" => (11, 16),
            "component-preparation" => (13, 8),
            "component-target-cost" => (14, 4),
            "component-lookup" => (15, 8),
            "representative-normalization" => (16, 4),
            "citation-failure" => (17, 4),
            "instantiated-fact" => (18, 4),
            "predicate-formation" => (19, 4),
            "target-equality-skip" => (20, 48),
            "representative-cache-hit" => (21, 48),
            "representative-cache-miss" => (22, 48),
            "head-classification" => (23, 8),
            "discovered-target-equality-skip" => (24, 48),
            _ => (12, 4),
        };
        let event = counters[index].fetch_add(1, Ordering::Relaxed);
        if event >= limit {
            return false;
        }
        eprintln!(
            "IDCASE owner={} symbol={:?} stage={stage} event={event} contradictory={} cases_only={} case_facts={} substitutions={} case_substitutions={} {detail}",
            name,
            self.machine_symbol,
            self.hypotheses_contradictory,
            self.case_facts_only,
            self.case_facts.len(),
            self.substitutions.len(),
            self.case_substitutions.len()
        );
        for (side, value) in terms {
            trace_term(side, value, 0, &mut 12);
        }
        stage == "query"
    }
}

fn trace_term(side: &str, value: &StructuralTerm, depth: usize, remaining: &mut usize) {
    if depth >= 6 || *remaining == 0 {
        return;
    }
    *remaining -= 1;
    match value {
        StructuralTerm::Application {
            result_type,
            target,
            selections,
            arguments,
            ..
        } => eprintln!(
            "IDCASE {side} depth={depth} application target={target:?} result={result_type:?} selections={} arguments={}",
            selections.len(),
            arguments.len()
        ),
        StructuralTerm::Constructor {
            identity, fields, ..
        } => match identity {
            super::ConstructorIdentity::Nominal {
                carrier,
                reference,
                declaration,
                case,
                member,
            } => eprintln!(
                "IDCASE {side} depth={depth} constructor carrier={carrier:?} reference={reference:?} declaration={declaration:?} case={case:?} member={member:?} fields={}",
                fields.len()
            ),
            super::ConstructorIdentity::Boolean(value) => {
                eprintln!("IDCASE {side} depth={depth} boolean={value}")
            }
        },
        StructuralTerm::ObservedValue {
            observed_type,
            descriptor,
            ..
        } => eprintln!(
            "IDCASE {side} depth={depth} observed={observed_type:?} descriptor={}",
            descriptor.is_some()
        ),
        StructuralTerm::IntegerEmbedding { reference, .. } => {
            eprintln!("IDCASE {side} depth={depth} embedding={reference:?}")
        }
        StructuralTerm::Projection { path, .. } => {
            eprintln!("IDCASE {side} depth={depth} projection={path}")
        }
        StructuralTerm::Variable(name) => eprintln!("IDCASE {side} depth={depth} variable={name}"),
        StructuralTerm::BoundValue(symbol) => {
            eprintln!("IDCASE {side} depth={depth} bound={symbol:?}")
        }
        StructuralTerm::BoundProjection { subject, path } => {
            eprintln!("IDCASE {side} depth={depth} bound_projection={subject:?}/{path}")
        }
        StructuralTerm::CapturedValue {
            expression,
            statement,
            callable,
            occurrence,
            result_type,
            ..
        } => eprintln!(
            "IDCASE {side} depth={depth} capture={expression:?}/{statement:?}/{callable:?}/{occurrence} result={result_type:?}"
        ),
        StructuralTerm::Opaque(text) => eprintln!(
            "IDCASE {side} depth={depth} opaque={:?}",
            text.chars().take(160).collect::<String>()
        ),
        _ => eprintln!(
            "IDCASE {side} depth={depth} kind={:?}",
            std::mem::discriminant(value)
        ),
    }
    if depth + 1 < 6 && *remaining != 0 {
        value.any_child(|child| {
            trace_term(side, child, depth + 1, remaining);
            *remaining == 0
        });
    }
}
