use super::review_signature_type_identity_with_binders;
use crate::record::PackageReviewDataField;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn project_data_field(
    compilation: &CheckedCompilation,
    field: &typed_trees::data::DataField,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[typed_trees::name::Identifier],
) -> Result<PackageReviewDataField, Vec<Diagnostic>> {
    Ok(PackageReviewDataField {
        identity: field.identity,
        name: field.name.as_str().to_owned(),
        relevance: field.relevance,
        type_identity: review_signature_type_identity_with_binders(
            compilation,
            field.type_reference,
            binders,
            lifetime_binders,
        )?,
    })
}
