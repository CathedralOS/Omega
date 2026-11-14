/*
Semantic judgments for owned structural values, structural type layouts, primitive places, static
paths, and affine cleanup obligations.
*/

pub(crate) mod atomic_cell {

    use semantic_vocabulary::{AtomicCellType, StructuralTypeId};
    use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

    pub fn atomic_cell_type<'types>(
        declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<AtomicCellType> {
        let cell = if let Some(field) =
            crate::runtime_structural_field(declarations.clone(), root, path)
        {
            field.field_type.atomic_cell_type()?
        } else {
            let declaration = crate::runtime_structural_path_tip(declarations, root, path)?;
            let StructuralTypeShape::AtomicCell(cell) = declaration.shape else {
                return None;
            };
            cell
        };
        cell.is_supported().then_some(cell)
    }
}
pub(crate) mod inline_carriers {

    use semantic_vocabulary::StructuralFieldId;
    use terminal_psi::{
        StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct InlineCarrierError {
        pub field: StructuralFieldId,
        pub reason: &'static str,
    }

    pub fn inline_carrier_declaration<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration>,
        field: &StructuralFieldDeclaration,
    ) -> Option<&'a StructuralTypeDeclaration> {
        if field.relevance.is_erased() {
            return None;
        }
        let identity = field.inline_carrier?;
        let carrier = crate::exactly_one::exactly_one(types, |candidate| candidate.id == identity)?;
        match (&field.field_type, &carrier.shape) {
            (
                StructuralFieldType::Scalar(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::BoundedInteger(_)
                | StructuralFieldType::RefinedScalar(_),
                StructuralTypeShape::PrimitiveScalar(scalar),
            ) if field.field_type.scalar_type() == Some(*scalar) => Some(carrier),
            (_, StructuralTypeShape::ByteSequence(declared))
                if field
                    .field_type
                    .byte_sequence_carrier()
                    .is_some_and(|field| {
                        crate::byte_carrier_geometry_matches(field, *declared)
                    }) =>
            {
                Some(carrier)
            }
            _ => None,
        }
    }

    pub fn validate_inline_carriers(
        types: &[StructuralTypeDeclaration],
        declaration: &StructuralTypeDeclaration,
    ) -> Result<(), InlineCarrierError> {
        let (fields, cases): (&[StructuralFieldDeclaration], &[StructuralCaseDeclaration]) =
            match &declaration.shape {
                StructuralTypeShape::Record { fields, .. } => (fields, &[]),
                StructuralTypeShape::Sum { cases } => (&[], cases),
                StructuralTypeShape::Mixed { fields, cases, .. } => (fields, cases),
                _ => (&[], &[]),
            };
        for field in fields
            .iter()
            .chain(cases.iter().flat_map(|case| &case.fields))
        {
            let Some(identity) = field.inline_carrier else {
                continue;
            };
            let fail = |reason| InlineCarrierError {
                field: field.id,
                reason,
            };
            let carrier =
                crate::exactly_one::exactly_one(types.iter(), |candidate| candidate.id == identity)
                    .ok_or_else(|| fail("inline field carrier has no exact declaration"))?;
            if field.relevance.is_erased() || field.field_type.as_structural().is_some() {
                return Err(fail(
                    "inline carrier cannot belong to an erased or structural field",
                ));
            }
            let compatible = match (&field.field_type, &carrier.shape) {
                (
                    StructuralFieldType::Scalar(_)
                    | StructuralFieldType::IeeeFloat(_)
                    | StructuralFieldType::BoundedInteger(_)
                    | StructuralFieldType::RefinedScalar(_),
                    StructuralTypeShape::PrimitiveScalar(scalar),
                ) => {
                    let plain = StructuralFieldType::Scalar(*scalar);
                    crate::scalar_field_entails(
                        &field.field_type,
                        carrier.scalar_restriction.as_ref().unwrap_or(&plain),
                    )
                }
                (_, StructuralTypeShape::ByteSequence(declared)) => field
                    .field_type
                    .byte_sequence_carrier()
                    .is_some_and(|field| crate::byte_carrier_geometry_matches(field, *declared)),
                _ => false,
            };
            if !compatible {
                return Err(fail(
                    "inline field does not establish its declared carrier contract",
                ));
            }
        }
        Ok(())
    }

    pub fn inline_field_presentation<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        field: &StructuralFieldDeclaration,
        expected: Option<semantic_vocabulary::StructuralTypeId>,
        contract: Option<StructuralFieldType>,
    ) -> Option<semantic_vocabulary::StructuralTypeId> {
        let declared = inline_carrier_declaration(types.clone(), field)?;
        let Some(expected) = expected else {
            return Some(declared.id);
        };
        let expected = crate::exactly_one::exactly_one(types.clone(), |row| row.id == expected)?;
        let compatible = match contract {
            Some(contract) => contract.declares_leaf(expected),
            None if field.field_type.byte_sequence_carrier().is_some() => {
                crate::borrowed_referent_presentation(types, declared.id, expected.id).is_some()
            }
            None => false,
        };
        compatible.then_some(expected.id)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{
            Proposition, ScalarRefinement, ScalarTerm, ScalarType, StructuralTypeId,
        };
        use terminal_psi::{
            BindingRelevance, StructuralPathSegment, StructuralRecordStorage, TerminalIdentityIndex,
        };

        fn scalar(raw: u64) -> StructuralTypeDeclaration {
            StructuralTypeDeclaration {
                requires: Vec::new(),
                id: StructuralTypeId::new(raw).unwrap(),
                identity: TerminalIdentityIndex(raw as u32),
                owns_linear_claim: false,
                copyable: true,
                no_code_affine_cleanup: true,
                descriptor_cleanup: true,
                quotient_carrier: None,
                scalar_restriction: None,
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            }
        }

        fn fixture() -> (Vec<StructuralTypeDeclaration>, Vec<StructuralPathSegment>) {
            let mut root = scalar(1);
            root.shape = StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: TerminalIdentityIndex(4),
                    relevance: BindingRelevance::Relevant,
                    storage: None,
                    field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                    inline_carrier: Some(StructuralTypeId::new(2).unwrap()),
                }],
            };
            (
                vec![root, scalar(2), scalar(3)],
                vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))],
            )
        }

        #[test]
        fn original_field_carrier_is_exact_among_equal_contracts() {
            let (mut declarations, path) = fixture();
            for _ in 0..2 {
                let root = declarations.iter().find(|row| row.id.get() == 1).unwrap();
                validate_inline_carriers(&declarations, root).unwrap();
                assert_eq!(
                    crate::runtime_structural_path_tip(declarations.iter(), root.id, &path)
                        .unwrap()
                        .id
                        .get(),
                    2
                );
                declarations.swap(1, 2);
            }
            let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
                panic!("record")
            };
            fields[0].inline_carrier = Some(StructuralTypeId::new(99).unwrap());
            assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
            assert!(
                crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
                    .is_none()
            );
        }

        #[test]
        fn stronger_field_predicate_survives_a_weaker_declared_carrier() {
            let (mut declarations, path) = fixture();
            let restriction = StructuralFieldType::RefinedScalar(
                ScalarRefinement::new(
                    ScalarType::Boolean,
                    Proposition::Equal(
                        ScalarTerm::value(ScalarRefinement::SUBJECT, ScalarType::Boolean),
                        ScalarTerm::boolean(true),
                    ),
                )
                .unwrap(),
            );
            let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
                panic!("record")
            };
            fields[0].field_type = restriction.clone();
            validate_inline_carriers(&declarations, &declarations[0]).unwrap();
            assert_eq!(
                crate::primitive_projection_restriction(
                    declarations.iter(),
                    declarations[0].id,
                    &path
                ),
                Some(&restriction)
            );
            declarations[1].scalar_restriction = Some(restriction);
            let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
                panic!("record")
            };
            fields[0].field_type = StructuralFieldType::Scalar(ScalarType::Boolean);
            assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
        }

        #[test]
        fn typed_field_presentation_keeps_the_requested_identity_or_refuses() {
            let (declarations, path) = fixture();
            let root = &declarations[0];
            let field =
                crate::runtime_structural_field(declarations.iter(), root.id, &path).unwrap();
            let expected = declarations[2].id;
            assert_eq!(
                inline_field_presentation(declarations.iter(), field, None, None),
                field.inline_carrier
            );
            assert_eq!(
                inline_field_presentation(
                    declarations.iter(),
                    field,
                    Some(expected),
                    field.field_type.canonical_leaf_type()
                ),
                Some(expected)
            );
            let missing = StructuralTypeId::new(99).unwrap();
            assert_eq!(
                inline_field_presentation(
                    declarations.iter(),
                    field,
                    Some(missing),
                    field.field_type.canonical_leaf_type()
                ),
                None
            );
            assert_eq!(
                crate::borrowed_primitive_projection_declaration(
                    declarations.iter(),
                    root.id,
                    &path,
                    Some(missing)
                ),
                None
            );
        }

        #[test]
        fn inline_carrier_rejects_wrong_scalar_atomic_and_erased_fields() {
            let (mut declarations, _) = fixture();
            for field_type in [
                StructuralFieldType::Scalar(ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        8,
                    )
                    .unwrap(),
                )),
                StructuralFieldType::Atomic(ScalarType::Boolean),
                StructuralFieldType::Structural(declarations[1].id),
                StructuralFieldType::Erased {
                    type_identity: TerminalIdentityIndex(4),
                },
            ] {
                let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
                    panic!("record")
                };
                fields[0].field_type = field_type;
                assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
            }
        }

        #[test]
        fn byte_field_keeps_exact_carrier_access_and_extent() {
            use terminal_psi::{ByteSequenceCarrier, StructuralAccess};
            let (mut declarations, path) = fixture();
            let carrier = ByteSequenceCarrier::BoundedView {
                access: Some(StructuralAccess::MutableBorrow),
                capacity: 6,
            };
            for declaration in &mut declarations[1..] {
                declaration.shape =
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                        access: None,
                        capacity: 6,
                    });
            }
            let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
                panic!("record")
            };
            fields[0].field_type = StructuralFieldType::ByteSequence(carrier);
            validate_inline_carriers(&declarations, &declarations[0]).unwrap();
            assert_eq!(
                crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
                    .unwrap()
                    .id,
                declarations[1].id
            );
            declarations[1].shape = StructuralTypeShape::ByteSequence(carrier);
            validate_inline_carriers(&declarations, &declarations[0]).unwrap();
            assert_eq!(
                crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
                    .unwrap()
                    .id,
                declarations[1].id
            );
            for wrong in [
                ByteSequenceCarrier::BoundedView {
                    access: Some(StructuralAccess::SharedBorrow),
                    capacity: 6,
                },
                ByteSequenceCarrier::BoundedView {
                    access: None,
                    capacity: 7,
                },
                ByteSequenceCarrier::BoundedView {
                    access: Some(StructuralAccess::MutableBorrow),
                    capacity: 7,
                },
            ] {
                declarations[1].shape = StructuralTypeShape::ByteSequence(wrong);
                assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
                assert!(
                    crate::runtime_structural_path_tip(
                        declarations.iter(),
                        declarations[0].id,
                        &path
                    )
                    .is_none()
                );
            }
            let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
                panic!("record")
            };
            fields[0].field_type =
                StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::MutableBorrow),
                });
            declarations[1].shape =
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::MutableBorrow),
                });
            validate_inline_carriers(&declarations, &declarations[0]).unwrap();
            assert!(
                crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
                    .is_some()
            );
            declarations[1].shape =
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                });
            assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
            assert!(
                crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
                    .is_none()
            );
        }
    }
}
pub mod nominal_cleanup {

    use terminal_psi::{
        NominalAffineCleanup, StructuralTypeDeclaration, TerminalMachine, TerminalMachineResult,
    };

    pub fn target<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration>,
        machines: impl Iterator<Item = &'a TerminalMachine> + Clone,
        cleanup: &NominalAffineCleanup,
    ) -> Result<&'a TerminalMachine, &'static str> {
        if !types
            .into_iter()
            .any(|declaration| declaration.id == cleanup.structural_type)
        {
            return Err("nominal cleanup subject declaration is absent");
        }
        let target = machines
            .clone()
            .find(|machine| machine.id == cleanup.cleanup_machine)
            .ok_or("nominal cleanup target is absent")?;
        if target.attachment != Some(cleanup.structural_type)
            || target.nominal_cleanup_subject != Some(cleanup.structural_type)
            || machines
                .filter(|machine| machine.nominal_cleanup_subject == Some(cleanup.structural_type))
                .count()
                != 1
        {
            return Err("nominal cleanup lacks its exact unique owner designation");
        }
        if target.result != TerminalMachineResult::Unit
            || !target.parameters.is_empty()
            || !target.entry_claims.is_empty()
            || !target.content_entry_claims.is_empty()
            || target.structural_parameters.len() > 1
            || target
                .structural_parameters
                .iter()
                .any(|parameter| !parameter.is_cleanup_receiver(cleanup))
        {
            return Err("nominal cleanup target has an incompatible receiver or result");
        }
        if cleanup
            .consumed_claims
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err("nominal cleanup claim roster is not strictly canonical");
        }
        Ok(target)
    }
}
pub mod owned_structure {
    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

    pub(crate) mod complete_claims {

        use super::child_types;
        use crate::exactly_one::exactly_one;
        use semantic_vocabulary::{IdSet, StructuralCaseId, StructuralTypeId};
        use std::collections::BTreeSet;
        use terminal_psi::{
            StructuralPathSegment as Path, StructuralTypeDeclaration, StructuralTypeShape,
        };

        pub fn validate_complete_claim_frontier<'a>(
            declaration: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
            root: StructuralTypeId,
            region: &[Path],
            claims: impl IntoIterator<Item = &'a [Path]>,
            selected: &dyn Fn(&[Path], StructuralCaseId) -> bool,
        ) -> Result<(), &'static str> {
            let mut held = BTreeSet::new();
            for path in claims
                .into_iter()
                .filter(|path| path.starts_with(region) || region.starts_with(path))
            {
                if !held.insert(path) {
                    return Err("complete value repeats a live claim path");
                }
            }
            let mut possible = ClaimTypes::default();
            let consumed = visit(
                &declaration,
                root,
                &mut Vec::new(),
                region,
                &held,
                selected,
                &mut possible,
            )?;
            if consumed != held.len() {
                return Err("complete value has a claim outside its declared frontier");
            }
            Ok(())
        }

        #[derive(Default)]
        pub(crate) struct ClaimTypes {
            known: IdSet<StructuralTypeId>,
            with_claims: IdSet<StructuralTypeId>,
            active: IdSet<StructuralTypeId>,
        }

        pub(crate) fn contains_claim<'a>(
            declaration: &impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
            current: StructuralTypeId,
            possible: &mut ClaimTypes,
        ) -> Result<bool, &'static str> {
            let row = declaration(current).ok_or("complete value type is absent")?;
            if row.owns_linear_claim {
                return Ok(true);
            }
            if possible.known.contains(current) {
                return Ok(possible.with_claims.contains(current));
            }
            if !possible.active.insert(current) {
                return Err("complete value has a cyclic by-value type");
            }
            let mut found = false;
            for child in child_types(row) {
                found |= contains_claim(declaration, child, possible)?;
            }
            possible.active.remove(current);
            possible.known.insert(current);
            if found {
                possible.with_claims.insert(current);
            }
            Ok(found)
        }

        fn visit<'a>(
            declaration: &impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
            current: StructuralTypeId,
            path: &mut Vec<Path>,
            region: &[Path],
            held: &BTreeSet<&[Path]>,
            selected: &dyn Fn(&[Path], StructuralCaseId) -> bool,
            possible: &mut ClaimTypes,
        ) -> Result<usize, &'static str> {
            let row = declaration(current).ok_or("complete value type is absent")?;
            if row.owns_linear_claim {
                return held
                    .contains(path.as_slice())
                    .then_some(1)
                    .ok_or("complete value is missing its declaration-owned claim");
            }
            if !contains_claim(declaration, current, possible)? {
                return Ok(0);
            }
            if let Some((step, tail)) = region.split_first() {
                let fields = match (&row.shape, step) {
                    (StructuralTypeShape::Record { fields, .. }, Path::Field(_))
                    | (StructuralTypeShape::Mixed { fields, .. }, Path::Field(_)) => fields,
                    (
                        StructuralTypeShape::Sum { cases }
                        | StructuralTypeShape::Mixed { cases, .. },
                        Path::Case(case),
                    ) => {
                        let active = exactly_one(cases.iter(), |case| selected(path, case.id))
                            .ok_or("complete value region has no unique established case")?;
                        if active.id != *case {
                            return Err("complete value region names an inactive case");
                        }
                        let Some((Path::Field(field), rest)) = tail.split_first() else {
                            return Err("complete value region ends in a case namespace");
                        };
                        let member = exactly_one(active.fields.iter(), |member| {
                            !member.relevance.is_erased() && member.identity == *field
                        })
                        .ok_or("complete value region has no exact payload field")?;
                        path.extend([step.clone(), Path::Field(*field)]);
                        let count = match member.field_type.as_structural() {
                            Some(child) => {
                                visit(declaration, child, path, rest, held, selected, possible)
                            }
                            None if rest.is_empty() => Ok(0),
                            None => Err("complete value region projects through a scalar"),
                        }?;
                        path.truncate(path.len() - 2);
                        return Ok(count);
                    }
                    (
                        StructuralTypeShape::FixedArray { element, length },
                        Path::FixedIndex(index),
                    ) => {
                        if length.literal().is_none_or(|length| *index >= length) {
                            return Err("complete value region has no exact array element");
                        }
                        path.push(step.clone());
                        let count =
                            visit(declaration, *element, path, tail, held, selected, possible)?;
                        path.pop();
                        return Ok(count);
                    }
                    _ => return Err("complete value region has no owned storage path"),
                };
                let Path::Field(field) = step else {
                    unreachable!("field branch selected above")
                };
                let member = exactly_one(fields.iter(), |member| {
                    !member.relevance.is_erased() && member.identity == *field
                })
                .ok_or("complete value region has no exact field")?;
                path.push(step.clone());
                let count = match member.field_type.as_structural() {
                    Some(child) => visit(declaration, child, path, tail, held, selected, possible),
                    None if tail.is_empty() => Ok(0),
                    None => Err("complete value region projects through a scalar"),
                }?;
                path.pop();
                return Ok(count);
            }
            if let StructuralTypeShape::FixedArray { element, length } = &row.shape {
                let length = length
                    .literal()
                    .ok_or("complete value requires a closed array extent")?;
                let mut count = 0;
                path.push(Path::FixedIndex(0));
                for index in 0..length {
                    *path.last_mut().expect("array path has its current index") =
                        Path::FixedIndex(index);
                    count += visit(declaration, *element, path, &[], held, selected, possible)?;
                }
                path.pop();
                return Ok(count);
            }
            let mut fields = |fields: &[terminal_psi::StructuralFieldDeclaration],
                              path: &mut Vec<Path>| {
                let mut count = 0;
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    if let Some(child) = field.field_type.as_structural() {
                        path.push(Path::Field(field.identity));
                        count += visit(declaration, child, path, &[], held, selected, possible)?;
                        path.pop();
                    }
                }
                Ok::<_, &'static str>(count)
            };
            match &row.shape {
                StructuralTypeShape::Record { fields: common, .. } => fields(common, path),
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    let case = exactly_one(cases.iter(), |case| selected(path, case.id))
                        .ok_or("complete value has no unique established case")?;
                    path.push(Path::Case(case.id));
                    let mut count = fields(&case.fields, path)?;
                    path.pop();
                    if let StructuralTypeShape::Mixed { fields: common, .. } = &row.shape {
                        count += fields(common, path)?;
                    }
                    Ok(count)
                }
                _ => Err("complete value claim frontier has no owned child geometry"),
            }
        }
    }
    pub use complete_claims::validate_complete_claim_frontier;

    fn child_types(declaration: &StructuralTypeDeclaration) -> Vec<StructuralTypeId> {
        let mut children = Vec::new();
        let mut fields = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
            children.extend(
                fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .filter_map(|field| field.field_type.as_structural()),
            );
        };
        match &declaration.shape {
            StructuralTypeShape::Record { fields: common, .. } => fields(common),
            StructuralTypeShape::Mixed {
                fields: common,
                cases,
            } => {
                fields(common);
                for case in cases {
                    fields(&case.fields);
                }
            }
            StructuralTypeShape::Sum { cases } => {
                for case in cases {
                    fields(&case.fields);
                }
            }
            StructuralTypeShape::FixedArray { element, length } if length.literal() != Some(0) => {
                children.push(*element)
            }
            _ => {}
        }
        children
    }

    pub fn contains_owned_boundary(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        boundaries: &BTreeSet<StructuralTypeId>,
        cache: &mut BTreeMap<StructuralTypeId, bool>,
        active: &mut BTreeSet<StructuralTypeId>,
        structural_type: StructuralTypeId,
    ) -> Option<bool> {
        if boundaries.contains(&structural_type) {
            return Some(true);
        }
        if let Some(result) = cache.get(&structural_type) {
            return Some(*result);
        }
        if !active.insert(structural_type) {
            return None;
        }
        let mut result = false;
        for child in child_types(types.get(&structural_type)?) {
            result |= contains_owned_boundary(types, boundaries, cache, active, child)?;
        }
        active.remove(&structural_type);
        cache.insert(structural_type, result);
        Some(result)
    }

    pub fn validate_accounted_descendants(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        boundaries: &BTreeSet<StructuralTypeId>,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        accounted: &[Vec<StructuralPathSegment>],
        selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    ) -> Result<(), &'static str> {
        fn visit(
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            boundaries: &BTreeSet<StructuralTypeId>,
            current: StructuralTypeId,
            path: &[StructuralPathSegment],
            accounted: &[Vec<StructuralPathSegment>],
            selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
            cache: &mut BTreeMap<StructuralTypeId, bool>,
            active: &mut BTreeSet<StructuralTypeId>,
        ) -> Result<(), &'static str> {
            let declaration = types
                .get(&current)
                .ok_or("owned descendant declaration is absent")?;
            if boundaries.contains(&current) && !accounted.iter().any(|claim| claim == path) {
                return Err("consumed parent has an unaccounted descendant obligation");
            }
            if !active.insert(current) {
                return Err("owned descendant graph is cyclic");
            }
            let mut relevant = false;
            for child in child_types(declaration) {
                relevant |=
                    contains_owned_boundary(types, boundaries, cache, &mut BTreeSet::new(), child)
                        .ok_or("owned descendant graph is unresolved")?;
            }
            if relevant {
                let mut visit_child = |child_path: &[StructuralPathSegment], child| {
                    if !contains_owned_boundary(
                        types,
                        boundaries,
                        cache,
                        &mut BTreeSet::new(),
                        child,
                    )
                    .ok_or("owned descendant graph is unresolved")?
                    {
                        return Ok(());
                    }
                    visit(
                        types, boundaries, child, child_path, accounted, selected, cache, active,
                    )
                };
                if let StructuralTypeShape::FixedArray { element, length } = declaration.shape {
                    let length = length
                        .literal()
                        .ok_or("owned descendant accounting requires a closed array extent")?;
                    let mut child_path = path.to_vec();
                    child_path.push(StructuralPathSegment::FixedIndex(0));
                    for element_index in 0..length {
                        if let Some(segment) = child_path.last_mut() {
                            *segment = StructuralPathSegment::FixedIndex(element_index);
                        }
                        visit_child(&child_path, element)?;
                    }
                } else {
                    let mut case_payload_required = false;
                    if let StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. } = &declaration.shape
                    {
                        for child in cases
                            .iter()
                            .flat_map(|case| &case.fields)
                            .filter(|field| !field.relevance.is_erased())
                            .filter_map(|field| field.field_type.as_structural())
                        {
                            case_payload_required |= contains_owned_boundary(
                                types,
                                boundaries,
                                &mut BTreeMap::new(),
                                &mut BTreeSet::new(),
                                child,
                            )
                            .ok_or("owned descendant graph is unresolved")?;
                        }
                    }
                    let children = selected_children(
                        types,
                        current,
                        path,
                        usize::MAX,
                        selected,
                        case_payload_required,
                    )
                    .ok_or("owned descendant occurrence or active case is unresolved")?;
                    for (child_path, child) in children {
                        visit_child(&child_path, child)?;
                    }
                }
            }
            active.remove(&current);
            Ok(())
        }
        visit(
            types,
            boundaries,
            root,
            path,
            accounted,
            selected,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )
    }

    pub fn selected_contains_owned_boundary(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        nominal: &BTreeSet<StructuralTypeId>,
        cache: &mut BTreeMap<StructuralTypeId, bool>,
        selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        limit: usize,
    ) -> Option<bool> {
        if nominal.contains(&structural_type) {
            return Some(true);
        }
        if !contains_owned_boundary(types, nominal, cache, &mut BTreeSet::new(), structural_type)? {
            return Some(false);
        }
        for (child_path, child) in
            selected_owned_children(types, structural_type, path, limit, selected)?
        {
            if selected_contains_owned_boundary(
                types,
                nominal,
                cache,
                selected,
                child,
                &child_path,
                limit,
            )? {
                return Some(true);
            }
        }
        Some(false)
    }

    pub fn validate_owned_claim_frontier<'types>(
        declaration: &impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
        selected_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        accounted: impl IntoIterator<Item = Vec<StructuralPathSegment>>,
        selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    ) -> Result<(), &'static str> {
        for (index, segment) in path.iter().enumerate() {
            if let StructuralPathSegment::Case(case) = segment
                && !selected(&path[..index], *case)
            {
                return Err("owned value path has no established active case");
            }
        }
        let mut remaining = BTreeSet::new();
        for claim in accounted {
            if !claim.starts_with(path) || !remaining.insert(claim) {
                return Err("owned value claims overlap an ancestor or repeat a path");
            }
        }
        enum Pending {
            Value(Vec<StructuralPathSegment>, StructuralTypeId),
            Array {
                path: Vec<StructuralPathSegment>,
                element: StructuralTypeId,
                next: u64,
                length: u64,
            },
        }
        let mut pending = vec![Pending::Value(path.to_vec(), selected_type)];
        let mut possible = complete_claims::ClaimTypes::default();
        while let Some(occurrence) = pending.pop() {
            let (path, current) = match occurrence {
                Pending::Value(path, current) => (path, current),
                Pending::Array {
                    path,
                    element,
                    next,
                    length,
                } => {
                    if next < length {
                        let mut child = path.clone();
                        child.push(StructuralPathSegment::FixedIndex(next));
                        pending.push(Pending::Array {
                            path,
                            element,
                            next: next + 1,
                            length,
                        });
                        pending.push(Pending::Value(child, element));
                    }
                    continue;
                }
            };
            let current_type = declaration(current).ok_or("owned value type is absent")?;
            if current_type.owns_linear_claim {
                if !remaining.remove(&path) {
                    return Err("owned value has no exact live nominal claim");
                }
                continue;
            }
            if !complete_claims::contains_claim(declaration, current, &mut possible)? {
                continue;
            }
            if let StructuralTypeShape::FixedArray { element, length } = &current_type.shape {
                pending.push(Pending::Array {
                    path,
                    element: *element,
                    next: 0,
                    length: length
                        .literal()
                        .ok_or("owned value array extent is not closed")?,
                });
                continue;
            }
            let mut fields =
                |prefix: &[StructuralPathSegment],
                 fields: &[terminal_psi::StructuralFieldDeclaration]| {
                    for field in fields
                        .iter()
                        .rev()
                        .filter(|field| !field.relevance.is_erased())
                    {
                        if let Some(child) = field.field_type.as_structural() {
                            let mut child_path = prefix.to_vec();
                            child_path.push(StructuralPathSegment::Field(field.identity));
                            pending.push(Pending::Value(child_path, child));
                        }
                    }
                };
            match &current_type.shape {
                StructuralTypeShape::Record { fields: common, .. } => fields(&path, common),
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    let mut payload_required = false;
                    for child in cases
                        .iter()
                        .flat_map(|case| &case.fields)
                        .filter(|field| !field.relevance.is_erased())
                        .filter_map(|field| field.field_type.as_structural())
                    {
                        payload_required |=
                            complete_claims::contains_claim(declaration, child, &mut possible)?;
                    }
                    if payload_required {
                        let mut admitted = false;
                        for case in cases.iter().filter(|case| selected(&path, case.id)) {
                            admitted = true;
                            let mut payload_path = path.clone();
                            payload_path.push(StructuralPathSegment::Case(case.id));
                            fields(&payload_path, &case.fields);
                        }
                        if !admitted {
                            return Err("owned value has no admitted payload case");
                        }
                    }
                    if let StructuralTypeShape::Mixed { fields: common, .. } = &current_type.shape {
                        fields(&path, common);
                    }
                }
                _ => return Err("owned value occurrence is unresolved"),
            }
        }
        if !remaining.is_empty() {
            return Err("owned value has claims outside its selected nominal frontier");
        }
        Ok(())
    }

    pub fn selected_owned_children(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        limit: usize,
        selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    ) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
        selected_children(types, structural_type, path, limit, selected, true)
    }

    fn selected_children(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        limit: usize,
        selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
        case_payload_required: bool,
    ) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
        let mut children = Vec::new();
        let mut fields = |fields: &[terminal_psi::StructuralFieldDeclaration],
                          prefix: &[StructuralPathSegment]| {
            for field in fields
                .iter()
                .rev()
                .filter(|field| !field.relevance.is_erased())
            {
                if let Some(child) = field.field_type.as_structural() {
                    let mut child_path = prefix.to_vec();
                    child_path.push(StructuralPathSegment::Field(field.identity));
                    children.push((child_path, child));
                }
            }
        };
        match &types.get(&structural_type)?.shape {
            StructuralTypeShape::Record { fields: common, .. } => fields(common, path),
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                if case_payload_required {
                    let case = exactly_one(cases.iter(), |case| selected(path, case.id))?;
                    let mut payload_path = path.to_vec();
                    payload_path.push(StructuralPathSegment::Case(case.id));
                    fields(&case.fields, &payload_path);
                }
                if let StructuralTypeShape::Mixed { fields: common, .. } =
                    &types.get(&structural_type)?.shape
                {
                    fields(common, path);
                }
            }
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                if u128::from(*length) > limit as u128 {
                    return None;
                }
                for index in (0..*length).rev() {
                    let mut child_path = path.to_vec();
                    child_path.push(StructuralPathSegment::FixedIndex(index));
                    children.push((child_path, *element));
                }
            }
            _ => return None,
        }
        Some(children)
    }
}
pub(crate) mod partial_affine {
    use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
    use std::collections::BTreeSet;
    use terminal_psi::{
        StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
        find_structural_case, find_structural_type,
    };

