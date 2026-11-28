//! A packed nominal owner retains its fields across type applications. The
//! storage log indexes fields by their application-local IDs. Record each
//! corresponding move using the two IDs of the very same declared field.
use super::*;
use std::cell::OnceCell;
use terminal_psi::TerminalPackageEvent;
use terminal_semantics::packages::{CheckedPackageSites, check_package_sites};

impl Storage<'_> {
    pub(super) fn package_field_moves(
        &self,
        operation: &Operation,
        checked: &OnceCell<Result<CheckedPackageSites, &'static str>>,
        writes: &mut Vec<(Lineage, Stored)>,
    ) -> Checked<()> {
        let catalog = &self.module.execution.packages.sites;
        let Ok(index) = catalog.binary_search_by_key(&(self.machine, operation.id), |site| {
            (site.machine, site.operation)
        }) else {
            return Ok(());
        };
        let concerns = Construct::Operation(operation.id);
        let mut fields = Vec::new();
        for (destination, stored) in writes.iter() {
            let Stored::Moved(source) = stored else {
                continue;
            };
            let field = match (&catalog[index].event, destination.segments.as_slice()) {
                (TerminalPackageEvent::Construct { .. }, [Segment::Field(field)]) => *field,
                (TerminalPackageEvent::Project { field, .. }, []) if matches!(source.segments.last(), Some(Segment::Field(id)) if id == field) => {
                    *field
                }
                _ => continue,
            };
            let (Shape::Typed(to), Shape::Typed(from)) = (
                self.shape(destination, concerns)?,
                self.shape(source, concerns)?,
            ) else {
                continue;
            };
            if from == to {
                continue;
            }
            let (left, right) = (
                self.declaration(from, concerns)?,
                self.declaration(to, concerns)?,
            );
            let (
                StructuralTypeShape::Record {
                    fields: supplied, ..
                },
                StructuralTypeShape::Record {
                    fields: declared, ..
                },
            ) = (&left.shape, &right.shape)
            else {
                continue;
            };
            let checked = checked
                .get_or_init(|| check_package_sites(self.module))
                .as_ref()
                .map_err(|detail| self.foreign(concerns, detail))?;
            let concrete = match catalog[index].event {
                TerminalPackageEvent::Construct { .. } => from,
                _ => to,
            };
            if checked
                .field_type(self.machine, operation.id, field)
                .and_then(terminal_psi::StructuralFieldType::as_structural)
                != Some(concrete)
                || left.declaration_identity.is_absent()
                || left.declaration_identity != right.declaration_identity
                || supplied.len() != declared.len()
            {
                return Err(self.foreign(
                    concerns,
                    "package field move lacks its checked nominal carrier",
                ));
            }
            for (input, output) in supplied.iter().zip(declared) {
                if input.identity != output.identity
                    || input.declaration_identity.is_absent()
                    || input.declaration_identity != output.declaration_identity
                    || input.relevance != output.relevance
                {
                    return Err(
                        self.foreign(concerns, "package field move changes a stored declaration")
                    );
                }
                if input.id != output.id && !input.relevance.is_erased() {
                    let mut target = destination.clone();
                    target.segments.push(Segment::Field(output.id));
                    let mut origin = source.clone();
                    origin.segments.push(Segment::Field(input.id));
                    fields.push((target, Stored::Moved(origin)));
                }
            }
        }
        writes.extend(fields);
        Ok(())
    }
}
