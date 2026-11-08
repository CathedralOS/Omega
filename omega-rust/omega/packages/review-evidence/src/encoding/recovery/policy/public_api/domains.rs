use super::super::{
    Error,
    contracts::contract_fact,
    declarations::{alias_atom, establishment_route},
    identity::{nominal, type_identity},
    reader::Reader,
};
use super::signatures::type_parameter;
use crate::record::{
    PackagePolicyDomainShape, PackageReviewDomainClassification, PackageReviewDomainSemanticRole,
};

pub(in crate::encoding::recovery) fn domain_shape(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyDomainShape, Error> {
    Ok(PackagePolicyDomainShape {
        identity: nominal(reader)?,
        type_parameters: reader.sequence(3, type_parameter)?,
        target_type: type_identity(reader)?,
        index_arguments: reader.sequence(8, type_identity)?,
        predicate_presence: super::super::tag!(reader => {
            0 => language_semantics::DomainPredicatePresence::Absent,
            1 => language_semantics::DomainPredicatePresence::Present,
        }),
        predicate_facts: reader.sequence(2, contract_fact)?,
        alias_expansion: reader.option(|reader| reader.sequence(2, alias_atom))?,
        classification: super::super::tag!(reader => {
            0 => None,
            1 => Some(PackageReviewDomainClassification::ProgressProfile),
        }),
        semantic_roles: reader.sequence(1, |reader| {
            Ok(super::super::tag!(reader => {
                0 => PackageReviewDomainSemanticRole::DenotationDimension,
                1 => PackageReviewDomainSemanticRole::ArithmeticPolicy,
            }))
        })?,
        establishment_routes: reader.sequence(42, establishment_route)?,
    })
}
