/*
We manage explicit matching-host filesystem delegation and its isolated guest
descriptor and handle namespaces across Darwin, Linux, and Windows.

The native worker's channel and shared-mapping files are never exposed as guest
descriptors. Exact OS mechanisms and normalized locators select adapters rather
than source requirement names, and only toolchain-owned filesystem services are
admitted. Standard descriptors (0, 1, 2) use the independently supplied modeled
I/O streams and never borrow the worker's real standard streams, while failed
close operations mark the guest descriptor or handle uncertain so OS number
reuse cannot revive a stale guest handle.
*/

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};
use target_vocabulary::NormalizedForeignCallBinding;
use terminal_interpreter::{TerminalEffectRejection, TerminalProgram, TerminalScalarValue};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
    StructuralTypeShape,
};

use super::{TerminalRuntimeError, matches_platform_c_symbol, native_provider::rejection};
use crate::execution::TerminalFilesystemDelegation;

pub(super) mod linux;
mod windows;
pub(super) use linux::LinuxFilesystemWrite;

#[derive(Clone)]
pub(super) struct FilesystemContext {
    /// Whether the host namespace is delegated at all.
    supplied: bool,
    standard_streams_only: bool,
    mapped: bool,
    /// Whether that namespace is rooted at a directory. The Linux kernel
    /// adapter resolves `AT_FDCWD` against its own root descriptor; calls the
    /// native sponsor makes (libc, Windows) resolve against the sponsor's
    /// working directory, which the host context starts in the same root.
    rooted: bool,
    windows: Rc<RefCell<windows::Handles>>,
    linux: Rc<RefCell<native_provider_runtime::LinuxFilesystem>>,
    /// The runner's private namespace; when present it answers every Linux
    /// kernel filesystem call instead of `linux`.
    private: Option<Rc<PrivateFilesystem>>,
    descriptors: Rc<RefCell<BTreeMap<i32, NativeDescriptor>>>,
}

struct PrivateFilesystem {
    state: RefCell<native_provider_runtime::PrivateLinuxFilesystem>,
    output_paths: Vec<Vec<u8>>,
    capture_bytes: usize,
}

enum NativeDescriptor {
    Open(i32),
    Uncertain, // Failed close may have retired the OS number; never let reuse revive a guest handle.
}

#[derive(Clone, Copy)]
enum DescriptorEffect {
    None,
    Acquire,
    Use {
        scalar: usize,
        current_directory: bool,
    },
    Close,
    Duplicate,
    OpenAt,
    Io,
}

pub(super) struct LibcFilesystemCall {
    context: FilesystemContext,
    descriptor: DescriptorEffect,
    namespace_admission: Result<(), &'static str>,
    paths: Vec<usize>,
    flags_without_mode: Option<usize>,
}

pub(super) enum FilesystemCall {
    Libc(LibcFilesystemCall),
    Windows(windows::Call),
}

