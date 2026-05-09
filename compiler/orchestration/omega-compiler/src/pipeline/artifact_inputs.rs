use omega_artifacts::{
    AstArtifact, AstFileArtifact, AstIdentityArtifact, SourceFileArtifact, SourceLoadArtifact,
};

use crate::ast::item::{CapabilityMember, DataMember, Item};
use crate::pipeline::compile::{LoadedFile, LoadedProgram};

pub(crate) fn source_load_artifact(loaded_program: &LoadedProgram) -> SourceLoadArtifact {
    SourceLoadArtifact {
        item_count: loaded_program.items.len(),
        files: loaded_program
            .files
            .iter()
            .map(|file| source_file_artifact(loaded_program, file))
            .collect(),
    }
}

pub(crate) fn ast_artifact(loaded_program: &LoadedProgram) -> AstArtifact {
    let identity_storage = crate::ast::identity::count_ast_identity_storage(&loaded_program.items);

    AstArtifact {
        file_count: loaded_program.files.len(),
        item_count: loaded_program.items.len(),
        identity: AstIdentityArtifact {
            owned_identifier_strings: identity_storage.owned_identifier_strings(),
            identifiers: identity_storage.identifiers,
            source_identifiers: identity_storage.source_identifiers,
            generated_identifiers: identity_storage.generated_identifiers,
            path_members: identity_storage.path_members,
            string_literals: identity_storage.string_literals,
            float_literals: identity_storage.float_literals,
            source_float_literals: identity_storage.source_float_literals,
            generated_float_literals: identity_storage.generated_float_literals,
        },
        files: loaded_program
            .files
            .iter()
            .map(|file| ast_file_artifact(loaded_program, file))
            .collect(),
    }
}

fn source_file_artifact(loaded_program: &LoadedProgram, file: &LoadedFile) -> SourceFileArtifact {
    let Some(source_file) = loaded_program.sources.get(file.file_id) else {
        return SourceFileArtifact {
            id: file.file_id.0,
            path: file.path.clone(),
            first_item: file.first_item,
            item_count: file.item_count,
            ..SourceFileArtifact::default()
        };
    };

    SourceFileArtifact {
        id: file.file_id.0,
        path: file.path.clone(),
        first_item: file.first_item,
        item_count: file.item_count,
        byte_count: source_file.source.len(),
        line_count: line_count(source_file.source.as_ref()),
        non_empty_line_count: source_file
            .source
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
    }
}

fn ast_file_artifact(loaded_program: &LoadedProgram, file: &LoadedFile) -> AstFileArtifact {
    let Some(items) = loaded_program
        .items
        .get(file.first_item..file.first_item + file.item_count)
    else {
        return AstFileArtifact {
            path: file.path.clone(),
            first_item: file.first_item,
            item_range_valid: false,
            ..AstFileArtifact::default()
        };
    };

    AstFileArtifact {
        path: file.path.clone(),
        first_item: file.first_item,
        expression_count: file.expression_count,
        type_reference_count: file.type_reference_count,
        type_constraint_count: file.type_constraint_count,
        item_summaries: items.iter().map(ast_item_summary).collect(),
        item_range_valid: true,
    }
}

fn line_count(source: &str) -> usize {
    if source.is_empty() {
        0
    } else {
        source
            .as_bytes()
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            + 1
    }
}

fn ast_item_summary(item: &Item) -> String {
    match item {
        Item::Capability(capability) => {
            let mut field_count = 0usize;
            let mut state_count = 0usize;
            let mut contract_count = 0usize;

            for member in &capability.members {
                match member {
                    CapabilityMember::Field(_) => field_count += 1,
                    CapabilityMember::State(state) => {
                        state_count += 1;
                        contract_count += state.contracts.len();
                    }
                }
            }

            format!(
                "capability `{}` fields {} states {} contracts {}",
                capability.name, field_count, state_count, contract_count
            )
        }
        Item::Data(data_definition) => {
            let mut field_count = 0usize;
            let mut variant_count = 0usize;

            for member in &data_definition.members {
                match member {
                    DataMember::Field(_) => field_count += 1,
                    DataMember::Variant(_) => variant_count += 1,
                }
            }

            format!(
                "data `{}` fields {} variants {}",
                data_definition.name, field_count, variant_count
            )
        }
        Item::Invariant(invariant) => {
            format!(
                "invariant `{}` constraints {}",
                invariant.name,
                invariant.constraints.len()
            )
        }
        Item::Library(library) => format!(
            "library `{}` path `{}` calling convention `{}` functions {} trusts {}",
            library
                .name
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "<anonymous>".to_owned()),
            library.path,
            library.calling_convention,
            library.functions.len(),
            library
                .functions
                .iter()
                .map(|function| function.trusts.len())
                .sum::<usize>()
        ),
        Item::TrustDefinition(trust_definition) => format!(
            "trust `{}` body tokens {}",
            trust_definition.name, trust_definition.token_count
        ),
        Item::Use(use_item) => format!("use {}", use_item.path.join("::")),
        Item::Machine(machine) => format!(
            "machine `{}` contains {} owned data {} states {}",
            machine.name,
            machine.contains.len(),
            machine.owned_data.len(),
            machine.states.len()
        ),
        Item::Platform(platform) => {
            format!(
                "platform `{}` states {}",
                platform.name,
                platform.states.len()
            )
        }
        Item::Target(target) => format!(
            "target `{}` host {} trust policies {}",
            target.name,
            target
                .host
                .as_ref()
                .map(|host| host.provider.join("::"))
                .unwrap_or_else(|| "none".to_owned()),
            target.trust_policies.len()
        ),
    }
}
