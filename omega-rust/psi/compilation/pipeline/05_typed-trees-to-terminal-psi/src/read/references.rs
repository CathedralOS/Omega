//! A reference is the existing canonical permission descriptor, with its
//! exact referent and outer region. Its type establishes no backing or loan;
//! formation, use and completion must retain those separate obligations.

use crate::body::{Qualification, RecordType, StorageShape, ValueType};
use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::StructuralAccess;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::{Program, places::RecordSource};
use crate::{Unserved, refuse};

impl Program<'_> {
    pub(super) fn reference_type(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<Option<ValueType>> {
        let table = &self.typed.type_reference_table;
        if !table.contains_type_reference(reference) {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("reference without its retained type occurrence"),
            ));
        }
        let TypeReferenceNode::Reference {
            referee, access, ..
        } = table.type_reference(reference)
        else {
            return Ok(None);
        };
        let (referee, access) = (*referee, *access);
        if !table.contains_type_reference(referee) {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("reference without its retained referent"),
            ));
        }
        // Slice descriptors keep their own length and backing operations.
        // They do not become an address to a pretend slice-sized object.
        if matches!(
            table.type_reference(table.unconstrained_type_reference(referee)),
            TypeReferenceNode::Slice { .. }
        ) {
            return Ok(None);
        }
        let access = match access {
            language_core::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
            language_core::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
            language_core::ReferenceAccess::WriteOnly => StructuralAccess::WriteOnlyBorrow,
        };
        let retained_owner = table.operand_lifetime_owner(reference);
        let owner = if retained_owner.is_valid() {
            retained_owner
        } else {
            self.lifetime_owner
        };
        super::places::reference_lifetime_declared(self.typed, machine, reference, owner)?;
        let occurrence = typed_trees::type_identity::applied_carrier_identity_in_region_scope(
            self.typed, reference, owner, owner,
        )
        .and_then(|identity| self.typed.type_identity_text(identity))
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| {
            refuse(
                machine,
                Construct::Machine,
                Unserved::Type("reference without its complete source type identity"),
            )
        })?;
        let outer = self.borrowed_view_region(machine, reference, owner, &occurrence)?;
        // Erase regions only for the shared storage row, through the existing
        // declared-carrier identity owner. The occurrence's qualification and
        // complete region tuple remain on ValueType and are compared at use.
        let identity = typed_trees::type_identity::applied_declared_carrier_identity(
            self.typed,
            reference,
            &[],
        )
        .and_then(|identity| self.typed.type_identity_text(identity))
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| {
            refuse(
                machine,
                Construct::Machine,
                Unserved::Type("reference without its declared storage identity"),
            )
        })?;
        let referent = self.value_type(machine, referee)?;
        let qualification = referent.qualification();
        let bare = referent.qualified(Qualification::NONE).ok_or_else(|| {
            refuse(
                machine,
                Construct::Machine,
                Unserved::Type("reference to a rigid value without storage"),
            )
        })?;
        let mut tuple = vec![outer];
        tuple.extend_from_slice(self.region_tuple(referent.regions()));
        let regions = self.intern_regions(tuple);
        let bare_referee = self
            .typed
            .type_reference_table
            .unconstrained_type_reference(referee);
        let referent = self.storage_type(machine, bare_referee, bare)?;
        let shape = StorageShape::Reference { referent, access };
        if let Some((id, row)) = self
            .records
            .iter()
            .find(|(_, record)| record.identity == identity)
        {
            if row.shape != shape {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("one reference identity has different referent storage"),
                ));
            }
            return Ok(Some(ValueType::Record(id, qualification, regions)));
        }
        let id = self.records.push(RecordType {
            owns_linear_claim: false,
            copyable: access == StructuralAccess::SharedBorrow,
            identity,
            shape,
            requires: Vec::new(),
        });
        self.record_sources.push(RecordSource {
            declaration: symbols::SymbolHandle::invalid(),
            application: Some(typed_trees::SourceTypeApplication {
                reference,
                substitutions: Vec::new(),
            }),
            region_owner: owner,
            fields: Vec::new(),
            cases: Vec::new(),
        });
        Ok(Some(ValueType::Record(id, qualification, regions)))
    }

    pub(super) fn reference_referent(
        &mut self,
        machine: MachineId,
        value: ValueType,
    ) -> Checked<Option<(ValueType, StructuralAccess)>> {
        let ValueType::Record(row, qualification, regions) = value else {
            return Ok(None);
        };
        let StorageShape::Reference { referent, access } = self.data_type(machine, row)?.shape
        else {
            return Ok(None);
        };
        let (_, nested) = self.region_tuple(regions).split_first().ok_or_else(|| {
            refuse(
                machine,
                Construct::Machine,
                Unserved::Type("reference without its outer lifetime"),
            )
        })?;
        let regions = self.intern_regions(nested.to_vec());
        let referent = match self.data_type(machine, referent)?.shape {
            StorageShape::PrimitiveScalar {
                scalar_type,
                policy,
            } => ValueType::Scalar(
                scalar_type,
                Qualification {
                    policy: qualification.policy.or(policy),
                    domains: qualification.domains,
                },
            ),
            _ => ValueType::Record(referent, qualification, regions),
        };
        Ok(Some((referent, access)))
    }
    /// Form the same loan a call argument forms, retaining it in a first-class
    /// descriptor. The descriptor's actual lifetime comes from the captured
    /// source, never from the lifetime its destination hopes to receive.
    pub(super) fn borrow_value(
        &mut self,
        machine: MachineId,
        slots: &[super::Slot],
        argument: typed_trees::expression::ExpressionHandle,
        expected: ValueType,
        source: crate::SourceOrigin,
    ) -> Checked<crate::Expression> {
        if let Some(view) = self.borrowed_byte_range(machine, slots, argument, expected, source)? {
            return Ok(view);
        }
        let Some((referent, access)) = self.reference_referent(machine, expected)? else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("borrow without a reference destination"),
            ));
        };
        let ValueType::Record(row, _, _) = expected else {
            unreachable!()
        };
        let StorageShape::Reference {
            referent: storage, ..
        } = self.data_type(machine, row)?.shape
        else {
            unreachable!()
        };
        // The exact referent row owns its storage identity. A different
        // occurrence of the reference must never borrow the first one's
        // lexical owner, domain spelling or source type argument.
        let home = referent
            .scalar()
            .map(|_| {
                self.data_type(machine, storage)
                    .map(|row| row.identity.clone())
            })
            .transpose()?;
        let access = match access {
            StructuralAccess::SharedBorrow => language_core::ReferenceAccess::Shared,
            StructuralAccess::MutableBorrow => language_core::ReferenceAccess::Mutable,
            StructuralAccess::WriteOnlyBorrow => language_core::ReferenceAccess::WriteOnly,
            StructuralAccess::Owned => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("reference descriptor with owned referent access"),
                ));
            }
        };
        let formal = crate::Borrowed {
            position: 0,
            access,
            referent: crate::body::BorrowedReferent::Value,
            regions: expected.regions(),
            home,
        };
        let (value, _) = self.lent(machine, slots, argument, &formal, referent)?;
        if !matches!(value.kind, crate::ExpressionKind::Borrow { .. }) {
            unreachable!()
        };
        // A view's loan result carries its chosen presentation; its backing
        // place retains the original type. Empty-value compatibility grants
        // no permission to reinterpret either storage type.
        if !self.same_type(value.result, referent) {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("reference formation changes its referent type"),
            ));
        }
        self.finish_reference_value(machine, slots, value, expected, source)
    }

    /// The destination describes the view; its lifetime still comes from the
    /// actual place the ordinary loan reader captured. A recast changes only
    /// the presentation, so the next stage must judge its representation.
    pub(super) fn finish_reference_value(
        &mut self,
        machine: MachineId,
        slots: &[super::Slot],
        mut value: crate::Expression,
        expected: ValueType,
        source: crate::SourceOrigin,
    ) -> Checked<crate::Expression> {
        let Some((referent, _)) = self.reference_referent(machine, expected)? else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("reference formation without its destination descriptor"),
            ));
        };
        let ValueType::Record(row, _, _) = expected else {
            unreachable!()
        };
        let crate::ExpressionKind::Borrow { place, .. } = &value.kind else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("reference formation without its captured source"),
            ));
        };
        let outer = self.formed_loan_regions(machine, slots, place)?;
        let mut tuple = self.region_tuple(outer).to_vec();
        tuple.extend_from_slice(self.region_tuple(referent.regions()));
        value.result = Some(ValueType::Record(
            row,
            referent.qualification(),
            self.intern_regions(tuple),
        ));
        value.source = source;
        Ok(value)
    }

    pub(super) fn loan_regions(
        &mut self,
        machine: MachineId,
        slots: &[super::Slot],
        place: &crate::Expression,
    ) -> Checked<crate::body::Regions> {
        if let Some(value) = place.result
            && self.is_borrowed_view(value)
        {
            let outer = *self.region_tuple(value.regions()).first().ok_or_else(|| {
                refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("borrowed view without its backing lifetime"),
                )
            })?;
            return Ok(self.intern_regions(vec![outer]));
        }
        self.formed_loan_regions(machine, slots, place)
    }

    /// The region of a loan formed here of the storage `place` names: some
    /// region within that storage's, which the places the loan is given to
    /// decide (`regions::RegionVariable`). A view passed on is no new loan
    /// and keeps its own region (`loan_regions`).
    fn formed_loan_regions(
        &mut self,
        machine: MachineId,
        slots: &[super::Slot],
        place: &crate::Expression,
    ) -> Checked<crate::body::Regions> {
        let storage = self.storage_loan_regions(machine, slots, place)?;
        let within = *self.region_tuple(storage).first().ok_or_else(|| {
            refuse(
                machine,
                Construct::Machine,
                Unserved::Type("loan of storage without its lifetime"),
            )
        })?;
        let loan = self.loan_region(within);
        Ok(self.intern_regions(vec![loan]))
    }

    /// Borrowing a descriptor's home is distinct from borrowing the storage
    /// the descriptor views. Its backing lifetime cannot keep that home live.
    pub(super) fn storage_loan_regions(
        &mut self,
        machine: MachineId,
        slots: &[super::Slot],
        place: &crate::Expression,
    ) -> Checked<crate::body::Regions> {
        let mut captures = super::storage_path::Captures::new(machine, &self.captures);
        let mut crossing = None;
        let root = super::storage_path::walk(
            place,
            |capture| captures.resolve(capture),
            |projection, descriptor| {
                if crossing.is_none()
                    && matches!(projection.kind, crate::ExpressionKind::Referent(_))
                {
                    crossing = Some(
                        descriptor
                            .result
                            .ok_or_else(|| {
                                refuse(
                                    machine,
                                    Construct::Machine,
                                    Unserved::Typing(
                                        "reference crossing without a descriptor value",
                                    ),
                                )
                            })?
                            .regions(),
                    );
                }
                Ok(())
            },
        )?;
        if let Some(regions) = crossing {
            let outer = *self.region_tuple(regions).first().ok_or_else(|| {
                refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("reference crossing without its source lifetime"),
                )
            })?;
            return Ok(self.intern_regions(vec![outer]));
        }
        let crate::ExpressionKind::Slot(root) = root.kind else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Expression("reference formation without an exact storage root"),
            ));
        };
        let slot = &slots[root];
        if slot.borrowed.is_some() {
            return Ok(slot.borrowed_regions);
        }
        let region = super::regions::Region::Storage {
            machine,
            root: slot.symbol,
        };
        Ok(self.intern_regions(vec![region]))
    }

    /// Every reference crossing must retain the descriptor's own ceiling.
    /// Possessing its carrier never grants ownership of the backing.
    ///
    /// A carrier reached through storage this machine was lent shared is
    /// itself only observed: the shared loan of its holder is the parent of
    /// what is asked through it, and a `Read` parent rejects a mutable and a
    /// write-only child (loans.md, "Reborrow lineage and access"). So an
    /// exclusive reference inside a record held `&Carrier` grants no write.
    pub(super) fn reference_place_access(
        &self,
        machine: MachineId,
        slots: &[super::Slot],
        place: &crate::Expression,
        requested: language_core::ReferenceAccess,
    ) -> Checked<bool> {
        let requested = match requested {
            language_core::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
            language_core::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
            language_core::ReferenceAccess::WriteOnly => StructuralAccess::WriteOnlyBorrow,
        };
        let mut crossed = false;
        let mut captures = super::storage_path::Captures::new(machine, &self.captures);
        let root = super::storage_path::walk(
            place,
            |capture| captures.resolve(capture),
            |projection, descriptor| {
                if matches!(projection.kind, crate::ExpressionKind::Referent(_)) {
                    let Some(ValueType::Record(row, _, _)) = descriptor.result else {
                        return Err(refuse(
                            machine,
                            Construct::Machine,
                            Unserved::Type("reference crossing without its declared descriptor"),
                        ));
                    };
                    let StorageShape::Reference { access, .. } =
                        self.data_type(machine, row)?.shape
                    else {
                        return Err(refuse(
                            machine,
                            Construct::Machine,
                            Unserved::Type(
                                "reference crossing through a different structural type",
                            ),
                        ));
                    };
                    // loans.md, "Reborrow lineage and access": a `Read`
                    // parent rejects a mutable and a write-only child, and a
                    // `Write-only` parent a shared and a mutable one. So a
                    // place asked for more than the reference it is reached
                    // through grants is wrong in every program.
                    if !requested.admits_referent(access) {
                        return Err(crate::reject(
                            machine,
                            projection.source,
                            format!(
                                "this place requires {} source access, and the reference it \
                                 is reached through grants {} access only: a reference admits \
                                 no child with access it does not itself hold",
                                access_named(requested),
                                access_named(access),
                            ),
                        ));
                    }
                    crossed = true;
                }
                Ok(())
            },
        )?;
        if crossed
            && requested != StructuralAccess::SharedBorrow
            && let crate::ExpressionKind::Slot(holder) = root.kind
            && slots.get(holder).and_then(|slot| slot.borrowed)
                == Some(language_core::ReferenceAccess::Shared)
        {
            return Err(crate::reject(
                machine,
                place.source,
                format!(
                    "this place requires {} source access, and the reference it is reached \
                     through is held in storage lent shared: a shared loan admits no {} \
                     child, whatever a reference stored inside it grants",
                    access_named(requested),
                    access_named(requested),
                ),
            ));
        }
        Ok(crossed)
    }
}