    fn is_partial_affine_path(
        types: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> bool {
        !path.is_empty()
            && path.iter().all(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::Field(_)
                        | StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::Case(_)
                )
            })
            && crate::runtime_structural_path_tip(types.iter(), root, path).is_some()
    }

    /// Tests typed coverage of a receiving region, not allocation or access
    /// authority. A live sibling or missing ancestor cannot be erased by a
    /// store into this region. Callers retain their own home and loan checks.
    pub fn structural_region_is_vacant(
        types: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        missing: &BTreeSet<Vec<StructuralPathSegment>>,
        region: &[StructuralPathSegment],
        admitted_case: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    ) -> bool {
        let Some(selected) = crate::runtime_structural_path_tip(types.iter(), root, region) else {
            return false;
        };
        if missing.iter().any(|absent| {
            crate::structural_paths_may_overlap(absent, region) && !absent.starts_with(region)
        }) {
            return false;
        }
        if missing.contains(region) {
            return true;
        }
        let children = missing
            .iter()
            .filter_map(|absent| absent.strip_prefix(region).map(<[_]>::to_vec))
            .collect();
        // The counting pass proves an empty typed complement before any
        // output is allocated or untouched array elements are enumerated.
        partition(types, selected.id, &children, None, 0, &|path, case| {
            let mut selected = region.to_vec();
            selected.extend_from_slice(path);
            admitted_case(&selected, case)
        })
        .is_some_and(|residuals| residuals.is_empty())
    }

    pub fn partial_affine_residuals(
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
        moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
        max_residuals: usize,
    ) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
        partition(
            types,
            root_type,
            moved_paths,
            None,
            max_residuals,
            &|_, _| false,
        )
    }

    /// Keeps only live residuals within the requested regions. Reconverging
    /// arrivals use the successor's vacancies as regions and their own moves
    /// as exclusions. The existing typed walk retains declaration order and
    /// proves the output bound before enumerating array elements.
    pub fn partial_affine_residuals_in_regions(
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
        moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
        regions: &BTreeSet<Vec<StructuralPathSegment>>,
        max_residuals: usize,
    ) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
        partition(
            types,
            root_type,
            moved_paths,
            Some(regions),
            max_residuals,
            &|_, _| false,
        )
    }

    fn partition(
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
        moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
        regions: Option<&BTreeSet<Vec<StructuralPathSegment>>>,
        max_residuals: usize,
        admitted_case: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    ) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
        find_structural_type(types, root_type)?;
        let valid = |paths: &BTreeSet<Vec<StructuralPathSegment>>, allow_root| {
            paths.iter().all(|path| {
                allow_root && path.is_empty() || is_partial_affine_path(types, root_type, path)
            }) && !paths.iter().enumerate().any(|(index, path)| {
                paths
                    .iter()
                    .enumerate()
                    .any(|(other_index, other)| index != other_index && path.starts_with(other))
            })
        };
        if moved_paths.is_empty() && regions.is_none()
            || !valid(moved_paths, regions.is_some())
            || regions.is_some_and(|paths| !valid(paths, true))
        {
            return None;
        }
        if moved_paths.iter().any(Vec::is_empty) || regions.is_some_and(BTreeSet::is_empty) {
            return Some(Vec::new());
        }
        let moved = moved_paths.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let regions = regions.map(|paths| paths.iter().map(Vec::as_slice).collect::<Vec<_>>());
        let regions = regions
            .as_deref()
            .filter(|paths| !paths.iter().any(|path| path.is_empty()));
        if moved.is_empty() && regions.is_none() {
            return (max_residuals != 0).then(|| vec![(Vec::new(), root_type)]);
        }
        let mut residuals = Vec::new();
        let count = visit(
            types,
            root_type,
            &moved,
            regions,
            &mut Vec::new(),
            max_residuals,
            &mut residuals,
            false,
            admitted_case,
        )?;
        if count > max_residuals {
            return None;
        }
        residuals.try_reserve_exact(count).ok()?;
        let emitted = visit(
            types,
            root_type,
            &moved,
            regions,
            &mut Vec::new(),
            max_residuals,
            &mut residuals,
            true,
            admitted_case,
        )?;
        (emitted == count && residuals.len() == count).then_some(residuals)
    }

    fn visit(
        types: &[StructuralTypeDeclaration],
        current: StructuralTypeId,
        moved: &[&[StructuralPathSegment]],
        regions: Option<&[&[StructuralPathSegment]]>,
        prefix: &mut Vec<StructuralPathSegment>,
        max_residuals: usize,
        residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
        emit: bool,
        admitted_case: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    ) -> Option<usize> {
        if regions.is_some_and(<[_]>::is_empty) {
            return Some(0);
        }
        let declaration = find_structural_type(types, current)?;
        let mut count = 0usize;
        match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => {
                if fields.is_empty() {
                    return None;
                }
                count = fields_residuals(
                    types,
                    fields,
                    moved,
                    regions,
                    prefix,
                    max_residuals,
                    residuals,
                    emit,
                    admitted_case,
                )?;
            }
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } if *length != 0 => {
                let mut touched = moved
                    .iter()
                    .map(|path| match path.first()? {
                        StructuralPathSegment::FixedIndex(index) if index < length => Some(*index),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                touched.sort_unstable();
                touched.dedup();
                if !emit && regions.is_none() {
                    count = usize::try_from(*length).ok()?.checked_sub(touched.len())?;
                }
                let mut requested = if let Some(regions) = regions {
                    regions
                        .iter()
                        .map(|path| match path.first()? {
                            StructuralPathSegment::FixedIndex(index) if index < length => {
                                Some(*index)
                            }
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()?
                } else {
                    touched
                };
                requested.sort_unstable();
                requested.dedup();
                let indexes = (0..if emit && regions.is_none() {
                    *length
                } else {
                    0
                })
                    .rev()
                    .chain(
                        requested
                            .into_iter()
                            .rev()
                            .filter(|_| regions.is_some() || !emit),
                    );
                for index in indexes {
                    count = count.checked_add(child(
                        types,
                        *element,
                        StructuralPathSegment::FixedIndex(index),
                        moved,
                        regions,
                        prefix,
                        max_residuals,
                        residuals,
                        emit,
                        admitted_case,
                    )?)?;
                    if count > max_residuals {
                        return None;
                    }
                }
            }
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                // A common-field cleanup does not observe the case. Retain
                // the payload namespace only when the receiving scope asks
                // for it; a requested payload still needs actual membership.
                let payload_regions = regions.map(|paths| {
                    paths
                        .iter()
                        .copied()
                        .filter(|path| matches!(path.first(), Some(StructuralPathSegment::Case(_))))
                        .collect::<Vec<_>>()
                });
                if payload_regions
                    .as_ref()
                    .is_none_or(|paths| !paths.is_empty())
                {
                    let selected_case = moved
                        .iter()
                        .find_map(|path| match path.first() {
                            Some(StructuralPathSegment::Case(case)) => Some(*case),
                            _ => None,
                        })
                        .or_else(|| {
                            let mut matching =
                                cases.iter().filter(|case| admitted_case(prefix, case.id));
                            let case = matching.next()?;
                            matching.next().is_none().then_some(case.id)
                        })?;
                    let selected = find_structural_case(cases, selected_case)?;
                    if moved.iter().any(|path| match path.first() {
                        Some(StructuralPathSegment::Case(case)) => *case != selected_case,
                        Some(StructuralPathSegment::Field(_)) => {
                            declaration.shape.direct_fields().is_none()
                        }
                        _ => true,
                    }) {
                        return None;
                    }
                    let payload_moves = moved
                        .iter()
                        .filter_map(|path| {
                            matches!(path.first(), Some(StructuralPathSegment::Case(_)))
                                .then_some(&path[1..])
                        })
                        .collect::<Vec<_>>();
                    prefix.push(StructuralPathSegment::Case(selected_case));
                    let case_regions = region_children(
                        payload_regions.as_deref(),
                        StructuralPathSegment::Case(selected_case),
                    );
                    count = fields_residuals(
                        types,
                        &selected.fields,
                        &payload_moves,
                        case_regions.as_deref(),
                        prefix,
                        max_residuals,
                        residuals,
                        emit,
                        admitted_case,
                    )?;
                    prefix.pop();
                }
                if let Some(common) = declaration.shape.direct_fields() {
                    let common_moves = moved
                        .iter()
                        .copied()
                        .filter(|path| {
                            matches!(path.first(), Some(StructuralPathSegment::Field(_)))
                        })
                        .collect::<Vec<_>>();
                    count = count.checked_add(fields_residuals(
                        types,
                        common,
                        &common_moves,
                        regions,
                        prefix,
                        max_residuals,
                        residuals,
                        emit,
                        admitted_case,
                    )?)?;
                }
            }
            _ => return None,
        }
        (count <= max_residuals).then_some(count)
    }

    fn fields_residuals(
        types: &[StructuralTypeDeclaration],
        fields: &[terminal_psi::StructuralFieldDeclaration],
        moved: &[&[StructuralPathSegment]],
        regions: Option<&[&[StructuralPathSegment]]>,
        prefix: &mut Vec<StructuralPathSegment>,
        max_residuals: usize,
        residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
        emit: bool,
        admitted_case: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    ) -> Option<usize> {
        let mut count = 0_usize;
        for field in fields.iter().rev() {
            // A scoped cleanup does not inspect presentation or erasure of
            // sibling storage it was not asked to dispose.
            if region_children(regions, StructuralPathSegment::Field(field.identity))
                .as_ref()
                .is_some_and(Vec::is_empty)
            {
                continue;
            }
            if field.relevance.is_erased() {
                return None;
            }
            let child_type = match &field.field_type {
                StructuralFieldType::Structural(child) => Some(*child),
                scalar if scalar.scalar_type().is_some() => None,
                bytes => match bytes.byte_sequence_carrier()? {
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => None,
                    _ => Some(crate::inline_carrier_declaration(types.iter(), field)?.id),
                },
            };
            if let Some(child_type) = child_type {
                count = count.checked_add(child(
                    types,
                    child_type,
                    StructuralPathSegment::Field(field.identity),
                    moved,
                    regions,
                    prefix,
                    max_residuals,
                    residuals,
                    emit,
                    admitted_case,
                )?)?;
            }
        }
        (count <= max_residuals).then_some(count)
    }

    fn region_children<'a>(
        regions: Option<&[&'a [StructuralPathSegment]]>,
        segment: StructuralPathSegment,
    ) -> Option<Vec<&'a [StructuralPathSegment]>> {
        let paths = regions?
            .iter()
            .filter_map(|path| {
                let (head, tail) = path.split_first()?;
                (*head == segment).then_some(tail)
            })
            .collect::<Vec<_>>();
        (!paths.iter().any(|path| path.is_empty())).then_some(paths)
    }

    fn child(
        types: &[StructuralTypeDeclaration],
        child_type: StructuralTypeId,
        segment: StructuralPathSegment,
        moved: &[&[StructuralPathSegment]],
        regions: Option<&[&[StructuralPathSegment]]>,
        prefix: &mut Vec<StructuralPathSegment>,
        max_residuals: usize,
        residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
        emit: bool,
        admitted_case: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    ) -> Option<usize> {
        let regions = region_children(regions, segment.clone());
        if regions.as_ref().is_some_and(Vec::is_empty) {
            return Some(0);
        }
        let descendants = moved
            .iter()
            .filter_map(|path| {
                let (head, tail) = path.split_first()?;
                (head == &segment).then_some(tail)
            })
            .collect::<Vec<_>>();
        prefix.push(segment);
        let count = if descendants.is_empty() && regions.is_none() {
            if emit {
                if residuals.len() == max_residuals {
                    return None;
                }
                residuals.push((prefix.clone(), child_type));
            }
            1
        } else if descendants.iter().all(|path| !path.is_empty()) {
            visit(
                types,
                child_type,
                &descendants,
                regions.as_deref(),
                prefix,
                max_residuals,
                residuals,
                emit,
                admitted_case,
            )?
        } else if descendants.len() != 1 {
            return None;
        } else {
            0
        };
        prefix.pop();
        Some(count)
    }
}
pub(crate) mod primitive_place {

