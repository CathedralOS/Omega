//! The three-process leg of the private-pipe installation contract.
//!
//! `installation.rs`'s `SimSupervisor` exercises custody accounting and
//! refusal plumbing with in-process members; it cannot show confinement —
//! that a member process holds *exactly* its assigned ends and nothing else.
//! `SpawnedSupervisor` is the test-harness OS supervisor: it re-executes this
//! test binary as each roster member (`spawned_member_body` below), hands the
//! child only its assigned pipe descriptors, and reads back the child's
//! whole descriptor table. The kernel — not the supervisor — decides what a
//! member can hold: every descriptor except the explicitly passed ones keeps
//! CLOEXEC, so a smuggled or inherited end would show up in the attested
//! table. Entry order, the installed-token echo, mediation refusals, and
//! generation lifecycle stay with the installer; this leg runs the same
//! claims across real process boundaries.
//!
//! Executable admission is content-bound: each roster artifact names
//! `identity_of` the spawned image, and `prepare_member` refuses an artifact
//! that names anything else, before any descriptor leaves installer custody.
//!
//! Unix hosts only: descriptor passing through `pre_exec` is the mechanism
//! the host adapter documents for private pipe ends. The Windows leg needs
//! anonymous pipe handles behind a handle-inheritance boundary and is
//! reported unavailable here, not exercised.
#![cfg(unix)]

mod support;

use std::collections::BTreeMap;
use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};
use support::*;
use topology_plan::deployment_plan::identity_of;
use topology_plan::topology_installation::*;
use topology_plan::*;

const MEMBER_MODE: &str = "TOPOLOGY_MEMBER_MODE";
const MEMBER_NAME: &str = "TOPOLOGY_MEMBER_NAME";
const MEMBER_CONTROL_READ: &str = "TOPOLOGY_MEMBER_CONTROL_READ";
const MEMBER_CONTROL_WRITE: &str = "TOPOLOGY_MEMBER_CONTROL_WRITE";
const MEMBER_ENDS: &str = "TOPOLOGY_MEMBER_ENDS";
const MEMBER_TEST: &str = "spawned_member_body";

/// The longest a member may take to exit after `QUIT`.
const QUIESCE_DEADLINE: Duration = Duration::from_secs(15);

fn role_code(role: ChannelEnd) -> &'static str {
    match role {
        ChannelEnd::RequestWrite => "reqw",
        ChannelEnd::RequestRead => "reqr",
        ChannelEnd::ResponseWrite => "resw",
        ChannelEnd::ResponseRead => "resr",
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("odd hex length".to_owned());
    }
    (0..text.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&text[index..index + 2], 16)
                .map_err(|error| format!("bad hex: {error}"))
        })
        .collect()
}

// ---- the member process ---------------------------------------------------

/// One assigned end as the child holds it.
struct MemberEnd {
    fd: u32,
    file: std::fs::File,
}

/// Spawned-member entrypoint. This is not a test result: `#[ignore]` keeps it
/// out of ordinary runs, and the supervisor re-executes this binary with
/// `--exact --ignored` selecting it by name. Without the member environment
/// it returns immediately so a manual `--ignored` sweep is harmless.
#[test]
#[ignore = "member body runs only inside a spawned child process"]
fn spawned_member_body() {
    if std::env::var_os(MEMBER_MODE).is_none() {
        return;
    }
    std::process::exit(member_main());
}