/// An access as source spells it, for the rejection above.
const fn access_named(access: StructuralAccess) -> &'static str {
    match access {
        StructuralAccess::SharedBorrow => "shared",
        StructuralAccess::MutableBorrow => "mutable",
        StructuralAccess::WriteOnlyBorrow => "write-only",
        StructuralAccess::Owned => "owned",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use source::{SourceMap, SourceOrigin, SourceSpan, Span};
    use std::{path::PathBuf, sync::Arc};
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{
        TypedTrees,
        name::Identifier,
        types::{DomainConstraint, TypeConstraintNode},
    };

    #[test]
    fn reference_occurrences_keep_their_own_domains_and_lifetime_owners() {
        // A receiving metadata model, not executable or domain authority.
        // Both declarations intentionally spell their parameter `a`.
        let text = "first second D E";
        let mut sources = SourceMap::default();
        let source = sources
            .add_with_metadata(
                PathBuf::from("references.omg"),
                text.into(),
                PathBuf::from("package"),
                semantic_vocabulary::PackageKeyIdentity::from_digest([42; 32]),
                SourceOrigin::User,
            )
            .source_id;
        let token = |name: &str| {
            let start = text.find(name).unwrap();
            SymbolNameRef::Source(SourceSpan::new(
                source,
                Span::new(start, start + name.len()),
            ))
        };
        let mut symbols = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let builtins = symbols::builtin_type_symbols();
        let count = builtins.len();
        let declarations: Vec<_> = SymbolTableBuilder::child_handles(symbols.insert_children(
            root,
            builtins.into_iter().chain([
                (SymbolKind::Machine, token("first")),
                (SymbolKind::Machine, token("second")),
                (SymbolKind::Domain, token("D")),
                (SymbolKind::Domain, token("E")),
            ]),
        ))
        .collect();
        let mut typed = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let first = declarations[count];
        let second = declarations[count + 1];
        for owner in [first, second] {
            typed.push_machine(typed_trees::machine::Machine {
                symbol: owner,
                lifetime_parameters: vec![Identifier::from("a"), Identifier::from("b")],
                ..Default::default()
            });
        }
        let scalar = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: declarations[symbols::BuiltinTypeAtom::Integer(
                numerics::integer_carrier::FixedInteger::unsigned(32),
            )
            .ordinal()],
            name: Identifier::from("u32"),
        });
        let domains = [declarations[count + 2], declarations[count + 3]];
        let mut references = Vec::new();
        for (domain, name) in domains.into_iter().zip(["D", "E"]) {
            typed.push_domain_definition(typed_trees::domain::DomainDefinition {
                symbol: domain,
                name: name.into(),
                subject_name: "self".into(),
                subject_symbol: symbols::SymbolHandle::invalid(),
                type_parameters: arena::HandleSpan::empty(),
                target_type: scalar,
                index_arguments: Vec::new(),
                is_public: false,
                cleanup_selection: None,
                alias: None,
                classification: None,
                predicate_presence: language_semantics::DomainPredicatePresence::Absent,
                facts: arena::HandleSpan::empty(),
                operators: arena::HandleSpan::empty(),
                semantic_clause_token_count: 0,
                semantic_id: Default::default(),
                semantic_roles: Default::default(),
                establishment_routes: Vec::new(),
                access_policy: None,
            });
            let constraints =
                typed
                    .type_reference_table
                    .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                        symbol: domain,
                        name: name.into(),
                        ..Default::default()
                    })]);
            let qualified = typed
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: scalar,
                    constraints,
                });
            references.push(
                typed
                    .type_reference_table
                    .insert(TypeReferenceNode::Reference {
                        referee: qualified,
                        access: language_core::ReferenceAccess::Shared,
                        lifetime: Some("a".into()),
                    }),
            );
        }
        let TypeReferenceNode::Reference { referee, .. } =
            typed.type_reference_table.type_reference(references[0])
        else {
            unreachable!()
        };
        let referee = *referee;
        let mutable_reference = typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee,
                access: language_core::ReferenceAccess::Mutable,
                lifetime: Some("a".into()),
            });
        let id = MachineId::new(1).unwrap();
        let mut program = Program::new(&typed, false);
        program.lifetime_owner = first;
        let d_first = program.reference_type(id, references[0]).unwrap().unwrap();
        let e_first = program.reference_type(id, references[1]).unwrap().unwrap();
        program.lifetime_owner = second;
        let d_second = program.reference_type(id, references[0]).unwrap().unwrap();
        let e_second = program.reference_type(id, references[1]).unwrap().unwrap();
        let ValueType::Record(row, _, _) = d_first else {
            unreachable!()
        };
        assert!(matches!(e_first, ValueType::Record(other, _, _) if other == row));
        assert!(matches!(d_second, ValueType::Record(other, _, _) if other == row));
        assert_ne!(d_first.qualification(), e_first.qualification());
        assert_ne!(d_first.regions(), d_second.regions());
        assert!(
            !program.agrees(Some(d_first), e_first),
            "a domain is not erased with the row"
        );
        assert!(
            !program.agrees(Some(d_first), d_second),
            "same spelling is not the same owner"
        );
        for (value, domain) in [
            (d_first, domains[0]),
            (d_second, domains[0]),
            (e_first, domains[1]),
            (e_second, domains[1]),
        ] {
            let (referent, _) = program.reference_referent(id, value).unwrap().unwrap();
            assert_eq!(
                referent
                    .qualification()
                    .domains
                    .members()
                    .collect::<Vec<_>>(),
                [domain]
            );
        }
        let mutable = program
            .reference_type(id, mutable_reference)
            .unwrap()
            .unwrap();
        assert!(
            !program.agrees(Some(d_second), mutable),
            "shared access cannot become mutable with the same referent"
        );
        assert!(program.source_record_correspondence(id).unwrap().is_empty());
        let StorageShape::Reference { referent, .. } = program.records.get(row).unwrap().shape
        else {
            unreachable!()
        };
        assert_eq!(
            program.records.get(referent).unwrap().identity,
            typed.normalized_type_identity_text(scalar)
        );
        let required_regions = program
            .own_regions(first)
            .into_iter()
            .map(|region| program.intern_regions(vec![region]))
            .collect::<Vec<_>>();
        let supplied_regions = program
            .own_regions(second)
            .into_iter()
            .map(|region| program.intern_regions(vec![region]))
            .collect::<Vec<_>>();
        let plain = ValueType::plain(program.scalar_type(id, scalar).unwrap());
        let signature = |regions: &[crate::body::Regions]| crate::Signature {
            operational: Default::default(),
            parameters: vec![plain, plain],
            erased: Vec::new(),
            borrowed: regions
                .iter()
                .enumerate()
                .map(|(position, regions)| crate::Borrowed {
                    position,
                    access: language_core::ReferenceAccess::Shared,
                    referent: crate::body::BorrowedReferent::Value,
                    regions: *regions,
                    home: Some("u32".into()),
                })
                .collect(),
            result: None,
            requires: Vec::new(),
            ensures: Vec::new(),
            reaches: crate::Ceiling::Inferred,
            invokes: crate::Ceiling::Inferred,
            crashes: crate::CrashCeiling::DeclaredNone,
            termination: crate::Termination::Silent,
            ranking: None,
        };
        let required = signature(&required_regions);
        let supplied = signature(&supplied_regions);
        assert!(
            program
                .same_signature(id, &required, &supplied, second)
                .unwrap(),
            "alpha-renamed lifetime owners must correspond"
        );
        let mut wrong_access = supplied.clone();
        wrong_access.borrowed[0].access = language_core::ReferenceAccess::Mutable;
        assert!(
            !program
                .same_signature(id, &required, &wrong_access, second)
                .unwrap(),
            "signature matching must retain actual borrow access"
        );
        let mut wrong_relation = supplied;
        wrong_relation.borrowed[1].regions = wrong_relation.borrowed[0].regions;
        let refusal = program
            .same_signature(id, &required, &wrong_relation, second)
            .unwrap_err();
        assert_eq!(
            refusal.reason,
            terminal_check_tables::RefusalReason::Unsupported
        );
        assert_eq!(
            refusal.detail,
            "typing call that gives one lifetime two regions"
        );
    }
}