    use crate::exactly_one::exactly_one;
    use crate::structural::static_path::canonical_structural_path_tip;
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IntegerSign, IntegerType, ScalarType, StructuralTypeId,
    };
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
        StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    };

    pub fn primitive_place_type<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Option<ScalarType> {
        if path
            .iter()
            .any(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)))
        {
            return None;
        }
        if let Some((CanonicalStructuralPathSegment::Field(field), carrier)) = path.split_last() {
            let declaration = canonical_structural_path_tip(declarations.clone(), root, carrier)?;
            if let StructuralTypeShape::Record { fields, .. } = &declaration.shape {
                let field = exactly_one(fields.iter(), |candidate| candidate.id == *field)?;
                if field.relevance.is_erased() {
                    return None;
                }
                if let Some(scalar) = plain_scalar_field(&field.field_type) {
                    return Some(scalar);
                }
            }
        }
        match canonical_structural_path_tip(declarations, root, path)?.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
            _ => None,
        }
    }

    pub fn fixed_array_place_shape<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Option<(ScalarType, u64)> {
        if path
            .iter()
            .any(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)))
        {
            return None;
        }
        let declaration = canonical_structural_path_tip(declarations.clone(), root, path)?;
        let StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } = declaration.shape
        else {
            return None;
        };
        let element_declaration =
            exactly_one(declarations, |declaration| declaration.id == element)?;
        match element_declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => Some((scalar, length)),
            _ => None,
        }
    }

    pub fn primitive_projection_type<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<ScalarType> {
        if path
            .iter()
            .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
        {
            return None;
        }
        walk_primitive_projection(declarations, root, path, plain_scalar_field, |_| {})?
            .primitive_scalar()
    }

    pub fn primitive_projection_declaration<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        match walk_primitive_projection(
            declarations.clone(),
            root,
            path,
            plain_scalar_field,
            |_| {},
        )? {
            ProjectionTip::Declared(declaration) => {
                return matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
                    .then_some(declaration.id);
            }
            ProjectionTip::ScalarField(_) => {
                return runtime_structural_path_tip(declarations, root, path)
                    .filter(|declaration| {
                        matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
                    })
                    .map(|declaration| declaration.id);
            }
            ProjectionTip::ByteElement => {}
        }
        let scalar = primitive_projection_type(declarations.clone(), root, path)?;
        let mut matching = declarations.filter(|declaration| {
            declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar)
                && declaration.scalar_restriction.is_none()
        });
        let declaration = matching.next()?;
        matching.next().is_none().then_some(declaration.id)
    }

    pub fn primitive_projection_restriction<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<&'a StructuralFieldType> {
        if let Some(field) = primitive_read_field(declarations.clone(), root, path)
            && matches!(
                field.field_type,
                StructuralFieldType::Scalar(_)
                    | StructuralFieldType::IeeeFloat(_)
                    | StructuralFieldType::BoundedInteger(_)
                    | StructuralFieldType::RefinedScalar(_)
            )
        {
            return crate::scalar_field_requires_evidence(&field.field_type)
                .then_some(&field.field_type);
        }
        runtime_structural_path_tip(declarations, root, path)?
            .scalar_restriction
            .as_ref()
    }

    pub fn primitive_read_requirement<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        result: semantic_vocabulary::ValueId,
    ) -> Option<semantic_vocabulary::Proposition> {
        let restriction = primitive_projection_restriction(declarations.clone(), root, path)
            .or_else(|| {
                primitive_read_field(declarations, root, path).map(|field| &field.field_type)
            })?;
        crate::scalar_field_requirement(restriction, result)
    }

    pub fn borrowed_primitive_projection_declaration<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        expected: Option<StructuralTypeId>,
    ) -> Option<StructuralTypeId> {
        let expected = match expected {
            Some(expected) => Some(crate::structural_declaration(
                declarations.clone(),
                expected,
            )?),
            None => None,
        };
        let tip = walk_primitive_projection(
            declarations.clone(),
            root,
            path,
            |field| {
                if expected
                    .is_some_and(|expected| expected.scalar_restriction.as_ref() == Some(field))
                {
                    return field.scalar_type();
                }
                match field.shared_observation_shape()? {
                    StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
                    _ => None,
                }
            },
            |_| {},
        )?;
        if let ProjectionTip::Declared(declaration) = tip
            && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
        {
            return Some(declaration.id);
        }
        let scalar = tip.primitive_scalar()?;
        if let Some(field) = crate::primitive_read_field(declarations.clone(), root, path) {
            let carrier = match expected {
                Some(expected) => expected,
                None => crate::structural_declaration(declarations.clone(), field.inline_carrier?)?,
            };
            return field
                .field_type
                .borrowed_presentation_type(StructuralAccess::SharedBorrow, Some(carrier))?
                .declares_leaf(carrier)
                .then_some(carrier.id);
        }
        let mut matching = declarations.filter(|declaration| {
            declaration.shape == StructuralTypeShape::PrimitiveScalar(scalar)
                && declaration.scalar_restriction.is_none()
        });
        let declaration = matching.next()?;
        matching.next().is_none().then_some(declaration.id)
    }

    pub fn primitive_read_projection_type<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<ScalarType> {
        walk_primitive_projection(
            declarations,
            root,
            path,
            primitive_read_scalar_field,
            |_| {},
        )
        .and_then(|tip| tip.primitive_scalar())
    }

    pub fn primitive_read_field<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<&'a StructuralFieldDeclaration> {
        let (StructuralPathSegment::Field(_), _) = path.split_last()? else {
            return None;
        };
        let field = crate::runtime_structural_field(declarations, root, path)?;
        integer_or_boolean_field(&field.field_type)?;
        Some(field)
    }

    fn integer_or_boolean_field(field: &StructuralFieldType) -> Option<ScalarType> {
        match field {
            StructuralFieldType::Scalar(ScalarType::Boolean | ScalarType::Integer(_))
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::RefinedScalar(_) => field.scalar_type(),
            _ => None,
        }
    }

    // Path identity and read typing must recognize the same primitive leaves.
    // Atomic and byte storage use their own operations rather than this reader.
    fn primitive_read_scalar_field(field: &StructuralFieldType) -> Option<ScalarType> {
        plain_scalar_field(field).or_else(|| integer_or_boolean_field(field))
    }

    pub fn canonical_primitive_path<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<Vec<CanonicalStructuralPathSegment>> {
        if path
            .iter()
            .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
        {
            return None;
        }
        canonical_primitive_read_path(declarations, root, path)
    }

    pub fn canonical_primitive_read_path<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<Vec<CanonicalStructuralPathSegment>> {
        let mut canonical = Some(Vec::with_capacity(path.len()));
        let tip = walk_primitive_projection(
            declarations,
            root,
            path,
            primitive_read_scalar_field,
            |step| match (step, &mut canonical) {
                (Some(segment), Some(canonical)) => canonical.push(segment),
                (None, canonical) => *canonical = None,
                (Some(_), None) => {}
            },
        )?;
        if matches!(tip, ProjectionTip::ByteElement) {
            return None;
        }
        tip.primitive_scalar()?;
        canonical
    }

    pub fn canonical_static_projection<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<(
        Vec<CanonicalStructuralPathSegment>,
        &'a StructuralTypeDeclaration,
    )> {
        let mut canonical = Some(Vec::with_capacity(path.len()));
        let ProjectionTip::Declared(tip) = walk_primitive_projection(
            declarations,
            root,
            path,
            plain_scalar_field,
            |step| match (step, &mut canonical) {
                (Some(segment), Some(canonical)) => canonical.push(segment),
                (None, canonical) => *canonical = None,
                (Some(_), None) => {}
            },
        )?
        else {
            return None;
        };
        Some((canonical?, tip))
    }

    pub enum Position<Scalar = ScalarType> {
        Declared(StructuralTypeId),
        ScalarField(Scalar),
        ByteElement,
    }

    /// A scalar field is a stored leaf even without a synthetic structural
    /// carrier row. Use the same primitive reader grammar as static paths;
    /// callers still distinguish byte elements and enforce their own access.
    pub fn runtime_read_projection_tip<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<Position> {
        match walk_primitive_projection(
            declarations,
            root,
            path,
            primitive_read_scalar_field,
            |_| {},
        )? {
            ProjectionTip::Declared(declaration) => Some(Position::Declared(declaration.id)),
            ProjectionTip::ScalarField(Some(scalar)) => Some(Position::ScalarField(scalar)),
            ProjectionTip::ScalarField(None) => None,
            ProjectionTip::ByteElement => Some(Position::ByteElement),
        }
    }

    pub fn runtime_structural_path_tip<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<&'a StructuralTypeDeclaration> {
        match walk_primitive_projection(
            declarations.clone(),
            root,
            path,
            plain_scalar_field,
            |_| {},
        )? {
            ProjectionTip::Declared(tip) => Some(tip),
            ProjectionTip::ScalarField(_) => {
                let field = crate::runtime_structural_field(declarations.clone(), root, path)?;
                crate::inline_carrier_declaration(declarations, field)
            }
            ProjectionTip::ByteElement => None,
        }
    }

    pub fn runtime_structural_copy_type<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        expected: Option<StructuralTypeId>,
    ) -> Option<StructuralTypeId> {
        for (position, segment) in path.iter().enumerate() {
            if matches!(segment, StructuralPathSegment::Referent) {
                let parent =
                    runtime_structural_path_tip(declarations.clone(), root, &path[..position])?;
                if !matches!(parent.shape, StructuralTypeShape::Reference { access, .. }
                    if access != terminal_psi::StructuralAccess::WriteOnlyBorrow)
                {
                    return None;
                }
            }
        }
        if matches!(path.last(), Some(StructuralPathSegment::Field(_))) {
            let field = crate::runtime_structural_field(declarations.clone(), root, path)?;
            if let StructuralFieldType::Structural(child) = field.field_type {
                return Some(child);
            }
            let expected =
                crate::structural_declaration(declarations, expected.or(field.inline_carrier)?)?;
            return field
                .field_type
                .leaf_copy_type(Some(expected))?
                .declares_leaf(expected)
                .then_some(expected.id);
        }
        runtime_structural_path_tip(declarations, root, path).map(|declaration| declaration.id)
    }

    pub fn projection_stays_in_inline_storage<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> bool {
        walk_primitive_projection_with_policy(
            declarations.clone(),
            root,
            path,
            plain_scalar_field,
            |_| {},
            |current, segment| {
                if matches!(segment, StructuralPathSegment::Referent) {
                    return false;
                }
                !matches!(
                    segment,
                    StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::RuntimeIndex { .. }
                        | StructuralPathSegment::RepresentationView { .. }
                ) || current.is_none_or(|current| {
                    !crate::descriptors::descriptor_results::is_descriptor(
                        declarations.clone(),
                        current,
                    )
                })
            },
            None,
        )
        .is_some()
    }

    pub fn inline_backing_container(
        declarations: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        projection: &crate::borrowed_storage::Projection,
    ) -> Option<Vec<StructuralPathSegment>> {
        let container = projection.enclosing_storage()?;
        if !projection_stays_in_inline_storage(declarations.iter(), root, &projection.path)
            || !projection_stays_in_inline_storage(declarations.iter(), root, &container.path)
        {
            return None;
        }
        match walk_primitive_projection(
            declarations.iter(),
            root,
            &container.path,
            plain_scalar_field,
            |_| {},
        )? {
            ProjectionTip::Declared(tip)
                if crate::descriptor_results::DescriptorSlotKind::BorrowedStorage
                    .is_slot(declarations, tip.id)
                    || crate::descriptor_results::DescriptorSlotKind::AccessOrigins
                        .is_slot(declarations, tip.id) =>
            {
                None
            }
            _ => Some(container.path),
        }
    }

    pub struct StructuralStorageSelection<'path> {
        path: &'path [StructuralPathSegment],
        pub place_count: usize,
    }

    impl<'path> StructuralStorageSelection<'path> {
        pub fn new<'types>(
            declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            root: StructuralTypeId,
            path: &'path [StructuralPathSegment],
        ) -> Option<Self> {
            runtime_structural_path_tip(declarations.clone(), root, path)?;
            let mut place_count = 1usize;
            for (position, segment) in path.iter().enumerate() {
                if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
                    let current =
                        runtime_structural_path_tip(declarations.clone(), root, &path[..position])?;
                    let terminal_psi::StructuralTypeShape::FixedArray {
                        length: terminal_psi::ArrayExtent::Literal(length),
                        ..
                    } = current.shape
                    else {
                        return None;
                    };
                    place_count = place_count.checked_mul(usize::try_from(length).ok()?)?;
                }
            }
            Some(Self { path, place_count })
        }

        pub fn relative_path<'stored>(
            &self,
            stored: &'stored [StructuralPathSegment],
        ) -> Option<&'stored [StructuralPathSegment]> {
            let prefix = stored.get(..self.path.len())?;
            prefix
                .iter()
                .zip(self.path)
                .all(|(resident, selected)| {
                    resident == selected
                        || matches!(
                            (selected, resident),
                            (
                                StructuralPathSegment::RuntimeIndex { .. },
                                StructuralPathSegment::FixedIndex(_)
                            )
                        )
                })
                .then(|| &stored[self.path.len()..])
        }
    }

    pub fn stored_descriptor_paths<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        maximum: usize,
    ) -> Option<Vec<Vec<StructuralPathSegment>>> {
        if maximum == 0 {
            return None;
        }
        runtime_structural_path_tip(declarations.clone(), root, path)?;
        let mut candidates = vec![Vec::new()];
        for (position, segment) in path.iter().enumerate() {
            if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
                let current =
                    runtime_structural_path_tip(declarations.clone(), root, &path[..position])?;
                let terminal_psi::StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Literal(length),
                    ..
                } = current.shape
                else {
                    return None;
                };
                let length = usize::try_from(length).ok()?;
                let count = candidates.len().checked_mul(length)?;
                if count == 0 || count > maximum {
                    return None;
                }
                let mut expanded = Vec::with_capacity(count);
                for candidate in candidates {
                    for index in 0..length {
                        let mut selected = candidate.clone();
                        selected.push(StructuralPathSegment::FixedIndex(
                            u64::try_from(index).ok()?,
                        ));
                        expanded.push(selected);
                    }
                }
                candidates = expanded;
            } else {
                for candidate in &mut candidates {
                    candidate.push(segment.clone());
                }
            }
        }
        Some(candidates)
    }

    enum ProjectionTip<'a> {
        Declared(&'a StructuralTypeDeclaration),
        ScalarField(Option<ScalarType>),
        ByteElement,
    }

    fn plain_scalar_field(field: &StructuralFieldType) -> Option<ScalarType> {
        match field {
            StructuralFieldType::Scalar(scalar @ ScalarType::IeeeFloat(_)) => Some(*scalar),
            StructuralFieldType::IeeeFloat(format) => Some(ScalarType::IeeeFloat(*format)),
            _ => None,
        }
    }

    pub fn byte_carrier_geometry_matches(
        field: ByteSequenceCarrier,
        declared: ByteSequenceCarrier,
    ) -> bool {
        match (field, declared) {
            (
                ByteSequenceCarrier::BoundedOwned { capacity: field },
                ByteSequenceCarrier::BoundedOwned { capacity: declared },
            ) => field == declared,
            (
                ByteSequenceCarrier::BorrowedView { access: field },
                ByteSequenceCarrier::BorrowedView { access: declared },
            ) => declared.is_none() || declared == field,
            (
                ByteSequenceCarrier::BoundedView {
                    capacity: field,
                    access: field_access,
                },
                ByteSequenceCarrier::BoundedView {
                    capacity: declared,
                    access: declared_access,
                },
            ) => {
                field == declared && (declared_access.is_none() || declared_access == field_access)
            }
            _ => false,
        }
    }

    impl ProjectionTip<'_> {
        fn primitive_scalar(&self) -> Option<ScalarType> {
            match self {
                Self::ScalarField(scalar) => *scalar,
                Self::Declared(declaration) => match declaration.shape {
                    StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
                    _ => None,
                },
                Self::ByteElement => Some(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                )),
            }
        }
    }

    fn walk_primitive_projection<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        scalar_field: impl Fn(&StructuralFieldType) -> Option<ScalarType>,
        step: impl FnMut(Option<CanonicalStructuralPathSegment>),
    ) -> Option<ProjectionTip<'a>> {
        walk_primitive_projection_with_policy(
            declarations,
            root,
            path,
            scalar_field,
            step,
            |_, _| true,
            None,
        )
    }

    pub fn projection_storage_dependencies<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        dependencies: &mut std::collections::BTreeSet<StructuralTypeId>,
    ) -> Option<Option<StructuralTypeId>> {
        let backing = match path.split_last() {
            Some((StructuralPathSegment::FixedRange { .. }, backing)) => backing,
            _ => path,
        };
        let tip = walk_primitive_projection_with_policy(
            declarations,
            root,
            backing,
            plain_scalar_field,
            |_| {},
            |_, _| true,
            Some(dependencies),
        )?;
        Some(match tip {
            ProjectionTip::Declared(declaration) => Some(declaration.id),
            ProjectionTip::ScalarField(_) | ProjectionTip::ByteElement => None,
        })
    }

    fn walk_primitive_projection_with_policy<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        scalar_field: impl Fn(&StructuralFieldType) -> Option<ScalarType>,
        mut step: impl FnMut(Option<CanonicalStructuralPathSegment>),
        mut may_step: impl FnMut(Option<StructuralTypeId>, &StructuralPathSegment) -> bool,
        mut dependencies: Option<&mut std::collections::BTreeSet<StructuralTypeId>>,
    ) -> Option<ProjectionTip<'a>> {
        let unique = |id: StructuralTypeId| {
            let mut matching = declarations
                .clone()
                .filter(move |declaration| declaration.id == id);
            let declaration = matching.next()?;
            matching.next().is_none().then_some(declaration)
        };
        let mut position = Position::Declared(root);
        let mut payload_fields = None;
        for (ordinal, segment) in path.iter().enumerate() {
            let declared = match position {
                Position::Declared(root) => Some(root),
                _ => None,
            };
            if let (Some(dependencies), Some(declared)) = (dependencies.as_mut(), declared) {
                dependencies.insert(declared);
            }
            if !may_step(declared, segment) {
                return None;
            }

            if let StructuralPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } = segment
            {
                if payload_fields.is_some()
                    || matches!(position, Position::Declared(root)
                        if matches!(unique(root)?.shape, StructuralTypeShape::Reference { .. }))
                {
                    return None;
                }
                let view = crate::references::reference_views::representation_view_declaration(
                    declarations.clone(),
                    *structural_type,
                    *stored_scalar,
                )?;
                step(None);
                position = Position::Declared(view.id);
                continue;
            }
            let Position::Declared(root) = position else {
                return None;
            };
            let declaration = unique(root)?;
            if let StructuralPathSegment::FixedRange { start, end } = segment {
                let StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Literal(length),
                    ..
                } = declaration.shape
                else {
                    return None;
                };
                if payload_fields.is_some()
                    || start > end
                    || *end > length
                    || !matches!(
                        path.get(ordinal + 1),
                        Some(StructuralPathSegment::RepresentationView { .. })
                    )
                {
                    return None;
                }
                step(None);
                continue;
            }
            if let StructuralPathSegment::Case(case) = segment {
                if payload_fields.is_some() {
                    return None;
                }
                let cases = declaration.shape.sum_cases()?;
                let selected = exactly_one(cases.iter(), |candidate| candidate.id == *case)?;
                payload_fields = Some(selected.fields.as_slice());
                step(Some(CanonicalStructuralPathSegment::Case(*case)));
                continue;
            }
            if payload_fields.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
                return None;
            }
            position = match (segment, &declaration.shape) {
                (
                    StructuralPathSegment::Referent,
                    StructuralTypeShape::Reference { referent, .. },
                ) => {
                    step(None);
                    Position::Declared(*referent)
                }
                (StructuralPathSegment::Field(identity), shape) => {
                    let fields = match payload_fields.take() {
                        Some(fields) => fields,
                        None => match shape {
                            terminal_psi::record_or_mixed_shape_pattern!(fields) => {
                                fields.as_slice()
                            }
                            _ => return None,
                        },
                    };
                    let field = terminal_psi::live_field(fields, *identity)?;
                    step(Some(CanonicalStructuralPathSegment::Field(field.id)));
                    match field.field_type {
                        StructuralFieldType::Structural(child) => Position::Declared(child),
                        _ => match field.field_type.byte_sequence_carrier() {
                            Some(carrier) => {
                                if !matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. }) {
                                    step(None);
                                }
                                Position::Declared(
                                    crate::inline_carrier_declaration(declarations.clone(), field)?
                                        .id,
                                )
                            }
                            None => Position::ScalarField((
                                &field.field_type,
                                scalar_field(&field.field_type),
                            )),
                        },
                    }
                }
                (
                    StructuralPathSegment::FixedIndex(index),
                    StructuralTypeShape::FixedArray { element, length },
                ) if length.literal().is_none_or(|length| *index < length) => {
                    step(
                        length
                            .literal()
                            .map(|_| CanonicalStructuralPathSegment::FixedIndex(*index)),
                    );
                    Position::Declared(*element)
                }
                (
                    StructuralPathSegment::FixedIndex(_),
                    StructuralTypeShape::ElementView { element, .. },
                ) => {
                    step(None);
                    Position::Declared(*element)
                }
                (
                    StructuralPathSegment::RuntimeIndex { .. },
                    StructuralTypeShape::FixedArray { element, .. }
                    | StructuralTypeShape::ElementView { element, .. },
                ) => {
                    step(None);
                    Position::Declared(*element)
                }
                (
                    StructuralPathSegment::FixedIndex(index),
                    StructuralTypeShape::ByteSequence(carrier),
                ) => {
                    let capacity = match carrier {
                        ByteSequenceCarrier::BoundedOwned { capacity }
                        | ByteSequenceCarrier::BoundedView { capacity, .. } => Some(capacity),
                        ByteSequenceCarrier::BorrowedView { .. } => None,
                    };
                    if capacity.is_some_and(|capacity| index >= capacity) {
                        return None;
                    }
                    step(capacity.map(|_| CanonicalStructuralPathSegment::FixedIndex(*index)));
                    Position::ByteElement
                }
                (
                    StructuralPathSegment::RuntimeIndex { .. },
                    StructuralTypeShape::ByteSequence(_),
                ) => {
                    step(None);
                    Position::ByteElement
                }
                _ => return None,
            };
        }
        if payload_fields.is_some() && dependencies.is_none() {
            return None;
        }
        match position {
            Position::Declared(root) => {
                let declaration = unique(root)?;
                if let Some(dependencies) = dependencies {
                    dependencies.insert(root);
                }
                Some(ProjectionTip::Declared(declaration))
            }
            Position::ScalarField((_, scalar)) => Some(ProjectionTip::ScalarField(scalar)),
            Position::ByteElement => Some(ProjectionTip::ByteElement),
        }
    }

    pub(crate) fn projection_tip_with_policy<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        may_step: impl FnMut(Option<StructuralTypeId>, &StructuralPathSegment) -> bool,
    ) -> Option<Option<StructuralTypeId>> {
        match walk_primitive_projection_with_policy(
            declarations,
            root,
            path,
            plain_scalar_field,
            |_| {},
            may_step,
            None,
        )? {
            ProjectionTip::Declared(tip) => Some(Some(tip.id)),
            ProjectionTip::ScalarField(_) | ProjectionTip::ByteElement => Some(None),
        }
    }
}
pub(crate) mod record_field {
    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralCaseId, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralFieldDeclaration, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape, find_structural_case, live_field_by_id,
    };

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum SequenceFieldObservation {
        Length,
        Contents,
    }

    pub fn sequence_field_allows_observation<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        field: &terminal_psi::StructuralFieldType,
        observation: SequenceFieldObservation,
    ) -> bool {
        let access = if let Some(carrier) = field.byte_sequence_carrier() {
            match carrier {
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => StructuralAccess::Owned,
                terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. } => {
                    access.unwrap_or(StructuralAccess::SharedBorrow)
                }
            }
        } else if let Some(identity) = field.as_structural() {
            let Some(declaration) = exactly_one(types, |row| row.id == identity) else {
                return false;
            };
            match declaration.shape {
                StructuralTypeShape::ElementView { access, .. }
                | StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { access },
                ) => access.unwrap_or(StructuralAccess::SharedBorrow),
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                ) => StructuralAccess::Owned,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
                ) => access.unwrap_or(StructuralAccess::SharedBorrow),
                _ => return false,
            }
        } else {
            return false;
        };
        observation == SequenceFieldObservation::Length
            || access != StructuralAccess::WriteOnlyBorrow
    }

    pub struct RecordFieldCarrier {
        pub structural_type: StructuralTypeId,
        pub path: Vec<StructuralPathSegment>,
    }

    pub fn record_field_carrier<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Option<RecordFieldCarrier> {
        let (structural_type, runtime_path, _) = walk_record_fields(types.clone(), root, path)?;
        matches!(
            crate::structural_shape(types.clone(), structural_type)?,
            StructuralTypeShape::Record { .. } | StructuralTypeShape::Mixed { .. }
        )
        .then_some(RecordFieldCarrier {
            structural_type,
            path: runtime_path,
        })
    }

    pub struct CasePayloadCarrier<'types> {
        pub sum: StructuralTypeId,
        pub case: StructuralCaseId,
        pub path: Vec<StructuralPathSegment>,
        pub fields: &'types [StructuralFieldDeclaration],
    }

    pub fn case_payload_carrier<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Option<CasePayloadCarrier<'types>> {
        let (CanonicalStructuralPathSegment::Case(case), prefix) = path.split_last()? else {
            return None;
        };
        let (sum, runtime_path, _) = walk_record_fields(types.clone(), root, prefix)?;
        let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
            crate::structural_shape(types.clone(), sum)?
        else {
            return None;
        };
        let selected = find_structural_case(cases, *case)?;
        Some(CasePayloadCarrier {
            sum,
            case: *case,
            path: runtime_path,
            fields: &selected.fields,
        })
    }

    pub fn observed_field_declaration<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> Option<&'types StructuralFieldDeclaration> {
        let fields = if let Some(carrier) = case_payload_carrier(types.clone(), root, path) {
            carrier.fields
        } else {
            let carrier = record_field_carrier(types.clone(), root, path)?;
            let (terminal_psi::record_or_mixed_shape_pattern!(fields)) =
                crate::structural_shape(types.clone(), carrier.structural_type)?
            else {
                return None;
            };
            fields
        };
        live_field_by_id(fields, field)
    }

    pub fn static_field_projection<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> Option<(
        Vec<CanonicalStructuralPathSegment>,
        &'types StructuralFieldDeclaration,
    )> {
        let (prefix, selected_case) = match path.split_last() {
            Some((StructuralPathSegment::Case(case), prefix)) => (prefix, Some(*case)),
            _ => (path, None),
        };
        let (mut canonical, carrier) =
            crate::canonical_static_projection(types.clone(), root, prefix)?;
        let fields = if let Some(case) = selected_case {
            let cases = carrier.shape.sum_cases()?;
            let selected = exactly_one(cases.iter(), |candidate| candidate.id == case)?;
            canonical.push(CanonicalStructuralPathSegment::Case(case));
            selected.fields.as_slice()
        } else {
            carrier.shape.direct_fields()?
        };
        let mut matching = fields
            .iter()
            .filter(|candidate| candidate.id == field && !candidate.relevance.is_erased());
        let field = matching.next()?;
        matching.next().is_none().then_some((canonical, field))
    }

    pub fn projected_field_declaration<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> Option<&'types StructuralFieldDeclaration> {
        let fields = projected_fields(types, root, path)?;
        let mut matching = fields
            .iter()
            .filter(|candidate| candidate.id == field && !candidate.relevance.is_erased());
        let field = matching.next()?;
        matching.next().is_none().then_some(field)
    }

    pub fn runtime_structural_field<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<&'types StructuralFieldDeclaration> {
        let (StructuralPathSegment::Field(identity), parent) = path.split_last()? else {
            return None;
        };
        terminal_psi::live_field(projected_fields(types, root, parent)?, *identity)
    }

    fn projected_fields<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<&'types [StructuralFieldDeclaration]> {
        let (prefix, selected_case) = match path.split_last() {
            Some((StructuralPathSegment::Case(case), prefix)) => (prefix, Some(*case)),
            _ => (path, None),
        };
        let carrier = crate::runtime_structural_path_tip(types, root, prefix)?;
        let fields = if let Some(case) = selected_case {
            let selected = exactly_one(carrier.shape.sum_cases()?.iter(), |candidate| {
                candidate.id == case
            })?;
            selected.fields.as_slice()
        } else {
            carrier.shape.direct_fields()?
        };
        Some(fields)
    }

    pub fn observed_field_backing_access<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
        access: StructuralAccess,
    ) -> Option<StructuralAccess> {
        let prefix = match path.split_last() {
            Some((CanonicalStructuralPathSegment::Case(_), prefix)) => prefix,
            _ => path,
        };
        let (_, _, crosses_view) = walk_record_fields(types, root, prefix)?;
        Some(
            if crosses_view && access != StructuralAccess::WriteOnlyBorrow {
                StructuralAccess::SharedBorrow
            } else {
                access
            },
        )
    }

    fn walk_record_fields<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Option<(StructuralTypeId, Vec<StructuralPathSegment>, bool)> {
        let mut structural_type = root;
        let mut runtime_path = Vec::with_capacity(path.len());
        let mut crosses_view = false;
        let mut payload_fields = None;
        for segment in path {
            let declaration = crate::structural_declaration(types.clone(), structural_type)?;
            if matches!(segment, CanonicalStructuralPathSegment::Referent) {
                if payload_fields.is_some() {
                    return None;
                }
                let StructuralTypeShape::Reference { referent, access } = declaration.shape else {
                    return None;
                };
                if access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                    return None;
                }
                runtime_path.push(StructuralPathSegment::Referent);
                structural_type = referent;
                crosses_view = true;
                continue;
            }
            if let CanonicalStructuralPathSegment::Case(case) = segment {
                if payload_fields.is_some() {
                    return None;
                }
                let cases = declaration.shape.sum_cases()?;
                payload_fields = Some(
                    cases
                        .iter()
                        .find(|candidate| candidate.id == *case)?
                        .fields
                        .as_slice(),
                );
                runtime_path.push(StructuralPathSegment::Case(*case));
                continue;
            }
            if payload_fields.is_some()
                && !matches!(segment, CanonicalStructuralPathSegment::Field(_))
            {
                return None;
            }
            if let CanonicalStructuralPathSegment::FixedIndex(element_index) = segment {
                let declaration = crate::structural_declaration(types.clone(), structural_type)?;
                let element = match &declaration.shape {
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } => {
                        if *element_index >= *length {
                            return None;
                        }
                        *element
                    }
                    StructuralTypeShape::ElementView { element, .. } => {
                        crosses_view = true;
                        *element
                    }
                    _ => return None,
                };
                runtime_path.push(StructuralPathSegment::FixedIndex(*element_index));
                structural_type = element;
                continue;
            }
            let CanonicalStructuralPathSegment::Field(field) = segment else {
                return None;
            };
            let declaration = crate::structural_declaration(types.clone(), structural_type)?;
            let fields = match payload_fields.take() {
                Some(fields) => fields,
                None => declaration.shape.direct_fields()?,
            };
            let field = live_field_by_id(fields, *field)?;
            let child = field.field_type.as_structural()?;
            runtime_path.push(StructuralPathSegment::Field(field.identity));
            structural_type = child;
        }
        if payload_fields.is_some() {
            return None;
        }
        Some((structural_type, runtime_path, crosses_view))
    }
}
pub(crate) mod shared_mutation {

