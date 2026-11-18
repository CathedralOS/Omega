//! A requested content qualification must be established with all outputs
//! of its conservation law. Raw construction first transports the memberships
//! its operands actually declare; a desired field type supplies no custody.

use super::Lowering;
use super::places::Memberships;
use semantic_vocabulary::{PlaceId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId};
use terminal_check_tables::Checked;
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralArgument, StructuralPathQualification,
    StructuralPathSegment,
};

#[derive(Default)]
pub(super) struct ContentMemberships {
    pub held: Memberships,
    pub requested: Memberships,
}

fn beneath((whole, projected): Memberships, path: &[StructuralPathSegment]) -> Memberships {
    if path.is_empty() {
        return (whole, projected);
    }
    let whole = projected
        .iter()
        .filter(|row| row.path == path)
        .map(|row| row.domain)
        .collect();
    let projected = projected
        .into_iter()
        .filter_map(|row| {
            let rest = row.path.strip_prefix(path)?;
            (!rest.is_empty()).then(|| StructuralPathQualification {
                path: rest.to_vec(),
                domain: row.domain,
            })
        })
        .collect();
    (whole, projected)
}

fn nest(into: &mut Memberships, value: Memberships, prefix: &[StructuralPathSegment]) {
    if prefix.is_empty() {
        into.0.extend(value.0);
    } else {
        into.1.extend(
            value
                .0
                .into_iter()
                .map(|domain| StructuralPathQualification {
                    path: prefix.to_vec(),
                    domain,
                }),
        );
    }
    into.1.extend(value.1.into_iter().map(|row| {
        let mut path = prefix.to_vec();
        path.extend(row.path);
        StructuralPathQualification {
            path,
            domain: row.domain,
        }
    }));
}