fn member_main() -> i32 {
    // The descriptors were inherited at the same numbers the supervisor
    // minted; taking ownership here means this process's exit releases them.
    let control_read: RawFd = std::env::var(MEMBER_CONTROL_READ)
        .expect("control read descriptor")
        .parse()
        .expect("control read is a descriptor number");
    let control_write: RawFd = std::env::var(MEMBER_CONTROL_WRITE)
        .expect("control write descriptor")
        .parse()
        .expect("control write is a descriptor number");
    // SAFETY: the supervisor cleared CLOEXEC on exactly these descriptors for
    // this exec; no other process aliases them.
    let mut commands = BufReader::new(unsafe { std::fs::File::from_raw_fd(control_read) });
    let mut events = unsafe { std::fs::File::from_raw_fd(control_write) };
    let mut ends: BTreeMap<u32, MemberEnd> = BTreeMap::new();
    let roster = std::env::var(MEMBER_ENDS).expect("assigned ends");
    if !roster.is_empty() {
        for spec in roster.split(',') {
            let mut parts = spec.split(':');
            let id = parts
                .next()
                .and_then(|part| part.parse::<u32>().ok())
                .expect("endpoint id");
            let fd = parts
                .next()
                .and_then(|part| part.parse::<u32>().ok())
                .expect("endpoint descriptor");
            ends.insert(
                id,
                MemberEnd {
                    fd,
                    // SAFETY: same contract as the control descriptors.
                    file: unsafe { std::fs::File::from_raw_fd(fd as RawFd) },
                },
            );
        }
    }

    // Attest the complete descriptor table above stdio — the kernel's answer
    // to "what does this member hold", not the member's claim.
    let mut table = Vec::new();
    for fd in 3..4096 {
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } != -1 {
            table.push(fd as u32);
        }
    }
    let listing = table
        .iter()
        .map(|fd| fd.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let _ = writeln!(events, "TABLE {listing}");
    for (id, end) in &ends {
        // The held end's kernel identity — pipe inode plus direction — is
        // the token the installer's route table was bound to.
        let token = match descriptor_token(end.fd as RawFd) {
            Ok(token) => token,
            Err(_) => return 4,
        };
        let _ = writeln!(events, "HOLD {id} {token}");
    }
    let _ = writeln!(events, "READY");

    let mut entered = false;
    let mut line = String::new();
    loop {
        line.clear();
        match commands.read_line(&mut line) {
            // The supervisor's command end is gone — supervision ended.
            Ok(0) | Err(_) => return 2,
            Ok(_) => {}
        }
        let fields: Vec<&str> = line.trim().split(' ').collect();
        let code = match fields.as_slice() {
            ["GO"] => {
                entered = true;
                "ENTERED".to_owned()
            }
            ["SEND", id, payload] if entered => {
                let id: u32 = id.parse().expect("endpoint id");
                let bytes = hex_decode(payload).expect("hex payload");
                match ends.get_mut(&id) {
                    Some(end) => match end.file.write_all(&bytes) {
                        Ok(()) => format!("SENT {id}"),
                        Err(error) => format!("ERR {id} {error}"),
                    },
                    None => format!("ERR {id} no such end"),
                }
            }
            ["RECV", id] if entered => {
                let id: u32 = id.parse().expect("endpoint id");
                match ends.get_mut(&id) {
                    Some(end) => {
                        let mut buffer = vec![0u8; MAX_FRAME_BYTES];
                        match end.file.read(&mut buffer) {
                            Ok(0) => format!("EOF {id}"),
                            Ok(count) => match decode_frame(&buffer[..count]) {
                                Ok(frame) => format!(
                                    "GOT {id} {} {}",
                                    frame.operation,
                                    hex_encode(&frame.payload)
                                ),
                                Err(error) => format!("BAD {id} {error}"),
                            },
                            Err(error) => format!("ERR {id} {error}"),
                        }
                    }
                    None => format!("ERR {id} no such end"),
                }
            }
            ["DIE"] => return 1,
            ["QUIT"] => {
                let _ = writeln!(events, "BYE");
                return 0;
            }
            _ => format!("REFUSED {}", line.trim()),
        };
        let _ = writeln!(events, "{code}");
    }
}

// ---- the supervisor --------------------------------------------------------

#[derive(Debug)]
struct SpawnedError(String);

impl fmt::Display for SpawnedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SpawnedError {}

impl From<std::io::Error> for SpawnedError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

/// One physical end installed into a member.
#[derive(Debug)]
struct SpawnedEnd {
    id: EndpointId,
    binding: u32,
    role: ChannelEnd,
    /// The descriptor number — identical in supervisor and child because the
    /// end is inherited, not remapped.
    fd: u32,
    /// The kernel-attested pipe token the member reported at the gate —
    /// what invocation checks compare a presented token against.
    token: u64,
}