    use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralTypeId};
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess, StructuralFieldType, StructuralTypeDeclaration,
        StructuralTypeShape,
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum SharedMutation {
        None,
        Inline,
        Unbounded,
    }

    pub fn shared_mutation<'types>(
        declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
    ) -> SharedMutation {
        if let Some((CanonicalStructuralPathSegment::Field(field), prefix)) = path.split_last() {
            return crate::observed_field_declaration(declarations.clone(), root, prefix, *field)
                .map(|field| field_mutation(declarations, &field.field_type, &mut Vec::new()))
                .unwrap_or(SharedMutation::Unbounded);
        }
        let Some(selected) = crate::canonical_structural_path_tip(declarations.clone(), root, path)
        else {
            return SharedMutation::Unbounded;
        };
        type_mutation(declarations, selected.id, &mut Vec::new())
    }

    fn type_mutation<'types>(
        declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        identity: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
    ) -> SharedMutation {
        if active.contains(&identity) {
            return SharedMutation::None;
        }
        let mut matching = declarations
            .clone()
            .filter(|candidate| candidate.id == identity);
        let Some(declaration) = matching.next() else {
            return SharedMutation::Unbounded;
        };
        if matching.next().is_some() {
            return SharedMutation::Unbounded;
        }
        active.push(identity);
        let result = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => SharedMutation::None,
            StructuralTypeShape::AtomicCell(_) => SharedMutation::Inline,
            StructuralTypeShape::ByteSequence(carrier) => byte_mutation(*carrier),
            StructuralTypeShape::Reference { referent, access } => {
                if *access != StructuralAccess::SharedBorrow
                    || type_mutation(declarations.clone(), *referent, active)
                        != SharedMutation::None
                {
                    SharedMutation::Unbounded
                } else {
                    SharedMutation::None
                }
            }
            StructuralTypeShape::ElementView { element, .. } => {
                if type_mutation(declarations.clone(), *element, active) == SharedMutation::None {
                    SharedMutation::None
                } else {
                    SharedMutation::Unbounded
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                if length.literal() == Some(0) {
                    SharedMutation::None
                } else {
                    type_mutation(declarations.clone(), *element, active)
                }
            }
            StructuralTypeShape::Record { fields, .. } => {
                fields_mutation(declarations.clone(), fields.iter(), active)
            }
            StructuralTypeShape::Sum { cases } => fields_mutation(
                declarations.clone(),
                cases.iter().flat_map(|case| &case.fields),
                active,
            ),
            StructuralTypeShape::Mixed { fields, cases } => fields_mutation(
                declarations.clone(),
                fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields)),
                active,
            ),
        };
        active.pop();
        result
    }

    fn fields_mutation<'types, 'fields>(
        declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        fields: impl Iterator<Item = &'fields terminal_psi::StructuralFieldDeclaration>,
        active: &mut Vec<StructuralTypeId>,
    ) -> SharedMutation {
        fields
            .filter(|field| !field.relevance.is_erased())
            .map(|field| field_mutation(declarations.clone(), &field.field_type, active))
            .max()
            .unwrap_or(SharedMutation::None)
    }

    fn field_mutation<'types>(
        declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        field: &StructuralFieldType,
        active: &mut Vec<StructuralTypeId>,
    ) -> SharedMutation {
        match field {
            StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_) => {
                SharedMutation::Inline
            }
            StructuralFieldType::Structural(identity) => {
                type_mutation(declarations, *identity, active)
            }
            StructuralFieldType::ByteSequence(carrier)
            | StructuralFieldType::PredicateByteSequence { carrier, .. } => byte_mutation(*carrier),
            StructuralFieldType::Scalar(_)
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::RefinedScalar(_)
            | StructuralFieldType::IeeeFloat(_)
            | StructuralFieldType::Erased { .. } => SharedMutation::None,
        }
    }

    fn byte_mutation(carrier: ByteSequenceCarrier) -> SharedMutation {
        match carrier {
            ByteSequenceCarrier::BorrowedView {
                access: Some(access),
            }
            | ByteSequenceCarrier::BoundedView {
                access: Some(access),
                ..
            } if access != StructuralAccess::SharedBorrow => SharedMutation::Unbounded,
            _ => SharedMutation::None,
        }
    }
}
pub(crate) mod static_path {

    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IntegerSign, IntegerValue, Proposition, ScalarTerm,
        ScalarType, StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        OperationKind, StructuralAccess, StructuralArgument, StructuralFieldType,
        StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    };

    pub fn constant_runtime_index(machine: &TerminalMachine, index: ValueId) -> Option<u64> {
        let producer = exactly_one(machine.operations(), |operation| {
            operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.id == index)
        })?;
        let OperationKind::IntegerConstant { value } = producer.kind else {
            return None;
        };
        match value {
            IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
            IntegerValue::Signed(value) => u64::try_from(value).ok(),
        }
    }

    pub fn runtime_index_bound(
        index: ValueId,
        index_type: ScalarType,
        extent: u64,
    ) -> Option<Proposition> {
        let integer_type = index_type.as_integer()?;
        if integer_type.is_address() {
            return None;
        }
        let subject = ScalarTerm::value(index, index_type);
        let mut conjuncts = Vec::with_capacity(2);
        if integer_type.sign() == IntegerSign::Signed {
            conjuncts.push(Proposition::LessOrEqual(
                ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).ok()?,
                subject.clone(),
            ));
        }
        let extent_value = match integer_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(i128::from(extent)),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(extent)),
        };
        if integer_type.admits(extent_value) {
            conjuncts.push(Proposition::LessThan(
                subject,
                ScalarTerm::integer(integer_type, extent_value).ok()?,
            ));
        }
        Some(match conjuncts.len() {
            0 => Proposition::Truth,
            1 => conjuncts.pop().expect("one conjunct"),
            _ => Proposition::Conjunction(conjuncts),
        })
    }

    pub fn runtime_index_selects(value: IntegerValue, extent: u64) -> Option<u64> {
        let selected = match value {
            IntegerValue::Unsigned(value) => u64::try_from(value).ok()?,
            IntegerValue::Signed(value) => u64::try_from(value).ok()?,
        };
        (selected < extent).then_some(selected)
    }

    pub fn structural_paths_select_same_location(
        left: &[StructuralPathSegment],
        right: &[StructuralPathSegment],
    ) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| match (left, right) {
                    (
                        StructuralPathSegment::RuntimeIndex { index: left, .. },
                        StructuralPathSegment::RuntimeIndex { index: right, .. },
                    ) => left == right,
                    _ => left == right,
                })
    }

    pub fn structural_paths_may_overlap(
        left: &[StructuralPathSegment],
        right: &[StructuralPathSegment],
    ) -> bool {
        for (left, right) in left.iter().zip(right) {
            if matches!(left, StructuralPathSegment::RepresentationView { .. })
                || matches!(right, StructuralPathSegment::RepresentationView { .. })
            {
                return true;
            }
            if let (StructuralPathSegment::Case(left), StructuralPathSegment::Case(right)) =
                (left, right)
                && left != right
            {
                return true;
            }
            let overlaps = match (left, right) {
                (
                    StructuralPathSegment::AllElements,
                    StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::RuntimeIndex { .. }
                    | StructuralPathSegment::FixedRange { .. },
                )
                | (
                    StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::RuntimeIndex { .. }
                    | StructuralPathSegment::FixedRange { .. },
                    StructuralPathSegment::AllElements,
                ) => true,
                (
                    StructuralPathSegment::FixedRange {
                        start: left_start,
                        end: left_end,
                    },
                    StructuralPathSegment::FixedRange {
                        start: right_start,
                        end: right_end,
                    },
                ) => {
                    left_start >= left_end
                        || right_start >= right_end
                        || (left_start < right_end && right_start < left_end)
                }
                (
                    StructuralPathSegment::FixedRange { start, end },
                    StructuralPathSegment::FixedIndex(index),
                )
                | (
                    StructuralPathSegment::FixedIndex(index),
                    StructuralPathSegment::FixedRange { start, end },
                ) => start >= end || (start <= index && index < end),
                (StructuralPathSegment::RuntimeIndex { .. }, _)
                | (_, StructuralPathSegment::RuntimeIndex { .. }) => true,
                _ => left == right,
            };
            if !overlaps {
                return false;
            }
        }
        true
    }
    pub fn interior_member_borrow(parent: &StructuralArgument, child: &StructuralArgument) -> bool {
        let admitted = match parent.access {
            terminal_psi::mutating_access_pair_pattern!() => matches!(
                child.access,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            ),
            StructuralAccess::WriteOnlyBorrow => {
                matches!(child.access, terminal_psi::exclusive_borrow_pair_pattern!())
            }
            StructuralAccess::SharedBorrow => false,
        };
        admitted
            && child.place == parent.place
            && child
                .path
                .strip_prefix(parent.path.as_slice())
                .is_some_and(|suffix| {
                    suffix.iter().any(|segment| {
                        !matches!(segment, StructuralPathSegment::RepresentationView { .. })
                    })
                })
    }

    pub fn canonical_structural_path_tip<'a>(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Option<&'a StructuralTypeDeclaration> {
        let selected = canonical_projection_type(declarations.clone(), root, path, None, None)?;
        let mut matching = declarations.filter(|declaration| declaration.id == selected);
        let declaration = matching.next()?;
        matching.next().is_none().then_some(declaration)
    }

    pub fn canonical_case_leaf_type<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        path.iter()
            .any(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)))
            .then_some(())?;
        canonical_projection_type(types, root, path, None, None)
    }

    pub fn canonical_structural_storage_path<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
    ) -> Option<Vec<StructuralPathSegment>> {
        let mut storage = Vec::with_capacity(path.len());
        canonical_projection_type(types, root, path, Some(&mut storage), None)?;
        Some(storage)
    }

    pub fn canonical_storage_dependencies<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
        dependencies: &mut std::collections::BTreeSet<StructuralTypeId>,
    ) -> Option<()> {
        canonical_projection_type(types, root, path, None, Some(dependencies))?;
        Some(())
    }

    fn canonical_projection_type<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        mut root: StructuralTypeId,
        path: &[CanonicalStructuralPathSegment],
        mut storage: Option<&mut Vec<StructuralPathSegment>>,
        mut dependencies: Option<&mut std::collections::BTreeSet<StructuralTypeId>>,
    ) -> Option<StructuralTypeId> {
        let declaration = |identity| {
            let mut matching = types.clone().filter(|candidate| candidate.id == identity);
            let selected = matching.next()?;
            matching.next().is_none().then_some(selected)
        };
        let mut case_fields: Option<&[terminal_psi::StructuralFieldDeclaration]> = None;
        for segment in path {
            if let Some(dependencies) = dependencies.as_mut() {
                dependencies.insert(root);
            }
            let shape = &declaration(root)?.shape;
            match segment {
                CanonicalStructuralPathSegment::Referent => {
                    if case_fields.is_some() {
                        return None;
                    }
                    let StructuralTypeShape::Reference { referent, .. } = shape else {
                        return None;
                    };
                    if let Some(storage) = storage.as_mut() {
                        storage.push(StructuralPathSegment::Referent);
                    }
                    root = *referent;
                }
                CanonicalStructuralPathSegment::Case(identity) => {
                    if case_fields.is_some() {
                        return None;
                    }
                    let (StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. }) = shape
                    else {
                        return None;
                    };
                    case_fields =
                        Some(&exactly_one(cases.iter(), |case| case.id == *identity)?.fields);
                    if let Some(storage) = storage.as_mut() {
                        storage.push(StructuralPathSegment::Case(*identity));
                    }
                }
                CanonicalStructuralPathSegment::Field(identity) => {
                    let fields = match case_fields.take() {
                        Some(fields) => fields,
                        None => match shape {
                            terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                            _ => return None,
                        },
                    };
                    let field = exactly_one(fields.iter(), |field| field.id == *identity)?;
                    if field.relevance.is_erased() {
                        return None;
                    }
                    let child = match &field.field_type {
                        StructuralFieldType::Structural(child) => *child,
                        leaf => {
                            leaf.canonical_leaf_type()?
                                .leaf_declaration(types.clone())?
                                .id
                        }
                    };
                    if let Some(storage) = storage.as_mut() {
                        storage.push(StructuralPathSegment::Field(field.identity));
                    }
                    root = child;
                }
                CanonicalStructuralPathSegment::FixedIndex(index) => {
                    if case_fields.is_some() {
                        return None;
                    }
                    root = match shape {
                        StructuralTypeShape::FixedArray { element, length }
                            if length
                                .literal()
                                .map_or(dependencies.is_some(), |length| *index < length) =>
                        {
                            *element
                        }
                        StructuralTypeShape::ElementView { element, .. } => *element,
                        _ => return None,
                    };
                    if let Some(storage) = storage.as_mut() {
                        storage.push(StructuralPathSegment::FixedIndex(*index));
                    }
                }
            }
        }
        let selected =
            (case_fields.is_none() || dependencies.is_some()).then_some(declaration(root)?.id)?;
        if let Some(dependencies) = dependencies {
            dependencies.insert(selected);
        }
        Some(selected)
    }

    pub fn structural_memberships_disjoint_from_write(
        whole: &[semantic_vocabulary::StructuralDomainId],
        projected: &[terminal_psi::StructuralPathQualification],
        written: &[StructuralPathSegment],
    ) -> bool {
        whole.is_empty()
            && projected
                .iter()
                .all(|row| !structural_paths_may_overlap(&row.path, written))
    }

    pub fn qualification_path_carrier<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        mut root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        let mut cursor = path;
        loop {
            let count = cursor
                .iter()
                .position(|step| matches!(step, StructuralPathSegment::AllElements))
                .unwrap_or(cursor.len());
            root = crate::runtime_structural_path_tip(types.clone(), root, &cursor[..count])?.id;
            if count == cursor.len() {
                return Some(root);
            }
            let declaration = exactly_one(types.clone(), |row| row.id == root)?;
            root = match declaration.shape {
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element, .. } => element,
                _ => return None,
            };
            cursor = &cursor[count + 1..];
        }
    }
}
pub(crate) mod structural_claims {
    use semantic_vocabulary::StructuralTypeId;
    use std::collections::BTreeSet;
    use terminal_psi::{
        StructuralPathSegment, StructuralResultClaimBinding, StructuralTypeDeclaration,
    };

    pub fn structural_claim_paths_may_overlap<'types>(
        declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        left: &[StructuralPathSegment],
        right: &[StructuralPathSegment],
    ) -> Option<bool> {
        crate::canonical_static_projection(declarations.clone(), root, left)?;
        crate::canonical_static_projection(declarations, root, right)?;
        Some(owned_paths_overlap(left, right))
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ResultClaimCatalogError {
        NonStaticOwnedPath,
        OverlappingPaths,
        ReturnPathMismatch,
    }

    pub fn validate_result_claim_catalog<'types, 'returns>(
        declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        catalogue: &[StructuralResultClaimBinding],
        returns: impl IntoIterator<Item = &'returns [StructuralResultClaimBinding]>,
    ) -> Result<(), ResultClaimCatalogError> {
        crate::canonical_static_projection(declarations.clone(), root, &[])
            .ok_or(ResultClaimCatalogError::NonStaticOwnedPath)?;
        for binding in catalogue {
            crate::canonical_static_projection(declarations.clone(), root, &binding.path)
                .ok_or(ResultClaimCatalogError::NonStaticOwnedPath)?;
        }
        if catalogue.iter().enumerate().any(|(index, binding)| {
            catalogue[index + 1..]
                .iter()
                .any(|other| owned_paths_overlap(&binding.path, &other.path))
        }) {
            return Err(ResultClaimCatalogError::OverlappingPaths);
        }
        matches_paths(
            catalogue.iter().map(|binding| binding.path.as_slice()),
            returns,
        )
        .then_some(())
        .ok_or(ResultClaimCatalogError::ReturnPathMismatch)
    }

    fn owned_paths_overlap(
        left: &[StructuralPathSegment],
        right: &[StructuralPathSegment],
    ) -> bool {
        left.starts_with(right) || right.starts_with(left)
    }

    pub fn result_claim_paths_match_returns<'returns>(
        catalogue: &[Vec<StructuralPathSegment>],
        returns: impl IntoIterator<Item = &'returns [StructuralResultClaimBinding]>,
    ) -> bool {
        matches_paths(catalogue.iter().map(Vec::as_slice), returns)
    }

    fn matches_paths<'catalogue, 'returns>(
        catalogue: impl IntoIterator<Item = &'catalogue [StructuralPathSegment]>,
        returns: impl IntoIterator<Item = &'returns [StructuralResultClaimBinding]>,
    ) -> bool {
        let mut catalogue_paths = BTreeSet::new();
        for path in catalogue {
            if !catalogue_paths.insert(path) {
                return false;
            }
        }
        for bindings in returns {
            let paths = bindings
                .iter()
                .map(|binding| binding.path.as_slice())
                .collect::<BTreeSet<_>>();
            if paths.len() != bindings.len()
                || bindings
                    .windows(2)
                    .any(|pair| pair[0].claim >= pair[1].claim)
                || paths.iter().any(|path| !catalogue_paths.contains(path))
                || catalogue_paths.iter().any(|path| {
                    !path
                        .iter()
                        .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
                        && !paths.contains(path)
                })
            {
                return false;
            }
        }
        true
    }
}
pub(crate) mod structural_declarations {