impl FilesystemCall {
    pub(super) fn readonly_record_argument(&self, argument: usize) -> bool {
        matches!(self, Self::Windows(call) if call.readonly_record_argument(argument))
    }
    pub(super) fn prepare_readonly_record(
        &self,
        argument: usize,
        record: &super::native_provider::ReadonlyRecord<'_>,
    ) -> Result<Option<Vec<u8>>, TerminalEffectRejection> {
        match self {
            Self::Windows(call) => call.prepare_readonly_record(argument, record),
            Self::Libc(_) => Err(rejection(
                "this filesystem supply has no readonly native record translation",
            )),
        }
    }
    pub(super) fn path_argument(&self, argument: usize) -> bool {
        match self {
            Self::Libc(call) => call.path_argument(argument),
            Self::Windows(call) => call.path_argument(argument),
        }
    }
    pub(super) fn validate_path(
        &self,
        argument: usize,
        bytes: &[u8],
    ) -> Result<(), TerminalEffectRejection> {
        match self {
            Self::Libc(call) => call.validate_path(argument, bytes),
            Self::Windows(call) => call.validate_path(argument, bytes),
        }
    }
    pub(super) fn descriptor_io(&self) -> bool {
        matches!(self, Self::Libc(call) if call.descriptor_io())
    }
    pub(super) fn modeled_stream(&self, arguments: &[TerminalScalarValue]) -> bool {
        matches!(self, Self::Libc(call) if call.modeled_stream(arguments))
    }
    pub(super) fn arguments(
        &self,
        arguments: &[TerminalScalarValue],
    ) -> Result<Vec<TerminalScalarValue>, TerminalEffectRejection> {
        match self {
            Self::Libc(call) => call.arguments(arguments),
            Self::Windows(call) => call.arguments(arguments),
        }
    }
    pub(super) fn result(
        &self,
        arguments: &[TerminalScalarValue],
        result: TerminalScalarValue,
    ) -> Result<TerminalScalarValue, TerminalEffectRejection> {
        match self {
            Self::Libc(call) => call.result(arguments, result),
            Self::Windows(call) => call.result(arguments, result),
        }
    }
    pub(super) fn output_scalar(
        &self,
        argument: usize,
        value: TerminalScalarValue,
        result: Option<TerminalScalarValue>,
    ) -> Result<Option<TerminalScalarValue>, TerminalEffectRejection> {
        match self {
            Self::Libc(_) => Ok(None),
            Self::Windows(call) => call.output_scalar(argument, value, result),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Formal {
    Signed(u16),
    Unsigned(u16),
    UnsignedArray(u64, u16, StructuralAccess),
    /// A borrowed fixed array of unsigned integers of this width holding at
    /// least `minimum` elements; the declared length is the caller's own.
    UnsignedWindow {
        bits: u16,
        minimum: u64,
        access: StructuralAccess,
    },
    ByteView(StructuralAccess),
    IntegerReference(IntegerSign, u16, StructuralAccess),
    IntegerRecord {
        fields: &'static [super::native_provider::IntegerRecordField],
        access: StructuralAccess,
    },
    SignedRecordArray {
        records: u64,
        words: usize,
        bits: u16,
        alignment: u16,
        access: StructuralAccess,
    },
}

impl FilesystemContext {
    pub(super) fn new(
        delegation: TerminalFilesystemDelegation,
    ) -> Result<Self, TerminalRuntimeError> {
        // A root that cannot be opened supplies no namespace at all, so every
        // filesystem call refuses rather than falling back to the process
        // directory.
        let standard_streams_only = matches!(
            delegation,
            TerminalFilesystemDelegation::StandardStreamsOnly
        );
        let mapped = matches!(delegation, TerminalFilesystemDelegation::Mapped(_));
        let mut private = None;
        let (supplied, rooted, linux) = match delegation {
            TerminalFilesystemDelegation::Unprovided
            | TerminalFilesystemDelegation::StandardStreamsOnly => {
                (false, false, Default::default())
            }
            TerminalFilesystemDelegation::Private {
                initial_files,
                output_paths,
                capture_bytes,
            } => {
                let state = native_provider_runtime::PrivateLinuxFilesystem::with_initial_files(
                    &initial_files,
                )
                .map_err(TerminalRuntimeError::PrivateFilesystemSetup)?;
                private = Some(Rc::new(PrivateFilesystem {
                    state: RefCell::new(state),
                    output_paths,
                    capture_bytes,
                }));
                (true, false, Default::default())
            }
            TerminalFilesystemDelegation::Mapped(mappings) => (
                true,
                false,
                native_provider_runtime::LinuxFilesystem::mapped(mappings).map_err(|error| {
                    TerminalRuntimeError::MissingProvision {
                        capability: "live filesystem directory mappings",
                        reason: error.to_string(),
                    }
                })?,
            ),
            TerminalFilesystemDelegation::HostNamespace => (true, false, Default::default()),
            TerminalFilesystemDelegation::HostDirectory(root) => {
                match native_provider_runtime::LinuxFilesystem::rooted(&root) {
                    Ok(linux) => (true, true, linux),
                    Err(_) => (false, true, Default::default()),
                }
            }
        };
        Ok(Self {
            supplied,
            standard_streams_only,
            mapped,
            rooted,
            windows: Rc::new(RefCell::new(windows::Handles::default())),
            descriptors: Rc::new(RefCell::new(BTreeMap::new())),
            linux: Rc::new(RefCell::new(linux)),
            private,
        })
    }

    /// The private namespace, when this execution's runner supplies one.
    pub(super) fn private(
        &self,
    ) -> Option<&RefCell<native_provider_runtime::PrivateLinuxFilesystem>> {
        self.private.as_ref().map(|private| &private.state)
    }

    pub(super) fn declared_files(&self) -> Result<crate::ObservedFiles, TerminalRuntimeError> {
        let mut observed = BTreeMap::new();
        if let Some(private) = &self.private {
            let state = private.state.borrow();
            let mut remaining = private.capture_bytes;
            for path in &private.output_paths {
                let file = state
                    .file_bytes(path, remaining)
                    .map_err(TerminalRuntimeError::FileObservation)?;
                remaining -= file.as_ref().map_or(0, Vec::len);
                observed.insert(path.clone(), file);
            }
        }
        Ok(observed)
    }

    /// The namespace the native sponsor's libc and Windows supplies use.
    fn process_namespace(&self) -> Result<(), &'static str> {
        match (self.supplied, self.rooted) {
            (true, _) => Ok(()),
            (false, true) => Err("the filesystem namespace root could not be opened"),
            (false, false) => Err("no explicit host filesystem namespace was supplied"),
        }
    }

    pub(super) fn admit(
        &self,
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        layouts: &[NativeBorrowedStorageLayout],
    ) -> Result<Option<FilesystemCall>, TerminalRuntimeError> {
        // A private namespace answers only kernel calls; no libc or Windows
        // filesystem import may reach the host in its place.
        if self.private.is_some() || self.mapped {
            return Ok(None);
        }
        if let Some(call) = windows::admit(self, program, declaration, binding, layouts)? {
            return Ok(Some(FilesystemCall::Windows(call)));
        }
        let Some(locator) = binding.foreign_import_locator() else {
            return Ok(None);
        };
        // Linux reaches libc only for `realpath`; every other Linux filesystem
        // operation is a kernel call the `linux` adapter executes directly.
        let linux = matches!(
            locator.target(),
            target::TargetProfile::LinuxX64 | target::TargetProfile::LinuxArm64
        ) && Some(locator.target()) == target::TargetProfile::host_if_supported();
        if !linux
            && (!matches!(locator.target(), target::TargetProfile::MacosArm64)
                || !cfg!(all(target_os = "macos", target_arch = "aarch64")))
        {
            return Ok(None);
        }
        use DescriptorEffect::{Acquire, Close, Duplicate, None as NoDescriptor, OpenAt, Use};
        use Formal::{ByteView, IntegerReference, Signed, Unsigned, UnsignedArray};
        use StructuralAccess::{MutableBorrow, SharedBorrow};
        let path = UnsignedArray(4096, 8, SharedBorrow);
        let realpath: &[Formal] = &[path, UnsignedArray(4096, 8, MutableBorrow)];
        let metadata = UnsignedArray(144, 8, MutableBorrow);
        let candidates: &[(&[u8], &[Formal], Formal, DescriptorEffect)] = &[
            (b"open", &[path, Signed(32)], Signed(32), Acquire),
            (
                b"__open",
                &[path, Signed(32), Unsigned(16)],
                Signed(32),
                Acquire,
            ),
            (b"creat", &[path, Unsigned(16)], Signed(32), Acquire),
            (b"close", &[Signed(32)], Signed(32), Close),
            (
                b"pread",
                &[Signed(32), ByteView(MutableBorrow), Signed(64)],
                Signed(64),
                Use {
                    scalar: 0,
                    current_directory: false,
                },
            ),
            (
                b"pwrite",
                &[Signed(32), ByteView(SharedBorrow), Signed(64)],
                Signed(64),
                Use {
                    scalar: 0,
                    current_directory: false,
                },
            ),
            (b"unlink", &[path], Signed(32), NoDescriptor),
            (b"rmdir", &[path], Signed(32), NoDescriptor),
            (b"realpath", realpath, Unsigned(64), NoDescriptor),
            (b"mkdir", &[path, Unsigned(16)], Signed(32), NoDescriptor),
            (b"rename", &[path, path], Signed(32), NoDescriptor),
            (b"link", &[path, path], Signed(32), NoDescriptor),
            (b"symlink", &[path, path], Signed(32), NoDescriptor),
            (b"chmod", &[path, Unsigned(16)], Signed(32), NoDescriptor),
            (
                b"chown",
                &[path, Unsigned(32), Unsigned(32)],
                Signed(32),
                NoDescriptor,
            ),
            (
                b"lchown",
                &[path, Unsigned(32), Unsigned(32)],
                Signed(32),
                NoDescriptor,
            ),
            (
                b"openat",
                &[Signed(32), path, Signed(32)],
                Signed(32),
                OpenAt,
            ),
            (
                b"unlinkat",
                &[Signed(32), path, Signed(32)],
                Signed(32),
                Use {
                    scalar: 0,
                    current_directory: true,
                },
            ),
            (
                b"fstatat",
                &[Signed(32), path, metadata, Signed(32)],
                Signed(32),
                Use {
                    scalar: 0,
                    current_directory: true,
                },
            ),
            (
                b"fstat",
                &[Signed(32), metadata],
                Signed(32),
                Use {
                    scalar: 0,
                    current_directory: false,
                },
            ),
            (
                b"fchmod",
                &[Signed(32), Unsigned(16)],
                Signed(32),
                Use {
                    scalar: 0,
                    current_directory: false,
                },
            ),
            (
                b"fchown",
                &[Signed(32), Unsigned(32), Unsigned(32)],
                Signed(32),
                Use {
                    scalar: 0,
                    current_directory: false,
                },
            ),
            (
                b"ftruncate",
                &[Signed(32), Signed(64)],
                Signed(32),
                Use {
                    scalar: 0,
                    current_directory: false,
                },
            ),
            (
                b"lseek",
                &[Signed(32), Signed(64), Signed(32)],
                Signed(64),
                Use {
                    scalar: 0,
                    current_directory: false,
                },
            ),
            (
                b"futimens",
                &[
                    Signed(32),
                    Formal::SignedRecordArray {
                        records: 2,
                        words: 2,
                        bits: 64,
                        alignment: 8,
                        access: SharedBorrow,
                    },
                ],
                Signed(32),
                Use {
                    scalar: 0,
                    current_directory: false,
                },
            ),
            (
                b"fsync",
                &[Signed(32)],
                Signed(32),
                Use {
                    scalar: 0,
                    current_directory: false,
                },
            ),
            (
                b"fdatasync",
                &[Signed(32)],
                Signed(32),
                Use {
                    scalar: 0,
                    current_directory: false,
                },
            ),
            (
                b"flock",
                &[Signed(32), Signed(32)],
                Signed(32),
                Use {
                    scalar: 0,
                    current_directory: false,
                },
            ),
            (b"dup", &[Signed(32)], Signed(32), Duplicate),
            (
                b"__getdirentries64",
                &[
                    Signed(32),
                    ByteView(MutableBorrow),
                    IntegerReference(IntegerSign::Signed, 64, MutableBorrow),
                ],
                Signed(64),
                Use {
                    scalar: 0,
                    current_directory: false,
                },
            ),
        ];
        let mut selected = None;
        if linux {
            if glibc_realpath(locator)? {
                selected = Some((Some(realpath), Unsigned(64), NoDescriptor));
            }
        } else {
            for &(symbol, parameters, result, descriptor) in candidates {
                if matches_platform_c_symbol(
                    binding,
                    symbol,
                    &[b"/usr/lib/libSystem.B.dylib"],
                    None,
                )? {
                    selected = Some((Some(parameters), result, descriptor));
                    break;
                }
            }
        }
        if !linux
            && self.process_namespace().is_ok()
            && matches!(
                runtime_abi::descriptor_import(locator),
                Some((runtime_abi::DescriptorAbi::DarwinLibSystem, _))
            )
        {
            selected = Some((None, Signed(64), DescriptorEffect::Io));
        }
        let Some((parameters, result, descriptor)) = selected else {
            return Ok(None);
        };
        self.process_namespace()
            .map_err(|reason| TerminalRuntimeError::missing("runtime-filesystem", reason))?;
        let namespace_admission = self.admit_services(program, declaration);
        if !matches!(descriptor, DescriptorEffect::Io) {
            namespace_admission
                .map_err(|reason| TerminalRuntimeError::missing("runtime-filesystem", reason))?;
        }
        if parameters.is_some_and(|parameters| {
            !matches_signature(program, declaration, parameters, result, layouts)
        }) {
            return Err(TerminalRuntimeError::InvalidProvision(
                "filesystem import differs from its complete OS contract",
            ));
        }
        let flags_without_mode = match (descriptor, parameters) {
            (Acquire, Some([UnsignedArray(4096, 8, SharedBorrow), Signed(32)])) => Some(0),
            (OpenAt, _) => Some(1),
            _ => None,
        };
        let mut structural = 0;
        let mut paths = Vec::new();
        for formal in parameters.into_iter().flatten() {
            match formal {
                UnsignedArray(count, bits, access) => {
                    if *count == 4096 && *bits == 8 && *access == SharedBorrow {
                        paths.push(structural);
                    }
                    structural += 1;
                }
                ByteView(_)
                | Formal::UnsignedWindow { .. }
                | IntegerReference(_, _, _)
                | Formal::IntegerRecord { .. }
                | Formal::SignedRecordArray { .. } => structural += 1,
                _ => {}
            }
        }
        Ok(Some(FilesystemCall::Libc(LibcFilesystemCall {
            context: self.clone(),
            descriptor,
            namespace_admission,
            paths,
            flags_without_mode,
        })))
    }

    fn admit_services(
        &self,
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
    ) -> Result<(), &'static str> {
        self.admit_service_ids(program.module(), declaration.reach_beyond_owning_supply())
    }

    fn admit_service_ids(
        &self,
        module: &terminal_psi::TerminalModule,
        services: impl IntoIterator<Item = semantic_vocabulary::ServiceId>,
    ) -> Result<(), &'static str> {
        for service in services {
            module
                .core_filesystem_service(service)
                .map_err(|_| "filesystem supply does not provide this exact additional service")?;
        }
        Ok(())
    }
}

