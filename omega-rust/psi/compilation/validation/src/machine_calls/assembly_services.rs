//! Assembly reaches the core service in the context that owns its body.
//!
//! Build and product imports create separate checked declarations even when
//! both import the same core bytes. Prepare their exact services once; a name
//! elsewhere in the program cannot supply either declaration.

use language_semantics::ServiceReachId;
use source::DependencyScope;
use symbols::SymbolHandle;

#[derive(Default)]
pub(super) struct AssemblyServices {
    build: ContextServices,
    product: ContextServices,
}

#[derive(Default)]
struct ContextServices {
    machine_control: Selection,
    port_io: Selection,
}

#[derive(Default)]
enum Selection {
    #[default]
    Missing,
    Unique(ServiceReachId),
    Ambiguous,
}

impl Selection {
    fn retain(&mut self, service: ServiceReachId) {
        *self = match self {
            Self::Missing => Self::Unique(service),
            Self::Unique(_) | Self::Ambiguous => Self::Ambiguous,
        };
    }

    fn require(&self) -> Result<ServiceReachId, &'static str> {
        match self {
            Self::Unique(service) => Ok(*service),
            Self::Missing => {
                Err("the core service is unavailable in this body's dependency context")
            }
            Self::Ambiguous => {
                Err("the core service is ambiguous in this body's dependency context")
            }
        }
    }
}

impl AssemblyServices {
    pub(super) fn new(program: &dyn crate::CheckingInput) -> Self {
        let mut services = Self::default();
        for definition in program.service_reaches.definitions() {
            if program.symbol_toolchain_relative_path(definition.symbol)
                != Some(std::path::Path::new("assembly.omg"))
            {
                continue;
            }
            let Some(file) = program
                .symbols
                .symbol_source_span(definition.symbol)
                .and_then(|span| program.symbols.source_file(span))
            else {
                continue;
            };
            let Ok(identity) = program.normalized_hermetic_symbol_identity(definition.symbol)
            else {
                continue;
            };
            let context = match file.dependency_scope {
                DependencyScope::Build => &mut services.build,
                DependencyScope::Product => &mut services.product,
            };
            let selection = match identity.as_str() {
                "toolchain::MachineControl" => &mut context.machine_control,
                "toolchain::PortIo" => &mut context.port_io,
                _ => continue,
            };
            if let Some(service) = program.service_reaches.id_for_symbol(definition.symbol) {
                selection.retain(service);
            }
        }
        services
    }

    pub(super) fn resolve(
        &self,
        program: &dyn crate::CheckingInput,
        owner: SymbolHandle,
        service: &str,
    ) -> Result<ServiceReachId, &'static str> {
        let file = program
            .symbols
            .symbol_provenance_source_span(owner)
            .and_then(|span| program.symbols.source_file(span))
            .ok_or("the assembly body has no retained dependency context")?;
        let context = match file.dependency_scope {
            DependencyScope::Build => &self.build,
            DependencyScope::Product => &self.product,
        };
        // The intrinsic catalog supplies this name, never authored lookup.
        match service {
            "MachineControl" => context.machine_control.require(),
            "PortIo" => context.port_io.require(),
            _ => Err("the assembly intrinsic names an unsupported core service"),
        }
    }
}
