//! Read one owned locator value from an immutable Terminal argument snapshot.
//!
//! The caller already authenticated the exact ForeignBinding nominal
//! application and its const widths. This plan retains only its finite reachable
//! semantic graph and result-descriptor labels. Labels decode ordinary values;
//! they cannot select a declaration, grant authority, or recover a native ABI.
//! Full Field/Case/FixedIndex paths and scalar carriers come from that graph.

use build_time_evaluation::BuildTimeValue;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerSign, IntegerType, IntegerValue, ScalarType,
    StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use terminal_interpreter::{
    TerminalBoundaryStorageValue, TerminalBoundaryStructuralBuffer, TerminalScalarValue,
    TerminalStorageContents,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralRecordStorage, StructuralTypeShape, TerminalModule,
};

pub(super) struct NativeBindingSchema {
    root_type: StructuralTypeId,
    root: usize,
    nodes: Vec<Node>,
}

struct Node {
    id: StructuralTypeId,
    label: String,
    shape: Shape,
}

enum Shape {
    Scalar(ScalarType),
    Record(Vec<Field>),
    Sum(Vec<Case>),
    Array {
        element: usize,
        length: terminal_psi::ArrayExtent,
    },
    Bytes(u64),
}

struct Field {
    id: StructuralFieldId,
    label: String,
    value: FieldValue,
}

enum FieldValue {
    Scalar {
        logical: ScalarType,
        stored: ScalarType,
    },
    Structural(usize),
    Bytes(u64),
}

struct Case {
    id: StructuralCaseId,
    label: String,
    fields: Vec<Field>,
}

impl NativeBindingSchema {
    pub(super) fn prepare(module: &TerminalModule, root: StructuralTypeId) -> Result<Self, String> {
        let mut schema = Self {
            root_type: root,
            root: 0,
            nodes: Vec::new(),
        };
        schema.root = schema.retain(module, root, &mut Vec::new())?;
        Ok(schema)
    }

    pub(super) fn read(
        &self,
        buffer: &TerminalBoundaryStructuralBuffer,
    ) -> Result<BuildTimeValue, String> {
        if buffer.access() != StructuralAccess::Owned || buffer.structural_type() != self.root_type
        {
            return Err("native locator requires its exact owned structural argument".to_owned());
        }
        let rows = buffer.values();
        if rows.windows(2).any(|pair| pair[0].path >= pair[1].path) {
            return Err(
                "native locator snapshot paths are duplicated or out of canonical order".to_owned(),
            );
        }
        let mut input = Input { rows, next: 0 };
        let result = self.read_node(self.root, &mut Vec::new(), &mut input)?;
        if input.next != rows.len() {
            return Err("native locator snapshot has extra or unselected contents".to_owned());
        }
        Ok(result)
    }