/// Whether `locator` is glibc's versioned `realpath` for its Linux target.
fn glibc_realpath(
    locator: &target::NormalizedForeignLocator,
) -> Result<bool, TerminalRuntimeError> {
    let target = locator.target();
    let version: &[u8] = match target {
        target::TargetProfile::LinuxX64 => b"GLIBC_2.3",
        target::TargetProfile::LinuxArm64 => b"GLIBC_2.17",
        _ => return Ok(false),
    };
    target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::ElfVersioned {
            object: b"libc.so.6".to_vec(),
            symbol: b"realpath".to_vec(),
            version: version.to_vec(),
        },
        target,
    )
    .map(|offered| offered == *locator)
    .map_err(|_| TerminalRuntimeError::InvalidProvision("glibc realpath is not a valid locator"))
}

fn matches_signature(
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    expected: &[Formal],
    result: Formal,
    layouts: &[NativeBorrowedStorageLayout],
) -> bool {
    matches_parameters(program, declaration, expected, layouts)
        && matches!(declaration.result, BoundaryMachineResult::Scalar(value) if scalar_matches(value.scalar_type, result))
}

fn scalar_matches(scalar: ScalarType, expected: Formal) -> bool {
    match (scalar, expected) {
        (ScalarType::Integer(integer), Formal::Signed(bits)) => {
            integer.sign() == IntegerSign::Signed && integer.bits() == Some(bits)
        }
        (ScalarType::Integer(integer), Formal::Unsigned(bits)) => {
            integer.sign() == IntegerSign::Unsigned && integer.bits() == Some(bits)
        }
        _ => false,
    }
}

