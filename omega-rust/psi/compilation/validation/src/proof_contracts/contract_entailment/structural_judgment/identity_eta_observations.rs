//! Temporary bounded observations of the already-computed first Int Id query.
//! No resolver, judgment, source replay or proposition intake runs here.

use super::{StructuralJudge, StructuralTerm};
use std::sync::{
    OnceLock,
    atomic::{AtomicUsize, Ordering},
};

impl StructuralJudge<'_> {
    pub(super) fn trace_identity_eta(
        &self,
        stage: &str,
        left: Option<&StructuralTerm>,
        right: Option<&StructuralTerm>,
        detail: std::fmt::Arguments<'_>,
    ) {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        static QUERIES: AtomicUsize = AtomicUsize::new(0);
        static RESULTS: AtomicUsize = AtomicUsize::new(0);
        static FIELDS: AtomicUsize = AtomicUsize::new(0);
        static GUARDS: AtomicUsize = AtomicUsize::new(0);
        if !*ENABLED.get_or_init(|| std::env::var_os("OMEGA_IDENTITY_ETA_TRACE").is_some()) {
            return;
        }
        let machine = self
            .program
            .machine_by_symbol(self.machine_symbol)
            .or_else(|| self.program.machine_holding_state(self.machine_symbol));
        if machine.is_none_or(|machine| machine.name.as_str() != "add_int_identity") {
            return;
        }
        let (counter, limit) = match stage {
            "query" => (&QUERIES, 2),
            "query-result" => (&RESULTS, 2),
            "eta-field" => (&FIELDS, 4),
            _ => (&GUARDS, 8),
        };
        let event = counter.fetch_add(1, Ordering::Relaxed);
        if event >= limit {
            return;
        }
        eprintln!(
            "IDETA event={event} owner={:?} stage={stage} {detail}",
            self.machine_symbol
        );
        for (side, value) in [("left", left), ("right", right)] {
            if let Some(value) = value {
                trace_term(side, value, 0, &mut 16);
            }
        }
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
        } => {
            eprintln!(
                "IDETA {side} depth={depth} application target={target:?} result={result_type:?} selections={} arguments={}",
                selections.len(),
                arguments.len()
            );
        }
        StructuralTerm::Constructor {
            identity, fields, ..
        } => {
            eprintln!(
                "IDETA {side} depth={depth} constructor={identity:?} fields={}",
                fields.len()
            );
        }
        StructuralTerm::ObservedValue {
            observed_type,
            descriptor,
            ..
        } => {
            eprintln!(
                "IDETA {side} depth={depth} observed_type={observed_type:?} descriptor={}",
                descriptor.is_some()
            );
        }
        StructuralTerm::IntegerEmbedding { reference, .. } => {
            eprintln!("IDETA {side} depth={depth} integer_embedding={reference:?}");
        }
        StructuralTerm::Projection { path, .. } => {
            eprintln!("IDETA {side} depth={depth} projection={path}");
        }
        StructuralTerm::Variable(name) => {
            eprintln!("IDETA {side} depth={depth} variable={name}");
        }
        StructuralTerm::BoundValue(symbol) => {
            eprintln!("IDETA {side} depth={depth} bound_value={symbol:?}");
        }
        StructuralTerm::BoundProjection { subject, path } => {
            eprintln!("IDETA {side} depth={depth} bound_projection={subject:?}/{path}");
        }
        StructuralTerm::Opaque(text) => {
            let prefix = text.chars().take(160).collect::<String>();
            eprintln!("IDETA {side} depth={depth} opaque={prefix:?}");
        }
        StructuralTerm::CapturedValue {
            expression,
            statement,
            occurrence,
            result_type,
            ..
        } => {
            eprintln!(
                "IDETA {side} depth={depth} capture={expression:?}/{statement:?}/{occurrence} result={result_type:?}"
            );
        }
        _ => eprintln!(
            "IDETA {side} depth={depth} kind={:?}",
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