    use semantic_vocabulary::StructuralTypeId;
    use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

    pub(crate) fn structural_declaration<'types>(
        mut types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        id: StructuralTypeId,
    ) -> Option<&'types StructuralTypeDeclaration> {
        types.find(|declaration| declaration.id == id)
    }

    pub(crate) fn structural_shape<'types>(
        mut types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        id: StructuralTypeId,
    ) -> Option<&'types StructuralTypeShape> {
        Some(&types.find(|declaration| declaration.id == id)?.shape)
    }
}
pub(crate) mod structural_effect {

    use semantic_vocabulary::{
        IntegerSign, IntegerType, ObligationId, PlaceId, Proposition, ScalarTerm, ScalarType,
        ServiceId, StructuralCaseId, StructuralCaseSubject, StructuralFieldId, ValueId,
    };
    use terminal_psi::{Operation, OperationKind, OperationResult, live_field_by_id};

    pub(crate) mod byte_equal {

        use super::StructuralEffectObservation;
        use semantic_vocabulary::{
            ByteSequenceStructuralField, Proposition, ScalarTerm, ScalarType, StructuralTypeId,
        };
        use terminal_psi::{StructuralTypeDeclaration, control_flow::ByteSequenceOperand};

        pub fn byte_equal_equation(
            observation: &StructuralEffectObservation,
            left_root_type: StructuralTypeId,
            right_root_type: StructuralTypeId,
            structural_types: &[StructuralTypeDeclaration],
        ) -> Option<Proposition> {
            let StructuralEffectObservation::ByteSequenceEqualEvaluated {
                left,
                right,
                result,
            } = observation
            else {
                return None;
            };
            let left = operand_field(left, left_root_type, structural_types)?;
            let right = operand_field(right, right_root_type, structural_types)?;
            let (left, right) = if left <= right {
                (left, right)
            } else {
                (right, left)
            };
            Some(Proposition::Implication {
                premise: Box::new(Proposition::Equal(
                    ScalarTerm::value(*result, ScalarType::Boolean),
                    ScalarTerm::Boolean(true),
                )),
                conclusion: Box::new(Proposition::ByteSequenceEqual { left, right }),
            })
        }

        fn operand_field(
            operand: &ByteSequenceOperand,
            root_type: StructuralTypeId,
            structural_types: &[StructuralTypeDeclaration],
        ) -> Option<ByteSequenceStructuralField> {
            let (place, path) =
                super::byte_sequence_field_path(operand, root_type, structural_types)?;
            ByteSequenceStructuralField::new(place, path).ok()
        }
    }
    pub(crate) mod byte_extent {

        use semantic_vocabulary::Proposition;
        use terminal_psi::{Operation, OperationKind};

        use super::StructuralEffectObservation;
        use crate::OperationSemanticError;

        pub fn literal_length_equation(
            observation: &StructuralEffectObservation,
            producer: &Operation,
        ) -> Result<Option<Proposition>, OperationSemanticError> {
            let StructuralEffectObservation::ByteSequenceLengthRead { source, result } =
                observation
            else {
                return Ok(None);
            };
            let OperationKind::EstablishByteSequenceLiteral {
                destination, bytes, ..
            } = &producer.kind
            else {
                return Ok(None);
            };
            if source.place != *destination || !source.path.is_empty() {
                return Ok(None);
            }
            super::structural_effect_leaf_observation(producer)?;
            super::u64_extent_proposition(*result, bytes.len() as u128)
        }

        pub fn subslice_length_equation(
            observation: &StructuralEffectObservation,
            producer: &StructuralEffectObservation,
        ) -> Result<Option<Proposition>, OperationSemanticError> {
            super::subslice_extent_equation(
                observation,
                producer,
                |observation| match observation {
                    StructuralEffectObservation::ByteSequenceLengthRead { source, result } => {
                        Some((source, *result))
                    }
                    _ => None,
                },
                |producer| match producer {
                    StructuralEffectObservation::ByteSequenceSubslice {
                        destination,
                        start,
                        end,
                        ..
                    } => Some((*destination, *start, *end)),
                    _ => None,
                },
            )
        }
    }
    pub(crate) mod byte_predicate {

        use super::StructuralEffectObservation;
        use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType, StructuralTypeId};
        use terminal_psi::{
            StructuralTypeDeclaration, StructuralTypeShape, control_flow::ByteSequenceOperand,
        };