fn matches_parameters(
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    expected: &[Formal],
    layouts: &[NativeBorrowedStorageLayout],
) -> bool {
    if declaration.parameter_order.len() != expected.len() {
        return false;
    }
    let (mut scalar, mut structural) = (0, 0);
    for (kind, expected) in declaration.parameter_order.iter().zip(expected) {
        let valid = match kind {
            BoundaryParameterKind::Scalar => {
                let value = declaration.scalar_parameters.get(scalar).copied();
                scalar += 1;
                value.is_some_and(|value| scalar_matches(value, *expected))
            }
            BoundaryParameterKind::Structural => {
                let value = declaration.structural_parameters.get(structural);
                structural += 1;
                match (value, expected) {
                    (
                        Some(value),
                        Formal::UnsignedWindow {
                            bits,
                            minimum,
                            access,
                        },
                    ) if value.access == *access => {
                        let Some(StructuralTypeShape::FixedArray { length, .. }) = program
                            .structural_type(value.structural_type)
                            .map(|row| &row.shape)
                        else {
                            return false;
                        };
                        let Some(count) = length.literal().filter(|count| count >= minimum) else {
                            return false;
                        };
                        super::native_provider::matches_integer_storage(
                            program,
                            layouts,
                            value.structural_type,
                            IntegerSign::Unsigned,
                            *bits,
                            Some(count),
                        )
                    }
                    (Some(value), Formal::ByteView(access)) if value.access == *access => {
                        matches!(program.structural_type(value.structural_type).map(|row| &row.shape),
                            Some(StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView { access: ceiling })) if ceiling.is_none_or(|ceiling| ceiling == *access))
                    }
                    (Some(value), Formal::IntegerReference(sign, bits, access))
                        if value.access == *access =>
                    {
                        super::native_provider::matches_integer_storage(
                            program,
                            layouts,
                            value.structural_type,
                            *sign,
                            *bits,
                            None,
                        )
                    }
                    (
                        Some(value),
                        Formal::SignedRecordArray {
                            records,
                            words,
                            bits,
                            alignment,
                            access,
                        },
                    ) if value.access == *access => {
                        super::native_provider::matches_signed_record_array(
                            program,
                            layouts,
                            value.structural_type,
                            *records,
                            *words,
                            *bits,
                            *alignment,
                        )
                    }
                    (Some(value), Formal::IntegerRecord { fields, access })
                        if value.access == *access =>
                    {
                        super::native_provider::matches_integer_record(
                            program,
                            layouts,
                            value.structural_type,
                            fields,
                        )
                    }
                    (Some(value), Formal::UnsignedArray(count, bits, access))
                        if value.access == *access =>
                    {
                        super::native_provider::matches_integer_storage(
                            program,
                            layouts,
                            value.structural_type,
                            IntegerSign::Unsigned,
                            *bits,
                            Some(*count),
                        )
                    }
                    _ => false,
                }
            }
        };
        if !valid {
            return false;
        }
    }
    scalar == declaration.scalar_parameters.len()
        && structural == declaration.structural_parameters.len()
}

