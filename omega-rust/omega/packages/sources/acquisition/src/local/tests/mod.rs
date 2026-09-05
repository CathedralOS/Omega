//! Local-source behavior tests grouped by resolver responsibility.

use super::{model::*, operations::*, snapshot::*};
use crate::error::SourceResolveError;
use crate::identity::*;
use crate::limits::*;
use crate::snapshot::permissions::*;
use crate::test_support::*;
use crate::tree::capture::*;
#[cfg(unix)]
use crate::tree::filesystem::*;
#[cfg(unix)]
use cap_std::ambient_authority;
#[cfg(unix)]
use cap_std::fs::Dir as CapabilityDirectory;
use std::collections::BTreeSet;
#[cfg(unix)]
use std::ffi::OsStr;
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

mod capture;
mod identity;
mod limits;
mod lock_file;
mod observation;
mod path_policy;
mod snapshots;
mod staging;
