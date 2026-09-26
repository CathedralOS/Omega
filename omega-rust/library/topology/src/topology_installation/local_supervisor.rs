//! The unix leg of the OS-backed supervisor: real process custody for the
//! installed roster.
//!
//! Where `topology_installation` sequences admission, gate, and mediation,
//! this module is what actually owns a member on a unix host:
//!
//! - **Executable admission** is by exact image digest: `admit_executable`
//!   hashes the image file, and `prepare_member` re-hashes it at spawn time
//!   — an image swapped after admission refuses before the process exists.
//!   The roster's `AdmittedArtifact.artifact` is that digest; the
//!   supervisor's registry is the installed-artifact store that resolves it
//!   to an image plus launch settings.
//! - **Confinement** rides the neutral process boundary. Each member spawns
//!   inside a `bounded-process` container (process-group custody, resource
//!   limits, owned reaping) with ambient descriptors closed on exec — only
//!   the member's assigned pipe ends and its one report channel are
//!   retained, each at its parent-assigned descriptor number. General
//!   handle inheritance stays disabled: the retained set is the complete
//!   inheritance contract, written into the child's environment as
//!   `OMEGA_TOPOLOGY_ENDPOINTS`/`OMEGA_TOPOLOGY_REPORTS` before exec.
//! - **The gate** is literal: the member echoes the kernel token of every
//!   descriptor it actually holds on its report channel, then blocks on the
//!   command pipe until `open_entry_gate` delivers `ENTER`. A member whose
//!   held ends diverge from the assignment is caught by the installer's
//!   echo check before any entry opens.
//! - **Mediation** uses those same kernel tokens: a channel operation is
//!   granted by `authorize_send`/`authorize_respond` against the token the
//!   member's custody attests — an instance number is never presented.
//!
//! Provider assumptions this leg discloses rather than proves: the install
//! echo is inside the admitted image's contract (a member that refuses to
//! complete it blocks at the gate and quiesce still owns it), the digest
//! re-check and spawn are not atomic against a racing filesystem writer,
//! and the host loader runs whatever the admitted image's entry does.

use super::{
    AdmittedArtifact, EndpointAssignment, EndpointId, InstallationLifecycle, PreparationEcho,
    ProcessSupervisor, RetainedMember, StdPipeEnd,
};
use crate::deployment_plan::{Identity, InstanceName, PlanInstance};
use bounded_process::{BoundedProcessChild, BoundedProcessLimits, BoundedProcessPrepared};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::{AsFd, AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command};
use std::time::{Duration, Instant};

/// The member's report-channel descriptor, named in its environment — the
/// one retained end outside the assigned endpoint set.
const REPORTS_ENV: &str = "OMEGA_TOPOLOGY_REPORTS";
/// `id:fd` assignments, comma-separated, at their child-side numbers.
const ENDPOINTS_ENV: &str = "OMEGA_TOPOLOGY_ENDPOINTS";
/// The member's roster name, for the image's own diagnostics.
const MEMBER_ENV: &str = "OMEGA_TOPOLOGY_MEMBER";
/// Report the member writes once its entry is running post-gate.
const READY_LINE: &str = "READY";
const ECHO_LINE_PREFIX: &str = "ECHO ";
const ECHO_DONE_LINE: &str = "ECHO-DONE";

/// One admitted executable image and its launch settings. The launch
/// details are provider admission policy — how the image is entered — not
/// plan data.
#[derive(Debug, Clone)]
pub struct ExecutableLaunch {
    /// The executable image on disk; re-hashed at every spawn.
    pub program: PathBuf,
    /// Arguments the image's entry consumes.
    pub arguments: Vec<OsString>,
    /// Extra environment entries the supervisor sets on the member.
    pub environment: Vec<(OsString, OsString)>,
}