    fn retain(
        &mut self,
        module: &TerminalModule,
        id: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
    ) -> Result<usize, String> {
        if active.contains(&id) {
            return Err("native locator schema has a structural cycle".to_owned());
        }
        if let Some(index) = self.nodes.iter().position(|node| node.id == id) {
            return Ok(index);
        }
        let mut declarations = module.structural_types.iter().filter(|row| row.id == id);
        let declaration = declarations
            .next()
            .ok_or("native locator schema has a missing structural type")?;
        if declarations.next().is_some() {
            return Err("native locator schema has an ambiguous structural type".to_owned());
        }
        if declaration.quotient_carrier.is_some() || declaration.owns_linear_claim {
            return Err("native locator cannot erase quotient or linear value custody".to_owned());
        }
        if declaration.scalar_restriction.is_some() {
            return Err("native locator schema has an unsupported restricted scalar".to_owned());
        }
        active.push(id);
        let shape = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => Shape::Scalar(plain_scalar(*scalar)?),
            StructuralTypeShape::Record { fields, storage } => {
                if matches!(
                    storage,
                    StructuralRecordStorage::PlacedView(_)
                        | StructuralRecordStorage::PlacedAccessor(_)
                ) {
                    return Err(
                        "native locator cannot publish a placed record as an owned value"
                            .to_owned(),
                    );
                }
                Shape::Record(self.retain_fields(module, fields, active)?)
            }
            StructuralTypeShape::Sum { cases } => {
                let mut retained = Vec::new();
                retained
                    .try_reserve_exact(cases.len())
                    .map_err(|_| "cannot retain native locator case schema")?;
                for case in cases {
                    let case_label = label(module, case.identity)?;
                    if retained
                        .iter()
                        .any(|known: &Case| known.id == case.id || known.label == case_label)
                    {
                        return Err(
                            "native locator schema repeats a case identity or label".to_owned()
                        );
                    }
                    retained.push(Case {
                        id: case.id,
                        label: case_label,
                        fields: self.retain_fields(module, &case.fields, active)?,
                    });
                }
                Shape::Sum(retained)
            }
            StructuralTypeShape::FixedArray { element, length } => Shape::Array {
                // Validate the element even for an empty array. A zero count
                // cannot conceal a reference, unknown declaration or cycle.
                element: self.retain(module, *element, active)?,
                length: *length,
            },
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) => {
                Shape::Bytes(*capacity)
            }
            _ => {
                return Err(
                    "native locator schema contains a reference, view, routed or unsupported shape"
                        .to_owned(),
                );
            }
        };
        active.pop();
        let index = self.nodes.len();
        self.nodes
            .try_reserve(1)
            .map_err(|_| "cannot retain native locator schema")?;
        self.nodes.push(Node {
            id,
            label: label(module, declaration.identity)?,
            shape,
        });
        Ok(index)
    }

    fn retain_fields(
        &mut self,
        module: &TerminalModule,
        fields: &[StructuralFieldDeclaration],
        active: &mut Vec<StructuralTypeId>,
    ) -> Result<Vec<Field>, String> {
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(fields.len())
            .map_err(|_| "cannot retain native locator fields")?;
        for field in fields {
            if field.relevance.is_erased() {
                return Err("native locator schema cannot omit an erased field".to_owned());
            }
            let field_label = label(module, field.identity)?;
            if retained
                .iter()
                .any(|known: &Field| known.id == field.id || known.label == field_label)
            {
                return Err("native locator schema repeats a field identity or label".to_owned());
            }
            let value = match field.field_type {
                StructuralFieldType::Scalar(scalar) => FieldValue::Scalar {
                    logical: plain_scalar(scalar)?,
                    stored: plain_scalar(field.storage.as_ref().and_then(|storage| storage.stored_scalar()).unwrap_or(scalar))?,
                },
                StructuralFieldType::Structural(child) => FieldValue::Structural(self.retain(module, child, active)?),
                StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) => FieldValue::Bytes(capacity),
                _ => return Err("native locator field has an erased, restricted, atomic, borrowed or unsupported carrier".to_owned()),
            };
            retained.push(Field {
                id: field.id,
                label: field_label,
                value,
            });
        }
        Ok(retained)
    }

    fn read_node(
        &self,
        node: usize,
        path: &mut Vec<Path>,
        input: &mut Input<'_>,
    ) -> Result<BuildTimeValue, String> {
        let node = &self.nodes[node];
        match &node.shape {
            Shape::Scalar(scalar) => read_scalar(input, path, *scalar, *scalar),
            Shape::Record(fields) => Ok(BuildTimeValue::Struct {
                type_name: node.label.clone(),
                fields: self.read_fields(fields, path, input)?,
            }),
            Shape::Sum(cases) => {
                let TerminalStorageContents::Case(selected) = input.take(path)? else {
                    return Err("native locator sum is missing its selected case".to_owned());
                };
                let case = cases
                    .iter()
                    .find(|case| case.id == selected)
                    .ok_or("native locator selected case is outside its nominal schema")?;
                path.push(Path::Case(selected));
                let payload = self.read_fields(&case.fields, path, input);
                path.pop();
                Ok(BuildTimeValue::Case {
                    variant: case.label.clone(),
                    payload: payload?,
                })
            }
            Shape::Bytes(length) => read_bytes(*length, path, input),
            Shape::Array { element, length } => {
                let terminal_psi::ArrayExtent::Literal(length) = length else {
                    return Err(
                        "native locator snapshot needs its admitted closed array extent".into(),
                    );
                };
                if matches!(self.nodes[*element].shape, Shape::Scalar(ScalarType::Integer(integer))
                    if !integer.is_address() && integer.sign() == IntegerSign::Unsigned && integer.bits() == 8)
                {
                    return read_bytes(*length, path, input);
                }
                let length = usize::try_from(*length)
                    .map_err(|_| "native locator array extent exceeds this host")?;
                let mut values = Vec::new();
                values
                    .try_reserve_exact(length)
                    .map_err(|_| "cannot retain native locator array value")?;
                for index in 0..length {
                    path.push(Path::FixedIndex(index as u64));
                    let value = self.read_node(*element, path, input);
                    path.pop();
                    values.push(value?);
                }
                Ok(BuildTimeValue::Array(values))
            }
        }
    }

    fn read_fields(
        &self,
        fields: &[Field],
        path: &mut Vec<Path>,
        input: &mut Input<'_>,
    ) -> Result<Vec<(String, BuildTimeValue)>, String> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(fields.len())
            .map_err(|_| "cannot retain native locator payload")?;
        for field in fields {
            path.push(Path::Field(field.id));
            let value = match field.value {
                FieldValue::Scalar { logical, stored } => read_scalar(input, path, logical, stored),
                FieldValue::Structural(child) => self.read_node(child, path, input),
                FieldValue::Bytes(length) => read_bytes(length, path, input),
            };
            path.pop();
            values.push((field.label.clone(), value?));
        }
        Ok(values)
    }
}

