/*
    PILOT ONLY, not for main: a run-time oracle for checker soundness.

    When OMEGA_SOUNDNESS_ORACLE names a file, every machine entry evaluates
    the callee's `requires` clauses against the arguments it actually received
    and appends one line per clause: `ok`, `VIOLATION` (closed and false) or
    `undecided` (the evaluator could not close it). An accepted program that
    produces a VIOLATION line ran with a proved precondition false.
*/

use crate::boundary::errors::TerminalInterpretError;
use crate::engine::execution::TerminalExecution;
use proof_admission::{ClosedIntegerEvaluator, closed_guard, closed_guard_with};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerMathTerm, IntegerValue, PlaceId, Proposition,
    ScalarReadProjection, ScalarTerm, ScalarType, ValueId,
};
use std::collections::BTreeMap;
use std::io::Write;

fn sink() -> Option<std::fs::File> {
    let path = std::env::var_os("OMEGA_SOUNDNESS_ORACLE")?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()
}

#[derive(Debug)]
struct Unreadable;

impl TerminalExecution<'_> {
    pub(crate) fn soundness_oracle_on_entry(&self, callee: &terminal_psi::TerminalMachine) {
        let Some(mut out) = sink() else { return };
        let clauses = &callee.contract.requires;
        if clauses.is_empty() {
            return;
        }
        let mut substitutions: BTreeMap<ValueId, ScalarTerm> = BTreeMap::new();
        for parameter in &callee.parameters {
            if let Some(literal) = self.values.get(&parameter.id).and_then(|v| v.literal()) {
                substitutions.insert(parameter.id, literal);
            }
        }
        for (position, clause) in clauses.iter().enumerate() {
            let clause =
                terminal_semantics::substitution::substitute_proposition_values(clause, &substitutions);
            let verdict = self.oracle_evaluate(callee, &clause);
            let label = match verdict {
                Ok(Some(true)) => "ok".to_string(),
                Ok(Some(false)) => "VIOLATION".to_string(),
                Ok(None) => "undecided".to_string(),
                Err(reason) => format!("undecided({reason})"),
            };
            let _ = writeln!(
                out,
                "requires machine={} clause={position} {label} :: {clause:?}",
                callee.id.get()
            );
        }
    }

    pub(crate) fn soundness_oracle_on_return(
        &self,
        machine: &terminal_psi::TerminalMachine,
        result: Option<crate::boundary::values::TerminalScalarValue>,
    ) {
        let Some(mut out) = sink() else { return };
        if machine.contract.ensures.is_empty() {
            return;
        }
        let mut substitutions: BTreeMap<ValueId, ScalarTerm> = BTreeMap::new();
        for parameter in &machine.parameters {
            if let Some(literal) = self.values.get(&parameter.id).and_then(|v| v.literal()) {
                substitutions.insert(parameter.id, literal);
            }
        }
        if let (Some(declaration), Some(value)) = (machine.result.scalar(), result) {
            if let Some(literal) = value.literal() {
                substitutions.insert(declaration.id, literal);
            }
        }
        for (position, clause) in machine.contract.ensures.iter().enumerate() {
            let clause = terminal_semantics::substitution::substitute_proposition_values(
                &clause.proposition,
                &substitutions,
            );
            let label = match self.oracle_evaluate(machine, &clause) {
                Ok(Some(true)) => "ok".to_string(),
                Ok(Some(false)) => "VIOLATION".to_string(),
                Ok(None) => "undecided".to_string(),
                Err(reason) => format!("undecided({reason})"),
            };
            let _ = writeln!(
                out,
                "ensures machine={} clause={position} {label} :: {clause:?}",
                machine.id.get()
            );
        }
    }

    fn oracle_evaluate(
        &self,
        callee: &terminal_psi::TerminalMachine,
        clause: &Proposition,
    ) -> Result<Option<bool>, String> {
        let mut integers = ClosedIntegerEvaluator::default();
        closed_guard_with(clause, |leaf| {
            let mut closed = leaf.clone();
            self.oracle_close_predicate(callee, &mut closed)
                .map_err(|_| "unreadable".to_string())?;
            closed_guard(&closed, &mut integers).map_err(|error| format!("{error:?}"))
        })
    }

    fn oracle_close_predicate(
        &self,
        callee: &terminal_psi::TerminalMachine,
        predicate: &mut Proposition,
    ) -> Result<(), Unreadable> {
        match predicate {
            semantic_vocabulary::ordered_comparison_operands!(left, right)
            | Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                self.oracle_close_scalar(callee, left)?;
                self.oracle_close_scalar(callee, right)?;
            }
            semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                self.oracle_close_integer(callee, left)?;
                self.oracle_close_integer(callee, right)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn oracle_close_integer(
        &self,
        callee: &terminal_psi::TerminalMachine,
        term: &mut IntegerMathTerm,
    ) -> Result<(), Unreadable> {
        if let IntegerMathTerm::Embed(scalar) = term {
            self.oracle_close_scalar(callee, scalar)?;
        } else {
            for operand in term.operands_mut() {
                self.oracle_close_integer(callee, operand)?;
            }
        }
        Ok(())
    }

    fn oracle_close_scalar(
        &self,
        callee: &terminal_psi::TerminalMachine,
        term: &mut ScalarTerm,
    ) -> Result<(), Unreadable> {
        for operand in term.operands_mut() {
            self.oracle_close_scalar(callee, operand)?;
        }
        let read = match term {
            ScalarTerm::StructuralRead {
                root,
                path,
                scalar_type,
            } => {
                let mut selected = Vec::with_capacity(path.len());
                for segment in path.iter() {
                    selected.push(match segment {
                        ScalarReadProjection::Static(segment) => *segment,
                        ScalarReadProjection::Referent => CanonicalStructuralPathSegment::Referent,
                        ScalarReadProjection::Index(index) => {
                            let (_, value) = index.integer_value().ok_or(Unreadable)?;
                            let index = match value {
                                IntegerValue::Unsigned(value) => u64::try_from(value),
                                IntegerValue::Signed(value) => u64::try_from(value),
                            }
                            .map_err(|_| Unreadable)?;
                            CanonicalStructuralPathSegment::FixedIndex(index)
                        }
                        ScalarReadProjection::Extent => return Err(Unreadable),
                    });
                }
                Some((*root, selected, *scalar_type))
            }
            ScalarTerm::BooleanField { root, path, .. } => {
                Some((*root, path.clone(), ScalarType::Boolean))
            }
            ScalarTerm::IntegerField {
                root,
                path,
                scalar_type,
                ..
            } => Some((*root, path.clone(), ScalarType::Integer(*scalar_type))),
            ScalarTerm::IeeeFloatField {
                root, path, format, ..
            } => Some((*root, path.clone(), ScalarType::IeeeFloat(*format))),
            _ => None,
        };
        if let Some((root, path, scalar_type)) = read {
            let value = self.oracle_read(callee, root, &path, scalar_type)?;
            *term = value.literal().ok_or(Unreadable)?;
        }
        Ok(())
    }

    fn oracle_read(
        &self,
        callee: &terminal_psi::TerminalMachine,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<crate::boundary::values::TerminalScalarValue, Unreadable> {
        let parameter = callee
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == root)
            .ok_or(Unreadable)?;
        let storage = terminal_semantics::canonical_structural_storage_path(
            self.program.structural_types().values(),
            parameter.structural_type,
            path,
        )
        .ok_or(Unreadable)?;
        self.read_primitive_place(root, &storage, scalar_type)
            .map_err(|_: TerminalInterpretError| Unreadable)
    }
}