/// The member's custody record: its process container, the supervisor's
/// command channel, its report channel, and the kernel tokens it attested
/// at the gate.
pub struct LocalMember {
    /// Roster name, for diagnostics and custody records.
    name: InstanceName,
    child: BoundedProcessChild,
    /// Supervisor→member command channel (`None` once quiesce takes it —
    /// dropping it sends EOF to a member draining its input).
    commands: Option<ChildStdin>,
    /// Member→supervisor report channel: echo, gate, and verdicts.
    reports: BufReader<std::io::PipeReader>,
    /// Every end the member's install handshake reported, keyed by issued
    /// endpoint id: the child-side descriptor number and the physical token
    /// the member's kernel measured for it.
    installed: BTreeMap<EndpointId, InstalledEnd>,
}

impl std::fmt::Debug for LocalMember {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalMember")
            .field("name", &self.name)
            .field("installed", &self.installed)
            .finish()
    }
}

/// One end as the member attested it.
#[derive(Debug, Clone, Copy)]
struct InstalledEnd {
    descriptor: RawFd,
    token: u64,
}

/// Why the local supervisor refused a member operation.
#[derive(Debug)]
pub enum LocalSupervisorError {
    /// No executable image was admitted for the roster entry's artifact.
    UnadmittedExecutable { member: InstanceName },
    /// The image on disk no longer matches the admitted digest — the
    /// executable was swapped after admission.
    ExecutableImageMismatch { member: InstanceName },
    /// The member's install handshake or control protocol failed. Custody
    /// of the member is unchanged — quiesce still owns it — unless this
    /// came from `prepare_member`, in which case the member never existed.
    Protocol {
        member: InstanceName,
        detail: String,
    },
    /// The process boundary or pipe plumbing itself failed.
    Io(io::Error),
}

impl std::fmt::Display for LocalSupervisorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnadmittedExecutable { member } => {
                write!(formatter, "no admitted executable for member {member}")
            }
            Self::ExecutableImageMismatch { member } => write!(
                formatter,
                "member {member}'s image no longer matches its admission"
            ),
            Self::Protocol { member, detail } => {
                write!(formatter, "member {member} protocol failure: {detail}")
            }
            Self::Io(error) => write!(formatter, "process custody I/O failed: {error}"),
        }
    }
}

impl std::error::Error for LocalSupervisorError {}

impl From<io::Error> for LocalSupervisorError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// The kernel's verdict on one descriptor write a member attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteVerdict {
    /// The bytes were accepted by the channel.
    Written,
    /// The member's install map holds no descriptor for the endpoint.
    NotHeld,
    /// The kernel refused — the raw `errno` (EBADF for a descriptor the
    /// member never held, EPIPE for a dead peer).
    Refused(i32),
}

/// What the member observed reading one of its descriptors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadVerdict {
    /// Bytes the channel delivered.
    Bytes(Vec<u8>),
    /// The peer's write end closed — a peer-failure witness.
    EndOfFile,
    /// The member's install map holds no descriptor for the endpoint.
    NotHeld,
    /// The kernel refused the read.
    Refused(i32),
}

/// The unix process supervisor: admits executable images by digest, spawns
/// each member in a bounded-process container with exactly its assigned
/// descriptors retained, holds entry behind the installation gate, and
/// mediates frames on the kernel-attested tokens the members echoed.
pub struct LocalProcessSupervisor {
    lifecycle: InstallationLifecycle,
    /// The installed-artifact store: artifact identity → executable launch.
    executables: BTreeMap<Identity, ExecutableLaunch>,
    /// Resource limits every member runs under — custody policy set by the
    /// provider, not the plan.
    member_limits: BoundedProcessLimits,
    /// Provider hook applied to each member's command before spawn —
    /// per-member arguments and environment a deployment needs beyond the
    /// topology protocol itself.
    configure_member: Option<Box<dyn FnMut(&mut BoundedProcessPrepared, &PlanInstance)>>,
}