/// A roster member as a real child process.
struct SpawnedMember {
    index: u32,
    name: String,
    child: Child,
    /// Supervisor-to-member control channel (the entry gate and commands).
    commands: std::io::PipeWriter,
    /// Member-to-supervisor attestation and event channel.
    events: BufReader<std::io::PipeReader>,
    ends: Vec<SpawnedEnd>,
    /// The child's complete descriptor table above stdio, as it attested.
    table: Vec<u32>,
    /// Child-side control descriptors, retained for the confinement check.
    control: [u32; 2],
    entered: bool,
}

impl SpawnedMember {
    fn end(&self, binding: u32, role: ChannelEnd) -> &SpawnedEnd {
        self.ends
            .iter()
            .find(|end| end.binding == binding && end.role == role)
            .expect("member holds this channel end")
    }

    fn command(&mut self, line: &str) -> std::io::Result<()> {
        self.commands.write_all(line.as_bytes())?;
        self.commands.write_all(b"\n")
    }

    /// One event line; `Err` on EOF means the member process vanished.
    fn event(&mut self) -> std::io::Result<String> {
        let mut line = String::new();
        match self.events.read_line(&mut line) {
            Ok(0) => Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "member event channel closed",
            )),
            Ok(_) => Ok(line.trim().to_owned()),
            Err(error) => Err(error),
        }
    }

    fn poll_exit(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().ok().flatten()
    }
}

/// Executable admission and process custody over real spawned members.
/// Endpoint identity is the inherited descriptor number: the route table
/// token and the child's attested hold name the same kernel object.
struct SpawnedSupervisor {
    lifecycle: InstallationLifecycle,
    executable: PathBuf,
    /// `identity_of` the image `prepare_member` will actually spawn.
    artifact: Identity,
    prepared: u32,
    log: Vec<String>,
}

impl SpawnedSupervisor {
    fn new() -> Self {
        let executable = std::env::current_exe().expect("this test binary");
        let artifact = identity_of(&std::fs::read(&executable).expect("read test binary"));
        Self {
            lifecycle: InstallationLifecycle::default(),
            executable,
            artifact,
            prepared: 0,
            log: Vec::new(),
        }
    }

    fn installation_request(
        &self,
        request_bytes: &[u8],
        occurrence: u64,
        forge_artifact_at: Option<usize>,
    ) -> InstallationRequest {
        InstallationRequest {
            expected_request: request_commitment(request_bytes),
            occurrence,
            artifacts: payment_components()
                .iter()
                .enumerate()
                .map(|(index, admission)| AdmittedArtifact {
                    artifact: if forge_artifact_at == Some(index) {
                        identity(0xEE)
                    } else {
                        self.artifact
                    },
                    component_subject: subject_of(admission),
                })
                .collect(),
        }
    }
}

impl ProcessSupervisor for SpawnedSupervisor {
    type Endpoint = StdPipeEnd;
    type Member = SpawnedMember;
    type Error = SpawnedError;

    fn installation_lifecycle(&self) -> &InstallationLifecycle {
        &self.lifecycle
    }

