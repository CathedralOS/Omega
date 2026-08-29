//! Responsibility-mirrored resolver regression suite.

use super::*;
use crate::source::identity::PackageName;
use std::time::{SystemTime, UNIX_EPOCH};

mod custody;
mod git;
mod local;
mod objects;
mod process;
mod request;
mod support;

use support::*;