impl Default for LocalProcessSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalProcessSupervisor {
    pub fn new() -> Self {
        Self {
            lifecycle: InstallationLifecycle::default(),
            executables: BTreeMap::new(),
            // Permissive-but-real custody bounds for a member: the
            // container's job here is confinement and reaping, not resource
            // starvation policy.
            member_limits: BoundedProcessLimits::new(
                300,
                8 << 30,
                256 << 20,
                256,
                8192,
                1 << 30,
                2 << 30,
            ),
            configure_member: None,
        }
    }

    /// Admit an executable image: the artifact identity is the digest of
    /// its exact bytes. The returned identity is what the owner's
    /// `AdmittedArtifact.artifact` must name for members on this image.
    pub fn admit_executable(&mut self, image: impl Into<PathBuf>) -> io::Result<Identity> {
        let program = image.into();
        let artifact = digest_image(&program)?;
        self.executables.insert(
            artifact,
            ExecutableLaunch {
                program,
                arguments: Vec::new(),
                environment: Vec::new(),
            },
        );
        Ok(artifact)
    }

    /// The launch record for an admitted artifact — provider policy for how
    /// the image is entered (arguments, environment), applied identically
    /// to every member bound to that image.
    pub fn launch(&mut self, artifact: &Identity) -> Option<&mut ExecutableLaunch> {
        self.executables.get_mut(artifact)
    }

    /// A provider hook run while each member's command is prepared. It may
    /// add arguments, environment, and supervision plumbing for that member
    /// alone; assigned endpoint ends are never addable this way — custody
    /// of ends stays with the installer's assignment.
    pub fn configure_member(
        &mut self,
        configure: impl FnMut(&mut BoundedProcessPrepared, &PlanInstance) + 'static,
    ) {
        self.configure_member = Some(Box::new(configure));
    }

    /// Replace the default member resource limits.
    pub fn set_member_limits(&mut self, limits: BoundedProcessLimits) {
        self.member_limits = limits;
    }

    /// The member's roster name.
    pub fn member_name(member: &LocalMember) -> &InstanceName {
        &member.name
    }

    /// Mutable access to the issuance record — owner intent is authorized
    /// through it before any installation may activate. The installer
    /// itself only ever reads it.
    pub fn lifecycle_mut(&mut self) -> &mut InstallationLifecycle {
        &mut self.lifecycle
    }

    /// The physical token the member attested for `endpoint` at the gate —
    /// the value `authorize_send`/`authorize_respond` compare against the
    /// route table. Custody evidence the member's own kernel produced, not
    /// a number any caller supplies.
    pub fn attested_token(&self, member: &LocalMember, endpoint: EndpointId) -> Option<u64> {
        member.installed.get(&endpoint).map(|end| end.token)
    }

    /// Whether the member's process is still running.
    pub fn member_running(&mut self, member: &mut LocalMember) -> io::Result<bool> {
        Ok(member.child.try_wait()?.is_none())
    }

    /// Force the member's process group down without draining — the peer
    /// failure a supervisor reports when a member dies uncommanded. The
    /// member stays in custody; `quiesce_member` still owns its release.
    pub fn force_down(&mut self, member: &mut LocalMember) -> io::Result<()> {
        member.child.terminate()
    }

    /// The member's self-reported live descriptor table — the confinement
    /// witness that no ambient descriptor survived exec. `FDS-OK` means the
    /// member's table is exactly its retained set plus the transient ends
    /// its own measurement opened; anything else names the extras.
    pub fn inspect_descriptors(
        &mut self,
        member: &mut LocalMember,
    ) -> Result<String, LocalSupervisorError> {
        self.round_trip(member, "FDS".to_owned())
    }

