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
use target_operations::NormalizedForeignCallBinding;
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
    supplied: bool,
    windows: Rc<RefCell<windows::Handles>>,
    linux: Rc<RefCell<native_provider_runtime::LinuxFilesystem>>,
    descriptors: Rc<RefCell<BTreeMap<i32, NativeDescriptor>>>,
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

pub(super) struct DarwinFilesystemCall {
    context: FilesystemContext,
    descriptor: DescriptorEffect,
    namespace_admission: Result<(), &'static str>,
    paths: Vec<usize>,
    flags_without_mode: Option<usize>,
}

pub(super) enum FilesystemCall {
    Darwin(DarwinFilesystemCall),
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
            Self::Darwin(_) => Err(rejection(
                "this filesystem supply has no readonly native record translation",
            )),
        }
    }
    pub(super) fn path_argument(&self, argument: usize) -> bool {
        match self {
            Self::Darwin(call) => call.path_argument(argument),
            Self::Windows(call) => call.path_argument(argument),
        }
    }
    pub(super) fn validate_path(
        &self,
        argument: usize,
        bytes: &[u8],
    ) -> Result<(), TerminalEffectRejection> {
        match self {
            Self::Darwin(call) => call.validate_path(argument, bytes),
            Self::Windows(call) => call.validate_path(argument, bytes),
        }
    }
    pub(super) fn descriptor_io(&self) -> bool {
        matches!(self, Self::Darwin(call) if call.descriptor_io())
    }
    pub(super) fn modeled_stream(&self, arguments: &[TerminalScalarValue]) -> bool {
        matches!(self, Self::Darwin(call) if call.modeled_stream(arguments))
    }
    pub(super) fn arguments(
        &self,
        arguments: &[TerminalScalarValue],
    ) -> Result<Vec<TerminalScalarValue>, TerminalEffectRejection> {
        match self {
            Self::Darwin(call) => call.arguments(arguments),
            Self::Windows(call) => call.arguments(arguments),
        }
    }
    pub(super) fn result(
        &self,
        arguments: &[TerminalScalarValue],
        result: TerminalScalarValue,
    ) -> Result<TerminalScalarValue, TerminalEffectRejection> {
        match self {
            Self::Darwin(call) => call.result(arguments, result),
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
            Self::Darwin(_) => Ok(None),
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
    pub(super) fn new(delegation: TerminalFilesystemDelegation) -> Self {
        Self {
            supplied: matches!(delegation, TerminalFilesystemDelegation::HostNamespace),
            windows: Rc::new(RefCell::new(windows::Handles::default())),
            descriptors: Rc::new(RefCell::new(BTreeMap::new())),
            linux: Rc::new(RefCell::new(
                native_provider_runtime::LinuxFilesystem::default(),
            )),
        }
    }

    pub(super) fn admit(
        &self,
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        layouts: &[NativeBorrowedStorageLayout],
    ) -> Result<Option<FilesystemCall>, TerminalRuntimeError> {
        if let Some(call) = windows::admit(self, program, declaration, binding, layouts)? {
            return Ok(Some(FilesystemCall::Windows(call)));
        }
        let Some((locator, _)) = binding.import() else {
            return Ok(None);
        };
        if !matches!(locator.target(), target::TargetProfile::MacosArm64)
            || !cfg!(all(target_os = "macos", target_arch = "aarch64"))
        {
            return Ok(None);
        }
        use DescriptorEffect::{Acquire, Close, Duplicate, None as NoDescriptor, OpenAt, Use};
        use Formal::{ByteView, IntegerReference, Signed, Unsigned, UnsignedArray};
        use StructuralAccess::{MutableBorrow, SharedBorrow};
        let path = UnsignedArray(4096, 8, SharedBorrow);
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
            (b"unlink", &[path], Signed(32), NoDescriptor),
            (b"rmdir", &[path], Signed(32), NoDescriptor),
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
        for &(symbol, parameters, result, descriptor) in candidates {
            if matches_platform_c_symbol(binding, symbol, &[b"/usr/lib/libSystem.B.dylib"], None)? {
                selected = Some((Some(parameters), result, descriptor));
                break;
            }
        }
        if self.supplied
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
        if !self.supplied {
            return Err(TerminalRuntimeError::missing(
                "runtime-filesystem",
                "no explicit host filesystem namespace was supplied",
            ));
        }
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
        Ok(Some(FilesystemCall::Darwin(DarwinFilesystemCall {
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
        for service in declaration
            .fixed_service_reach
            .iter()
            .chain(&declaration.published_service_ceiling)
        {
            if Some(*service) == declaration.owning_service {
                continue;
            }
            let supplied = program
                .module()
                .services
                .iter()
                .find(|row| row.id == *service)
                .and_then(|row| program.module().identities.text(row.identity))
                .is_some_and(|identity| {
                    matches!(
                        identity,
                        "toolchain::FilesystemContentRead"
                            | "toolchain::FilesystemContentWrite"
                            | "toolchain::FilesystemMetadataQuery"
                            | "toolchain::FilesystemMetadataMutation"
                            | "toolchain::DirectoryEnumeration"
                            | "toolchain::FilesystemNamespaceMutation"
                    )
                });
            if !supplied {
                return Err("filesystem supply does not provide this exact additional service");
            }
        }
        Ok(())
    }
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
            integer.sign() == IntegerSign::Signed && integer.bits() == bits
        }
        (ScalarType::Integer(integer), Formal::Unsigned(bits)) => {
            integer.sign() == IntegerSign::Unsigned && integer.bits() == bits
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

impl DarwinFilesystemCall {
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

    fn descriptor(value: i32) -> TerminalScalarValue {
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
            value: IntegerValue::Signed(i128::from(value)),
        }
    }

    fn call(namespace_admission: Result<(), &'static str>) -> DarwinFilesystemCall {
        DarwinFilesystemCall {
            context: FilesystemContext::new(TerminalFilesystemDelegation::HostNamespace),
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
}
