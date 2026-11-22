use omega_prepared::{PolicyPreparation, PolicySchemaInput, PreparedValue};
use omega_resolved::DeclarationId;
use omega_toolchain::{SourcePackage, core_interface};
use omega_typed::{TypedTrees, ValueType};
use std::collections::HashMap;
use std::fmt::Write;

pub(crate) fn prepare(
    typed: &TypedTrees<'_>,
    source: &SourcePackage<'_>,
    boundary_slots: &[omega_typed::BoundarySlot],
) -> HashMap<DeclarationId, PolicyPreparation> {
    typed
        .access_policies
        .iter()
        .map(|selection| {
            let domain = &selection.domain;
            let scalar = typed
                .domain_carriers
                .get(domain)
                .and_then(ScalarRoot::from_type);
            let package_key = source.package_key(&domain.package);
            let schema = match (scalar, package_key) {
                (None, _) => PolicySchemaInput::UnsupportedSubject,
                (Some(_), None) => PolicySchemaInput::NeedsPackageOwner,
                (Some(root), Some(key)) => root.argument(domain, key),
            };
            let pending_construction = match &schema {
                PolicySchemaInput::Ready { argument, .. } => super::policy_execution::prepare(typed, selection, argument, boundary_slots),
                _ => None,
            };
            (domain.clone(), PolicyPreparation::pending(package_key.copied(), schema, pending_construction))
        })
        .collect()
}

struct ScalarRoot {
    atom: String,
    name: String,
    kind: u64,
    width: u64,
    signed: bool,
}

impl ScalarRoot {
    fn from_type(carrier: &ValueType) -> Option<Self> {
        let (atom, kind, width, signed) = match carrier {
            ValueType::Bool => ("bool".into(), 0, 0, false),
            ValueType::Integer { signed, width: Some(width) } if *width > 0 => (format!("{}{width}", if *signed { "i" } else { "u" }), 1, *width, *signed),
            ValueType::Address => ("address".into(), 2, 0, false),
            ValueType::Float32 => ("f32".into(), 3, 0, false),
            ValueType::Float64 => ("f64".into(), 4, 0, false),
            _ => return None,
        };
        let name = if carrier == &ValueType::Address { "addr".into() } else { atom.clone() };
        Some(Self {
            atom,
            name,
            kind,
            width,
            signed,
        })
    }

    fn argument(
        self,
        domain: &DeclarationId,
        key: &[u8; 32],
    ) -> PolicySchemaInput {
        let owner = format!("compiler-type(atom({}))", self.atom);
        let application = format!(r"named(name(compiler-type\(atom\({}\)\)))", self.atom);
        let mut requester = String::from("package:");
        for byte in key {
            write!(&mut requester, "{byte:02x}").expect("writing to a string");
        }
        let path = domain.path.replace('\\', "::");
        let escaped_path: String = path
            .chars()
            .flat_map(|c| {
                matches!(c, '(' | ')' | ',' | '\\')
                    .then_some('\\')
                    .into_iter()
                    .chain([c])
            })
            .collect();
        write!(&mut requester, "::declaration(path({escaped_path}),owner({application}))").expect("writing to a string");

        let image = self.image(&requester, &owner, &application);
        let mut subject_revision = b"omega.reflect.primitive.v2\0".to_vec();
        subject_revision.extend_from_slice(application.as_bytes());
        PolicySchemaInput::Ready {
            argument: PreparedValue::Record {
                owner: core_interface().id(r"reflect\TypeSchema").clone(),
                fields: [PreparedValue::SharedBytes(image.into())].into(),
            },
            subject_revision,
        }
    }

    /// Schema image v2. The directory contains node zero (invalid) and the
    /// scalar declaration. Span offsets are local to the image or node owning them.
    fn image(
        &self,
        requester: &str,
        owner: &str,
        application: &str,
    ) -> Vec<u8> {
        const NODE_WORDS: usize = 34;
        let mut declaration = vec![0; NODE_WORDS * 8];
        for (slot, value) in [
            (0, 1),
            (1, self.kind),
            (2, self.width),
            (3, u64::from(self.signed)),
            (6, 4),
            (7, 1),
        ] {
            set_word(&mut declaration, slot, value);
        }
        // Owner, name, closed application, supply, multiplicity, carry, members,
        // retired members, type parameters and lifetime parameters.
        for (slot, text) in [
            (8, owner),
            (10, self.name.as_str()),
            (16, application),
            (18, ""),
            (20, "Copy"),
            (22, ""),
            (24, ""),
            (26, ""),
            (30, ""),
            (32, ""),
        ] {
            append_span(&mut declaration, slot, text.as_bytes());
        }

        let mut image = vec![0; (8 + 2 * 2) * 8];
        let revision = self.graph_revision(requester, owner, application);
        for (slot, value) in [(0, 2), (1, 2), (2, 1), (3, revision), (4, 1)] {
            set_word(&mut image, slot, value);
        }
        append_span(&mut image, 5, requester.as_bytes());
        append_span(&mut image, 8, &[0; NODE_WORDS * 8]);
        append_span(&mut image, 10, &declaration);
        image
    }

    fn graph_revision(
        &self,
        requester: &str,
        owner: &str,
        application: &str,
    ) -> u64 {
        // The graph revision hashes its semantic fields, independent of byte offsets.
        let mut fields = b"omega.reflect.schema.v2".to_vec();
        fields.push(1); // foreign requester of a compiler-owned declaration
        counted_text(&mut fields, requester);
        fields.extend_from_slice(&2u64.to_le_bytes());
        fields.extend_from_slice(&[0, 1]); // invalid node, declaration
        counted_text(&mut fields, owner);
        fields.push(1); // hermetic owner
        counted_text(&mut fields, &self.name);
        fields.push(4); // primitive shape
        counted_text(&mut fields, &self.atom);
        counted_text(&mut fields, "");
        counted_text(&mut fields, "Copy");
        fields.push(0); // no explicit carry
        fields.extend_from_slice(&[0; 16]); // type and lifetime parameter counts
        fields.push(1);
        counted_text(&mut fields, application);
        fields.extend_from_slice(&[0; 16]); // member and retired-member counts
        fields
            .into_iter()
            .fold(0xcbf29ce484222325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3))
            .max(1)
    }
}

fn counted_text(
    bytes: &mut Vec<u8>,
    text: &str,
) {
    bytes.extend_from_slice(&(text.len() as u64).to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
}

fn set_word(
    bytes: &mut [u8],
    slot: usize,
    value: u64,
) {
    bytes[slot * 8..(slot + 1) * 8].copy_from_slice(&value.to_le_bytes());
}

fn append_span(
    bytes: &mut Vec<u8>,
    slot: usize,
    payload: &[u8],
) {
    let start = bytes.len() as u64;
    set_word(bytes, slot, start);
    set_word(bytes, slot + 1, payload.len() as u64);
    bytes.extend_from_slice(payload);
}