    /// Instruct the member to write `bytes` raw on the descriptor it holds
    /// for `endpoint` — bytes actually cross the kernel channel, not a
    /// simulated hop.
    pub fn write_endpoint(
        &mut self,
        member: &mut LocalMember,
        endpoint: EndpointId,
        bytes: &[u8],
    ) -> Result<WriteVerdict, LocalSupervisorError> {
        let Some(end) = member.installed.get(&endpoint) else {
            return Ok(WriteVerdict::NotHeld);
        };
        self.write_descriptor(member, end.descriptor, bytes)
    }

    /// Instruct the member to write `bytes` on descriptor `descriptor` —
    /// which may be a number the member never received. The kernel's own
    /// verdict comes back.
    pub fn write_descriptor(
        &mut self,
        member: &mut LocalMember,
        descriptor: RawFd,
        bytes: &[u8],
    ) -> Result<WriteVerdict, LocalSupervisorError> {
        let reply = self.round_trip(
            member,
            format!("WRITE-FD {descriptor} {}", hex_encode(bytes)),
        )?;
        Ok(match reply.as_str() {
            "WROTE" => WriteVerdict::Written,
            "NO-END" => WriteVerdict::NotHeld,
            _ => reply
                .strip_prefix("IO-ERR ")
                .and_then(|code| code.parse::<i32>().ok())
                .map(WriteVerdict::Refused)
                .unwrap_or(WriteVerdict::NotHeld),
        })
    }

    /// Instruct the member to read at most `cap` bytes once from the
    /// descriptor it holds for `endpoint`. A closed peer reports EOF.
    pub fn read_endpoint(
        &mut self,
        member: &mut LocalMember,
        endpoint: EndpointId,
        cap: usize,
    ) -> Result<ReadVerdict, LocalSupervisorError> {
        let Some(end) = member.installed.get(&endpoint) else {
            return Ok(ReadVerdict::NotHeld);
        };
        let reply = self.round_trip(member, format!("READ-FD {} {cap}", end.descriptor))?;
        Ok(match reply.as_str() {
            "EOF" => ReadVerdict::EndOfFile,
            "NO-END" => ReadVerdict::NotHeld,
            _ => {
                if let Some(hex) = reply.strip_prefix("BYTES ") {
                    ReadVerdict::Bytes(hex_decode(hex))
                } else if let Some(code) = reply.strip_prefix("IO-ERR ") {
                    ReadVerdict::Refused(code.parse().unwrap_or(-1))
                } else {
                    ReadVerdict::NotHeld
                }
            }
        })
    }

    /// Ask the member's kernel what physical token a descriptor carries —
    /// used to probe what an end *actually* is, not what was claimed.
    pub fn probe_descriptor(
        &mut self,
        member: &mut LocalMember,
        descriptor: RawFd,
    ) -> Result<Option<u64>, LocalSupervisorError> {
        let reply = self.round_trip(member, format!("TOKEN-FD {descriptor}"))?;
        Ok(reply
            .strip_prefix("TOKEN ")
            .and_then(|token| token.parse().ok()))
    }

    /// One command/response exchange on the member's control channels: a
    /// line on its stdin, one verdict line back on the report pipe.
    fn round_trip(
        &mut self,
        member: &mut LocalMember,
        command: String,
    ) -> Result<String, LocalSupervisorError> {
        let name = member.name.clone();
        let commands = member
            .commands
            .as_mut()
            .ok_or_else(|| LocalSupervisorError::Protocol {
                member: name.clone(),
                detail: "command channel already closed".to_owned(),
            })?;
        if commands
            .write_all(command.as_bytes())
            .and_then(|()| commands.write_all(b"\n"))
            .and_then(|()| commands.flush())
            .is_err()
        {
            return Err(LocalSupervisorError::Protocol {
                member: name,
                detail: "command channel is closed — the member process is gone".to_owned(),
            });
        }
        let mut line = String::new();
        match member.reports.read_line(&mut line) {
            Ok(0) | Err(_) => Err(LocalSupervisorError::Protocol {
                member: name,
                detail: "report channel closed before a verdict arrived".to_owned(),
            }),
            Ok(_) => Ok(line.trim_end().to_owned()),
        }
    }

