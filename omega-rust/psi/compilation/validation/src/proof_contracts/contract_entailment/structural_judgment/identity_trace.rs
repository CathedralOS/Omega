//! Temporary diagnostics show metadata omitted by semantic Debug formatting.
//! Debug is also a polynomial key, so tracing must not change its output.

use super::{ConstructorIdentity, StructuralJudge, StructuralTerm};

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn trace_identity_terms<'term>(
        &self,
        role: &str,
        values: impl ExactSizeIterator<Item = &'term StructuralTerm>,
    ) {
        if std::env::var_os("OMEGA_PROVIDER_DEBUG").is_none() {
            return;
        }
        eprintln!(
            "IDTRACE {role} owner={:?} name={:?} runtime={} applications={} closed={} values={}",
            self.machine_symbol,
            self.program
                .machine(self.machine_symbol)
                .map(|machine| &machine.name),
            self.runtime_body_values,
            self.resolve_applications,
            self.require_closed_computation,
            values.len(),
        );
        for (index, value) in values.enumerate() {
            trace_value(self.program, &format!("{role}.{index}"), value);
        }
    }

    pub(super) fn trace_unknown_identity(&self, left: &[StructuralTerm], right: &[StructuralTerm]) {
        if std::env::var_os("OMEGA_PROVIDER_DEBUG").is_none() {
            return;
        }
        self.trace_identity_terms("unknown.left", left.iter());
        self.trace_identity_terms("unknown.right", right.iter());
        eprintln!(
            "IDTRACE receipts owner={:?} reads={} equations={} guarantees={}",
            self.machine_symbol,
            self.completed_nominal_values.len(),
            self.captured_result_equations.len(),
            self.normal_guarantees.len(),
        );
        for (index, read) in self.completed_nominal_values.iter().enumerate() {
            eprintln!(
                "IDTRACE read.{index} owner={:?} anchor={:?} reference={:?}",
                self.machine_symbol, read.anchor, read.reference,
            );
            trace_value(self.program, &format!("read.{index}.capture"), &read.value);
            trace_value(
                self.program,
                &format!("read.{index}.source"),
                &read.observed_value,
            );
        }
        for (index, equation) in self.captured_result_equations.iter().enumerate() {
            eprintln!(
                "IDTRACE equation.{index} data={} call={}",
                equation.is_data_identity(),
                equation.is_completed_call_identity(),
            );
            trace_value(
                self.program,
                &format!("equation.{index}.left"),
                &equation.left,
            );
            trace_value(
                self.program,
                &format!("equation.{index}.right"),
                &equation.right,
            );
        }
        for (index, (left, right)) in self.normal_guarantees.iter().enumerate() {
            trace_value(self.program, &format!("guarantee.{index}.left"), left);
            trace_value(self.program, &format!("guarantee.{index}.right"), right);
        }
    }
}

fn trace_value(program: &dyn crate::CheckingInput, role: &str, value: &StructuralTerm) {
    eprintln!("IDTRACE term {role} semantic={value:?}");
    let mut pending = vec![(value, String::from("root"), 0usize)];
    let mut visited = 0usize;
    while let Some((value, path, depth)) = pending.pop() {
        if visited >= 256 || depth >= 32 {
            eprintln!("IDTRACE metadata {role} path={path} truncated=true");
            break;
        }
        visited += 1;
        match value {
            StructuralTerm::ObservedValue {
                observed_type,
                descriptor,
                ..
            } => {
                let node = observed_type
                    .is_valid()
                    .then(|| program.type_reference_table.type_reference(*observed_type));
                eprintln!(
                    "IDTRACE metadata {role} path={path} observed_type={observed_type:?} node={node:?} descriptor={descriptor:?}"
                );
            }
            StructuralTerm::Constructor {
                identity:
                    ConstructorIdentity::Nominal {
                        carrier,
                        reference,
                        declaration,
                        case,
                        member,
                    },
                data,
                case: name,
                ..
            } => {
                let node = reference
                    .is_valid()
                    .then(|| program.type_reference_table.type_reference(*reference));
                eprintln!(
                    "IDTRACE metadata {role} path={path} nominal={carrier:?} reference={reference:?} node={node:?} declaration={declaration:?} case={case:?} member={member:?} data={data:?} name={name:?}"
                );
            }
            StructuralTerm::ScalarBinary { references, .. } => {
                eprintln!("IDTRACE metadata {role} path={path} scalar_references={references:?}")
            }
            StructuralTerm::IndexedValue { element_type, .. } => {
                eprintln!("IDTRACE metadata {role} path={path} element_type={element_type:?}")
            }
            _ => {}
        }
        let mut child = 0usize;
        value.for_each_child(|value| {
            pending.push((value, format!("{path}.{child}"), depth + 1));
            child += 1;
        });
    }
}