impl LibcFilesystemCall {
    pub(super) fn validate_path(
        &self,
        argument: usize,
        bytes: &[u8],
    ) -> Result<(), TerminalEffectRejection> {
        if self.paths.contains(&argument) && !bytes.contains(&0) {
            return Err(rejection(
                "native filesystem path has no terminator inside its captured window",
            ));
        }
        Ok(())
    }

    pub(super) fn path_argument(&self, argument: usize) -> bool {
        self.paths.contains(&argument)
    }
    pub(super) fn descriptor_io(&self) -> bool {
        matches!(self.descriptor, DescriptorEffect::Io)
    }

    pub(super) fn modeled_stream(&self, arguments: &[TerminalScalarValue]) -> bool {
        self.descriptor_io()
            && matches!(
                arguments.first(),
                Some(TerminalScalarValue::Integer {
                    value: IntegerValue::Signed(0..=2),
                    ..
                })
            )
    }

    pub(super) fn arguments(
        &self,
        arguments: &[TerminalScalarValue],
    ) -> Result<Vec<TerminalScalarValue>, TerminalEffectRejection> {
        self.namespace_admission.map_err(rejection)?;
        if let Some(index) = self.flags_without_mode {
            let Some(TerminalScalarValue::Integer {
                value: IntegerValue::Signed(flags),
                ..
            }) = arguments.get(index)
            else {
                return Err(rejection("native open lost its declared flags"));
            };
            if flags & 0x200 != 0 {
                return Err(rejection(
                    "native open with O_CREAT requires its explicit mode argument",
                ));
            }
        }
        let mut actual = arguments.to_vec();
        let (index, current_directory) = match self.descriptor {
            DescriptorEffect::None | DescriptorEffect::Acquire => return Ok(actual),
            DescriptorEffect::Use {
                scalar,
                current_directory,
            } => (scalar, current_directory),
            DescriptorEffect::OpenAt => (0, true),
            DescriptorEffect::Close | DescriptorEffect::Duplicate | DescriptorEffect::Io => {
                (0, false)
            }
        };
        let Some(TerminalScalarValue::Integer {
            value: IntegerValue::Signed(guest),
            ..
        }) = actual.get_mut(index)
        else {
            return Err(rejection("filesystem descriptor is not a signed scalar"));
        };
        let guest_descriptor = i32::try_from(*guest)
            .map_err(|_| rejection("filesystem descriptor exceeds its carrier"))?;
        if (0..=2).contains(&guest_descriptor) {
            return Err(rejection(
                "filesystem operation has no supplied standard-stream contract",
            ));
        }
        *guest = i128::from(if current_directory && guest_descriptor == -2 {
            -2
        } else {
            match self.context.descriptors.borrow().get(&guest_descriptor) {
                Some(NativeDescriptor::Open(native)) => *native,
                Some(NativeDescriptor::Uncertain) => {
                    return Err(rejection(
                        "filesystem descriptor state is uncertain after a failed close",
                    ));
                }
                None => -1,
            }
        });
        Ok(actual)
    }