impl Lowering<'_> {
    pub(super) fn materialize_content_result(
        &mut self,
        source: PlaceId,
        origin: crate::lowering::SourceOrigin,
    ) -> Checked<PlaceId> {
        use semantic_vocabulary::{ContentPlaceVersion, ContentTerm, Proposition};
        use terminal_check_tables::obligations::{ExecutionSite, ObligationKind};
        let Some(crate::lowering::body::ValueType::Record(record, qualification, _)) =
            self.signature.result
        else {
            return Ok(source);
        };
        let desired = self.memberships(record, qualification)?;
        if !self.has_content(&desired) {
            return Ok(source);
        }
        let held =
            self.declared_memberships(&StructuralArgument::whole(source, StructuralAccess::Owned))?;
        let mut missing = desired.clone();
        missing
            .0
            .retain(|domain| self.content_domain(*domain) && !held.0.contains(domain));
        missing
            .1
            .retain(|row| self.content_domain(row.domain) && !held.1.contains(row));
        if missing.0.is_empty() && missing.1.is_empty() {
            return Ok(source);
        }
        let requested = &self
            .roots
            .get(source)
            .ok_or_else(|| self.internal("content result has no source root"))?
            .content_requests;
        if requested != &missing {
            return Err(crate::lowering::refuse(
                self.id,
                terminal_check_tables::Construct::Machine,
                crate::lowering::Unserved::Expression(
                    "content result lacks its explicit qualification requests",
                ),
            ));
        }
        let result = self
            .result_subject
            .ok_or_else(|| self.internal("content return has no declared result subject"))?;
        fn all(
            term: &ContentTerm,
            predicate: &impl Fn(&semantic_vocabulary::ContentStructuralPlace) -> bool,
        ) -> bool {
            match term {
                ContentTerm::Projection { subject, .. } => predicate(subject),
                ContentTerm::Separate(parts) => {
                    !parts.is_empty() && parts.iter().all(|part| all(part, predicate))
                }
            }
        }
        fn unconditional<'a>(
            proposition: &'a Proposition,
            out: &mut Vec<&'a semantic_vocabulary::ContentConservation>,
        ) {
            match proposition {
                Proposition::ContentConservation(law) => out.push(law),
                Proposition::Conjunction(parts) => {
                    for part in parts {
                        unconditional(part, out);
                    }
                }
                _ => {}
            }
        }
        let mut laws = Vec::new();
        let mut parents = Vec::new();
        for authored in &self.signature.ensures {
            let canonical = self.qualifications.content_proposition(self.id, authored)?;
            let mut candidates = Vec::new();
            unconditional(&canonical, &mut candidates);
            for law in candidates {
                let output = |subject: &semantic_vocabulary::ContentStructuralPlace| {
                    subject.root == result && subject.version == ContentPlaceVersion::Current
                };
                let input = |subject: &semantic_vocabulary::ContentStructuralPlace| {
                    subject.root != result
                        && subject.version == ContentPlaceVersion::Entry
                        && self.roots.get(subject.root).is_some_and(|root| {
                            matches!(root.kind, StructuralPlaceKind::Parameter { .. })
                        })
                        && self.borrowed_roots.binary_search(&subject.root).is_err()
                };
                let input_term = if all(law.left(), &input) && all(law.right(), &output) {
                    law.left()
                } else if all(law.right(), &input) && all(law.left(), &output) {
                    law.right()
                } else {
                    continue;
                };
                let mut terms = vec![input_term];
                while let Some(term) = terms.pop() {
                    match term {
                        ContentTerm::Separate(parts) => terms.extend(parts),
                        ContentTerm::Projection { subject, .. } => {
                            let root = self
                                .roots
                                .get(subject.root)
                                .ok_or_else(|| self.internal("content law has no input root"))?;
                            let path =
                                terminal_semantics::content_transformations::structural_path(
                                    self.structural_types.iter(),
                                    self.qualifications.identities.as_table(),
                                    root.record,
                                    &subject.segments,
                                )
                                .ok_or_else(|| {
                                    self.internal("content law has no exact input path")
                                })?;
                            parents.push(StructuralArgument {
                                place: subject.root,
                                path,
                                access: StructuralAccess::Owned,
                            });
                        }
                    }
                }
                laws.push(law.clone());
            }
        }
        laws.sort();
        laws.dedup();
        parents.sort();
        parents.dedup();
        if laws.is_empty() || parents.is_empty() {
            return Err(crate::lowering::refuse(
                self.id,
                terminal_check_tables::Construct::Machine,
                crate::lowering::Unserved::Expression(
                    "content partition needs a complete authored conservation law",
                ),
            ));
        }
        let operation = self.operation_identity(origin);
        let before = self.here()?;
        let derivations = laws
            .into_iter()
            .map(|conservation| {
                let obligation = self.obligation(
                    ObligationKind::OperationSafety {
                        site: ExecutionSite::Operation(operation),
                    },
                    before,
                    origin,
                );
                terminal_psi::ContentTransformationDerivation {
                    obligation,
                    conservation,
                    lineage_roots: Vec::new(),
                    ancestry: Vec::new(),
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                    custody: terminal_psi::ContentTransformationCustody::Derived,
                }
            })
            .collect();
        let (result, exact) = self.produce(
            operation,
            self.signature.result.unwrap(),
            OperationKind::TransformContent {
                source: StructuralArgument::whole(source, StructuralAccess::Owned),
                parents,
                derivations,
            },
        )?;
        self.write(
            exact,
            terminal_check_tables::ProgramPoint::AfterOperation(operation),
            terminal_check_tables::bindings::Content::Overwritten,
        )?;
        self.write(
            self.exact_root(source)?,
            terminal_check_tables::ProgramPoint::AfterOperation(operation),
            terminal_check_tables::bindings::Content::Vacated,
        )?;
        Ok(result)
    }

    pub(super) fn content_domain(&self, domain: StructuralDomainId) -> bool {
        self.qualifications
            .structural
            .binary_search_by_key(&domain, |row| row.id)
            .ok()
            .is_some_and(|position| {
                self.qualifications.structural[position]
                    .content_projection
                    .is_some()
            })
    }

    pub(super) fn has_content(&self, memberships: &Memberships) -> bool {
        memberships
            .0
            .iter()
            .chain(memberships.1.iter().map(|row| &row.domain))
            .any(|domain| self.content_domain(*domain))
    }

    pub(super) fn defer_content_cast(
        &self,
        source: PlaceId,
        desired: &mut Memberships,
        added: &[StructuralDomainId],
        displayed: &[StructuralPathSegment],
    ) -> Checked<Memberships> {
        let argument = StructuralArgument::whole(source, StructuralAccess::Owned);
        let held = self.declared_memberships(&argument)?;
        let root = self
            .roots
            .get(source)
            .ok_or_else(|| self.internal("content cast lost its source"))?;
        let mut requested = root.content_requests.clone();
        requested.0.retain(|domain| desired.0.contains(domain));
        requested.1.retain(|row| desired.1.contains(row));
        for domain in added
            .iter()
            .copied()
            .filter(|domain| self.content_domain(*domain))
        {
            if displayed.is_empty() {
                if !held.0.contains(&domain) {
                    requested.0.push(domain);
                }
            } else {
                let qualification = StructuralPathQualification {
                    path: displayed.to_vec(),
                    domain,
                };
                if !held.1.contains(&qualification) {
                    requested.1.push(qualification);
                }
            }
        }
        // Existing custody may be explicitly erased, but a source request is
        // never promoted into held membership before the joint operation.
        desired
            .0
            .retain(|domain| !self.content_domain(*domain) || held.0.contains(domain));
        desired
            .1
            .retain(|row| !self.content_domain(row.domain) || held.1.contains(row));
        requested.0.sort_unstable();
        requested.0.dedup();
        requested.1.sort_unstable();
        requested.1.dedup();
        Ok(requested)
    }

    fn content_argument(&self, argument: &StructuralArgument) -> Checked<ContentMemberships> {
        let root = self
            .roots
            .get(argument.place)
            .ok_or_else(|| self.internal("content request has no root"))?;
        let mut held = self.declared_memberships(argument)?;
        held.0.retain(|domain| self.content_domain(*domain));
        held.1.retain(|row| self.content_domain(row.domain));
        Ok(ContentMemberships {
            held,
            requested: beneath(root.content_requests.clone(), &argument.path),
        })
    }

    /// Transparent value operations carry source requests beside their raw
    /// payload. A constructor does not grant a domain from its field type.
    pub(super) fn transferred_content(
        &self,
        record: StructuralTypeId,
        kind: &OperationKind,
    ) -> Checked<Option<ContentMemberships>> {
        let selected = |place, path| {
            self.content_argument(&StructuralArgument {
                place,
                path,
                access: StructuralAccess::Owned,
            })
        };
        match kind {
            OperationKind::MoveStructuralValue { source } => {
                self.content_argument(source).map(Some)
            }
            OperationKind::StructuralLeafCopy { source, path } => {
                selected(*source, path.clone()).map(Some)
            }
            OperationKind::StructuralCaseLeafCopy { source, path } => {
                let root = self
                    .roots
                    .get(*source)
                    .ok_or_else(|| self.internal("content case transfer has no carrier"))?;
                let path = terminal_semantics::canonical_structural_storage_path(
                    self.structural_types.iter(),
                    root.record,
                    path,
                )
                .ok_or_else(|| self.internal("content case transfer has no exact storage path"))?;
                selected(*source, path).map(Some)
            }
            OperationKind::MoveStructuralField {
                source,
                path,
                field,
            } => {
                let mut path = path.clone();
                if let Some(field) = field {
                    let root = self
                        .roots
                        .get(*source)
                        .ok_or_else(|| self.internal("content field transfer has no carrier"))?;
                    let field = terminal_semantics::projected_field_declaration(
                        self.structural_types.iter(),
                        root.record,
                        &path,
                        *field,
                    )
                    .ok_or_else(|| self.internal("content field transfer has no declared field"))?;
                    path.push(StructuralPathSegment::Field(field.identity));
                }
                selected(*source, path).map(Some)
            }
            OperationKind::EstablishRecord { fields }
            | OperationKind::EstablishStructuralCase { fields, .. } => {
                let mut result = ContentMemberships::default();
                let prefix = match kind {
                    OperationKind::EstablishStructuralCase { result_case, .. } => {
                        vec![StructuralPathSegment::Case(*result_case)]
                    }
                    _ => Vec::new(),
                };
                for initializer in fields {
                    let Some(argument) = initializer.value.as_structural() else {
                        continue;
                    };
                    let value = self.content_argument(argument)?;
                    let field = terminal_semantics::projected_field_declaration(
                        self.structural_types.iter(),
                        record,
                        &prefix,
                        initializer.field,
                    )
                    .ok_or_else(|| self.internal("content constructor has no declared field"))?;
                    let mut path = prefix.clone();
                    path.push(StructuralPathSegment::Field(field.identity));
                    nest(&mut result.held, value.held, &path);
                    nest(&mut result.requested, value.requested, &path);
                }
                Ok(Some(result))
            }
            OperationKind::EstablishStructuralArray { elements } => {
                let mut values = elements
                    .iter()
                    .map(|element| self.content_argument(element));
                let Some(first) = values.next().transpose()? else {
                    return Ok(Some(ContentMemberships::default()));
                };
                for value in values {
                    let value = value?;
                    if value.held != first.held || value.requested != first.requested {
                        return Err(
                            self.internal("array elements have different content obligations")
                        );
                    }
                }
                let mut result = ContentMemberships::default();
                nest(
                    &mut result.held,
                    first.held,
                    &[StructuralPathSegment::AllElements],
                );
                nest(
                    &mut result.requested,
                    first.requested,
                    &[StructuralPathSegment::AllElements],
                );
                Ok(Some(result))
            }
            OperationKind::EstablishReference { source, .. } => {
                let value = self.content_argument(source)?;
                let mut result = ContentMemberships::default();
                nest(
                    &mut result.held,
                    value.held,
                    &[StructuralPathSegment::Referent],
                );
                nest(
                    &mut result.requested,
                    value.requested,
                    &[StructuralPathSegment::Referent],
                );
                Ok(Some(result))
            }
            OperationKind::EstablishStructuralLocal {} => Ok(Some(ContentMemberships::default())),
            _ => Ok(None),
        }
    }

    /// Read the emitted declaration, rather than reconstructing qualifications
    /// from a source type that may request content not yet established.
    pub(super) fn declared_memberships(
        &self,
        argument: &StructuralArgument,
    ) -> Checked<Memberships> {
        let (kind, _) = self
            .roots
            .get(argument.place)
            .ok_or_else(|| self.internal("content transport has no declared structural root"))?
            .storage();
        let (whole, projected) = match *kind {
            StructuralPlaceKind::Parameter { position } => self
                .input_memberships
                .get(position as usize)
                .cloned()
                .ok_or_else(|| self.internal("content transport has no input membership row"))?,
            StructuralPlaceKind::BlockParameter { block, position } => {
                let formal = self
                    .blocks
                    .get(block)
                    .and_then(|block| block.structural_parameters.get(position as usize))
                    .filter(|formal| formal.place == argument.place)
                    .ok_or_else(|| {
                        self.internal("content transport has no exact block parameter")
                    })?;
                (
                    formal.qualifications.clone(),
                    formal.projected_qualifications.clone(),
                )
            }
            StructuralPlaceKind::OperationResult { producer, .. } => {
                let (block, ordinal) = self
                    .operation_locations
                    .get(producer)
                    .copied()
                    .flatten()
                    .ok_or_else(|| self.internal("content transport has no emitted producer"))?;
                let result = self
                    .blocks
                    .get(block)
                    .and_then(|block| block.operations.get(ordinal))
                    .filter(|operation| operation.id == producer)
                    .and_then(|operation| operation.result.as_structural())
                    .filter(|result| result.place == argument.place)
                    .ok_or_else(|| self.internal("content transport has another result root"))?;
                (
                    result.qualifications.clone(),
                    result.projected_qualifications.clone(),
                )
            }
            _ => return Err(self.internal("content transport has no value declaration")),
        };
        Ok(beneath((whole, projected), &argument.path))
    }
}