struct Input<'a> {
    rows: &'a [TerminalBoundaryStorageValue],
    next: usize,
}

impl Input<'_> {
    fn take(&mut self, path: &[Path]) -> Result<TerminalStorageContents, String> {
        let row = self
            .rows
            .get(self.next)
            .ok_or_else(|| format!("native locator snapshot is missing contents at {path:?}"))?;
        if row.path != path {
            return Err(format!(
                "native locator snapshot has unexpected contents at {:?}; expected {path:?}",
                row.path
            ));
        }
        self.next += 1;
        Ok(row.contents)
    }
}

fn plain_scalar(scalar: ScalarType) -> Result<ScalarType, String> {
    match scalar {
        ScalarType::Boolean => Ok(scalar),
        ScalarType::Integer(integer) if !integer.is_address() => Ok(scalar),
        _ => Err("native locator scalar must be a fixed integer or Boolean".to_owned()),
    }
}

fn read_scalar(
    input: &mut Input<'_>,
    path: &[Path],
    logical: ScalarType,
    stored: ScalarType,
) -> Result<BuildTimeValue, String> {
    let TerminalStorageContents::Scalar {
        value,
        stored_scalar,
    } = input.take(path)?
    else {
        return Err("native locator scalar path contains a case selector".to_owned());
    };
    if value.scalar_type() != logical || stored_scalar != stored {
        return Err(
            "native locator scalar disagrees with its declared logical or stored carrier"
                .to_owned(),
        );
    }
    match value {
        TerminalScalarValue::Boolean(value) => Ok(BuildTimeValue::Bool(value)),
        TerminalScalarValue::Integer { scalar_type, value } if scalar_type.admits(value) => {
            let value = match value {
                IntegerValue::Signed(value) => i64::try_from(value),
                IntegerValue::Unsigned(value) => i64::try_from(value),
            }
            .map_err(|_| "native locator integer exceeds the compile-time value representation")?;
            Ok(BuildTimeValue::Int(value))
        }
        _ => Err("native locator integer payload is outside its declared carrier".to_owned()),
    }
}

fn read_bytes(
    length: u64,
    path: &mut Vec<Path>,
    input: &mut Input<'_>,
) -> Result<BuildTimeValue, String> {
    let length =
        usize::try_from(length).map_err(|_| "native locator byte extent exceeds this host")?;
    if length > input.rows.len().saturating_sub(input.next) {
        return Err("native locator byte array is missing scalar cells".to_owned());
    }
    let byte = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8)
            .map_err(|_| "native locator byte carrier is unavailable")?,
    );
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| "cannot retain native locator bytes")?;
    for index in 0..length {
        path.push(Path::FixedIndex(index as u64));
        let value = read_scalar(input, path, byte, byte);
        path.pop();
        let BuildTimeValue::Int(value) = value? else {
            return Err("native locator byte array contains a noninteger cell".to_owned());
        };
        bytes.push(u8::try_from(value).map_err(|_| "native locator byte is outside u8")?);
    }
    Ok(BuildTimeValue::Text(bytes))
}

fn label(
    module: &TerminalModule,
    identity: terminal_psi::TerminalIdentityIndex,
) -> Result<String, String> {
    module
        .identities
        .text(identity)
        .filter(|label| !label.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "native locator schema lost its result-descriptor label".to_owned())
}
