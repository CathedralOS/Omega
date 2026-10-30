//! Exact source occurrences keep their authored identity and follow each new
//! static invocation. Rebinding these rows invents neither a loan nor a proof.

use super::IdentityMap;
use crate::lowering_error::LoweringError;
use lowered_psi::{LoweredSourceCallOccurrence, LoweredSourceCallSite, LoweredSourceLoanCapture};
use terminal_psi::{LoanCompletion, ScalarBlockInvariant};

impl IdentityMap {
    pub(crate) fn loan_capture(&mut self, capture: &mut LoweredSourceLoanCapture) {
        capture.formation = self.operation(capture.formation);
        capture.carrier = self.place(capture.carrier);
        self.argument(&mut capture.source);
    }

    pub(crate) fn source_call(&mut self, call: &mut LoweredSourceCallOccurrence) {
        call.terminal_site = match call.terminal_site {
            LoweredSourceCallSite::Operation(operation) => {
                LoweredSourceCallSite::Operation(self.operation(operation))
            }
            LoweredSourceCallSite::InvokeNoNormal(edge) => {
                LoweredSourceCallSite::InvokeNoNormal(self.edge(edge))
            }
            LoweredSourceCallSite::ExternalCompletion(edge) => {
                LoweredSourceCallSite::ExternalCompletion(self.edge(edge))
            }
        };
        let frontier = &mut call.frontier_before_call;
        for capture in &mut frontier.loan_captures {
            self.loan_capture(capture);
        }
        for binding in &mut frontier.scalars {
            binding.value.id = self.value(binding.value.id);
        }
        for binding in &mut frontier.structural {
            self.argument(&mut binding.argument);
        }
        for (_, claim) in &mut frontier.claims {
            *claim = self.claim(*claim);
        }
    }

    pub(crate) fn loan_completion(&mut self, completion: &mut LoanCompletion) {
        completion.operation = self.operation(completion.operation);
        completion.place = self.place(completion.place);
        self.path(&mut completion.path);
        for claim in &mut completion.claims {
            claim.claim = self.claim(claim.claim);
            self.path(&mut claim.path);
        }
        completion.claims.sort_by_key(|row| row.claim);
    }

    /// Arrivals must first be selected for this particular specialized header;
    /// the worklist owns which original edges actually reach it.
    pub(crate) fn block_invariant(
        &mut self,
        invariant: &mut ScalarBlockInvariant,
    ) -> Result<(), LoweringError> {
        invariant.header = self.block_id(invariant.header);
        self.proposition(&mut invariant.predicate)?;
        for arrival in &mut invariant.arrivals {
            arrival.edge = self.edge(arrival.edge);
            arrival.obligation = self.obligation(arrival.obligation);
        }
        invariant.arrivals.sort_by_key(|arrival| arrival.edge);
        Ok(())
    }
}