    pub(super) fn result(
        &self,
        arguments: &[TerminalScalarValue],
        mut result: TerminalScalarValue,
    ) -> Result<TerminalScalarValue, TerminalEffectRejection> {
        // A descriptor-free call such as `realpath` returns its own unsigned
        // value (a pointer into the sponsor's storage, tested only for null).
        if matches!(self.descriptor, DescriptorEffect::None)
            && matches!(
                result,
                TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(_),
                    ..
                }
            )
        {
            return Ok(result);
        }
        let TerminalScalarValue::Integer {
            value: IntegerValue::Signed(value),
            ..
        } = &mut result
        else {
            return Err(rejection(
                "filesystem completion is not its signed OS result",
            ));
        };
        if matches!(self.descriptor, DescriptorEffect::Close) {
            let Some(TerminalScalarValue::Integer {
                value: IntegerValue::Signed(guest),
                ..
            }) = arguments.first()
            else {
                return Err(rejection("closed descriptor lost its guest identity"));
            };
            let guest = i32::try_from(*guest)
                .map_err(|_| rejection("closed descriptor exceeds its carrier"))?;
            let mut descriptors = self.context.descriptors.borrow_mut();
            if *value == 0 {
                descriptors.remove(&guest);
            } else if let Some(state) = descriptors.get_mut(&guest) {
                *state = NativeDescriptor::Uncertain;
            }
            return Ok(result);
        }
        if *value < 0 {
            return Ok(result);
        }
        match self.descriptor {
            DescriptorEffect::Acquire | DescriptorEffect::OpenAt | DescriptorEffect::Duplicate => {
                let native = i32::try_from(*value)
                    .map_err(|_| rejection("native descriptor exceeds its OS carrier"))?;
                let mut descriptors = self.context.descriptors.borrow_mut();
                let guest = (3..=i32::MAX)
                    .find(|candidate| !descriptors.contains_key(candidate))
                    .ok_or_else(|| {
                        rejection("guest filesystem descriptor namespace is exhausted")
                    })?;
                descriptors.insert(guest, NativeDescriptor::Open(native));
                *value = i128::from(guest);
            }
            _ => {}
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::IntegerType;

    fn canonical_filesystem_module() -> terminal_psi::TerminalModule {
        use application_identity::type_identity::encoding::{
            CoordinateTag, atom, bare, byte_atom, compound,
        };
        use application_identity::type_identity::vocabulary::{NameForm, OwnerForm};
        use language_core::core_vocabulary::{
            CoreVocabulary, CoreVocabularyCatalog, FilesystemService,
        };
        use service_surface::{ServiceReceiverApplication, ServiceSurface};
        use terminal_psi::{
            ServiceDeclaration, TerminalIdentityTableBuilder, TerminalModule, TerminalServiceSchema,
        };

        let mut module = TerminalModule::declarations();
        let mut identities = TerminalIdentityTableBuilder::default();
        for (index, service) in FilesystemService::ALL.into_iter().enumerate() {
            let site =
                CoreVocabularyCatalog::declaration(CoreVocabulary::FilesystemService(service))
                    .unwrap();
            let revision = language_core::bundled_files::core_revision::FILESYSTEM_AUTHORITY;
            let nominal = |owner| {
                compound(
                    NameForm::Nominal,
                    [owner, atom(CoordinateTag::Path, site.declaration)],
                )
            };
            let declaration =
                identities.intern(&nominal(byte_atom(OwnerForm::ToolchainSource, &revision)));
            let application = compound(
                CoordinateTag::TraitApplication,
                [
                    atom(
                        CoordinateTag::Declaration,
                        &nominal(bare(OwnerForm::Toolchain)),
                    ),
                    compound(CoordinateTag::Arguments, []),
                    compound(CoordinateTag::Lifetimes, []),
                ],
            );
            let mut surface = ServiceSurface::default();
            surface.trait_name = surface.identities.intern(site.declaration);
            surface
                .receiver_applications
                .push(ServiceReceiverApplication {
                    application: surface.identities.intern(&application),
                    trait_name: surface.trait_name,
                    trait_package_identity: None,
                    argument_identities: vec![],
                    lifetime_argument_identities: vec![],
                    requirement_identities: vec![],
                });
            let application = identities.intern(&application);
            module.interfaces.services.push(ServiceDeclaration {
                id: semantic_vocabulary::ServiceId::new(index as u64 + 1).unwrap(),
                identity: application,
                parents: vec![],
            });
            module
                .interfaces
                .service_schemas
                .push(TerminalServiceSchema {
                    declaration,
                    application,
                    surface: std::sync::Arc::new(surface),
                });
        }
        module.identities = identities.build();
        module
    }

    #[test]
    fn canonical_filesystem_services_receive_the_existing_host_supply() {
        let module = canonical_filesystem_module();
        let context = FilesystemContext::new(TerminalFilesystemDelegation::HostNamespace).unwrap();
        assert_eq!(
            context.admit_service_ids(&module, module.interfaces.services.iter().map(|row| row.id)),
            Ok(())
        );
        assert_eq!(
            context.admit_service_ids(
                &module,
                module
                    .interfaces
                    .services
                    .iter()
                    .map(|row| row.id)
                    .chain([semantic_vocabulary::ServiceId::new(99).unwrap()])
            ),
            Err("filesystem supply does not provide this exact additional service")
        );
    }

    #[test]
    fn filesystem_supply_rejects_an_inconsistent_core_schema() {
        let original = canonical_filesystem_module();
        let service = original.interfaces.services[2].id;
        let exact = original
            .identities
            .text(original.interfaces.services[2].identity)
            .unwrap();
        for text in [
            format!("{exact}trailing"),
            exact[..exact.len() - 1].to_owned(),
            "toolchain::FilesystemMetadataQuery::Other".to_owned(),
            "package:foreign::FilesystemMetadataQuery".to_owned(),
        ] {
            let mut module = original.clone();
            let mut identities = std::mem::take(&mut module.identities).into_builder();
            let changed = identities.intern(&text);
            module.identities = identities.build();
            module.interfaces.services[2].identity = changed;
            module.interfaces.service_schemas[2].application = changed;
            let surface =
                std::sync::Arc::make_mut(&mut module.interfaces.service_schemas[2].surface);
            surface.receiver_applications[0].application = surface.identities.intern(&text);
            let context =
                FilesystemContext::new(TerminalFilesystemDelegation::HostNamespace).unwrap();
            assert_eq!(
                context.admit_service_ids(&module, [service]),
                Err("filesystem supply does not provide this exact additional service"),
                "accepted {text}"
            );
        }
    }

    #[test]
    fn a_filesystem_name_without_its_core_schema_supplies_no_authority() {
        let mut module = terminal_psi::TerminalModule::declarations();
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        for (index, label) in [
            "toolchain::FilesystemContentRead",
            "toolchain::FilesystemContentWrite",
            "toolchain::FilesystemMetadataQuery",
            "toolchain::FilesystemMetadataMutation",
            "toolchain::DirectoryEnumeration",
            "toolchain::FilesystemNamespaceMutation",
        ]
        .into_iter()
        .enumerate()
        {
            module
                .interfaces
                .services
                .push(terminal_psi::ServiceDeclaration {
                    id: semantic_vocabulary::ServiceId::new(index as u64 + 1).unwrap(),
                    identity: identities.intern(label),
                    parents: vec![],
                });
        }
        module.identities = identities.build();
        let context = FilesystemContext::new(TerminalFilesystemDelegation::HostNamespace).unwrap();
        for service in &module.interfaces.services {
            assert_eq!(
                context.admit_service_ids(&module, [service.id]),
                Err("filesystem supply does not provide this exact additional service")
            );
        }
    }

    #[test]
    fn scalar_formals_require_known_width_and_preserve_signedness() {
        let signed = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        assert!(scalar_matches(signed, Formal::Signed(32)));
        assert!(!scalar_matches(signed, Formal::Unsigned(32)));
        assert!(!scalar_matches(signed, Formal::Signed(64)));
        let parameter = ScalarType::Integer(IntegerType::parameterized(
            IntegerSign::Signed,
            semantic_vocabulary::WidthParameter {
                owner: semantic_vocabulary::MachineId::new(1).unwrap(),
                value: semantic_vocabulary::ValueId::new(1).unwrap(),
            },
        ));
        assert!(!scalar_matches(parameter, Formal::Signed(32)));
        assert!(!scalar_matches(parameter, Formal::Unsigned(32)));
    }

    fn descriptor(value: i32) -> TerminalScalarValue {
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
            value: IntegerValue::Signed(i128::from(value)),
        }
    }