    fn prepare_member(
        &mut self,
        instance: &PlanInstance,
        artifact: &AdmittedArtifact,
        endpoints: Vec<EndpointAssignment<StdPipeEnd>>,
    ) -> Result<PreparationEcho<SpawnedMember>, SpawnedError> {
        let index = self.prepared;
        self.prepared += 1;
        self.log.push(format!("prepare {}", instance.name));

        // Executable admission: the spawned image must be the admitted
        // artifact — a forged identity refuses before any descriptor moves.
        if artifact.artifact != self.artifact {
            return Err(SpawnedError(
                "admitted artifact does not name the spawned image".to_owned(),
            ));
        }

        // Control pair: supervisor commands on `gate`, member events on
        // `events`. Only the child-facing ends are handed over.
        let (gate_read, gate_write) = std::io::pipe()?;
        let (event_read, event_write) = std::io::pipe()?;
        let gate_read_fd = gate_read.into_raw_fd();
        let event_write_fd = event_write.into_raw_fd();
        let mut handed = vec![gate_read_fd, event_write_fd];
        let mut ends = Vec::new();
        let mut spec = String::new();
        for (position, assignment) in endpoints.into_iter().enumerate() {
            let fd = match assignment.handle {
                StdPipeEnd::Read(end) => end.into_raw_fd(),
                StdPipeEnd::Write(end) => end.into_raw_fd(),
            };
            if position > 0 {
                spec.push(',');
            }
            spec.push_str(&format!(
                "{}:{}:{}",
                assignment.id.0,
                fd,
                role_code(assignment.role)
            ));
            handed.push(fd);
            ends.push(SpawnedEnd {
                id: assignment.id,
                binding: assignment.binding,
                role: assignment.role,
                fd: fd as u32,
                token: 0,
            });
        }

        let mut command = Command::new(&self.executable);
        command
            .args(["--ignored", "--exact", MEMBER_TEST])
            .env(MEMBER_MODE, "1")
            .env(MEMBER_NAME, instance.name.as_str())
            .env(MEMBER_CONTROL_READ, gate_read_fd.to_string())
            .env(MEMBER_CONTROL_WRITE, event_write_fd.to_string())
            .env(MEMBER_ENDS, spec)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        // SAFETY: the closure runs in the forked child before exec and only
        // clears CLOEXEC on the descriptors this member was assigned; every
        // other descriptor still closes on exec — that asymmetry is the
        // confinement boundary this leg verifies.
        unsafe {
            let passed = handed.clone();
            command.pre_exec(move || {
                for fd in &passed {
                    let flags = libc::fcntl(*fd, libc::F_GETFD);
                    if flags == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::fcntl(*fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                // The ends were taken into custody; release them exactly.
                for fd in &handed {
                    // SAFETY: the parent still owns each descriptor.
                    drop(unsafe { std::fs::File::from_raw_fd(*fd) });
                }
                return Err(SpawnedError(format!("member spawn failed: {error}")));
            }
        };
        // The child owns its descriptors now; the parent's copies close.
        for fd in &handed {
            // SAFETY: these are this process's own inherited copies.
            drop(unsafe { std::fs::File::from_raw_fd(*fd) });
        }
        let _ = child;

        let mut member = SpawnedMember {
            index,
            name: instance.name.as_str().to_owned(),
            child,
            commands: gate_write,
            events: BufReader::new(event_read),
            ends,
            table: Vec::new(),
            control: [gate_read_fd as u32, event_write_fd as u32],
            entered: false,
        };

        // The child attests its whole descriptor table, then each held end.
        let mut installed = Vec::new();
        loop {
            let line = member.event().map_err(|error| {
                let _ = member.child.kill();
                let _ = member.child.wait();
                SpawnedError(format!("member attestation failed: {error}"))
            })?;
            match line.split_once(' ') {
                Some(("TABLE", listing)) => {
                    member.table = listing
                        .split(',')
                        .filter_map(|part| part.parse::<u32>().ok())
                        .collect();
                }
                Some(("HOLD", held)) => {
                    let mut parts = held.split(' ');
                    let id = parts.next().and_then(|part| part.parse::<u32>().ok());
                    let token = parts.next().and_then(|part| part.parse::<u64>().ok());
                    let (id, token) = (id.expect("HOLD id"), token.expect("HOLD token"));
                    let id = EndpointId(id);
                    if let Some(end) = member.ends.iter_mut().find(|end| end.id == id) {
                        end.token = token;
                    }
                    installed.push((id, token));
                }
                None if line == "READY" => break,
                _ => {
                    let _ = member.child.kill();
                    let _ = member.child.wait();
                    return Err(SpawnedError(format!(
                        "member attestation out of order: {line}"
                    )));
                }
            }
        }
        Ok(PreparationEcho { member, installed })
    }

    fn open_entry_gate(&mut self, member: &mut SpawnedMember) -> Result<(), SpawnedError> {
        self.log.push(format!("enter {}", member.index));
        member.command("GO")?;
        match member.event()?.as_str() {
            "ENTERED" => {
                member.entered = true;
                Ok(())
            }
            other => Err(SpawnedError(format!("entry gate refused: {other}"))),
        }
    }

    fn quiesce_member(
        &mut self,
        mut member: SpawnedMember,
    ) -> Result<(), RetainedMember<SpawnedMember, SpawnedError>> {
        self.log.push(format!("quiesce {}", member.index));
        // A dead member's command pipe refuses; the exit status still
        // decides whether quiescence actually happened.
        let _ = member.command("QUIT");
        let deadline = Instant::now() + QUIESCE_DEADLINE;
        loop {
            match member.child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(status)) => {
                    return Err(RetainedMember {
                        member,
                        error: SpawnedError(format!("member exited {status}")),
                    });
                }
                Ok(None) if Instant::now() >= deadline => {
                    let _ = member.child.kill();
                    let _ = member.child.wait();
                    return Err(RetainedMember {
                        member,
                        error: SpawnedError("member refused quiesce".to_owned()),
                    });
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    return Err(RetainedMember {
                        member,
                        error: error.into(),
                    });
                }
            }
        }
    }
}

// ---- fixtures ---------------------------------------------------------------

fn checked_payment_plan() -> (CheckedPlan, Vec<u8>) {
    let (request_bytes, plan_bytes, _) = payment_pair();
    let checked = verify_plan(&plan_bytes, &request_bytes, &payment_components())
        .expect("golden plan verifies");
    (checked, request_bytes)
}

fn spawned_payment_installation(
    supervisor: &mut SpawnedSupervisor,
    occurrence: u64,
) -> InstalledTopology<StdPipeAdapter, SpawnedMember> {
    let (checked, request_bytes) = checked_payment_plan();
    let authorization = supervisor
        .lifecycle
        .authorize(supervisor.installation_request(&request_bytes, occurrence, None))
        .expect("owner authorization");
    prepare_installation(
        checked,
        authorization,
        StdPipeAdapter,
        payment_operation_schemas(),
    )
    .expect("preparation admits real pipes")
    .activate(supervisor)
    .expect("activation admits the spawned roster")
}

fn framed(operation: u32, payload: &[u8]) -> String {
    hex_encode(
        &encode_frame(&Frame {
            operation,
            payload: payload.to_vec(),
        })
        .expect("frame encodes"),
    )
}

// ---- the three-process customer ---------------------------------------------

#[test]
fn spawned_members_hold_exactly_assigned_ends_and_flow_frames() {
    let mut supervisor = SpawnedSupervisor::new();
    let mut installed = spawned_payment_installation(&mut supervisor, 1);

    // Every member was prepared before any entry opened — canonical order.
    assert_eq!(
        supervisor.log,
        [
            "prepare api",
            "prepare authorization",
            "prepare billing",
            "enter 0",
            "enter 1",
            "enter 2",
        ]
    );

    // Confinement: each child's complete descriptor table above stdio is
    // exactly its assigned ends plus its control pair — no inherited ends.
    for member in installed.members() {
        assert!(member.entered);
        let mut expected: Vec<u32> = member.ends.iter().map(|end| end.fd).collect();
        expected.extend(member.control);
        expected.sort_unstable();
        assert_eq!(
            member.table, expected,
            "{}'s descriptor table is exactly its assignment",
            member.name
        );
    }

    // One real request/response over the kernel's private pipes: api's
    // request travels binding 0's request channel to authorization, the
    // response returns on the response channel.
    let api_request = installed.members()[0].end(0, ChannelEnd::RequestWrite).id;
    let api_request_token = installed.members()[0]
        .end(0, ChannelEnd::RequestWrite)
        .token;
    let authorization_read = installed.members()[1].end(0, ChannelEnd::RequestRead).id;
    let authorization_response = installed.members()[1].end(0, ChannelEnd::ResponseWrite).id;
    let authorization_response_token = installed.members()[1]
        .end(0, ChannelEnd::ResponseWrite)
        .token;
    let api_response = installed.members()[0].end(0, ChannelEnd::ResponseRead).id;

    let grant = installed
        .authorize_send(api_request, api_request_token)
        .expect("api's import invocation is granted");
    assert_eq!(grant.deliver_to, 1);
    installed.members_mut()[0]
        .command(&format!(
            "SEND {} {}",
            api_request.0,
            framed(1, b"authorize")
        ))
        .unwrap();
    assert_eq!(
        installed.members_mut()[0].event().unwrap(),
        format!("SENT {}", api_request.0)
    );
    installed.members_mut()[1]
        .command(&format!("RECV {}", authorization_read.0))
        .unwrap();
    assert_eq!(
        installed.members_mut()[1].event().unwrap(),
        format!(
            "GOT {} 1 {}",
            authorization_read.0,
            hex_encode(b"authorize")
        )
    );

    let reply = installed
        .authorize_respond(authorization_response, authorization_response_token)
        .expect("the reply stays on the response channel");
    assert_eq!(reply.deliver_to, 0);
    installed.members_mut()[1]
        .command(&format!(
            "SEND {} {}",
            authorization_response.0,
            framed(1, b"granted")
        ))
        .unwrap();
    installed.members_mut()[0]
        .command(&format!("RECV {}", api_response.0))
        .unwrap();
    assert_eq!(
        installed.members_mut()[0].event().unwrap(),
        format!("GOT {} 1 {}", api_response.0, hex_encode(b"granted"))
    );

    // The refusals hold with real processes behind the route table: an
    // endpoint the installer never issued, and a real endpoint named by a
    // sender that does not hold it.
    assert_eq!(
        installed.authorize_send(EndpointId(999), 0),
        Err(InvocationRefusal::UngrantedEndpoint {
            endpoint: EndpointId(999)
        })
    );
    let foreign = installed.members()[1].end(1, ChannelEnd::RequestWrite).id;
    let foreign_token = installed.members()[1]
        .end(1, ChannelEnd::RequestWrite)
        .token;
    assert_eq!(
        installed.authorize_send(foreign, 0),
        Err(InvocationRefusal::SubstitutedMapping {
            endpoint: foreign,
            expected: foreign_token,
            presented: 0,
        })
    );

    if let Err(retained) = installed.quiesce(&mut supervisor) {
        panic!(
            "the whole roster should quiesce cleanly; {} retained",
            retained.len()
        );
    }
    assert_eq!(supervisor.log[6..], ["quiesce 0", "quiesce 1", "quiesce 2"]);
}

#[test]
fn a_spawned_peer_failure_and_eof_close_their_binding() {
    let mut supervisor = SpawnedSupervisor::new();
    let mut installed = spawned_payment_installation(&mut supervisor, 1);

    // A completed binding-1 request: authorization asks billing to post.
    let authorization_request = installed.members()[1].end(1, ChannelEnd::RequestWrite).id;
    let authorization_request_token = installed.members()[1]
        .end(1, ChannelEnd::RequestWrite)
        .token;
    let billing_read = installed.members()[2].end(1, ChannelEnd::RequestRead).id;
    installed
        .authorize_send(authorization_request, authorization_request_token)
        .expect("authorization's import invocation is granted");
    installed.members_mut()[1]
        .command(&format!(
            "SEND {} {}",
            authorization_request.0,
            framed(1, b"post")
        ))
        .unwrap();
    installed.members_mut()[2]
        .command(&format!("RECV {}", billing_read.0))
        .unwrap();
    assert_eq!(
        installed.members_mut()[2].event().unwrap(),
        format!("GOT {} 1 {}", billing_read.0, hex_encode(b"post"))
    );

    // billing dies: the kernel closes every end it held, and the supervisor
    // observes a real exit — not a simulated quiesce refusal.
    installed.members_mut()[2].command("DIE").unwrap();
    let deadline = Instant::now() + QUIESCE_DEADLINE;
    while installed.members_mut()[2].poll_exit().is_none() {
        assert!(Instant::now() < deadline, "billing's exit must be observed");
        std::thread::sleep(Duration::from_millis(10));
    }
    // The response can never arrive; the binding closes and stays closed.
    installed.close_binding(1);
    assert_eq!(
        installed.authorize_send(authorization_request, authorization_request_token),
        Err(InvocationRefusal::BindingClosed { binding: 1 })
    );

    // authorization dies too: api's pending request end sees EOF on read —
    // a dead peer cannot even refuse.
    let api_request = installed.members()[0].end(0, ChannelEnd::RequestWrite).id;
    let api_request_token = installed.members()[0]
        .end(0, ChannelEnd::RequestWrite)
        .token;
    let api_response = installed.members()[0].end(0, ChannelEnd::ResponseRead).id;
    installed
        .authorize_send(api_request, api_request_token)
        .expect("a fresh request is granted");
    installed.members_mut()[1].command("DIE").unwrap();
    let deadline = Instant::now() + QUIESCE_DEADLINE;
    while installed.members_mut()[1].poll_exit().is_none() {
        assert!(
            Instant::now() < deadline,
            "authorization's exit must be observed"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    installed.members_mut()[0]
        .command(&format!("RECV {}", api_response.0))
        .unwrap();
    assert_eq!(
        installed.members_mut()[0].event().unwrap(),
        format!("EOF {}", api_response.0)
    );
    installed.close_binding(0);
    assert_eq!(
        installed.authorize_send(api_request, api_request_token),
        Err(InvocationRefusal::BindingClosed { binding: 0 })
    );

    // Two members could not be quiesced — they are dead but never silently
    // counted clean: supervision reports them retained.
    let retained = installed
        .quiesce(&mut supervisor)
        .expect_err("dead members stay named under supervision");
    assert_eq!(retained.len(), 2);
}

#[test]
fn a_mismatched_executable_refuses_before_any_member_entry() {
    let mut supervisor = SpawnedSupervisor::new();
    let (checked, request_bytes) = checked_payment_plan();
    let authorization = supervisor
        .lifecycle
        .authorize(supervisor.installation_request(&request_bytes, 1, Some(1)))
        .expect("owner authorization");
    let prepared = prepare_installation(
        checked,
        authorization,
        StdPipeAdapter,
        payment_operation_schemas(),
    )
    .expect("preparation admits real pipes");
    let failure = prepared
        .activate(&mut supervisor)
        .expect_err("authorization's artifact is not the spawned image");
    assert_eq!(failure.instance, 1);
    assert!(matches!(failure.cause, ActivationCause::Supervisor(_)));
    assert!(failure.retained.is_empty());
    assert!(failure.leaked.is_empty());
    // api was spawned, then quiesced by rollback; billing never started.
    assert_eq!(
        supervisor.log,
        ["prepare api", "prepare authorization", "quiesce 0"]
    );
}

#[test]
fn a_spawned_garbage_frame_closes_the_binding() {
    let mut supervisor = SpawnedSupervisor::new();
    let mut installed = spawned_payment_installation(&mut supervisor, 1);

    let api_request = installed.members()[0].end(0, ChannelEnd::RequestWrite).id;
    let api_request_token = installed.members()[0]
        .end(0, ChannelEnd::RequestWrite)
        .token;
    let authorization_read = installed.members()[1].end(0, ChannelEnd::RequestRead).id;
    let authorization_response = installed.members()[1].end(0, ChannelEnd::ResponseWrite).id;
    let authorization_response_token = installed.members()[1]
        .end(0, ChannelEnd::ResponseWrite)
        .token;
    let api_response = installed.members()[0].end(0, ChannelEnd::ResponseRead).id;

    installed
        .authorize_send(api_request, api_request_token)
        .expect("api's import invocation is granted");
    installed.members_mut()[0]
        .command(&format!(
            "SEND {} {}",
            api_request.0,
            framed(1, b"authorize")
        ))
        .unwrap();
    assert_eq!(
        installed.members_mut()[0].event().unwrap(),
        format!("SENT {}", api_request.0)
    );
    installed.members_mut()[1]
        .command(&format!("RECV {}", authorization_read.0))
        .unwrap();
    assert_eq!(
        installed.members_mut()[1].event().unwrap(),
        format!(
            "GOT {} 1 {}",
            authorization_read.0,
            hex_encode(b"authorize")
        )
    );

    // The response grant exists, but what arrives on the wire is not a
    // frame: decode fails and the binding closes by rule.
    installed
        .authorize_respond(authorization_response, authorization_response_token)
        .expect("response channel granted");
    installed.members_mut()[1]
        .command(&format!("SEND {} 00ff", authorization_response.0))
        .unwrap();
    installed.members_mut()[0]
        .command(&format!("RECV {}", api_response.0))
        .unwrap();
    let event = installed.members_mut()[0].event().unwrap();
    assert!(
        event.starts_with(&format!("BAD {}", api_response.0)),
        "an invalid frame must surface: {event}"
    );
    installed.close_binding(0);
    assert_eq!(
        installed.authorize_send(api_request, api_request_token),
        Err(InvocationRefusal::BindingClosed { binding: 0 })
    );

    if let Err(retained) = installed.quiesce(&mut supervisor) {
        panic!(
            "remaining members should quiesce; {} retained",
            retained.len()
        );
    }
}