    /// Read one report line while the member is still installing; a member
    /// that already exited is reported, not waited on.
    fn read_install_line(member: &mut LocalMember) -> Result<String, LocalSupervisorError> {
        if let Ok(Some(_)) = member.child.try_wait() {
            return Err(LocalSupervisorError::Protocol {
                member: member.name.clone(),
                detail: "member exited before completing its install handshake".to_owned(),
            });
        }
        let mut line = String::new();
        match member.reports.read_line(&mut line) {
            Ok(0) => Err(LocalSupervisorError::Protocol {
                member: member.name.clone(),
                detail: "report channel closed during installation".to_owned(),
            }),
            Err(error) => Err(LocalSupervisorError::Io(error)),
            Ok(_) => Ok(line.trim_end().to_owned()),
        }
    }
}

impl ProcessSupervisor for LocalProcessSupervisor {
    type Endpoint = StdPipeEnd;
    type Member = LocalMember;
    type Error = LocalSupervisorError;

    fn installation_lifecycle(&self) -> &InstallationLifecycle {
        &self.lifecycle
    }

    fn prepare_member(
        &mut self,
        instance: &PlanInstance,
        artifact: &AdmittedArtifact,
        endpoints: Vec<EndpointAssignment<Self::Endpoint>>,
    ) -> Result<PreparationEcho<Self::Member>, Self::Error> {
        // Executable admission: resolve the artifact identity to an image
        // and re-hash the image on disk — a swapped executable refuses
        // before any process exists.
        let Some(launch) = self.executables.get(&artifact.artifact) else {
            return Err(LocalSupervisorError::UnadmittedExecutable {
                member: instance.name.clone(),
            });
        };
        let launch = launch.clone();
        if digest_image(&launch.program)? != artifact.artifact {
            return Err(LocalSupervisorError::ExecutableImageMismatch {
                member: instance.name.clone(),
            });
        }

        // The member's report channel: a private pipe outside the assigned
        // set, retained alongside the ends and named in the environment.
        let (reports_read, reports_write) = io::pipe()?;

        let mut command = Command::new(&launch.program);
        command.args(&launch.arguments);
        let mut prepared =
            BoundedProcessPrepared::new(command, self.member_limits, "topology member")?;
        prepared.stdin_piped().stdout_null().stderr_null();
        prepared.env(MEMBER_ENV, instance.name.as_str());
        prepared.env(REPORTS_ENV, reports_write.as_raw_fd().to_string());
        for (key, value) in &launch.environment {
            prepared.env(key, value);
        }
        let descriptors: Vec<(EndpointId, RawFd)> = endpoints
            .iter()
            .map(|assignment| (assignment.id, assignment.handle.as_fd().as_raw_fd()))
            .collect();
        prepared.env(
            ENDPOINTS_ENV,
            descriptors
                .iter()
                .map(|(id, descriptor)| format!("{}:{descriptor}", id.0))
                .collect::<Vec<_>>()
                .join(","),
        );
        prepared.retain_descriptor(&reports_write);
        for assignment in &endpoints {
            prepared.retain_descriptor(&assignment.handle);
        }
        if let Some(configure) = &mut self.configure_member {
            configure(&mut prepared, instance);
        }

        let mut child = BoundedProcessChild::spawn(prepared)?;
        // Parent custody of the assigned ends and the report-write end ends
        // here: the child's retained copies are the only live ones.
        drop(reports_write);
        drop(endpoints);

        let commands = child
            .take_stdin()
            .ok_or_else(|| LocalSupervisorError::Protocol {
                member: instance.name.clone(),
                detail: "the member's command pipe was not established".to_owned(),
            })?;
        let mut member = LocalMember {
            name: instance.name.clone(),
            child,
            commands: Some(commands),
            reports: BufReader::new(reports_read),
            installed: BTreeMap::new(),
        };

        // The member's install handshake: it reports the kernel token of
        // every descriptor it actually holds before the gate can open.
        let mut echoed = BTreeMap::new();
        loop {
            let line = Self::read_install_line(&mut member)?;
            if line == ECHO_DONE_LINE {
                break;
            }
            let Some(rest) = line.strip_prefix(ECHO_LINE_PREFIX) else {
                return Err(LocalSupervisorError::Protocol {
                    member: instance.name.clone(),
                    detail: format!("unexpected install report: {line:?}"),
                });
            };
            let mut fields = rest.split(' ');
            let (Some(id), Some(token)) = (fields.next(), fields.next()) else {
                return Err(LocalSupervisorError::Protocol {
                    member: instance.name.clone(),
                    detail: format!("malformed install report: {line:?}"),
                });
            };
            let (Ok(id), Ok(token)) = (id.parse::<u32>(), token.parse::<u64>()) else {
                return Err(LocalSupervisorError::Protocol {
                    member: instance.name.clone(),
                    detail: format!("malformed install report: {line:?}"),
                });
            };
            echoed.insert(EndpointId(id), token);
        }
        member.installed = descriptors
            .iter()
            .map(|(id, descriptor)| {
                (
                    *id,
                    InstalledEnd {
                        descriptor: *descriptor,
                        token: echoed.get(id).copied().unwrap_or(0),
                    },
                )
            })
            .collect();
        Ok(PreparationEcho {
            member,
            installed: echoed.into_iter().collect(),
        })
    }