        pub fn byte_predicate_equation(
            observation: &StructuralEffectObservation,
            root_type: StructuralTypeId,
            structural_types: &[StructuralTypeDeclaration],
        ) -> Option<Proposition> {
            let StructuralEffectObservation::ByteSequencePredicateEvaluated {
                operand,
                predicate,
                result,
            } = observation
            else {
                return None;
            };
            let (root, path) = match operand {
                ByteSequenceOperand::Whole { source, .. } => {
                    let (path, declaration) = crate::canonical_static_projection(
                        structural_types.iter(),
                        root_type,
                        &source.path,
                    )?;
                    if !matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
                        && !(matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
                            && crate::fixed_byte_extent(structural_types.iter(), declaration.id)
                                .is_some())
                    {
                        return None;
                    }
                    (source.place, path)
                }
                ByteSequenceOperand::Field { .. } => {
                    super::byte_sequence_field_path(operand, root_type, structural_types)?
                }
            };
            Some(Proposition::Equal(
                ScalarTerm::value(*result, ScalarType::Boolean),
                ScalarTerm::ByteSequencePredicate {
                    predicate: *predicate,
                    root,
                    path,
                },
            ))
        }
    }
    pub use byte_equal::byte_equal_equation;
    pub use byte_predicate::byte_predicate_equation;
    pub(crate) mod observation {
        use super::*;
        use terminal_psi::StructuralArgument;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum StructuralEffectObservation {
            NativeCodeLayoutEstablished {
                destination: semantic_vocabulary::PlaceId,
                machine: semantic_vocabulary::MachineId,
            },
            StorageAddressExposed {
                source: StructuralArgument,
                result: ValueId,
            },
            ReferenceEstablished {
                source: terminal_psi::StructuralArgument,
                destination: PlaceId,
            },
            ReferenceReleased {
                source: PlaceId,
                path: Vec<terminal_psi::StructuralPathSegment>,
            },
            ScalarArrayEstablished {
                destination: PlaceId,
                elements: Vec<ValueId>,
            },
            PrimitiveLocalEstablished {
                destination: PlaceId,
                value: ValueId,
                range_obligation: Option<semantic_vocabulary::ObligationId>,
            },
            PrimitiveRead {
                source: PlaceId,
                path: Vec<terminal_psi::StructuralPathSegment>,
                result: ValueId,
            },
            CaseMembershipObserved {
                source: PlaceId,
                path: Vec<terminal_psi::StructuralPathSegment>,
                case: StructuralCaseId,
                result: ValueId,
            },
            ByteSequenceFieldLengthRead {
                source: PlaceId,
                path: Vec<terminal_psi::StructuralPathSegment>,
                field: StructuralFieldId,
                result: ValueId,
            },
            ByteSequenceFieldByteStored {
                destination: PlaceId,
                path: Vec<terminal_psi::StructuralPathSegment>,
                field: StructuralFieldId,
                index: ValueId,
                value: ValueId,
                length: ValueId,
                obligation: ObligationId,
            },
            ByteSequenceFieldByteRead {
                source: PlaceId,
                path: Vec<terminal_psi::StructuralPathSegment>,
                field: StructuralFieldId,
                index: ValueId,
                length: ValueId,
                result: ValueId,
                obligation: ObligationId,
            },
            ByteSequenceStored {
                destination: PlaceId,
                source: PlaceId,
                length: ValueId,
                obligation: ObligationId,
            },
            ByteSequenceFieldStored {
                destination: PlaceId,
                path: Vec<terminal_psi::StructuralPathSegment>,
                field: StructuralFieldId,
                source: PlaceId,
                length: ValueId,
                obligation: ObligationId,
            },
            ByteViewFieldStored {
                destination: PlaceId,
                path: Vec<terminal_psi::StructuralPathSegment>,
                field: Option<StructuralFieldId>,
                source: PlaceId,
            },
            PrimitiveStored {
                destination: PlaceId,
                path: Vec<terminal_psi::StructuralPathSegment>,
                value: semantic_vocabulary::ValueId,
            },
            ScalarFieldStored {
                destination: PlaceId,
                path: Vec<terminal_psi::StructuralPathSegment>,
                field: StructuralFieldId,
                value: ValueId,
            },
            ByteSequencePlaceEstablished {
                destination: PlaceId,
            },
            ByteSequenceLengthRead {
                source: StructuralArgument,
                result: ValueId,
            },
            ByteSequencePredicateEvaluated {
                operand: terminal_psi::control_flow::ByteSequenceOperand,
                predicate: terminal_psi::ByteSequencePredicate,
                result: ValueId,
            },
            ByteSequenceEqualEvaluated {
                left: terminal_psi::control_flow::ByteSequenceOperand,
                right: terminal_psi::control_flow::ByteSequenceOperand,
                result: ValueId,
            },
            ByteSequenceRead {
                source: PlaceId,
                index: ValueId,
                length: ValueId,
                result: ValueId,
                obligation: ObligationId,
            },
            ByteSequenceWrite {
                destination: PlaceId,
                index: ValueId,
                value: ValueId,
                length: ValueId,
                obligation: ObligationId,
            },
            ByteSequenceSubslice {
                source: StructuralArgument,
                start: ValueId,
                end: ValueId,
                length: ValueId,
                destination: PlaceId,
                obligation: ObligationId,
            },
            ElementViewEstablished {
                source: terminal_psi::StructuralArgument,
                destination: PlaceId,
                region: terminal_psi::ElementViewRegion,
            },
            ElementViewLengthRead {
                source: terminal_psi::StructuralArgument,
                result: ValueId,
            },
            ElementViewRead {
                source: PlaceId,
                index: ValueId,
                length: ValueId,
                result: ValueId,
                obligation: ObligationId,
            },
            ElementViewWrite {
                destination: StructuralArgument,
                path: Vec<terminal_psi::StructuralPathSegment>,
                index: ValueId,
                value: ValueId,
                length: ValueId,
                obligation: ObligationId,
            },
            ElementViewSubslice {
                source: StructuralArgument,
                start: ValueId,
                end: ValueId,
                length: ValueId,
                destination: PlaceId,
                obligation: ObligationId,
            },
            BooleanFieldEquation(Proposition),
            ScalarFieldEquation(Proposition),
            PortWrite {
                service: ServiceId,
                port: u16,
                value: u8,
            },
            AffinePlaceEstablished {
                destination: PlaceId,
            },
            StructuralArrayEstablished {
                destination: PlaceId,
                elements: Vec<terminal_psi::StructuralArgument>,
            },
            RecordEstablished {
                destination: PlaceId,
                fields: Vec<terminal_psi::RecordFieldInitializer>,
            },
            StructuralCaseEstablished {
                membership: Proposition,
                destination: PlaceId,
                fields: Vec<terminal_psi::RecordFieldInitializer>,
            },
            ScalarCaseEstablished {
                membership: Proposition,
                fields: Vec<terminal_psi::ScalarCaseField>,
            },
        }

        impl StructuralEffectObservation {}

        pub fn structural_effect_leaf_observation_in(
            operation: &Operation,
            rows: &[StructuralEffectSemanticRow],
        ) -> Result<Option<StructuralEffectObservation>, OperationSemanticError> {
            let tag = OperationSemanticTag::for_operation(&operation.kind);
            if is_structural_effect_tag(tag) {
                validate_structural_effect_semantic_rows(rows)?;
            }
            let Some(row) = exact_structural_effect_semantic_row_in(tag, rows)? else {
                return Ok(None);
            };
            let schema = row.schema;
            validate_structural_effect_result(operation, tag, schema)?;
            let observation = match (schema.action, &operation.kind) {
                (
                    StructuralEffectAction::EstablishReference,
                    OperationKind::EstablishReference { source, .. },
                )
                | (
                    StructuralEffectAction::EstablishAccessReference,
                    OperationKind::EstablishAccessReference { source, .. },
                ) => {
                    let result = operation.result.structural().ok_or(
                        OperationSemanticError::StructuralEffectResultShapeMismatch(tag),
                    )?;
                    if !matches!(
                        source.access,
                        terminal_psi::StructuralAccess::MutableBorrow
                            | terminal_psi::StructuralAccess::SharedBorrow
                    ) || result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
                        || !result.qualifications.is_empty()
                        || result.projected_qualifications.iter().any(|row| {
                            row.path.first() != Some(&terminal_psi::StructuralPathSegment::Referent)
                        })
                        || !result.claims.is_empty()
                    {
                        return Err(OperationSemanticError::StructuralEffectActionShapeMismatch(
                            tag,
                        ));
                    }
                    StructuralEffectObservation::ReferenceEstablished {
                        source: source.clone(),
                        destination: result.place,
                    }
                }
                (
                    StructuralEffectAction::ReleaseReference,
                    OperationKind::ReleaseReference { source, path },
                ) => StructuralEffectObservation::ReferenceReleased {
                    source: *source,
                    path: path.clone(),
                },
                (
                    StructuralEffectAction::EstablishPrimitiveLocal,
                    OperationKind::EstablishPrimitiveLocal {
                        value,
                        range_obligation,
                    },
                ) => {
                    let result = operation.result.structural().ok_or(
                        OperationSemanticError::StructuralEffectResultShapeMismatch(tag),
                    )?;
                    StructuralEffectObservation::PrimitiveLocalEstablished {
                        destination: result.place,
                        value: *value,
                        range_obligation: *range_obligation,
                    }
                }
                (
                    StructuralEffectAction::ExposeStorageAddress,
                    OperationKind::StorageAddress { source },
                ) => {
                    let result = operation.result.scalar().filter(|result|
                        matches!(result.scalar_type, ScalarType::Integer(integer) if integer.is_address()))
                        .ok_or(OperationSemanticError::StructuralEffectResultShapeMismatch(tag))?;
                    if source.access != terminal_psi::StructuralAccess::SharedBorrow {
                        return Err(OperationSemanticError::StructuralEffectResultShapeMismatch(
                            tag,
                        ));
                    }
                    StructuralEffectObservation::StorageAddressExposed {
                        source: source.clone(),
                        result: result.id,
                    }
                }
                (
                    StructuralEffectAction::ReadPrimitive,
                    OperationKind::PrimitiveScalarRead { source, path },
                ) => {
                    let result = operation.result.scalar().ok_or(
                        OperationSemanticError::StructuralEffectResultShapeMismatch(tag),
                    )?;
                    StructuralEffectObservation::PrimitiveRead {
                        source: *source,
                        path: path.clone(),
                        result: result.id,
                    }
                }
                (
                    StructuralEffectAction::ObserveCaseMembership,
                    OperationKind::StructuralCaseMembership { source, path, case },
                ) => StructuralEffectObservation::CaseMembershipObserved {
                    source: *source,
                    path: path.clone(),
                    case: *case,
                    result: operation.result.expect_scalar().id,
                },
                (
                    StructuralEffectAction::ReadByteSequenceFieldLength,
                    OperationKind::StructuralByteSequenceFieldLength {
                        source,
                        path,
                        field,
                    },
                ) => StructuralEffectObservation::ByteSequenceFieldLengthRead {
                    source: *source,
                    path: path.clone(),
                    field: *field,
                    result: operation.result.expect_scalar().id,
                },
                (
                    StructuralEffectAction::StoreByteSequenceFieldByte,
                    OperationKind::StructuralByteSequenceFieldByteStore {
                        destination,
                        path,
                        field,
                        index,
                        value,
                        length,
                        obligation,
                    },
                ) => StructuralEffectObservation::ByteSequenceFieldByteStored {
                    destination: *destination,
                    path: path.clone(),
                    field: *field,
                    index: *index,
                    value: *value,
                    length: *length,
                    obligation: *obligation,
                },
                (
                    StructuralEffectAction::ReadByteSequenceFieldByte,
                    OperationKind::StructuralByteSequenceFieldRead {
                        source,
                        path,
                        field,
                        index,
                        length,
                        obligation,
                    },
                ) => StructuralEffectObservation::ByteSequenceFieldByteRead {
                    source: *source,
                    path: path.clone(),
                    field: *field,
                    index: *index,
                    length: *length,
                    result: operation.result.expect_scalar().id,
                    obligation: *obligation,
                },
                (
                    StructuralEffectAction::EstablishByteSequenceSubslice,
                    OperationKind::ByteSequenceSubslice {
                        source,
                        start,
                        end,
                        length,
                        obligation,
                    },
                ) => StructuralEffectObservation::ByteSequenceSubslice {
                    source: source.clone(),
                    start: *start,
                    end: *end,
                    length: *length,
                    destination: operation
                        .result
                        .structural()
                        .expect("validated structural result")
                        .place,
                    obligation: *obligation,
                },
                (
                    StructuralEffectAction::EstablishElementView,
                    OperationKind::EstablishElementView {
                        destination,
                        source,
                        region,
                        ..
                    },
                )
                | (
                    StructuralEffectAction::EstablishMutableElementView,
                    OperationKind::EstablishMutableElementView {
                        destination,
                        source,
                        region,
                        ..
                    },
                ) => {
                    operation.result.structural().ok_or(
                        OperationSemanticError::StructuralEffectResultShapeMismatch(tag),
                    )?;
                    StructuralEffectObservation::ElementViewEstablished {
                        source: source.clone(),
                        destination: *destination,
                        region: *region,
                    }
                }
                (
                    StructuralEffectAction::ReadElementViewLength,
                    OperationKind::ElementViewLength { source },
                ) => StructuralEffectObservation::ElementViewLengthRead {
                    source: source.clone(),
                    result: operation.result.expect_scalar().id,
                },
                (
                    StructuralEffectAction::ReadElementView,
                    OperationKind::ElementViewRead {
                        source,
                        index,
                        length,
                        obligation,
                        ..
                    },
                ) => StructuralEffectObservation::ElementViewRead {
                    source: *source,
                    index: *index,
                    length: *length,
                    result: operation.result.expect_scalar().id,
                    obligation: *obligation,
                },
                (
                    StructuralEffectAction::EstablishElementViewSubslice,
                    OperationKind::ElementViewSubslice {
                        source,
                        start,
                        end,
                        length,
                        obligation,
                    },
                ) => StructuralEffectObservation::ElementViewSubslice {
                    source: source.clone(),
                    start: *start,
                    end: *end,
                    length: *length,
                    destination: operation
                        .result
                        .structural()
                        .expect("validated structural result")
                        .place,
                    obligation: *obligation,
                },
                (
                    StructuralEffectAction::ReadByteSequence,
                    OperationKind::ByteSequenceRead {
                        source,
                        index,
                        length,
                        obligation,
                    },
                ) => StructuralEffectObservation::ByteSequenceRead {
                    source: *source,
                    index: *index,
                    length: *length,
                    result: operation.result.expect_scalar().id,
                    obligation: *obligation,
                },
                (
                    StructuralEffectAction::WriteByteSequence,
                    OperationKind::ByteSequenceWrite {
                        destination,
                        index,
                        value,
                        length,
                        obligation,
                    },
                ) => StructuralEffectObservation::ByteSequenceWrite {
                    destination: *destination,
                    index: *index,
                    value: *value,
                    length: *length,
                    obligation: *obligation,
                },
                (
                    StructuralEffectAction::ReadByteSequenceLength,
                    OperationKind::ByteSequenceLength { source },
                ) => StructuralEffectObservation::ByteSequenceLengthRead {
                    source: source.clone(),
                    result: operation.result.expect_scalar().id,
                },
                (
                    StructuralEffectAction::EvaluateByteSequencePredicate,
                    OperationKind::ByteSequencePredicate { operand, predicate },
                ) => StructuralEffectObservation::ByteSequencePredicateEvaluated {
                    operand: operand.clone(),
                    predicate: *predicate,
                    result: operation.result.expect_scalar().id,
                },
                (
                    StructuralEffectAction::EvaluateByteSequenceEqual,
                    OperationKind::ByteSequenceEqual { left, right },
                ) => StructuralEffectObservation::ByteSequenceEqualEvaluated {
                    left: left.clone(),
                    right: right.clone(),
                    result: operation.result.expect_scalar().id,
                },
                (
                    StructuralEffectAction::WriteElementView,
                    OperationKind::ElementViewWrite {
                        destination,
                        path,
                        index,
                        value,
                        length,
                        obligation,
                    },
                ) => StructuralEffectObservation::ElementViewWrite {
                    destination: destination.clone(),
                    path: path.clone(),
                    index: *index,
                    value: *value,
                    length: *length,
                    obligation: *obligation,
                },
                (
                    StructuralEffectAction::StorePrimitive,
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination,
                        value,
                        path,
                        ..
                    },
                ) => StructuralEffectObservation::PrimitiveStored {
                    destination: *destination,
                    path: path.clone(),
                    value: *value,
                },
                (
                    StructuralEffectAction::StoreByteSequence,
                    OperationKind::ByteSequenceStore {
                        destination,
                        source,
                        length,
                        obligation,
                    },
                ) => StructuralEffectObservation::ByteSequenceStored {
                    destination: *destination,
                    source: *source,
                    length: *length,
                    obligation: *obligation,
                },
                (
                    StructuralEffectAction::StoreByteSequenceField,
                    OperationKind::StructuralByteSequenceFieldStore {
                        destination,
                        path,
                        field,
                        source,
                        length,
                        obligation,
                    },
                ) => StructuralEffectObservation::ByteSequenceFieldStored {
                    destination: *destination,
                    path: path.clone(),
                    field: *field,
                    source: *source,
                    length: *length,
                    obligation: *obligation,
                },
                (
                    StructuralEffectAction::StoreByteViewField,
                    OperationKind::StructuralByteViewFieldStore {
                        destination,
                        path,
                        field,
                        source,
                        ..
                    },
                ) => StructuralEffectObservation::ByteViewFieldStored {
                    destination: *destination,
                    path: path.clone(),
                    field: *field,
                    source: *source,
                },
                (
                    StructuralEffectAction::StoreScalarField,
                    OperationKind::StructuralScalarFieldStore {
                        destination,
                        path,
                        field,
                        value,
                        ..
                    },
                ) => StructuralEffectObservation::ScalarFieldStored {
                    destination: *destination,
                    path: path.clone(),
                    field: *field,
                    value: *value,
                },
                (
                    StructuralEffectAction::EstablishScalarArray,
                    OperationKind::EstablishScalarArray { elements },
                ) => {
                    let result = operation
                        .result
                        .structural()
                        .ok_or(OperationSemanticError::StructuralEffectSchemaMismatch(tag))?;
                    StructuralEffectObservation::ScalarArrayEstablished {
                        destination: result.place,
                        elements: elements.clone(),
                    }
                }
                (
                    StructuralEffectAction::EstablishScalarCase,
                    OperationKind::EstablishScalarCase {
                        result_case,
                        fields,
                    },
                ) => {
                    let result = operation
                        .result
                        .structural()
                        .expect("validated scalar-case structural result");
                    StructuralEffectObservation::ScalarCaseEstablished {
                        membership: Proposition::StructuralCaseMembership {
                            subject: StructuralCaseSubject::new(result.place, Vec::new()),
                            case: *result_case,
                        },
                        fields: fields.clone(),
                    }
                }
                (
                    StructuralEffectAction::EstablishStructuralCase,
                    OperationKind::EstablishStructuralCase {
                        result_case,
                        fields,
                    },
                ) => {
                    let result = operation
                        .result
                        .structural()
                        .expect("validated structural-case structural result");
                    StructuralEffectObservation::StructuralCaseEstablished {
                        membership: Proposition::StructuralCaseMembership {
                            subject: StructuralCaseSubject::new(result.place, Vec::new()),
                            case: *result_case,
                        },
                        destination: result.place,
                        fields: fields.clone(),
                    }
                }
                (
                    StructuralEffectAction::ReadScalarField,
                    OperationKind::ScalarStructuralField {
                        source,
                        path,
                        field,
                    },
                ) => {
                    let result = operation
                        .result
                        .scalar_ref()
                        .expect("validated scalar structural-field result");
                    let field_path = path
                        .iter()
                        .copied()
                        .chain(std::iter::once(
                            semantic_vocabulary::CanonicalStructuralPathSegment::Field(*field),
                        ))
                        .collect();
                    let field_term = match result.scalar_type {
                        ScalarType::Integer(integer) => {
                            ScalarTerm::integer_field_path(*source, field_path, integer)
                        }
                        ScalarType::IeeeFloat(format) => {
                            ScalarTerm::ieee_float_field_path(*source, field_path, format)
                        }
                        ScalarType::Boolean => ScalarTerm::boolean_field_path(*source, field_path),
                    };
                    StructuralEffectObservation::ScalarFieldEquation(Proposition::Equal(
                        ScalarTerm::value(result.id, result.scalar_type),
                        field_term,
                    ))
                }
                (
                    StructuralEffectAction::EstablishByteSequencePlace,
                    OperationKind::EstablishByteSequenceLiteral { destination, .. },
                ) => StructuralEffectObservation::ByteSequencePlaceEstablished {
                    destination: *destination,
                },
                (
                    StructuralEffectAction::ReadBooleanField,
                    OperationKind::BooleanStructuralField {
                        source,
                        path,
                        field,
                    },
                ) => {
                    let result = operation
                        .result
                        .scalar_ref()
                        .expect("validated Boolean structural-field result");
                    StructuralEffectObservation::BooleanFieldEquation(Proposition::Equal(
                        ScalarTerm::value(result.id, result.scalar_type),
                        ScalarTerm::boolean_field_path(
                            *source,
                            path.iter()
                                .copied()
                                .chain(std::iter::once(
                                    semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                                        *field,
                                    ),
                                ))
                                .collect(),
                        ),
                    ))
                }
                (
                    StructuralEffectAction::EmitPortWrite,
                    OperationKind::PortWrite {
                        service,
                        port,
                        value,
                    },
                ) => StructuralEffectObservation::PortWrite {
                    service: *service,
                    port: *port,
                    value: *value,
                },
                (
                    StructuralEffectAction::EstablishAffinePlace,
                    OperationKind::EstablishTrivialAffineLocal { destination },
                ) => StructuralEffectObservation::AffinePlaceEstablished {
                    destination: *destination,
                },
                (
                    StructuralEffectAction::EstablishStructuralArray,
                    OperationKind::EstablishStructuralArray { elements },
                ) => {
                    let result = operation
                        .result
                        .structural()
                        .expect("validated complete array result");
                    StructuralEffectObservation::StructuralArrayEstablished {
                        destination: result.place,
                        elements: elements.clone(),
                    }
                }
                (
                    StructuralEffectAction::EstablishNativeCodeLayout,
                    OperationKind::EstablishNativeCodeLayout { machine },
                ) => {
                    let result = operation.result.structural().ok_or(
                        OperationSemanticError::StructuralEffectActionShapeMismatch(tag),
                    )?;
                    StructuralEffectObservation::NativeCodeLayoutEstablished {
                        destination: result.place,
                        machine: *machine,
                    }
                }
                (
                    StructuralEffectAction::EstablishRecord,
                    OperationKind::EstablishRecord { fields },
                ) => {
                    let result = operation
                        .result
                        .structural()
                        .expect("validated scalar-record structural result");
                    StructuralEffectObservation::RecordEstablished {
                        destination: result.place,
                        fields: fields.clone(),
                    }
                }
                _ => {
                    return Err(OperationSemanticError::StructuralEffectActionShapeMismatch(
                        tag,
                    ));
                }
            };
            Ok(Some(observation))
        }

        pub fn structural_effect_leaf_observation(
            operation: &Operation,
        ) -> Result<Option<StructuralEffectObservation>, OperationSemanticError> {
            structural_effect_leaf_observation_in(operation, &StructuralEffectSemanticRow::ALL)
        }

        pub fn same_descriptor_coordinate(
            left: &terminal_psi::StructuralArgument,
            right: &terminal_psi::StructuralArgument,
        ) -> bool {
            left.place == right.place
                && left.access == right.access
                && left.path.len() == right.path.len()
                && left
                    .path
                    .iter()
                    .zip(&right.path)
                    .all(|(left, right)| match (left, right) {
                        (
                            terminal_psi::StructuralPathSegment::RuntimeIndex {
                                index: left, ..
                            },
                            terminal_psi::StructuralPathSegment::RuntimeIndex {
                                index: right, ..
                            },
                        ) => left == right,
                        _ => left == right,
                    })
        }
    }
    pub use observation::{
        StructuralEffectObservation, same_descriptor_coordinate,
        structural_effect_leaf_observation, structural_effect_leaf_observation_in,
    };
    pub(crate) mod element_extent {

        use semantic_vocabulary::{
            IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType, StructuralTypeId,
        };
        use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

        use super::StructuralEffectObservation;
        use crate::OperationSemanticError;
        use terminal_psi::StructuralFieldType;

        pub fn element_subslice_length_equation(
            observation: &StructuralEffectObservation,
            producer: &StructuralEffectObservation,
        ) -> Result<Option<Proposition>, OperationSemanticError> {
            super::subslice_extent_equation(
                observation,
                producer,
                |observation| match observation {
                    StructuralEffectObservation::ElementViewLengthRead { source, result } => {
                        Some((source, *result))
                    }
                    _ => None,
                },
                |producer| match producer {
                    StructuralEffectObservation::ElementViewSubslice {
                        destination,
                        start,
                        end,
                        ..
                    } => Some((*destination, *start, *end)),
                    _ => None,
                },
            )
        }

        pub fn element_establishment_length_equation(
            observation: &StructuralEffectObservation,
            producer: &StructuralEffectObservation,
            collection_length: u64,
        ) -> Result<Option<Proposition>, OperationSemanticError> {
            let (source, result) = match observation {
                StructuralEffectObservation::ElementViewLengthRead { source, result }
                    if source.path.is_empty() =>
                {
                    (source.place, *result)
                }
                StructuralEffectObservation::ByteSequenceLengthRead { source, result }
                    if source.path.is_empty() =>
                {
                    (source.place, *result)
                }
                _ => return Ok(None),
            };
            let StructuralEffectObservation::ElementViewEstablished {
                destination,
                region: terminal_psi::ElementViewRegion::WholeCollection,
                ..
            } = producer
            else {
                return Ok(None);
            };
            if source != *destination {
                return Ok(None);
            }
            super::u64_extent_proposition(result, u128::from(collection_length))
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ElementViewLeaf {
            pub scalar_type: ScalarType,
            pub restriction: Option<StructuralFieldType>,
        }
        pub fn element_view_leaf(
            types: &[StructuralTypeDeclaration],
            element: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Option<ElementViewLeaf> {
            if path.iter().any(|segment| {
                !matches!(
                    segment,
                    StructuralPathSegment::Field(_)
                        | StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::RuntimeIndex { .. }
                )
            }) {
                return None;
            }
            let plain = |scalar_type| ElementViewLeaf {
                scalar_type,
                restriction: None,
            };
            let primitive = |identity: StructuralTypeId| {
                types
                    .iter()
                    .find(|row| row.id == identity)
                    .and_then(|row| match row.shape {
                        StructuralTypeShape::PrimitiveScalar(scalar) => match &row
                            .scalar_restriction
                        {
                            None => Some(plain(scalar)),
                            Some(StructuralFieldType::BoundedInteger(bounds)) => {
                                Some(ElementViewLeaf {
                                    scalar_type: scalar,
                                    restriction: Some(StructuralFieldType::BoundedInteger(*bounds)),
                                })
                            }
                            Some(restriction @ StructuralFieldType::RefinedScalar(_))
                                if restriction.scalar_type() == Some(scalar) =>
                            {
                                Some(ElementViewLeaf {
                                    scalar_type: scalar,
                                    restriction: Some(restriction.clone()),
                                })
                            }
                            _ => None,
                        },
                        _ => None,
                    })
            };
            let Some((last, prefix)) = path.split_last() else {
                return primitive(element);
            };
            let parent = crate::runtime_structural_path_tip(types.iter(), element, prefix)?;
            match (last, &parent.shape) {
                (
                    StructuralPathSegment::Field(identity),
                    StructuralTypeShape::Record { fields, .. },
                ) => {
                    let field = terminal_psi::live_field_by_identity(fields, *identity)?;
                    match field.field_type {
                        StructuralFieldType::Structural(child) => primitive(child),
                        StructuralFieldType::BoundedInteger(bounds) => Some(ElementViewLeaf {
                            scalar_type: ScalarType::Integer(bounds.integer_type()),
                            restriction: Some(StructuralFieldType::BoundedInteger(bounds)),
                        }),
                        StructuralFieldType::RefinedScalar(ref restriction) => {
                            Some(ElementViewLeaf {
                                scalar_type: restriction.carrier(),
                                restriction: Some(StructuralFieldType::RefinedScalar(
                                    restriction.clone(),
                                )),
                            })
                        }
                        StructuralFieldType::Scalar(scalar) => Some(plain(scalar)),
                        StructuralFieldType::IeeeFloat(format) => {
                            Some(plain(ScalarType::IeeeFloat(format)))
                        }
                        _ => None,
                    }
                }
                (
                    StructuralPathSegment::FixedIndex(index),
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    },
                ) if index < length => primitive(*element),
                (
                    StructuralPathSegment::RuntimeIndex { .. },
                    StructuralTypeShape::FixedArray { element, .. },
                ) => primitive(*element),
                _ => None,
            }
        }
        pub fn element_view_leaf_scalar(
            types: &[StructuralTypeDeclaration],
            element: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Option<ScalarType> {
            element_view_leaf(types, element, path).map(|leaf| leaf.scalar_type)
        }
        pub fn element_view_write_goal(
            types: &[StructuralTypeDeclaration],
            element: StructuralTypeId,
            path: &[StructuralPathSegment],
            index: semantic_vocabulary::ValueId,
            length: semantic_vocabulary::ValueId,
            value: semantic_vocabulary::ValueId,
        ) -> Option<Proposition> {
            if path.iter().any(|segment| segment.runtime_index().is_some()) {
                return None;
            }
            let leaf = element_view_leaf(types, element, path)?;
            let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).ok()?);
            let index = Proposition::LessThan(
                ScalarTerm::value(index, count),
                ScalarTerm::value(length, count),
            );
            match leaf.restriction {
                None => Some(index),
                Some(restriction) => Some(Proposition::Conjunction(vec![
                    index,
                    crate::scalar_field_requirement(&restriction, value)?,
                ])),
            }
        }
    }
    pub use byte_extent::{literal_length_equation, subslice_length_equation};
    pub use element_extent::{
        ElementViewLeaf, element_establishment_length_equation, element_subslice_length_equation,
        element_view_leaf, element_view_leaf_scalar, element_view_write_goal,
    };

    use crate::OperationSemanticError;
    use crate::semantic_rows::OperationSemanticTag;
    use semantic_vocabulary::CanonicalStructuralPathSegment;

    pub(crate) fn byte_sequence_field_path(
        operand: &terminal_psi::control_flow::ByteSequenceOperand,
        root_type: semantic_vocabulary::StructuralTypeId,
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<(PlaceId, Vec<CanonicalStructuralPathSegment>)> {
        let terminal_psi::control_flow::ByteSequenceOperand::Field { source, field, .. } = operand
        else {
            return None;
        };
        let (mut path, parent) =
            crate::canonical_static_projection(structural_types.iter(), root_type, &source.path)?;
        let fields = match &parent.shape {
            terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
            _ => return None,
        };
        let selected = live_field_by_id(fields, *field)?;
        selected.field_type.byte_sequence_carrier()?;
        path.push(CanonicalStructuralPathSegment::Field(*field));
        Some((source.place, path))
    }

    pub(crate) fn u64_extent_proposition(
        result: ValueId,
        count: u128,
    ) -> Result<Option<Proposition>, OperationSemanticError> {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
        let count = ScalarTerm::integer(
            integer_type,
            semantic_vocabulary::IntegerValue::Unsigned(count),
        )
        .map_err(OperationSemanticError::InvalidProposition)?;
        Ok(Some(Proposition::Equal(
            ScalarTerm::value(result, ScalarType::Integer(integer_type)),
            count,
        )))
    }

    pub(crate) fn u64_extent_difference_proposition(
        result: ValueId,
        end: ValueId,
        start: ValueId,
    ) -> Result<Option<Proposition>, OperationSemanticError> {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
        let value = |identity| ScalarTerm::value(identity, ScalarType::Integer(integer_type));
        let difference = ScalarTerm::exact_integer_subtract(integer_type, value(end), value(start))
            .map_err(OperationSemanticError::InvalidProposition)?;
        Ok(Some(Proposition::Equal(value(result), difference)))
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StructuralEffectResultShape {
        Scalar,
        Boolean,
        Integer,
        ByteCount,
        ElementCount,
        Byte,
        Structural,
        Unit,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StructuralEffectCustody {
        ExactReferenceSource,
        ExactReferenceCarrier,
        ExactPrimitiveLocal,
        ExactReadablePrimitiveRoot,
        ExactAddressableReferent,
        ExactReadableSumProjection,
        ExactWriteOnlyPrimitiveRoot,
        ExactStructuralScalarField,
        ExactStructuralByteSequenceField,
        ExactByteSequenceLiteral,
        ExactImmutableByteView,
        ExactMutableByteView,
        ExactBoundedByteOwner,
        ExactByteViewMetadata,
        ExactElementViewSource,
        ExactImmutableElementView,
        ExactMutableElementView,
        ExactElementViewMetadata,
        ExactLiveBooleanField,
        ExactLiveScalarField,
        ExactPublishedService,
        ExactEmptyAffineLocal,
        ExactStructuralArray,
        ExactRecord,
        ExactScalarCase,
        ExactStructuralCase,
        ExactScalarArray,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StructuralEffectAction {
        EstablishNativeCodeLayout,
        EstablishAccessReference,
        EstablishReference,
        ReleaseReference,
        EstablishPrimitiveLocal,
        ReadPrimitive,
        ExposeStorageAddress,
        ObserveCaseMembership,
        StorePrimitive,
        StoreScalarField,
        StoreByteSequenceField,
        StoreByteViewField,
        StoreByteSequence,
        ReadByteSequenceFieldLength,
        StoreByteSequenceFieldByte,
        ReadByteSequenceFieldByte,
        EstablishByteSequencePlace,
        ReadByteSequenceLength,
        ReadByteSequence,
        WriteByteSequence,
        EstablishByteSequenceSubslice,
        EvaluateByteSequencePredicate,
        EvaluateByteSequenceEqual,
        EstablishElementView,
        EstablishMutableElementView,
        ReadElementViewLength,
        ReadElementView,
        WriteElementView,
        EstablishElementViewSubslice,
        ReadBooleanField,
        ReadScalarField,
        EmitPortWrite,
        EstablishAffinePlace,
        EstablishStructuralArray,
        EstablishRecord,
        EstablishScalarCase,
        EstablishStructuralCase,
        EstablishScalarArray,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StructuralEffectExternalEffect {
        None,
        PortWrite,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StructuralEffectWorkPolicy {
        ConsumeOne,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StructuralEffectGoalShape {
        None,
        ByteIndexInBounds,
        ByteRangeInBounds,
        ByteLengthWithinFieldCapacity,
        ByteLengthWithinOwnerCapacity,
        ElementIndexInBounds,
        ElementRangeInBounds,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum StructuralEffectFrontierPolicy {
        CapturesLoanAndAddsReferenceCarrier,
        ReleasesReferenceCarrier,
        TransfersOwnedChildrenAndAddsOwnedPlace,
        RequiresAndKeepsWriteOnlyPrimitivePlace,
        RequiresAndKeepsStructuralPlace,
        RequiresViewAndEstablishesBorrowedView,
        AddsUnrestrictedPlace,
        RequiresAndKeepsAffinePlace,
        KeepsPlaceFrontier,
        AddsAffinePlace,
        AddsOwnedPlace,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct StructuralEffectLeafSchema {
        result: StructuralEffectResultShape,
        custody: StructuralEffectCustody,
        action: StructuralEffectAction,
        external_effect: StructuralEffectExternalEffect,
        work: StructuralEffectWorkPolicy,
        goal: StructuralEffectGoalShape,
        frontier: StructuralEffectFrontierPolicy,
    }

    impl StructuralEffectLeafSchema {
        pub const fn result(self) -> StructuralEffectResultShape {
            self.result
        }

        pub const fn custody(self) -> StructuralEffectCustody {
            self.custody
        }

        pub const fn action(self) -> StructuralEffectAction {
            self.action
        }

        pub const fn external_effect(self) -> StructuralEffectExternalEffect {
            self.external_effect
        }

        pub const fn work(self) -> StructuralEffectWorkPolicy {
            self.work
        }

        pub const fn goal(self) -> StructuralEffectGoalShape {
            self.goal
        }

        pub const fn frontier(self) -> StructuralEffectFrontierPolicy {
            self.frontier
        }
    }

    const fn structural_effect_leaf(
        result: StructuralEffectResultShape,
        custody: StructuralEffectCustody,
        action: StructuralEffectAction,
        external_effect: StructuralEffectExternalEffect,
        frontier: StructuralEffectFrontierPolicy,
    ) -> StructuralEffectLeafSchema {
        StructuralEffectLeafSchema {
            result,
            custody,
            action,
            external_effect,
            work: StructuralEffectWorkPolicy::ConsumeOne,
            goal: match action {
                StructuralEffectAction::StoreByteSequence => {
                    StructuralEffectGoalShape::ByteLengthWithinOwnerCapacity
                }
                StructuralEffectAction::StoreByteSequenceField => {
                    StructuralEffectGoalShape::ByteLengthWithinFieldCapacity
                }
                StructuralEffectAction::ReadByteSequence
                | StructuralEffectAction::WriteByteSequence
                | StructuralEffectAction::StoreByteSequenceFieldByte
                | StructuralEffectAction::ReadByteSequenceFieldByte => {
                    StructuralEffectGoalShape::ByteIndexInBounds
                }
                StructuralEffectAction::EstablishByteSequenceSubslice => {
                    StructuralEffectGoalShape::ByteRangeInBounds
                }
                StructuralEffectAction::ReadElementView
                | StructuralEffectAction::WriteElementView => {
                    StructuralEffectGoalShape::ElementIndexInBounds
                }
                StructuralEffectAction::EstablishElementViewSubslice => {
                    StructuralEffectGoalShape::ElementRangeInBounds
                }
                _ => StructuralEffectGoalShape::None,
            },
            frontier,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StructuralEffectSemanticRow {
        tag: OperationSemanticTag,
        schema: StructuralEffectLeafSchema,
    }

    impl StructuralEffectSemanticRow {
        pub const fn tag(self) -> OperationSemanticTag {
            self.tag
        }

        pub const fn schema(self) -> StructuralEffectLeafSchema {
            self.schema
        }
    }

    macro_rules! structural_effect_rows {
        ($( $tag:ident => ($result:ident, $custody:ident, $action:ident, $external:ident, $frontier:ident) ),+ $(,)?) => {
            impl StructuralEffectSemanticRow {
                pub const ALL: [Self; structural_effect_rows!(@count $( $tag )+)] = [
                    $( Self {
                        tag: OperationSemanticTag::$tag,
                        schema: structural_effect_leaf(
                            StructuralEffectResultShape::$result,
                            StructuralEffectCustody::$custody,
                            StructuralEffectAction::$action,
                            StructuralEffectExternalEffect::$external,
                            StructuralEffectFrontierPolicy::$frontier,
                        ),
                    } ),+
                ];
            }

            crate::semantic_rows::operation_semantic_cohort! {
                rows: StructuralEffectSemanticRow,
                tags: STRUCTURAL_EFFECT_TAGS,
                is_tag: is_structural_effect_tag,
                exact_row: exact_structural_effect_semantic_row_in,
                reject_unexpected: reject_unexpected_structural_effect_rows,
                missing: MissingStructuralEffectRow,
                duplicate: DuplicateStructuralEffectRow,
                unexpected: UnexpectedStructuralEffectRow,
                $( $tag ),+
            }
        };
        (@count $( $variant:ident )+) => {
            <[()]>::len(&[$(structural_effect_rows!(@unit $variant)),+])
        };
        (@unit $variant:ident) => { () };
    }

    structural_effect_rows! {
        StorageAddress => (Scalar, ExactAddressableReferent, ExposeStorageAddress, None, RequiresAndKeepsStructuralPlace),
        EstablishAccessReference => (Structural, ExactReferenceSource, EstablishAccessReference, None, CapturesLoanAndAddsReferenceCarrier),
        EstablishReference => (Structural, ExactReferenceSource, EstablishReference, None, CapturesLoanAndAddsReferenceCarrier),
        ReleaseReference => (Unit, ExactReferenceCarrier, ReleaseReference, None, ReleasesReferenceCarrier),
        EstablishScalarArray => (Structural, ExactScalarArray, EstablishScalarArray, None, AddsUnrestrictedPlace),
        EstablishPrimitiveLocal => (Structural, ExactPrimitiveLocal, EstablishPrimitiveLocal, None, AddsUnrestrictedPlace),
        PrimitiveScalarRead => (Scalar, ExactReadablePrimitiveRoot, ReadPrimitive, None, RequiresAndKeepsStructuralPlace),
        StructuralCaseMembership => (Boolean, ExactReadableSumProjection, ObserveCaseMembership, None, RequiresAndKeepsStructuralPlace),
        StructuralByteSequenceFieldLength => (ByteCount, ExactStructuralByteSequenceField, ReadByteSequenceFieldLength, None, RequiresAndKeepsStructuralPlace),
        StructuralByteSequenceFieldByteStore => (Unit, ExactStructuralByteSequenceField, StoreByteSequenceFieldByte, None, RequiresAndKeepsStructuralPlace),
        ByteSequenceStore => (Unit, ExactBoundedByteOwner, StoreByteSequence, None, RequiresAndKeepsStructuralPlace),
        StructuralByteSequenceFieldStore => (Unit, ExactStructuralByteSequenceField, StoreByteSequenceField, None, RequiresAndKeepsStructuralPlace),
        StructuralByteViewFieldStore => (Unit, ExactStructuralByteSequenceField, StoreByteViewField, None, RequiresAndKeepsStructuralPlace),
        StructuralByteSequenceFieldRead => (Byte, ExactStructuralByteSequenceField, ReadByteSequenceFieldByte, None, RequiresAndKeepsStructuralPlace),
        ByteSequenceSubslice => (Structural, ExactImmutableByteView, EstablishByteSequenceSubslice, None, RequiresViewAndEstablishesBorrowedView),
        ByteSequencePredicate => (Boolean, ExactImmutableByteView, EvaluateByteSequencePredicate, None, RequiresAndKeepsStructuralPlace),
        ByteSequenceEqual => (Boolean, ExactImmutableByteView, EvaluateByteSequenceEqual, None, RequiresAndKeepsStructuralPlace),
        EstablishElementView => (Structural, ExactElementViewSource, EstablishElementView, None, RequiresViewAndEstablishesBorrowedView),
        EstablishMutableElementView => (Structural, ExactElementViewSource, EstablishMutableElementView, None, RequiresViewAndEstablishesBorrowedView),
        ElementViewLength => (ElementCount, ExactElementViewMetadata, ReadElementViewLength, None, RequiresAndKeepsStructuralPlace),
        ElementViewRead => (Scalar, ExactImmutableElementView, ReadElementView, None, RequiresAndKeepsStructuralPlace),
        ElementViewWrite => (Unit, ExactMutableElementView, WriteElementView, None, RequiresAndKeepsStructuralPlace),
        ElementViewSubslice => (Structural, ExactImmutableElementView, EstablishElementViewSubslice, None, RequiresViewAndEstablishesBorrowedView),
        ByteSequenceRead => (Byte, ExactImmutableByteView, ReadByteSequence, None, RequiresAndKeepsStructuralPlace),
        ByteSequenceWrite => (Unit, ExactMutableByteView, WriteByteSequence, None, RequiresAndKeepsStructuralPlace),
        ByteSequenceLength => (ByteCount, ExactByteViewMetadata, ReadByteSequenceLength, None, RequiresAndKeepsStructuralPlace),
        WriteOnlyPrimitiveStore => (Unit, ExactWriteOnlyPrimitiveRoot, StorePrimitive, None, RequiresAndKeepsWriteOnlyPrimitivePlace),
        StructuralScalarFieldStore => (Unit, ExactStructuralScalarField, StoreScalarField, None, RequiresAndKeepsStructuralPlace),
        EstablishScalarCase => (Structural, ExactScalarCase, EstablishScalarCase, None, AddsOwnedPlace),
        EstablishStructuralCase => (Structural, ExactStructuralCase, EstablishStructuralCase, None, TransfersOwnedChildrenAndAddsOwnedPlace),
        EstablishByteSequenceLiteral => (Unit, ExactByteSequenceLiteral, EstablishByteSequencePlace, None, AddsUnrestrictedPlace),
        BooleanStructuralField => (Boolean, ExactLiveBooleanField, ReadBooleanField, None, RequiresAndKeepsAffinePlace),
        ScalarStructuralField => (Scalar, ExactLiveScalarField, ReadScalarField, None, RequiresAndKeepsStructuralPlace),
        PortWrite => (Unit, ExactPublishedService, EmitPortWrite, PortWrite, KeepsPlaceFrontier),
        EstablishTrivialAffineLocal => (Unit, ExactEmptyAffineLocal, EstablishAffinePlace, None, AddsAffinePlace),
        EstablishStructuralArray => (Structural, ExactStructuralArray, EstablishStructuralArray, None, TransfersOwnedChildrenAndAddsOwnedPlace),
        EstablishRecord => (Structural, ExactRecord, EstablishRecord, None, TransfersOwnedChildrenAndAddsOwnedPlace),
        EstablishNativeCodeLayout => (Structural, ExactRecord, EstablishNativeCodeLayout, None, AddsOwnedPlace),
    }

    pub fn structural_effect_semantic_row(
        operation: &OperationKind,
    ) -> Result<Option<&'static StructuralEffectSemanticRow>, OperationSemanticError> {
        validate_structural_effect_semantic_rows(&StructuralEffectSemanticRow::ALL)?;
        exact_structural_effect_semantic_row_in(
            OperationSemanticTag::for_operation(operation),
            &StructuralEffectSemanticRow::ALL,
        )
    }

    pub fn validate_structural_effect_semantic_rows(
        rows: &[StructuralEffectSemanticRow],
    ) -> Result<(), OperationSemanticError> {
        reject_unexpected_structural_effect_rows(rows)?;
        for tag in STRUCTURAL_EFFECT_TAGS {
            let row = exact_structural_effect_semantic_row_in(tag, rows)?
                .expect("the requested tag belongs to the structural/effect cohort");
            validate_structural_effect_schema(tag, row.schema)?;
        }
        Ok(())
    }

    fn validate_structural_effect_schema(
        tag: OperationSemanticTag,
        schema: StructuralEffectLeafSchema,
    ) -> Result<(), OperationSemanticError> {
        let action_tag = match schema.action {
            StructuralEffectAction::EstablishNativeCodeLayout => {
                OperationSemanticTag::EstablishNativeCodeLayout
            }
            StructuralEffectAction::EstablishAccessReference => {
                OperationSemanticTag::EstablishAccessReference
            }
            StructuralEffectAction::EstablishReference => OperationSemanticTag::EstablishReference,
            StructuralEffectAction::ReleaseReference => OperationSemanticTag::ReleaseReference,
            StructuralEffectAction::EstablishPrimitiveLocal => {
                OperationSemanticTag::EstablishPrimitiveLocal
            }
            StructuralEffectAction::ReadPrimitive => OperationSemanticTag::PrimitiveScalarRead,
            StructuralEffectAction::ExposeStorageAddress => OperationSemanticTag::StorageAddress,
            StructuralEffectAction::ObserveCaseMembership => {
                OperationSemanticTag::StructuralCaseMembership
            }
            StructuralEffectAction::ReadByteSequenceFieldLength => {
                OperationSemanticTag::StructuralByteSequenceFieldLength
            }
            StructuralEffectAction::StoreByteSequenceFieldByte => {
                OperationSemanticTag::StructuralByteSequenceFieldByteStore
            }
            StructuralEffectAction::ReadByteSequenceFieldByte => {
                OperationSemanticTag::StructuralByteSequenceFieldRead
            }
            StructuralEffectAction::StoreByteSequence => OperationSemanticTag::ByteSequenceStore,
            StructuralEffectAction::StoreByteSequenceField => {
                OperationSemanticTag::StructuralByteSequenceFieldStore
            }
            StructuralEffectAction::StoreByteViewField => {
                OperationSemanticTag::StructuralByteViewFieldStore
            }
            StructuralEffectAction::EstablishByteSequenceSubslice => {
                OperationSemanticTag::ByteSequenceSubslice
            }
            StructuralEffectAction::EvaluateByteSequencePredicate => {
                OperationSemanticTag::ByteSequencePredicate
            }
            StructuralEffectAction::EvaluateByteSequenceEqual => {
                OperationSemanticTag::ByteSequenceEqual
            }
            StructuralEffectAction::EstablishElementView => {
                OperationSemanticTag::EstablishElementView
            }
            StructuralEffectAction::EstablishMutableElementView => {
                OperationSemanticTag::EstablishMutableElementView
            }
            StructuralEffectAction::ReadElementViewLength => {
                OperationSemanticTag::ElementViewLength
            }
            StructuralEffectAction::ReadElementView => OperationSemanticTag::ElementViewRead,
            StructuralEffectAction::WriteElementView => OperationSemanticTag::ElementViewWrite,
            StructuralEffectAction::EstablishElementViewSubslice => {
                OperationSemanticTag::ElementViewSubslice
            }
            StructuralEffectAction::WriteByteSequence => OperationSemanticTag::ByteSequenceWrite,
            StructuralEffectAction::ReadByteSequence => OperationSemanticTag::ByteSequenceRead,
            StructuralEffectAction::ReadByteSequenceLength => {
                OperationSemanticTag::ByteSequenceLength
            }
            StructuralEffectAction::StorePrimitive => OperationSemanticTag::WriteOnlyPrimitiveStore,
            StructuralEffectAction::StoreScalarField => {
                OperationSemanticTag::StructuralScalarFieldStore
            }
            StructuralEffectAction::EstablishByteSequencePlace => {
                OperationSemanticTag::EstablishByteSequenceLiteral
            }
            StructuralEffectAction::ReadBooleanField => {
                OperationSemanticTag::BooleanStructuralField
            }
            StructuralEffectAction::ReadScalarField => OperationSemanticTag::ScalarStructuralField,
            StructuralEffectAction::EmitPortWrite => OperationSemanticTag::PortWrite,
            StructuralEffectAction::EstablishAffinePlace => {
                OperationSemanticTag::EstablishTrivialAffineLocal
            }
            StructuralEffectAction::EstablishStructuralArray => {
                OperationSemanticTag::EstablishStructuralArray
            }
            StructuralEffectAction::EstablishRecord => OperationSemanticTag::EstablishRecord,
            StructuralEffectAction::EstablishScalarCase => {
                OperationSemanticTag::EstablishScalarCase
            }
            StructuralEffectAction::EstablishStructuralCase => {
                OperationSemanticTag::EstablishStructuralCase
            }
            StructuralEffectAction::EstablishScalarArray => {
                OperationSemanticTag::EstablishScalarArray
            }
        };
        let expected_goal = match schema.action {
            StructuralEffectAction::StoreByteSequence => {
                StructuralEffectGoalShape::ByteLengthWithinOwnerCapacity
            }
            StructuralEffectAction::StoreByteSequenceField => {
                StructuralEffectGoalShape::ByteLengthWithinFieldCapacity
            }
            StructuralEffectAction::EstablishByteSequenceSubslice => {
                StructuralEffectGoalShape::ByteRangeInBounds
            }
            StructuralEffectAction::ReadByteSequence
            | StructuralEffectAction::WriteByteSequence
            | StructuralEffectAction::StoreByteSequenceFieldByte
            | StructuralEffectAction::ReadByteSequenceFieldByte => {
                StructuralEffectGoalShape::ByteIndexInBounds
            }
            StructuralEffectAction::ReadElementView | StructuralEffectAction::WriteElementView => {
                StructuralEffectGoalShape::ElementIndexInBounds
            }
            StructuralEffectAction::EstablishElementViewSubslice => {
                StructuralEffectGoalShape::ElementRangeInBounds
            }
            _ => StructuralEffectGoalShape::None,
        };
        let valid =
            action_tag == tag && schema.goal == expected_goal && match schema.action {
                StructuralEffectAction::EstablishNativeCodeLayout => {
                    schema.result == StructuralEffectResultShape::Structural
                        && schema.custody == StructuralEffectCustody::ExactRecord
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier == StructuralEffectFrontierPolicy::AddsOwnedPlace
                }
                StructuralEffectAction::EstablishAccessReference
                | StructuralEffectAction::EstablishReference => {
                    schema.result == StructuralEffectResultShape::Structural
                        && schema.custody == StructuralEffectCustody::ExactReferenceSource
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::CapturesLoanAndAddsReferenceCarrier
                }
                StructuralEffectAction::ReleaseReference => {
                    schema.result == StructuralEffectResultShape::Unit
                        && schema.custody == StructuralEffectCustody::ExactReferenceCarrier
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::ReleasesReferenceCarrier
                }
                StructuralEffectAction::EstablishPrimitiveLocal => {
                    schema.result == StructuralEffectResultShape::Structural
                        && schema.custody == StructuralEffectCustody::ExactPrimitiveLocal
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier == StructuralEffectFrontierPolicy::AddsUnrestrictedPlace
                }
                StructuralEffectAction::ExposeStorageAddress => {
                    schema.result == StructuralEffectResultShape::Scalar
                        && schema.custody == StructuralEffectCustody::ExactAddressableReferent
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::ReadPrimitive => {
                    schema.result == StructuralEffectResultShape::Scalar
                        && schema.custody == StructuralEffectCustody::ExactReadablePrimitiveRoot
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::ObserveCaseMembership => {
                    schema.result == StructuralEffectResultShape::Boolean
                        && schema.custody == StructuralEffectCustody::ExactReadableSumProjection
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::EvaluateByteSequencePredicate
                | StructuralEffectAction::EvaluateByteSequenceEqual => {
                    schema.result == StructuralEffectResultShape::Boolean
                        && schema.custody == StructuralEffectCustody::ExactImmutableByteView
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::ReadByteSequenceFieldLength
                | StructuralEffectAction::StoreByteSequenceFieldByte
                | StructuralEffectAction::ReadByteSequenceFieldByte => {
                    schema.result
                        == match schema.action {
                            StructuralEffectAction::ReadByteSequenceFieldLength => {
                                StructuralEffectResultShape::ByteCount
                            }
                            StructuralEffectAction::ReadByteSequenceFieldByte => {
                                StructuralEffectResultShape::Byte
                            }
                            _ => StructuralEffectResultShape::Unit,
                        }
                        && schema.custody
                            == StructuralEffectCustody::ExactStructuralByteSequenceField
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::StoreByteSequence => {
                    schema.result == StructuralEffectResultShape::Unit
                        && schema.custody == StructuralEffectCustody::ExactBoundedByteOwner
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::StoreByteSequenceField
                | StructuralEffectAction::StoreByteViewField => {
                    schema.result == StructuralEffectResultShape::Unit
                        && schema.custody
                            == StructuralEffectCustody::ExactStructuralByteSequenceField
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::EstablishByteSequenceSubslice => schema.result
                    == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactImmutableByteView
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresViewAndEstablishesBorrowedView,
                StructuralEffectAction::ReadByteSequence => {
                    schema.result == StructuralEffectResultShape::Byte
                        && schema.custody == StructuralEffectCustody::ExactImmutableByteView
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::WriteByteSequence => {
                    schema.result == StructuralEffectResultShape::Unit
                        && schema.custody == StructuralEffectCustody::ExactMutableByteView
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::ReadByteSequenceLength => {
                    schema.result == StructuralEffectResultShape::ByteCount
                        && schema.custody == StructuralEffectCustody::ExactByteViewMetadata
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::EstablishElementView
                | StructuralEffectAction::EstablishMutableElementView => schema.result
                    == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactElementViewSource
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresViewAndEstablishesBorrowedView,
                StructuralEffectAction::ReadElementViewLength => {
                    schema.result == StructuralEffectResultShape::ElementCount
                        && schema.custody == StructuralEffectCustody::ExactElementViewMetadata
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::ReadElementView => {
                    schema.result == StructuralEffectResultShape::Scalar
                        && schema.custody == StructuralEffectCustody::ExactImmutableElementView
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::WriteElementView => {
                    schema.result == StructuralEffectResultShape::Unit
                        && schema.custody == StructuralEffectCustody::ExactMutableElementView
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::EstablishElementViewSubslice => schema.result
                    == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactImmutableElementView
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresViewAndEstablishesBorrowedView,
                StructuralEffectAction::StorePrimitive => schema.result
                    == StructuralEffectResultShape::Unit
                    && schema.custody == StructuralEffectCustody::ExactWriteOnlyPrimitiveRoot
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsWriteOnlyPrimitivePlace,
                StructuralEffectAction::StoreScalarField => {
                    schema.result == StructuralEffectResultShape::Unit
                        && schema.custody == StructuralEffectCustody::ExactStructuralScalarField
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::EstablishByteSequencePlace => {
                    schema.result == StructuralEffectResultShape::Unit
                        && schema.custody == StructuralEffectCustody::ExactByteSequenceLiteral
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier == StructuralEffectFrontierPolicy::AddsUnrestrictedPlace
                }
                StructuralEffectAction::ReadBooleanField => {
                    schema.result == StructuralEffectResultShape::Boolean
                        && schema.custody == StructuralEffectCustody::ExactLiveBooleanField
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsAffinePlace
                }
                StructuralEffectAction::ReadScalarField => {
                    schema.result == StructuralEffectResultShape::Scalar
                        && schema.custody == StructuralEffectCustody::ExactLiveScalarField
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::EmitPortWrite => {
                    schema.result == StructuralEffectResultShape::Unit
                        && schema.custody == StructuralEffectCustody::ExactPublishedService
                        && schema.external_effect == StructuralEffectExternalEffect::PortWrite
                        && schema.frontier == StructuralEffectFrontierPolicy::KeepsPlaceFrontier
                }
                StructuralEffectAction::EstablishAffinePlace => {
                    schema.result == StructuralEffectResultShape::Unit
                        && schema.custody == StructuralEffectCustody::ExactEmptyAffineLocal
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier == StructuralEffectFrontierPolicy::AddsAffinePlace
                }
                StructuralEffectAction::EstablishStructuralArray => schema.result
                    == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactStructuralArray
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::TransfersOwnedChildrenAndAddsOwnedPlace,
                StructuralEffectAction::EstablishRecord => schema.result
                    == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactRecord
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::TransfersOwnedChildrenAndAddsOwnedPlace,
                StructuralEffectAction::EstablishScalarCase => {
                    schema.result == StructuralEffectResultShape::Structural
                        && schema.custody == StructuralEffectCustody::ExactScalarCase
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier == StructuralEffectFrontierPolicy::AddsOwnedPlace
                }
                StructuralEffectAction::EstablishStructuralCase => schema.result
                    == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactStructuralCase
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::TransfersOwnedChildrenAndAddsOwnedPlace,
                StructuralEffectAction::EstablishScalarArray => {
                    schema.result == StructuralEffectResultShape::Structural
                        && schema.custody == StructuralEffectCustody::ExactScalarArray
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier == StructuralEffectFrontierPolicy::AddsUnrestrictedPlace
                }
            } && schema.work == StructuralEffectWorkPolicy::ConsumeOne;
        valid
            .then_some(())
            .ok_or(OperationSemanticError::StructuralEffectSchemaMismatch(tag))
    }

    fn validate_structural_effect_result(
        operation: &Operation,
        tag: OperationSemanticTag,
        schema: StructuralEffectLeafSchema,
    ) -> Result<(), OperationSemanticError> {
        let valid = match schema.result {
            StructuralEffectResultShape::Scalar => operation.result.scalar_ref().is_some(),
            StructuralEffectResultShape::Byte => operation.result.scalar_ref().is_some_and(|result| {
                matches!(result.scalar_type, ScalarType::Integer(integer) if integer == IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"))
            }),
            StructuralEffectResultShape::ByteCount | StructuralEffectResultShape::ElementCount => {
                operation.result.scalar_ref().is_some_and(|result| {
                    matches!(result.scalar_type, ScalarType::Integer(integer) if integer == IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"))
                })
            }
            StructuralEffectResultShape::Boolean => operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.scalar_type == ScalarType::Boolean),
            StructuralEffectResultShape::Integer => operation
                .result
                .scalar_ref()
                .is_some_and(|result| matches!(result.scalar_type, ScalarType::Integer(_))),
            StructuralEffectResultShape::Structural => operation.result.structural().is_some(),
            StructuralEffectResultShape::Unit => matches!(operation.result, OperationResult::Unit)
        };
        valid
            .then_some(())
            .ok_or(OperationSemanticError::StructuralEffectResultShapeMismatch(
                tag,
            ))
    }

    pub(crate) fn subslice_extent_equation(
        observation: &StructuralEffectObservation,
        producer: &StructuralEffectObservation,
        read: impl Fn(
            &StructuralEffectObservation,
        ) -> Option<(&terminal_psi::StructuralArgument, ValueId)>,
        subslice: impl Fn(&StructuralEffectObservation) -> Option<(PlaceId, ValueId, ValueId)>,
    ) -> Result<Option<Proposition>, OperationSemanticError> {
        let Some((source, result)) = read(observation) else {
            return Ok(None);
        };
        let Some((destination, start, end)) = subslice(producer) else {
            return Ok(None);
        };
        if source.place != destination || !source.path.is_empty() {
            return Ok(None);
        }
        u64_extent_difference_proposition(result, end, start)
    }
}
pub(crate) mod structural_store {
    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use terminal_psi::StructuralRecordStorage;
    use terminal_psi::{
        ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType,
        StructuralMultiplicity, StructuralStoreDisposition, StructuralTypeDeclaration,
        StructuralTypeShape, TerminalMachine,
    };

    pub fn structural_store_value_admitted<'types>(
        machine: &TerminalMachine,
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        disposition: StructuralStoreDisposition,
        value: PlaceId,
    ) -> bool {
        if disposition == StructuralStoreDisposition::Vacant {
            return true;
        }
        let declaration = machine
            .structural_parameters
            .iter()
            .chain(machine.block_structural_parameters())
            .find(|parameter| parameter.place == value)
            .map(|parameter| (parameter.structural_type, parameter.multiplicity, false))
            .or_else(|| {
                machine.operations().find_map(|operation| {
                    operation
                        .result
                        .structural()
                        .filter(|result| result.place == value)
                        .map(|result| {
                            (
                                result.structural_type,
                                result.multiplicity,
                                !result.claims.is_empty(),
                            )
                        })
                })
            });
        declaration.is_some_and(|(identity, multiplicity, claims)| {
            !machine
                .entry_claims
                .iter()
                .chain(&machine.block_entry_claims)
                .any(|claim| claim.input == value)
                && !machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == value)
                && discardable_store_value(types, identity, multiplicity, claims)
        })
    }

    pub fn discardable_store_value<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        identity: StructuralTypeId,
        multiplicity: StructuralMultiplicity,
        has_claims: bool,
    ) -> bool {
        let descriptor_cleanup = types
            .clone()
            .any(|declaration| declaration.id == identity && declaration.descriptor_cleanup);
        (multiplicity == StructuralMultiplicity::Unrestricted
            || multiplicity == StructuralMultiplicity::Affine
                && types.clone().any(|declaration| {
                    declaration.id == identity
                        && (declaration.no_code_affine_cleanup || descriptor_cleanup)
                }))
            && !has_claims
            && disposable_storage(types, identity, descriptor_cleanup, &mut Vec::new())
    }

    fn disposable_storage<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        identity: StructuralTypeId,
        descriptor_cleanup: bool,
        visiting: &mut Vec<StructuralTypeId>,
    ) -> bool {
        if visiting.contains(&identity) {
            return false;
        }
        let Some(declaration) =
            exactly_one(types.clone(), |declaration| declaration.id == identity)
        else {
            return false;
        };
        visiting.push(identity);
        let valid = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => true,
            StructuralTypeShape::AtomicCell(_) => true,
            StructuralTypeShape::ByteSequence(carrier) => {
                descriptor_cleanup || matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. })
            }
            StructuralTypeShape::Record {
                storage:
                    StructuralRecordStorage::AccessView(_) | StructuralRecordStorage::Accessor(_),
                ..
            } => false,
            StructuralTypeShape::Record { fields, .. } => {
                fields_are_disposable(types.clone(), fields, descriptor_cleanup, visiting)
            }
            StructuralTypeShape::FixedArray { element, .. } => {
                disposable_storage(types.clone(), *element, descriptor_cleanup, visiting)
            }
            StructuralTypeShape::Sum { cases } => cases.iter().all(|case| {
                fields_are_disposable(types.clone(), &case.fields, descriptor_cleanup, visiting)
            }),
            StructuralTypeShape::Mixed { fields, cases } => {
                fields_are_disposable(types.clone(), fields, descriptor_cleanup, visiting)
                    && cases.iter().all(|case| {
                        fields_are_disposable(
                            types.clone(),
                            &case.fields,
                            descriptor_cleanup,
                            visiting,
                        )
                    })
            }
            StructuralTypeShape::Reference { access, .. } => {
                descriptor_cleanup && *access != terminal_psi::StructuralAccess::Owned
            }
            StructuralTypeShape::ElementView { .. } => descriptor_cleanup,
        };
        visiting.pop();
        valid
    }

    fn fields_are_disposable<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        fields: &[StructuralFieldDeclaration],
        descriptor_cleanup: bool,
        visiting: &mut Vec<StructuralTypeId>,
    ) -> bool {
        fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .all(|field| match &field.field_type {
                StructuralFieldType::Scalar(_)
                | StructuralFieldType::BoundedInteger(_)
                | StructuralFieldType::RefinedScalar(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::Atomic(_)
                | StructuralFieldType::AtomicBoundedInteger(_) => true,
                StructuralFieldType::Structural(identity) => {
                    disposable_storage(types.clone(), *identity, descriptor_cleanup, visiting)
                }
                StructuralFieldType::ByteSequence(carrier)
                | StructuralFieldType::PredicateByteSequence { carrier, .. } => {
                    descriptor_cleanup
                        || matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. })
                }
                StructuralFieldType::Erased { .. } => false,
            })
    }
}
pub(crate) mod structural_window {

    use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

    pub struct StructuralWindowProjection {
        pub path: Vec<StructuralPathSegment>,
        pub structural_type: StructuralTypeId,
    }

    pub fn structural_window_origins<'parameters>(
        parameters: impl Iterator<Item = &'parameters terminal_psi::StructuralParameterDeclaration>
        + Clone,
        origins: impl IntoIterator<Item = PlaceId>,
        required_access: terminal_psi::StructuralAccess,
        mut live_storage: impl FnMut(PlaceId) -> bool,
    ) -> bool {
        let mut found_origin = false;
        for root in origins {
            found_origin = true;
            if !live_storage(root)
                && !parameters.clone().any(|parameter| {
                    parameter.place == root
                        && required_access.admits_referent(parameter.access)
                        && parameter.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                })
            {
                return false;
            }
        }
        found_origin
    }

    pub fn structural_window_selects_child(
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
    ) -> bool {
        field.is_some()
            || path
                .iter()
                .rev()
                .take_while(|segment| !matches!(segment, StructuralPathSegment::Referent))
                .any(|segment| {
                    matches!(
                        segment,
                        StructuralPathSegment::Field(_)
                            | StructuralPathSegment::FixedIndex(_)
                            | StructuralPathSegment::RuntimeIndex { .. }
                    )
                })
    }

    pub fn structural_window_projection<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
    ) -> Option<StructuralWindowProjection> {
        let mut complete = path.to_vec();
        if let Some(identity) = field {
            let field = crate::projected_field_declaration(types.clone(), root, path, identity)?;
            field.field_type.as_structural()?;
            complete.push(StructuralPathSegment::Field(field.identity));
        }
        let structural_type = crate::structural::primitive_place::projection_tip_with_policy(
            types.clone(),
            root,
            &complete,
            |current, segment| match segment {
                StructuralPathSegment::Field(_)
                | StructuralPathSegment::Case(_)
                | StructuralPathSegment::Referent => true,
                StructuralPathSegment::FixedIndex(_) => current.is_some_and(|current| {
                    types.clone().any(|declaration| {
                        declaration.id == current
                            && matches!(
                                declaration.shape,
                                StructuralTypeShape::FixedArray {
                                    length: terminal_psi::ArrayExtent::Literal(_),
                                    ..
                                }
                            )
                    })
                }),
                StructuralPathSegment::RuntimeIndex { .. } => current.is_some_and(|current| {
                    types.clone().any(|declaration| {
                        declaration.id == current
                            && matches!(
                                declaration.shape,
                                StructuralTypeShape::FixedArray { .. }
                                    | StructuralTypeShape::ElementView { .. }
                            )
                    })
                }),
                _ => false,
            },
        )??;
        Some(StructuralWindowProjection {
            path: complete,
            structural_type,
        })
    }
}
pub(crate) mod zeroed_structural {

    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, ScalarType, StructuralCaseId, StructuralTypeId,
    };
    use terminal_psi::{
        StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
    };

    const MAX_ZEROED_LEAVES: usize = 131_072;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ZeroedStructuralLeaf {
        RecordField {
            path: Vec<StructuralPathSegment>,
            field: semantic_vocabulary::StructuralFieldId,
            scalar_type: ScalarType,
        },
        // The zero value of a readable descriptor is its established empty
        // view. A typed descriptor and an inline byte field share this owner;
        // neither creates an element or borrows unknown storage.
        EmptyView {
            path: Vec<StructuralPathSegment>,
            field: Option<semantic_vocabulary::StructuralFieldId>,
            carrier: StructuralFieldType,
        },
        CaseField {
            path: Vec<StructuralPathSegment>,
            case: StructuralCaseId,
            field: semantic_vocabulary::StructuralFieldId,
            scalar_type: ScalarType,
        },
        ArrayScalar {
            path: Vec<StructuralPathSegment>,
            scalar_type: ScalarType,
        },
        ByteArray {
            path: Vec<StructuralPathSegment>,
            length: u64,
        },
        EmptyScalarArray {
            path: Vec<StructuralPathSegment>,
            structural_type: StructuralTypeId,
        },
        SumSelection {
            path: Vec<StructuralPathSegment>,
            structural_type: StructuralTypeId,
            case: StructuralCaseId,
            scalar_type: ScalarType,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct EstablishedErasedStructuralField {
        pub field_path: Vec<TerminalIdentityIndex>,
        pub carrier_type_identity: TerminalIdentityIndex,
    }

    fn established_field_identity(
        field: &StructuralFieldDeclaration,
    ) -> Option<TerminalIdentityIndex> {
        match &field.field_type {
            StructuralFieldType::Erased { type_identity } if !type_identity.is_absent() => {
                Some(*type_identity)
            }
            _ => None,
        }
    }

    impl ZeroedStructuralLeaf {
        pub fn path(&self) -> &[StructuralPathSegment] {
            match self {
                ZeroedStructuralLeaf::RecordField { path, .. }
                | ZeroedStructuralLeaf::EmptyView { path, .. }
                | ZeroedStructuralLeaf::CaseField { path, .. }
                | ZeroedStructuralLeaf::ArrayScalar { path, .. }
                | ZeroedStructuralLeaf::ByteArray { path, .. }
                | ZeroedStructuralLeaf::EmptyScalarArray { path, .. }
                | ZeroedStructuralLeaf::SumSelection { path, .. } => path,
            }
        }
    }

    pub fn zeroed_structural_shape<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
    ) -> bool {
        zeroed_structural_fields_satisfy(types, root, |_| true)
    }

    pub fn zeroed_structural_fields_satisfy<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        accepts_field: impl Fn(&StructuralFieldDeclaration) -> bool,
    ) -> bool {
        let declarations: Vec<_> = types.collect();
        zeroed_declaration_graph(&declarations, root, false, false, &accepts_field)
    }

    fn zeroed_declaration_graph(
        declarations: &[&StructuralTypeDeclaration],
        root: StructuralTypeId,
        allow_established_fields: bool,
        modulo_references: bool,
        accepts_field: &impl Fn(&StructuralFieldDeclaration) -> bool,
    ) -> bool {
        let mut active = vec![false; declarations.len()];
        let mut complete = vec![false; declarations.len()];
        let mut pending = vec![(root, false)];
        while let Some((current, exiting)) = pending.pop() {
            let Some(position) = declarations
                .iter()
                .position(|declaration| declaration.id == current)
            else {
                return false;
            };
            if exiting {
                active[position] = false;
                complete[position] = true;
                continue;
            }
            if complete[position] {
                continue;
            }
            if active[position] {
                return false;
            }
            if declarations[position]
                .scalar_restriction
                .as_ref()
                .is_some_and(|_| !crate::scalar_declaration_accepts_zero(declarations[position]))
            {
                return false;
            }
            let shape = &declarations[position].shape;
            let (fields, cases) = match shape {
                StructuralTypeShape::PrimitiveScalar(_) => (&[][..], None),
                StructuralTypeShape::AtomicCell(cell) => {
                    if !cell.is_supported() || !crate::scalar_field_accepts_zero(&(*cell).into()) {
                        return false;
                    }
                    (&[][..], None)
                }
                StructuralTypeShape::ElementView {
                    element,
                    access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                } => {
                    // Empty has no member value whose default domain needs
                    // establishment. The element declaration must still exist.
                    if !declarations
                        .iter()
                        .any(|declaration| declaration.id == *element)
                    {
                        return false;
                    }
                    (&[][..], None)
                }
                StructuralTypeShape::FixedArray { element, length } => {
                    active[position] = true;
                    pending.push((current, true));
                    if length.literal() != Some(0) {
                        pending.push((*element, false));
                    } else if !declarations
                        .iter()
                        .any(|declaration| declaration.id == *element)
                    {
                        return false;
                    }
                    continue;
                }
                StructuralTypeShape::Record {
                    storage:
                        terminal_psi::StructuralRecordStorage::AccessView(_)
                        | terminal_psi::StructuralRecordStorage::Accessor(_),
                    ..
                } => return false,
                StructuralTypeShape::Record { fields, .. } => (fields.as_slice(), None),
                StructuralTypeShape::Sum { cases } => (&[][..], Some(cases)),
                StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), Some(cases)),
                _ => return false,
            };
            if let Some(cases) = cases {
                let Some(first) = cases.first() else {
                    return false;
                };
                for field in &first.fields {
                    if field.relevance.is_erased() || !accepts_field(field) {
                        return false;
                    }
                    if let Some(member) = field.field_type.as_structural() {
                        if modulo_references
                            && declarations.iter().any(|declaration| {
                                declaration.id == member
                                    && matches!(
                                        declaration.shape,
                                        StructuralTypeShape::Reference { .. }
                                    )
                            })
                        {
                            continue;
                        }
                        pending.push((member, false));
                    } else if zeroed_field_scalar(field).is_none() {
                        return false;
                    }
                }
            }
            active[position] = true;
            pending.push((current, true));
            for field in fields {
                if allow_established_fields && established_field_identity(field).is_some() {
                    continue;
                }
                if field.relevance.is_erased() || !accepts_field(field) {
                    return false;
                }
                if let Some(member) = field.field_type.as_structural() {
                    if modulo_references
                        && declarations.iter().any(|declaration| {
                            declaration.id == member
                                && matches!(
                                    declaration.shape,
                                    StructuralTypeShape::Reference { .. }
                                )
                        })
                    {
                        continue;
                    }
                    pending.push((member, false));
                } else if zeroed_field_scalar(field).is_none() && !zeroed_byte_field(field) {
                    return false;
                }
            }
        }
        true
    }

    fn zeroed_field_scalar(field: &StructuralFieldDeclaration) -> Option<ScalarType> {
        if field.relevance.is_erased() {
            return None;
        }
        crate::scalar_field_accepts_zero(&field.field_type)
            .then(|| field.field_type.scalar_type())
            .flatten()
    }

    fn zeroed_byte_field(field: &StructuralFieldDeclaration) -> bool {
        !field.relevance.is_erased()
            && matches!(
                field.field_type.byte_sequence_carrier(),
                Some(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        | terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                        }
                )
            )
            && field.field_type.byte_predicates().holds_for(&[])
    }

    pub fn zeroed_structural_leaves<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
    ) -> Option<Vec<ZeroedStructuralLeaf>> {
        zeroed_structural_leaves_with_established_erased_fields(types, root, &[])
    }

    pub fn zeroed_structural_leaves_with_established_erased_fields<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        established_fields: &[EstablishedErasedStructuralField],
    ) -> Option<Vec<ZeroedStructuralLeaf>> {
        zeroed_structural_leaves_impl(types, root, established_fields, false)
    }

    pub fn zeroed_structural_leaves_modulo_references<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
    ) -> Option<Vec<ZeroedStructuralLeaf>> {
        zeroed_structural_leaves_impl(types, root, &[], true)
    }

    fn zeroed_structural_leaves_impl<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        established_fields: &[EstablishedErasedStructuralField],
        modulo_references: bool,
    ) -> Option<Vec<ZeroedStructuralLeaf>> {
        let declarations: Vec<&StructuralTypeDeclaration> = types.collect();
        if !zeroed_declaration_graph(
            &declarations,
            root,
            !established_fields.is_empty(),
            modulo_references,
            &|_| true,
        ) {
            return None;
        }
        let mut consumed = vec![false; established_fields.len()];
        let mut leaves = Vec::new();
        let mut pending = vec![(root, Vec::new())];
        let mut visited_cells = 0usize;
        while let Some((current, path)) = pending.pop() {
            let declaration = declarations
                .iter()
                .find(|declaration| declaration.id == current)?;
            if !matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_)) {
                visited_cells += 1;
                if visited_cells > MAX_ZEROED_LEAVES {
                    return None;
                }
            }
            if let terminal_psi::record_or_mixed_shape_pattern!(fields) = &declaration.shape {
                for field in fields {
                    if let Some(carrier) = established_field_identity(field) {
                        let mut field_path = path
                            .iter()
                            .map(|segment| match segment {
                                StructuralPathSegment::Field(identity) => Some(*identity),
                                _ => None,
                            })
                            .collect::<Option<Vec<_>>>()?;
                        field_path.push(field.identity);
                        let (index, _) = exactly_one(
                            established_fields.iter().enumerate(),
                            |(_, established)| {
                                established.field_path == field_path
                                    && established.carrier_type_identity == carrier
                            },
                        )?;
                        if consumed[index] {
                            return None;
                        }
                        consumed[index] = true;
                        continue;
                    }
                    if let Some(member) = field.field_type.as_structural() {
                        if modulo_references
                            && declarations.iter().any(|declaration| {
                                declaration.id == member
                                    && matches!(
                                        declaration.shape,
                                        StructuralTypeShape::Reference { .. }
                                    )
                            })
                        {
                            continue;
                        }
                        let mut member_path = path.clone();
                        member_path.push(StructuralPathSegment::Field(field.identity));
                        pending.push((member, member_path));
                    } else if zeroed_byte_field(field) {
                        leaves.push(ZeroedStructuralLeaf::EmptyView {
                            path: path.clone(),
                            field: Some(field.id),
                            carrier: field.field_type.clone(),
                        });
                    } else {
                        leaves.push(ZeroedStructuralLeaf::RecordField {
                            path: path.clone(),
                            field: field.id,
                            scalar_type: zeroed_field_scalar(field)?,
                        });
                    }
                    if leaves.len().saturating_add(pending.len()) > MAX_ZEROED_LEAVES {
                        return None;
                    }
                }
            }
            match &declaration.shape {
                StructuralTypeShape::AtomicCell(cell) => {
                    leaves.push(ZeroedStructuralLeaf::ArrayScalar {
                        path,
                        scalar_type: cell.scalar_type(),
                    });
                }
                StructuralTypeShape::ElementView { access: None | Some(terminal_psi::StructuralAccess::SharedBorrow), .. } => {
                    leaves.push(ZeroedStructuralLeaf::EmptyView {
                        path,
                        field: None,
                        carrier: StructuralFieldType::Structural(current),
                    });
                }
                StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                    leaves.push(ZeroedStructuralLeaf::ArrayScalar {
                        path,
                        scalar_type: *scalar_type,
                    });
                }
                StructuralTypeShape::FixedArray { .. }
                    if crate::scalar_array_leaf_shape(declarations.iter().copied(), current)
                        .is_some_and(|(_, count)| count == 0) =>
                {
                    leaves.push(ZeroedStructuralLeaf::EmptyScalarArray {
                        path,
                        structural_type: current,
                    });
                }
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } if *length != 0
                    && declarations.iter().any(|declaration| {
                        declaration.id == *element
                            && matches!(declaration.shape,
                                    StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                                        if integer.sign() == IntegerSign::Unsigned
                                            && integer.bits() == 8
                                            && !integer.is_address())
                    }) =>
                {
                    leaves.push(ZeroedStructuralLeaf::ByteArray {
                        path,
                        length: *length,
                    });
                }
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } => {
                    let extent = usize::try_from(*length).ok()?;
                    if leaves
                        .len()
                        .saturating_add(pending.len())
                        .saturating_add(extent)
                        > MAX_ZEROED_LEAVES
                    {
                        return None;
                    }
                    for index in 0..extent {
                        let mut element_path = path.clone();
                        element_path.push(StructuralPathSegment::FixedIndex(index as u64));
                        pending.push((*element, element_path));
                    }
                }
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    let first = cases.first()?;
                    let tag_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).ok()?);
                    leaves.push(ZeroedStructuralLeaf::SumSelection {
                        path: path.clone(),
                        structural_type: declaration.id,
                        case: first.id,
                        scalar_type: tag_type,
                    });
                    for field in &first.fields {
                        if let Some(member) = field.field_type.as_structural() {
                            let mut member_path = path.clone();
                            member_path.push(StructuralPathSegment::Case(first.id));
                            member_path.push(StructuralPathSegment::Field(field.identity));
                            pending.push((member, member_path));
                        } else {
                            leaves.push(ZeroedStructuralLeaf::CaseField {
                                path: path.clone(),
                                case: first.id,
                                field: field.id,
                                scalar_type: zeroed_field_scalar(field)?,
                            });
                        }
                        if leaves.len().saturating_add(pending.len()) > MAX_ZEROED_LEAVES {
                            return None;
                        }
                    }
                }
                StructuralTypeShape::Record { .. } => {}
                _ => return None,
            }
            if leaves.len() + pending.len() > MAX_ZEROED_LEAVES {
                return None;
            }
        }
        consumed.iter().all(|consumed| *consumed).then_some(leaves)
    }

    pub fn zeroed_projected_byte_qualifications_hold<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'a terminal_psi::StructuralDomainDeclaration> + Clone,
        root: StructuralTypeId,
        qualifications: &[terminal_psi::StructuralPathQualification],
    ) -> bool {
        qualifications.iter().all(|row| {
            if row.path.is_empty() { return false; }
            for (index, segment) in row.path.iter().enumerate() {
                if let StructuralPathSegment::Case(selected) = segment {
                    let Some(parent) = crate::runtime_structural_path_tip(types.clone(), root, &row.path[..index]) else { return false; };
                    let cases = match &parent.shape {
                        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
                        _ => return false,
                    };
                    if cases.first().map(|case| case.id) != Some(*selected) { return false; }
                }
            }
            let Some(carrier) = crate::runtime_structural_path_tip(types.clone(), root, &row.path) else { return false; };
            let StructuralTypeShape::FixedArray { element, length: terminal_psi::ArrayExtent::Literal(length) } = carrier.shape else { return false; };
            if !types.clone().any(|ty| ty.id == element && matches!(ty.shape,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                    if integer.sign() == IntegerSign::Unsigned && integer.bits() == 8 && !integer.is_address())) {
                return false;
            }
            let Some(domain) = domains.clone().find(|domain| domain.id == row.domain) else { return false; };
            domain.carrier == carrier.id
                && domain.content_projection.is_none()
                && domain.establishment_routes.is_none()
                && domain.has_no_required_memberships()
                && domain.byte_predicate.is_some_and(|predicate| match predicate {
                    terminal_psi::ByteSequencePredicate::ValidUtf8 | terminal_psi::ByteSequencePredicate::AsciiOnly => true,
                    terminal_psi::ByteSequencePredicate::NoNul => length == 0,
                    terminal_psi::ByteSequencePredicate::NonEmpty => length != 0,
                })
        })
    }
}
