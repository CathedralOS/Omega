//! Explicit matching-host filesystem supply and its descriptor namespace.
//! The native worker's channel and mapping files are never guest descriptors.
//! Exact OS mechanisms select adapters; source requirement names do not.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{TerminalEffectRejection, TerminalProgram, TerminalScalarValue};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
    StructuralTypeShape,
};

use super::{TerminalRuntimeError, native_provider::rejection};
use crate::execution::TerminalFilesystemDelegation;

#[derive(Clone)]
pub(super) struct FilesystemContext {
    supplied: bool,
    descriptors: Rc<RefCell<BTreeMap<i32, i32>>>,
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

pub(super) struct FilesystemCall {
    context: FilesystemContext,
    descriptor: DescriptorEffect,
    paths: Vec<usize>,
    flags_without_mode: Option<usize>,
}

#[derive(Clone, Copy)]
enum Formal {
    Signed(u16),
    Unsigned(u16),
    Bytes(u64, StructuralAccess),
}

impl FilesystemContext {
    pub(super) fn new(delegation: TerminalFilesystemDelegation) -> Self {
        Self {
            supplied: matches!(delegation, TerminalFilesystemDelegation::HostNamespace),
            descriptors: Rc::new(RefCell::new(BTreeMap::new())),
        }
    }

    pub(super) fn admit(
        &self,
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
    ) -> Result<Option<FilesystemCall>, TerminalRuntimeError> {
        let Some((locator, _)) = binding.import() else {
            return Ok(None);
        };
        if !matches!(locator.target(), target::TargetProfile::MacosArm64)
            || !cfg!(all(target_os = "macos", target_arch = "aarch64"))
        {
            return Ok(None);
        }
        use DescriptorEffect::{Acquire, Close, Duplicate, None as NoDescriptor, OpenAt, Use};
        use Formal::{Bytes, Signed, Unsigned};
        use StructuralAccess::{MutableBorrow, SharedBorrow};
        // The SDK declares full fixed-array extents. These are exact C ABI
        // contracts, not a license to widen a one-byte loan into an allocation.
        let path = Bytes(4096, SharedBorrow);
        let metadata = Bytes(144, MutableBorrow);
        let candidates: &[(&[u8], &[Formal], Formal, DescriptorEffect)] = &[
            (b"open", &[path, Signed(32)], Signed(32), Acquire),
            (
                b"__open",
                &[path, Signed(32), Signed(32)],
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
        ];
        let mut selected = None;
        for &(symbol, parameters, result, descriptor) in candidates {
            if super::matches_platform_c_symbol(
                binding,
                symbol,
                &[b"/usr/lib/libSystem.B.dylib"],
                None,
            )? {
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
        // Only exact toolchain-owned filesystem services are supplied. A
        // similarly named package declaration or unrelated extra reach rejects.
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
                return Err(TerminalRuntimeError::missing(
                    "runtime-filesystem",
                    "filesystem supply does not provide this exact additional service",
                ));
            }
        }
        if parameters
            .is_some_and(|parameters| !matches_signature(program, declaration, parameters, result))
        {
            return Err(TerminalRuntimeError::InvalidProvision(
                "filesystem import differs from its complete OS contract",
            ));
        }
        let flags_without_mode = match (descriptor, parameters) {
            (Acquire, Some([Bytes(4096, SharedBorrow), Signed(32)])) => Some(0),
            (OpenAt, _) => Some(1),
            _ => None,
        };
        let mut structural = 0;
        let mut paths = Vec::new();
        for formal in parameters.into_iter().flatten() {
            if let Bytes(count, access) = formal {
                if *count == 4096 && *access == SharedBorrow {
                    paths.push(structural);
                }
                structural += 1;
            }
        }
        Ok(Some(FilesystemCall {
            context: self.clone(),
            descriptor,
            paths,
            flags_without_mode,
        }))
    }
}

fn matches_signature(
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    expected: &[Formal],
    result: Formal,
) -> bool {
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
                    (Some(value), Formal::Bytes(count, access)) if value.access == *access => {
                        match program
                            .structural_type(value.structural_type)
                            .map(|row| &row.shape)
                        {
                            Some(StructuralTypeShape::FixedArray { element, length })
                                if length.literal() == Some(*count) =>
                            {
                                matches!(program.structural_type(*element).map(|row| &row.shape), Some(StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))) if integer.sign() == IntegerSign::Unsigned && integer.bits() == 8)
                            }
                            _ => false,
                        }
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
        && matches!(declaration.result, BoundaryMachineResult::Scalar(value) if scalar_matches(value.scalar_type, result))
}

impl FilesystemCall {
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
        // Darwin AT_FDCWD is an explicitly supplied current-directory root.
        // Other unknown integers reach the actual OS as an invalid descriptor,
        // so errno comes from the selected native call, never an invented code.
        *guest = i128::from(if current_directory && guest_descriptor == -2 {
            -2
        } else {
            self.context
                .descriptors
                .borrow()
                .get(&guest_descriptor)
                .copied()
                .unwrap_or(-1)
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
                descriptors.insert(guest, native);
                *value = i128::from(guest);
            }
            DescriptorEffect::Close if *value == 0 => {
                let Some(TerminalScalarValue::Integer {
                    value: IntegerValue::Signed(guest),
                    ..
                }) = arguments.first()
                else {
                    return Err(rejection("closed descriptor lost its guest identity"));
                };
                let guest = i32::try_from(*guest)
                    .map_err(|_| rejection("closed descriptor exceeds its carrier"))?;
                self.context.descriptors.borrow_mut().remove(&guest);
            }
            _ => {}
        }
        Ok(result)
    }
}
