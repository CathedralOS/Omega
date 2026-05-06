use crate::diagnostics::Diagnostic;
use crate::native::layout::MachineLayout;
use crate::native::plan::NativePlan;
use crate::native::target::{NativeTarget, ObjectFormat};
use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectPlan {
    pub target: NativeTarget,
    pub sections: Arena<SectionPlan>,
    pub symbols: Arena<SymbolPlan>,
    pub entry_symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionPlan {
    pub name: String,
    pub kind: SectionKind,
    pub size: usize,
    pub alignment: usize,
}

impl Default for SectionPlan {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: SectionKind::Text,
            size: 0,
            alignment: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Text,
    Data,
    Bss,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolPlan {
    pub name: String,
    pub section: Option<String>,
    pub offset: usize,
    pub size: usize,
    pub kind: SymbolKind,
}

impl Default for SymbolPlan {
    fn default() -> Self {
        Self {
            name: String::new(),
            section: None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Object,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Object,
}

pub fn build_object_plan(native_plan: &NativePlan) -> Result<ObjectPlan, Diagnostic> {
    let main_layout = native_plan
        .layouts
        .machine_layouts
        .iter()
        .find(|layout| layout.name == native_plan.entry_machine)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "missing native layout for entry machine `{}`",
                native_plan.entry_machine
            ))
        })?;
    let entry_symbol = entry_symbol_name(native_plan.target);

    let mut object_plan = ObjectPlan {
        target: native_plan.target,
        sections: Arena::new(),
        symbols: Arena::new(),
        entry_symbol,
    };

    object_plan.sections.insert_many([
        SectionPlan {
            name: section_name(native_plan.target, SectionKind::Text),
            kind: SectionKind::Text,
            size: 0,
            alignment: 16,
        },
        SectionPlan {
            name: section_name(native_plan.target, SectionKind::Data),
            kind: SectionKind::Data,
            size: 0,
            alignment: native_plan.target.pointer_alignment,
        },
        SectionPlan {
            name: section_name(native_plan.target, SectionKind::Bss),
            kind: SectionKind::Bss,
            size: main_layout.layout.size,
            alignment: main_layout.layout.alignment,
        },
    ]);

    object_plan.symbols.insert_many([
        SymbolPlan {
            name: object_plan.entry_symbol.clone(),
            section: Some(section_name(native_plan.target, SectionKind::Text)),
            offset: 0,
            size: 0,
            kind: SymbolKind::Function,
        },
        SymbolPlan {
            name: machine_storage_symbol(main_layout),
            section: Some(section_name(native_plan.target, SectionKind::Bss)),
            offset: 0,
            size: main_layout.layout.size,
            kind: SymbolKind::Object,
        },
    ]);

    Ok(object_plan)
}

fn entry_symbol_name(target: NativeTarget) -> String {
    match target.object_format {
        ObjectFormat::MachO => "_main".to_owned(),
        ObjectFormat::Elf | ObjectFormat::Coff => "main".to_owned(),
    }
}

fn machine_storage_symbol(machine_layout: &MachineLayout) -> String {
    format!("omega_machine_{}_storage", machine_layout.name)
}

fn section_name(target: NativeTarget, kind: SectionKind) -> String {
    match (target.object_format, kind) {
        (ObjectFormat::MachO, SectionKind::Text) => "__TEXT,__text".to_owned(),
        (ObjectFormat::MachO, SectionKind::Data) => "__DATA,__data".to_owned(),
        (ObjectFormat::MachO, SectionKind::Bss) => "__DATA,__bss".to_owned(),
        (_, SectionKind::Text) => ".text".to_owned(),
        (_, SectionKind::Data) => ".data".to_owned(),
        (_, SectionKind::Bss) => ".bss".to_owned(),
    }
}