    fn call(namespace_admission: Result<(), &'static str>) -> LibcFilesystemCall {
        LibcFilesystemCall {
            context: FilesystemContext::new(TerminalFilesystemDelegation::HostNamespace).unwrap(),
            descriptor: DescriptorEffect::Io,
            namespace_admission,
            paths: Vec::new(),
            flags_without_mode: None,
        }
    }

    #[test]
    fn modeled_stream_supply_does_not_authorize_native_descriptor_namespace() {
        let call = call(Err("unprovided extra service"));
        for stream in 0..=2 {
            assert!(call.modeled_stream(&[descriptor(stream)]));
        }
        call.context
            .descriptors
            .borrow_mut()
            .insert(3, NativeDescriptor::Open(91));
        assert!(!call.modeled_stream(&[descriptor(3)]));
        assert!(call.arguments(&[descriptor(3)]).is_err());
        assert!(call.arguments(&[descriptor(-1)]).is_err());
    }

    #[test]
    fn authorized_native_route_translates_only_guest_namespace_descriptors() {
        let call = call(Ok(()));
        call.context
            .descriptors
            .borrow_mut()
            .insert(3, NativeDescriptor::Open(91));
        assert_eq!(call.arguments(&[descriptor(3)]).unwrap(), [descriptor(91)]);
        assert_eq!(
            call.arguments(&[descriptor(999)]).unwrap(),
            [descriptor(-1)]
        );
        for stream in 0..=2 {
            assert!(call.arguments(&[descriptor(stream)]).is_err());
        }
    }

