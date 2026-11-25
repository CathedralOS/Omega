//! Bind a source guarantee to the final owner projection on the declared subject.
use super::Lowering;
use crate::body::ProjectedContent;
use crate::{Unserved, refuse};
use semantic_vocabulary::{
    ContentAlgebra, ContentConservation, ContentTerm, MachineId, ObligationId, Proposition,
};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{ContractClause, MachineContract};

impl Lowering<'_> {
    pub(super) fn qualify_content_equations(&self, contract: &mut MachineContract) -> Checked<()> {
        if !self.signature.content_ensures.is_empty()
            && contract
                .ensures
                .iter()
                .any(|clause| matches!(clause.proposition, Proposition::ContentConservation(_)))
        {
            return Err(invalid(
                self.id,
                "content equations require one authored source per guarantee",
            ));
        }
        let mut last = contract
            .ensures
            .iter()
            .map(|clause| clause.obligation.get())
            .max()
            .unwrap_or(0);
        for equation in &self.signature.content_ensures {
            let left = self.content_term(&equation.algebra, &equation.left)?;
            let right = self.content_term(&equation.algebra, &equation.right)?;
            let proposition = Proposition::ContentConservation(ContentConservation::new(
                equation.algebra.clone(),
                left,
                right,
            ));
            if contract
                .ensures
                .iter()
                .any(|prior| prior.proposition == proposition)
            {
                return Err(invalid(
                    self.id,
                    "duplicate normal content conservation guarantee",
                ));
            }
            let obligation = if self.published.is_empty() {
                last.checked_add(1).and_then(ObligationId::new)
                    .ok_or_else(|| invalid(self.id, "content guarantee obligation overflow"))?
            } else {
                *self.published.get(contract.ensures.len())
                    .ok_or_else(|| invalid(self.id, "content equation lost its published normal-return obligation"))?
            };
            last = obligation.get();
            contract.ensures.push(ContractClause {
                obligation,
                proposition,
            });
        }
        Ok(())
    }

    fn content_term(
        &self,
        algebra: &ContentAlgebra,
        source: &ProjectedContent,
    ) -> Checked<ContentTerm> {
        let (domain, carrier, expression, subject) = match source {
            ProjectedContent::Projection {
                domain,
                carrier,
                expression,
                subject,
            } => (domain, carrier, expression, subject),
            ProjectedContent::Separate(children) => {
                let children = children
                    .iter()
                    .map(|child| self.content_term(algebra, child))
                    .collect::<Checked<Vec<_>>>()?;
                return ContentTerm::separate(children).map_err(|_| {
                    invalid(self.id, "separation must contain distinct content operands")
                });
            }
        };
        let owner = self.qualifications.structural_domain_id(*domain, *carrier)
            .and_then(|id| self.qualifications.structural.iter().find(|row| row.id == id && row.carrier == *carrier))
            .and_then(|row| row.content_projection.as_ref())
            .filter(|projection| projection.algebra == *algebra && projection.expression == *expression)
            .ok_or_else(|| invalid(self.id, "content guarantee lost the declared owner's normalized projection or its algebra"))?;
        Ok(ContentTerm::Projection {
            projection: owner.identity,
            subject: subject.clone(),
        })
    }
}

fn invalid(id: MachineId, detail: &'static str) -> terminal_check_tables::Refusal {
    refuse(id, Construct::Machine, Unserved::Contract(detail))
}
