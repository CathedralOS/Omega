use super::error::DependencyProjectionError;
use super::model::{DependencySourceRequest, PackageSelection};
use crate::declarations::{AliasName, PackageName};

use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) const SOURCE_TYPE_NAME: &str = "Source";
pub(super) const PACKAGE_SELECTION_TYPE_NAME: &str = "PackageSelection";

pub(super) fn project_source_literal(
    syntax_trees: &SyntaxTrees,
    source_handle: ExpressionHandle,
    explicit_alias: Option<AliasName>,
) -> Result<DependencySourceRequest, DependencyProjectionError> {
    let ExpressionNode::StructLiteral(literal) = syntax_trees.expressions.expression(source_handle)
    else {
        return Err(DependencyProjectionError::SourceNotLiteral);
    };
    if literal.type_name.as_str() != SOURCE_TYPE_NAME {
        return Err(DependencyProjectionError::WrongSourceType);
    }
    let Some(case_name) = literal.case_name.as_ref() else {
        return Err(DependencyProjectionError::MissingSourceCase);
    };
    let fields = syntax_trees.expressions.struct_fields(literal.fields);
    match case_name.as_str() {
        "Path" => {
            let [field] = fields else {
                return Err(DependencyProjectionError::WrongSourceFields {
                    case_name: "Path".to_owned(),
                });
            };
            if field.name.as_str() != "location" {
                return Err(DependencyProjectionError::WrongSourceFields {
                    case_name: "Path".to_owned(),
                });
            }
            Ok(DependencySourceRequest::Path {
                explicit_alias,
                location: string_field(syntax_trees, "location", field.value)?,
            })
        }
        "Git" => {
            if !(2..=3).contains(&fields.len())
                || fields
                    .iter()
                    .filter(|field| field.name.as_str() == "repository")
                    .count()
                    != 1
                || fields
                    .iter()
                    .filter(|field| field.name.as_str() == "selection")
                    .count()
                    > 1
                || fields.iter().any(|field| {
                    !matches!(field.name.as_str(), "repository" | "revision" | "selection")
                })
                || fields
                    .iter()
                    .filter(|field| field.name.as_str() == "revision")
                    .count()
                    != 1
            {
                return Err(DependencyProjectionError::WrongSourceFields {
                    case_name: "Git".to_owned(),
                });
            }
            let repository = fields
                .iter()
                .find(|field| field.name.as_str() == "repository")
                .expect("validated Git repository field");
            let revision = fields
                .iter()
                .find(|field| field.name.as_str() == "revision")
                .expect("validated Git revision field");
            let selection = fields
                .iter()
                .find(|field| field.name.as_str() == "selection")
                .map(|field| project_package_selection(syntax_trees, field.value))
                .transpose()?
                .unwrap_or(PackageSelection::Root);
            Ok(DependencySourceRequest::Git {
                explicit_alias,
                repository: string_field(syntax_trees, "repository", repository.value)?,
                revision: string_field(syntax_trees, "revision", revision.value)?,
                selection,
            })
        }
        unsupported => Err(DependencyProjectionError::UnsupportedSourceCase {
            case_name: unsupported.to_owned(),
        }),
    }
}

fn project_package_selection(
    syntax_trees: &SyntaxTrees,
    selection_handle: ExpressionHandle,
) -> Result<PackageSelection, DependencyProjectionError> {
    let ExpressionNode::StructLiteral(literal) =
        syntax_trees.expressions.expression(selection_handle)
    else {
        return Err(DependencyProjectionError::SelectionNotLiteral);
    };
    if literal.type_name.as_str() != PACKAGE_SELECTION_TYPE_NAME {
        return Err(DependencyProjectionError::WrongSelectionType);
    }
    let Some(case_name) = literal.case_name.as_ref() else {
        return Err(DependencyProjectionError::MissingSelectionCase);
    };
    let fields = syntax_trees.expressions.struct_fields(literal.fields);
    match case_name.as_str() {
        "Root" if fields.is_empty() => Ok(PackageSelection::Root),
        "Root" => Err(DependencyProjectionError::WrongSelectionFields {
            case_name: "Root".to_owned(),
        }),
        "Named" if fields.len() == 1 && fields[0].name.as_str() == "package" => {
            let package = string_field(syntax_trees, "selection.package", fields[0].value)?;
            PackageName::parse(&package)
                .map(PackageSelection::Named)
                .map_err(|_| DependencyProjectionError::InvalidSelectedPackage { package })
        }
        "Named" => Err(DependencyProjectionError::WrongSelectionFields {
            case_name: "Named".to_owned(),
        }),
        unsupported => Err(DependencyProjectionError::UnsupportedSelectionCase {
            case_name: unsupported.to_owned(),
        }),
    }
}

pub(super) fn project_alias_literal(
    syntax_trees: &SyntaxTrees,
    alias_handle: ExpressionHandle,
) -> Result<AliasName, DependencyProjectionError> {
    let ExpressionNode::String(bytes) = syntax_trees.expressions.expression(alias_handle) else {
        return Err(DependencyProjectionError::AliasNotString);
    };
    let alias = std::str::from_utf8(bytes).map_err(|_| DependencyProjectionError::AliasNotUtf8)?;
    AliasName::parse(alias).map_err(|_| DependencyProjectionError::InvalidAlias {
        alias: alias.to_owned(),
    })
}

fn string_field(
    syntax_trees: &SyntaxTrees,
    field: &str,
    value: ExpressionHandle,
) -> Result<String, DependencyProjectionError> {
    let ExpressionNode::String(bytes) = syntax_trees.expressions.expression(value) else {
        return Err(DependencyProjectionError::SourceFieldNotString {
            field: field.to_owned(),
        });
    };
    std::str::from_utf8(bytes).map(str::to_owned).map_err(|_| {
        DependencyProjectionError::SourceFieldNotUtf8 {
            field: field.to_owned(),
        }
    })
}