    #[test]
    fn positioned_io_preserves_the_offset_and_requires_its_guest_descriptor() {
        let mut call = call(Ok(()));
        call.descriptor = DescriptorEffect::Use {
            scalar: 0,
            current_directory: false,
        };
        call.context
            .descriptors
            .borrow_mut()
            .insert(3, NativeDescriptor::Open(91));
        let offset = TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Signed, 64).unwrap(),
            value: IntegerValue::Signed(123),
        };
        assert_eq!(
            call.arguments(&[descriptor(3), offset]).unwrap(),
            [descriptor(91), offset]
        );
        for stream in 0..=2 {
            assert!(!call.modeled_stream(&[descriptor(stream), offset]));
            assert!(call.arguments(&[descriptor(stream), offset]).is_err());
        }
        assert_eq!(
            call.arguments(&[descriptor(999), offset]).unwrap(),
            [descriptor(-1), offset]
        );
        call.context
            .descriptors
            .borrow_mut()
            .insert(3, NativeDescriptor::Uncertain);
        assert!(call.arguments(&[descriptor(3), offset]).is_err());
        call.namespace_admission = Err("unprovided extra service");
        assert!(call.arguments(&[descriptor(999), offset]).is_err());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn positioned_imports_require_the_exact_contract_and_filesystem_supply() {
        use language_core::core_vocabulary::FilesystemService;
        use semantic_vocabulary::{BoundaryMachineId, PlaceId, StructuralTypeId};
        use terminal_psi::{
            BoundaryCompletion, BoundaryDeclarationRole, ByteSequenceCarrier,
            NominalDeclarationIdentity, StructuralMultiplicity, StructuralParameterDeclaration,
            StructuralTypeDeclaration,
        };

        let integer =
            |bits| ScalarType::Integer(IntegerType::new(IntegerSign::Signed, bits).unwrap());
        let mut module = canonical_filesystem_module();
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let identity = identities.intern("positioned_io_test");
        module.identities = identities.build();
        let bytes = StructuralTypeId::new(1).unwrap();
        module
            .types
            .structural_types
            .push(StructuralTypeDeclaration {
                id: bytes,
                identity,
                declaration_identity: Default::default(),
                quotient_carrier: None,
                owns_linear_claim: false,
                multiplicity: StructuralMultiplicity::Unrestricted,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                nominal_cleanup: None,
                shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None,
                }),
                scalar_restriction: None,
                requires: Vec::new(),
            });
        let closed = terminal_codec::ClosedTerminalModule::new(module).unwrap();
        let program = {
            let record =
                terminal_codec::build_identity_optimization_execution_record(closed.module())
                    .unwrap();
            let artifact =
                terminal_codec::CanonicalTerminalArtifact::from_closed_parts(closed, record, None)
                    .unwrap();
            TerminalProgram::retain(artifact.retain_code()).unwrap()
        };
        for (symbol, access, reach) in [
            (
                b"_pread".as_slice(),
                StructuralAccess::MutableBorrow,
                FilesystemService::ContentRead,
            ),
            (
                b"_pwrite".as_slice(),
                StructuralAccess::SharedBorrow,
                FilesystemService::ContentWrite,
            ),
        ] {
            let service = program
                .module()
                .interfaces
                .services
                .iter()
                .find(|row| program.module().core_filesystem_service(row.id).ok() == Some(reach))
                .unwrap()
                .id;
            let locator = target::normalize_foreign_locator(
                target::ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                    symbol: symbol.to_vec(),
                },
                target::TargetProfile::MacosArm64,
            )
            .unwrap();
            let signature = calling_conventions::CallSignature {
                parameters: vec![
                    calling_conventions::ValueShape::integer(4, 4),
                    calling_conventions::ValueShape::integer(8, 8),
                    calling_conventions::ValueShape::integer(8, 8),
                    calling_conventions::ValueShape::integer(8, 8),
                ],
                result: Some(calling_conventions::ValueShape::integer(8, 8)),
            };
            let binding = NormalizedForeignCallBinding {
                boundary_entry_plan: calling_conventions::evaluate_ordinary_boundary_entry_plan(
                    calling_conventions::CallingPolicy::DarwinAapcs64,
                    &signature,
                )
                .unwrap()
                .plan()
                .clone(),
                mechanism: target_vocabulary::NormalizedForeignCallMechanism::BuildImport {
                    stack: target_vocabulary::NativeBindingStack::Hosted,
                    locator,
                    receipt: [1; 32],
                    requirement_identity: "positioned filesystem test".into(),
                },
            };
            let declaration = BoundaryMachineDeclaration {
                id: BoundaryMachineId::new(1).unwrap(),
                declaration: None,
                numeric_intrinsic: None,
                role: BoundaryDeclarationRole::Callable,
                kind: semantic_vocabulary::BoundaryRequirementKind::Machine,
                completion: BoundaryCompletion::NormalReturn,
                unconditional_termination: false,
                identity,
                owner: NominalDeclarationIdentity::new(identity),
                attachment: None,
                parameter_order: vec![
                    BoundaryParameterKind::Scalar,
                    BoundaryParameterKind::Structural,
                    BoundaryParameterKind::Scalar,
                ],
                scalar_parameters: vec![integer(32), integer(64)],
                crash_routes: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: PlaceId::new(1).unwrap(),
                    position: 1,
                    structural_type: bytes,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                }],
                result: BoundaryMachineResult::Scalar(integer(64).into()),
                result_reference_regions: Vec::new(),
                result_lifetimes: None,
                requires: Vec::new(),
                normal_qualifications: Vec::new(),
                scalar_domain_requirements: Vec::new(),
                membership_ensures: Vec::new(),
                scalar_requires: Vec::new(),
                scalar_ensures: Vec::new(),
                reference_outputs: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                fixed_service_reach: vec![service],
                service_reach_is_installation_bound: false,
                owning_service: None,
                published_service_ceiling: Vec::new(),
                published_synchronous_entries: Vec::new(),
            };
            let context =
                FilesystemContext::new(TerminalFilesystemDelegation::HostNamespace).unwrap();
            let admitted = context
                .admit(&program, &declaration, &binding, &[])
                .map(|call| matches!(call, Some(FilesystemCall::Libc(_))));
            assert!(matches!(admitted, Ok(true)), "{symbol:?}: {admitted:?}");
            let unavailable =
                FilesystemContext::new(TerminalFilesystemDelegation::Unprovided).unwrap();
            assert!(
                unavailable
                    .admit(&program, &declaration, &binding, &[])
                    .is_err()
            );
            for changed in 0..4 {
                let mut invalid = declaration.clone();
                match changed {
                    0 => invalid.scalar_parameters[0] = integer(64),
                    1 => invalid.scalar_parameters[1] = integer(32),
                    2 => invalid.structural_parameters[0].access = StructuralAccess::Owned,
                    3 => invalid.result = BoundaryMachineResult::Scalar(integer(32).into()),
                    _ => unreachable!(),
                }
                assert!(context.admit(&program, &invalid, &binding, &[]).is_err());
            }
        }
    }
}