    fn open_entry_gate(&mut self, member: &mut Self::Member) -> Result<(), Self::Error> {
        let reply = self.round_trip(member, "ENTER".to_owned())?;
        if reply == READY_LINE {
            Ok(())
        } else {
            Err(LocalSupervisorError::Protocol {
                member: member.name.clone(),
                detail: format!("gate opened but member answered {reply:?}"),
            })
        }
    }

    fn quiesce_member(
        &mut self,
        mut member: Self::Member,
    ) -> Result<(), RetainedMember<Self::Member, Self::Error>> {
        match quiesce(&mut member) {
            Ok(()) => Ok(()),
            Err(error) => Err(RetainedMember { member, error }),
        }
    }
}

/// Stop one member: a polite QUIT on its command channel, a bounded wait,
/// then the container's group kill and reap. Either way the child's exit
/// status is collected; a member that cannot be reaped is reported
/// retained — never silently dropped.
fn quiesce(member: &mut LocalMember) -> Result<(), LocalSupervisorError> {
    // Let the member drain on its own command first, then close the command
    // channel so a wedged reader sees EOF either way.
    if let Some(mut commands) = member.commands.take() {
        let _ = commands
            .write_all(b"QUIT\n")
            .and_then(|()| commands.flush());
        drop(commands);
    }
    let drain_deadline = Instant::now() + Duration::from_millis(500);
    loop {
        if member.child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= drain_deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // The container kill is idempotent for an already-exited member and
    // closes descendants still in the group.
    member.child.terminate()?;
    let reap_deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if member.child.try_wait()?.is_some() {
            return Ok(());
        }
        if Instant::now() >= reap_deadline {
            return Err(LocalSupervisorError::Protocol {
                member: member.name.clone(),
                detail: "member did not reap after container termination".to_owned(),
            });
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Hash the image file — the artifact identity admission binds.
fn digest_image(program: &Path) -> io::Result<Identity> {
    let mut digest = Sha256::new();
    let mut image = std::fs::File::open(program)?;
    io::copy(&mut image, &mut digest)?;
    Ok(digest.finalize().into())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}

fn hex_decode(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks(2)
        .filter_map(|pair| std::str::from_utf8(pair).ok())
        .filter_map(|pair| u8::from_str_radix(pair, 16).ok())
        .collect()
}
