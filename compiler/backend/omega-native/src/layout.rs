use crate::target::NativeTarget;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_typed_program::Program;
use omega_typed_program::data::{DataDefinition, DataMember, DataShapeKind};
use omega_typed_program::machine::Machine;
use omega_typed_program::types::{PrimitiveType, TypeConstraint, TypeReference};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeLayout {
    pub size: usize,
    pub alignment: usize,
}

impl Default for TypeLayout {
    fn default() -> Self {
        Self {
            size: 0,
            alignment: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLayout {
    pub name: String,
    pub offset: usize,
    pub type_name: String,
    pub layout: TypeLayout,
}

impl Default for FieldLayout {
    fn default() -> Self {
        Self {
            name: String::new(),
            offset: 0,
            type_name: String::new(),
            layout: TypeLayout::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataShape {
    Enum { variants: Vec<String> },
    Record { fields: HandleSpan<FieldLayout> },
}

impl Default for DataShape {
    fn default() -> Self {
        Self::Record {
            fields: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLayout {
    pub name: String,
    pub shape: DataShape,
    pub layout: TypeLayout,
}

impl Default for DataLayout {
    fn default() -> Self {
        Self {
            name: String::new(),
            shape: DataShape::default(),
            layout: TypeLayout::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineLayout {
    pub name: String,
    pub fields: HandleSpan<FieldLayout>,
    pub layout: TypeLayout,
}

impl Default for MachineLayout {
    fn default() -> Self {
        Self {
            name: String::new(),
            fields: HandleSpan::empty(),
            layout: TypeLayout::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutPlan {
    pub data_layouts: Arena<DataLayout>,
    pub fields: Arena<FieldLayout>,
    pub machine_layouts: Arena<MachineLayout>,
}

pub fn build_layout_plan(
    program: &Program,
    target: NativeTarget,
) -> Result<LayoutPlan, Diagnostic> {
    let mut builder = LayoutBuilder::new(program, target);

    for data_definition in &program.data_definitions {
        builder.layout_data_definition(&data_definition.name)?;
    }

    for machine in &program.machines {
        builder.layout_machine(&machine.name)?;
    }

    Ok(LayoutPlan {
        data_layouts: builder.data_layouts,
        fields: builder.fields,
        machine_layouts: builder.machine_layouts,
    })
}

struct LayoutBuilder<'program> {
    data_definitions: &'program [DataDefinition],
    data_layouts: Arena<DataLayout>,
    data_visiting: Vec<String>,
    fields: Arena<FieldLayout>,
    machine_definitions: &'program [Machine],
    machine_layouts: Arena<MachineLayout>,
    machine_visiting: Vec<String>,
    target: NativeTarget,
    type_constraints: &'program Arena<TypeConstraint>,
}

impl<'program> LayoutBuilder<'program> {
    fn new(program: &'program Program, target: NativeTarget) -> Self {
        Self {
            data_definitions: &program.data_definitions,
            data_layouts: Arena::new(),
            data_visiting: Vec::new(),
            fields: Arena::new(),
            machine_definitions: &program.machines,
            machine_layouts: Arena::new(),
            machine_visiting: Vec::new(),
            target,
            type_constraints: &program.type_constraints,
        }
    }

    fn layout_data_definition(&mut self, name: &str) -> Result<TypeLayout, Diagnostic> {
        if let Some(data_layout) = self
            .data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.name == name)
            .map(|(_, data_layout)| data_layout)
        {
            return Ok(data_layout.layout);
        }

        if self.data_visiting.iter().any(|visiting| visiting == name) {
            return Err(Diagnostic::error(format!(
                "recursive data layout is not supported yet for `{name}`"
            )));
        }

        self.data_visiting.push(name.to_owned());

        let definition = self.data_definition(name)?;
        let data_layout = self.compute_data_layout(definition)?;
        let layout = data_layout.layout;

        self.data_layouts.insert(data_layout);
        self.data_visiting.pop();

        Ok(layout)
    }

    fn compute_data_layout(
        &mut self,
        definition: &DataDefinition,
    ) -> Result<DataLayout, Diagnostic> {
        if definition.shape_kind() == DataShapeKind::Enum {
            let variants = definition
                .members
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) => Some(variant.name.clone()),
                    DataMember::Field(_) => None,
                })
                .collect();

            return Ok(DataLayout {
                name: definition.name.clone(),
                shape: DataShape::Enum { variants },
                layout: TypeLayout {
                    size: 4,
                    alignment: 4,
                },
            });
        }

        let fields = definition
            .members
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => Some(field),
                DataMember::Variant(_) => None,
            })
            .map(|field| {
                let layout = self.layout_type_reference(&field.type_reference)?;
                Ok(PlannedField {
                    name: field.name.clone(),
                    type_name: field
                        .type_reference
                        .display_name_with_constraints(self.type_constraints),
                    layout,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let (fields, layout) = pack_fields(fields);
        let fields = self.fields.insert_many(fields);

        Ok(DataLayout {
            name: definition.name.clone(),
            shape: DataShape::Record { fields },
            layout,
        })
    }

    fn layout_machine(&mut self, name: &str) -> Result<TypeLayout, Diagnostic> {
        if let Some(machine_layout) = self
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.name == name)
            .map(|(_, machine_layout)| machine_layout)
        {
            return Ok(machine_layout.layout);
        }

        if self
            .machine_visiting
            .iter()
            .any(|visiting| visiting == name)
        {
            return Err(Diagnostic::error(format!(
                "recursive machine layout is not supported yet for `{name}`"
            )));
        }

        self.machine_visiting.push(name.to_owned());

        let machine = self.machine_definition(name)?;
        let machine_layout = self.compute_machine_layout(machine)?;
        let layout = machine_layout.layout;

        self.machine_layouts.insert(machine_layout);
        self.machine_visiting.pop();

        Ok(layout)
    }

    fn compute_machine_layout(&mut self, machine: &Machine) -> Result<MachineLayout, Diagnostic> {
        let mut fields = Vec::new();

        for owned_data in &machine.owned_data {
            fields.push(PlannedField {
                name: owned_data.name.clone(),
                type_name: owned_data
                    .type_reference
                    .display_name_with_constraints(self.type_constraints),
                layout: self.layout_type_reference(&owned_data.type_reference)?,
            });
        }

        for contained_object in &machine.contains {
            if self.machine_definition(&contained_object.type_name).is_ok() {
                fields.push(PlannedField {
                    name: contained_object.name.clone(),
                    type_name: contained_object.type_name.clone(),
                    layout: self.layout_machine(&contained_object.type_name)?,
                });
            }
        }

        let (fields, layout) = pack_fields(fields);
        let fields = self.fields.insert_many(fields);

        Ok(MachineLayout {
            name: machine.name.clone(),
            fields,
            layout,
        })
    }

    fn layout_type_reference(
        &mut self,
        type_reference: &TypeReference,
    ) -> Result<TypeLayout, Diagnostic> {
        match type_reference {
            TypeReference::Constrained { base_type, .. } => self.layout_type_reference(base_type),
            TypeReference::FixedArray {
                element_type,
                length,
            } => {
                let element_layout = self.layout_type_reference(element_type)?;

                Ok(TypeLayout {
                    size: element_layout.size * length,
                    alignment: element_layout.alignment,
                })
            }
            TypeReference::Generic { base_name, .. } => Err(Diagnostic::error(format!(
                "native layout for generic type `{base_name}` is not implemented yet"
            ))),
            TypeReference::Named(name) => self.layout_named_type(name),
            TypeReference::Unit => Ok(TypeLayout {
                size: 0,
                alignment: 1,
            }),
        }
    }

    fn layout_named_type(&mut self, name: &str) -> Result<TypeLayout, Diagnostic> {
        if let Some(primitive_type) = PrimitiveType::from_name(name) {
            return Ok(self.layout_primitive_type(primitive_type));
        }

        self.layout_data_definition(name)
    }

    fn layout_primitive_type(&self, primitive_type: PrimitiveType) -> TypeLayout {
        match primitive_type {
            PrimitiveType::Bool => TypeLayout {
                size: 1,
                alignment: 1,
            },
            PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 => TypeLayout {
                size: 4,
                alignment: 4,
            },
            PrimitiveType::F64 | PrimitiveType::U64 => TypeLayout {
                size: 8,
                alignment: 8,
            },
            PrimitiveType::Usize => TypeLayout {
                size: self.target.pointer_size,
                alignment: self.target.pointer_alignment,
            },
            PrimitiveType::String => TypeLayout {
                size: self.target.pointer_size * 2,
                alignment: self.target.pointer_alignment,
            },
        }
    }

    fn data_definition(&self, name: &str) -> Result<&'program DataDefinition, Diagnostic> {
        self.data_definitions
            .iter()
            .find(|definition| definition.name == name)
            .ok_or_else(|| Diagnostic::error(format!("unknown data type `{name}`")))
    }

    fn machine_definition(&self, name: &str) -> Result<&'program Machine, Diagnostic> {
        self.machine_definitions
            .iter()
            .find(|machine| machine.name == name)
            .ok_or_else(|| Diagnostic::error(format!("unknown machine `{name}`")))
    }
}

#[derive(Debug)]
struct PlannedField {
    name: String,
    type_name: String,
    layout: TypeLayout,
}

fn pack_fields(fields: Vec<PlannedField>) -> (Vec<FieldLayout>, TypeLayout) {
    let mut offset = 0;
    let mut max_alignment = 1;
    let mut packed_fields = Vec::new();

    for field in fields {
        offset = align_to(offset, field.layout.alignment);
        max_alignment = max_alignment.max(field.layout.alignment);
        packed_fields.push(FieldLayout {
            name: field.name,
            offset,
            type_name: field.type_name,
            layout: field.layout,
        });
        offset += field.layout.size;
    }

    let size = align_to(offset, max_alignment);

    (
        packed_fields,
        TypeLayout {
            size,
            alignment: max_alignment,
        },
    )
}

fn align_to(value: usize, alignment: usize) -> usize {
    if alignment == 0 {
        value
    } else {
        value.div_ceil(alignment) * alignment
    }
}
