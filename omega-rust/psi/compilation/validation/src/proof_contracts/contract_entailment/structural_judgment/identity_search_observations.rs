//! Temporary bounded output of existing Nat Id query decisions. No proof,
//! resolution, source observation or proposition intake is performed here.

use super::{StructuralJudge, StructuralTerm};
use std::sync::{
    OnceLock,
    atomic::{AtomicUsize, Ordering},
};

impl StructuralJudge<'_> {
    pub(super) fn trace_identity_search(
        &self,
        stage: &str,
        terms: Option<(&StructuralTerm, &StructuralTerm)>,
        detail: std::fmt::Arguments<'_>,
    ) -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        static MUL: [AtomicUsize; 4] = [const { AtomicUsize::new(0) }; 4];
        static GAP: [AtomicUsize; 4] = [const { AtomicUsize::new(0) }; 4];
        if !*ENABLED.get_or_init(|| std::env::var_os("OMEGA_IDENTITY_SEARCH_TRACE").is_some()) {
            return false;
        }
        let Some(machine) = self
            .program
            .machine_by_symbol(self.machine_symbol)
            .or_else(|| self.program.machine_holding_state(self.machine_symbol))
        else {
            return false;
        };
        let counters = match machine.name.as_str() {
            "mul_identity" => &MUL,
            "nat_gap_zero_left" => &GAP,
            _ => return false,
        };
        let (index, limit) = match stage {
            "query" => (0, 2),
            "result" => (1, 2),
            "established" => (2, 12),
            _ => (3, 32),
        };
        let event = counters[index].fetch_add(1, Ordering::Relaxed);
        if event >= limit {
            return false;
        }
        eprintln!(
            "IDPATH owner={} symbol={:?} stage={stage} event={event} {detail}",
            machine.name, self.machine_symbol
        );
        if let Some((left, right)) = terms {
            trace_term("left", left, 0, &mut 16);
            trace_term("right", right, 0, &mut 16);
        }
        stage == "query" && event == 0
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
            "IDPATH {side} depth={depth} application target={target:?} result={result_type:?} selections={} arguments={}",
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
                "IDPATH {side} depth={depth} constructor carrier={carrier:?} reference={reference:?} declaration={declaration:?} case={case:?} member={member:?} fields={}",
                fields.len()
            ),
            super::ConstructorIdentity::Boolean(value) => {
                eprintln!("IDPATH {side} depth={depth} boolean={value}")
            }
        },
        StructuralTerm::ObservedValue {
            observed_type,
            descriptor,
            ..
        } => eprintln!(
            "IDPATH {side} depth={depth} observed={observed_type:?} descriptor={}",
            descriptor.is_some()
        ),
        StructuralTerm::IntegerEmbedding { reference, .. } => {
            eprintln!("IDPATH {side} depth={depth} embedding={reference:?}")
        }
        StructuralTerm::Projection { path, .. } => {
            eprintln!("IDPATH {side} depth={depth} projection={path}")
        }
        StructuralTerm::Variable(name) => eprintln!("IDPATH {side} depth={depth} variable={name}"),
        StructuralTerm::BoundValue(symbol) => {
            eprintln!("IDPATH {side} depth={depth} bound={symbol:?}")
        }
        StructuralTerm::BoundProjection { subject, path } => {
            eprintln!("IDPATH {side} depth={depth} bound_projection={subject:?}/{path}")
        }
        StructuralTerm::CapturedValue {
            expression,
            statement,
            callable,
            occurrence,
            result_type,
            ..
        } => eprintln!(
            "IDPATH {side} depth={depth} capture={expression:?}/{statement:?}/{callable:?}/{occurrence} result={result_type:?}"
        ),
        StructuralTerm::Opaque(text) => eprintln!(
            "IDPATH {side} depth={depth} opaque={:?}",
            text.chars().take(160).collect::<String>()
        ),
        _ => eprintln!(
            "IDPATH {side} depth={depth} kind={:?}",
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
