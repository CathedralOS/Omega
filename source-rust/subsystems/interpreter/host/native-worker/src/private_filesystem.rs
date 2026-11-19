/*
A test invocation's private in-memory filesystem, presented to the guest as the
Linux kernel's filesystem calls (wiki/spec/build/testing.md#group-configuration-and-authority).

A test group that names std's `FilesystemService` slot runs each invocation over
state the runner creates empty, the way an operating system holds a real
filesystem: std's ordinary POSIX provider makes the same kernel calls it makes
on a host, and this table answers them instead of the host. Nothing here can
reach a host path, so a program cannot widen the backing to the real
filesystem; an absolute path names this namespace's own root.

The native engine runs the same invocation in a fresh mount namespace over an
empty tmpfs, as an unprivileged root with umask 022 and a 64 MiB limit. These
semantics follow that kernel: owner permission bits are enforced, directory
sizes and block counts take tmpfs's values, `dup` shares one open file
description, and a removed file lives until its last description closes.
Results are negated `errno` values, the kernel's own convention, so the
interpreter's adapter passes them through exactly as it does a host status.

Storage is dense: nodes and open file descriptions are indexed by their
numbers, and each directory keeps its entries sorted by name.
*/

use crate::host_filesystem::{KernelOperand, LINUX_AT_FDCWD};

/// One Linux filesystem system call, independent of its per-target number.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinuxFilesystemOperation {
    Openat,
    Close,
    Dup,
    Lseek,
    Mkdirat,
    Unlinkat,
    Newfstatat,
    Fstat,
    Getdents64,
    Pread64,
    Pwrite64,
    Readlinkat,
    Fchmodat,
    Fchmod,
    Renameat,
    Linkat,
    Symlinkat,
    Ftruncate,
    Utimensat,
    Fsync,
    Fdatasync,
    Flock,
    Fchownat,
    Fchown,
}

const EPERM: i64 = 1;
const ENOENT: i64 = 2;
const EBADF: i64 = 9;
const EAGAIN: i64 = 11;
const EACCES: i64 = 13;
const EBUSY: i64 = 16;
const EEXIST: i64 = 17;
const ENOTDIR: i64 = 20;
const EISDIR: i64 = 21;
const EINVAL: i64 = 22;
const EMFILE: i64 = 24;
const EFBIG: i64 = 27;
const ENOSPC: i64 = 28;
const EDEADLK: i64 = 35;
const ENAMETOOLONG: i64 = 36;
const ENOTEMPTY: i64 = 39;
const ELOOP: i64 = 40;

const O_ACCMODE: i64 = 3;
const O_WRONLY: i64 = 1;
const O_RDWR: i64 = 2;
const O_CREAT: i64 = 0o100;
const O_EXCL: i64 = 0o200;
const O_TRUNC: i64 = 0o1000;
const O_APPEND: i64 = 0o2000;
const AT_SYMLINK_NOFOLLOW: i64 = 0x100;
const AT_REMOVEDIR: i64 = 0x200;
const AT_SYMLINK_FOLLOW: i64 = 0x400;
const AT_EMPTY_PATH: i64 = 0x1000;
const UTIME_NOW: i64 = (1 << 30) - 1;
const UTIME_OMIT: i64 = (1 << 30) - 2;
const LOCK_SH: i64 = 1;
const LOCK_EX: i64 = 2;
const LOCK_NB: i64 = 4;
const LOCK_UN: i64 = 8;

const S_IFREG: u32 = 0o100000;
const S_IFDIR: u32 = 0o040000;
const S_IFLNK: u32 = 0o120000;
const UMASK: u32 = 0o022;
const NAME_MAX: usize = 255;
const PATH_MAX: usize = 4096;
const SYMLINK_DEPTH: usize = 40;
const DESCRIPTOR_LIMIT: usize = 1024;
/// tmpfs's `size=64m` mount: file contents count against it.
const CAPACITY: usize = 64 << 20;
const PAGE: u64 = 4096;
/// tmpfs reports a directory's size as 20 bytes per entry, `.` and `..` included.
const DIRECTORY_ENTRY_SIZE: i64 = 20;

type NodeId = usize;
const ROOT: NodeId = 0;

#[derive(Clone, Copy, Default)]
struct Time {
    seconds: i64,
    nanoseconds: i64,
}

enum Contents {
    File(Vec<u8>),
    Directory {
        parent: NodeId,
        entries: Vec<(Vec<u8>, NodeId)>, // Sorted by name.
    },
    Symlink(Vec<u8>),
}

struct Node {
    contents: Contents,
    permissions: u32, // Permission bits only (`0o7777`).
    uid: u32,
    gid: u32,
    links: u64,        // Directory entries naming a file; unused for directories.
    descriptions: u64, // Open file descriptions holding the node.
    accessed: Time,
    modified: Time,
    changed: Time,
    flock: Option<(LockKind, Vec<usize>)>, // Holders by open file description.
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LockKind {
    Shared,
    Exclusive,
}

struct Description {
    node: NodeId,
    readable: bool,
    writable: bool,
    append: bool,
    offset: u64,
    references: usize, // Guest descriptors sharing this description.
}

enum Guest {
    ModeledStandard, // 0, 1 and 2 until closed; the runtime serves them.
    Open(usize),     // Open file description.
    Free,
}

pub struct PrivateLinuxFilesystem {
    arm64: bool,
    nodes: Vec<Option<Node>>,
    descriptions: Vec<Option<Description>>,
    guests: Vec<Guest>,
    stored: usize,
}

/// What path resolution found.
struct Lookup {
    parent: NodeId,
    name: Vec<u8>, // Empty for the root or a path ending in `.`/`..`.
    node: Option<NodeId>,
    trailing_slash: bool,
}

impl Default for PrivateLinuxFilesystem {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivateLinuxFilesystem {
    /// An empty namespace: one root directory, mode `0755`, owned by root.
    pub fn new() -> Self {
        let now = now();
        Self {
            arm64: false,
            nodes: vec![Some(Node {
                contents: Contents::Directory {
                    parent: ROOT,
                    entries: Vec::new(),
                },
                permissions: 0o755,
                uid: 0,
                gid: 0,
                links: 1,
                descriptions: 0,
                accessed: now,
                modified: now,
                changed: now,
                flock: None,
            })],
            descriptions: Vec::new(),
            guests: (0..3).map(|_| Guest::ModeledStandard).collect(),
            stored: 0,
        }
    }

    /// Present records in `linux_arm64`'s layouts (open flags, `struct stat`)
    /// rather than `linux_x86_64`'s. The namespace itself is host-independent.
    pub fn select_target(&mut self, arm64: bool) {
        self.arm64 = arm64;
    }

    pub fn models_standard(&self, guest: i32) -> bool {
        usize::try_from(guest)
            .ok()
            .and_then(|index| self.guests.get(index))
            .is_some_and(|guest| matches!(guest, Guest::ModeledStandard))
    }

    /// The layout of one `struct stat` on this target.
    pub fn metadata_extent(&self) -> usize {
        if self.arm64 { 128 } else { 144 }
    }

    pub fn call(
        &mut self,
        operation: LinuxFilesystemOperation,
        operands: &mut [KernelOperand<'_>],
    ) -> i64 {
        self.dispatch(operation, operands)
            .unwrap_or_else(|errno| -errno)
    }

    pub fn read(&mut self, guest: i32, bytes: &mut [u8]) -> i64 {
        self.transfer_in(guest, bytes, None)
            .unwrap_or_else(|errno| -errno)
    }

    pub fn write(&mut self, guest: i32, bytes: &[u8]) -> i64 {
        self.transfer_out(guest, bytes, None)
            .unwrap_or_else(|errno| -errno)
    }

    pub fn close(&mut self, guest: i32) -> i64 {
        self.close_guest(guest).map_or_else(|errno| -errno, |()| 0)
    }

    fn dispatch(
        &mut self,
        operation: LinuxFilesystemOperation,
        operands: &mut [KernelOperand<'_>],
    ) -> Result<i64, i64> {
        use KernelOperand as K;
        use LinuxFilesystemOperation as Op;
        match (operation, operands) {
            (
                Op::Openat,
                [
                    K::Directory(at),
                    K::Path(path),
                    K::Value(flags),
                    K::Value(mode),
                ],
            ) => self.openat(*at, path.to_bytes(), *flags, *mode),
            (Op::Close, [K::Descriptor(fd)]) => self.close_guest(*fd).map(|()| 0),
            (Op::Dup, [K::Descriptor(fd)]) => {
                let description = self.description_of(*fd)?;
                self.description_mut(description)?.references += 1;
                self.allocate_guest(description)
            }
            (Op::Lseek, [K::Descriptor(fd), K::Value(offset), K::Value(whence)]) => {
                self.lseek(*fd, *offset, *whence)
            }
            (Op::Mkdirat, [K::Directory(at), K::Path(path), K::Value(mode)]) => {
                self.mkdirat(*at, path.to_bytes(), *mode).map(|()| 0)
            }
            (Op::Unlinkat, [K::Directory(at), K::Path(path), K::Value(flags)]) => {
                self.unlinkat(*at, path.to_bytes(), *flags).map(|()| 0)
            }
            (
                Op::Newfstatat,
                [
                    K::Directory(at),
                    K::Path(path),
                    K::Output(buffer),
                    K::Value(flags),
                ],
            ) => {
                let node = if path.to_bytes().is_empty() && *flags & AT_EMPTY_PATH != 0 {
                    self.directory_operand_node(*at)?
                } else {
                    let follow = *flags & AT_SYMLINK_NOFOLLOW == 0;
                    self.existing(*at, path.to_bytes(), follow)?
                };
                self.stat(node, buffer)?;
                Ok(0)
            }
            (Op::Fstat, [K::Descriptor(fd), K::Output(buffer)]) => {
                let node = self.node_of(*fd)?;
                self.stat(node, buffer)?;
                Ok(0)
            }
            (Op::Getdents64, [K::Descriptor(fd), K::OutputView(buffer)]) => {
                self.getdents64(*fd, buffer)
            }
            (Op::Pread64, [K::Descriptor(fd), K::OutputView(buffer), K::Value(offset)]) => {
                let offset = u64::try_from(*offset).map_err(|_| EINVAL)?;
                self.transfer_in(*fd, buffer, Some(offset))
            }
            (Op::Pwrite64, [K::Descriptor(fd), K::InputView(bytes), K::Value(offset)]) => {
                let offset = u64::try_from(*offset).map_err(|_| EINVAL)?;
                self.transfer_out(*fd, bytes, Some(offset))
            }
            (Op::Readlinkat, [K::Directory(at), K::Path(path), K::OutputView(buffer)]) => {
                let node = self.existing(*at, path.to_bytes(), false)?;
                let Contents::Symlink(target) = &self.node(node)?.contents else {
                    return Err(EINVAL);
                };
                if buffer.is_empty() {
                    return Err(EINVAL);
                }
                let count = target.len().min(buffer.len());
                buffer[..count].copy_from_slice(&target[..count]);
                Ok(count as i64)
            }
            (Op::Fchmodat, [K::Directory(at), K::Path(path), K::Value(mode)]) => {
                let node = self.existing(*at, path.to_bytes(), true)?;
                self.chmod(node, *mode).map(|()| 0)
            }
            (Op::Fchmod, [K::Descriptor(fd), K::Value(mode)]) => {
                let node = self.node_of(*fd)?;
                self.chmod(node, *mode).map(|()| 0)
            }
            (
                Op::Renameat,
                [
                    K::Directory(from_at),
                    K::Path(from),
                    K::Directory(to_at),
                    K::Path(to),
                ],
            ) => self
                .renameat(*from_at, from.to_bytes(), *to_at, to.to_bytes())
                .map(|()| 0),
            (
                Op::Linkat,
                [
                    K::Directory(from_at),
                    K::Path(from),
                    K::Directory(to_at),
                    K::Path(to),
                    K::Value(flags),
                ],
            ) => self
                .linkat(*from_at, from.to_bytes(), *to_at, to.to_bytes(), *flags)
                .map(|()| 0),
            (Op::Symlinkat, [K::Path(target), K::Directory(at), K::Path(link)]) => self
                .symlinkat(target.to_bytes(), *at, link.to_bytes())
                .map(|()| 0),
            (Op::Ftruncate, [K::Descriptor(fd), K::Value(length)]) => {
                self.ftruncate(*fd, *length).map(|()| 0)
            }
            (
                Op::Utimensat,
                [
                    K::Descriptor(fd),
                    K::Value(_null),
                    K::Input(times),
                    K::Value(_flags),
                ],
            ) => self.utimens(*fd, times).map(|()| 0),
            (Op::Fsync | Op::Fdatasync, [K::Descriptor(fd)]) => {
                self.description_of(*fd)?;
                Ok(0)
            }
            (Op::Flock, [K::Descriptor(fd), K::Value(operation)]) => {
                self.flock(*fd, *operation).map(|()| 0)
            }
            (
                Op::Fchownat,
                [
                    K::Directory(at),
                    K::Path(path),
                    K::Value(uid),
                    K::Value(gid),
                    K::Value(flags),
                ],
            ) => {
                let follow = *flags & AT_SYMLINK_NOFOLLOW == 0;
                let node = self.existing(*at, path.to_bytes(), follow)?;
                self.chown(node, *uid, *gid).map(|()| 0)
            }
            (Op::Fchown, [K::Descriptor(fd), K::Value(uid), K::Value(gid)]) => {
                let node = self.node_of(*fd)?;
                self.chown(node, *uid, *gid).map(|()| 0)
            }
            // The admitted call table and this dispatch name the same operand
            // shapes; any other combination is not a call this kernel answers.
            _ => Err(EINVAL),
        }
    }

    fn node(&self, node: NodeId) -> Result<&Node, i64> {
        self.nodes.get(node).and_then(Option::as_ref).ok_or(EBADF)
    }

    fn node_mut(&mut self, node: NodeId) -> Result<&mut Node, i64> {
        self.nodes
            .get_mut(node)
            .and_then(Option::as_mut)
            .ok_or(EBADF)
    }

    fn description_mut(&mut self, description: usize) -> Result<&mut Description, i64> {
        self.descriptions
            .get_mut(description)
            .and_then(Option::as_mut)
            .ok_or(EBADF)
    }

    fn description_of(&self, guest: i32) -> Result<usize, i64> {
        match usize::try_from(guest)
            .ok()
            .and_then(|index| self.guests.get(index))
        {
            Some(Guest::Open(description)) => Ok(*description),
            _ => Err(EBADF),
        }
    }

    fn node_of(&self, guest: i32) -> Result<NodeId, i64> {
        let description = self.description_of(guest)?;
        Ok(self.descriptions[description].as_ref().ok_or(EBADF)?.node)
    }

    fn is_directory(&self, node: NodeId) -> bool {
        self.node(node)
            .is_ok_and(|node| matches!(node.contents, Contents::Directory { .. }))
    }

    // Whether the owner bit `bit` (4 read, 2 write, 1 search) is set. The
    // process owns every node, and without capabilities owner bits decide.
    fn permits(&self, node: NodeId, bit: u32) -> bool {
        self.node(node)
            .is_ok_and(|node| node.permissions & (bit << 6) != 0)
    }

    fn directory_operand_node(&self, at: i32) -> Result<NodeId, i64> {
        if at == LINUX_AT_FDCWD {
            // The working directory is the namespace root.
            Ok(ROOT)
        } else {
            self.node_of(at)
        }
    }

    fn entry(&self, directory: NodeId, name: &[u8]) -> Option<NodeId> {
        let Ok(Node {
            contents: Contents::Directory { entries, .. },
            ..
        }) = self.node(directory)
        else {
            return None;
        };
        entries
            .binary_search_by(|(entry, _)| entry.as_slice().cmp(name))
            .ok()
            .map(|index| entries[index].1)
    }

    /*
        Resolve `path` relative to `at`. Every intermediate component must be a
        searchable directory, following symlinks; the final component follows a
        symlink only when `follow` is set. A trailing slash requires a directory
        and therefore always follows.
    */
    fn lookup(&self, at: i32, path: &[u8], follow: bool) -> Result<Lookup, i64> {
        if path.is_empty() {
            return Err(ENOENT);
        }
        if path.len() >= PATH_MAX {
            return Err(ENAMETOOLONG);
        }
        let start = if path.first() == Some(&b'/') {
            ROOT
        } else {
            let start = self.directory_operand_node(at)?;
            if !self.is_directory(start) {
                return Err(ENOTDIR);
            }
            start
        };
        let mut depth = 0;
        self.walk(start, path, follow, &mut depth)
    }

    fn walk(
        &self,
        start: NodeId,
        path: &[u8],
        follow: bool,
        depth: &mut usize,
    ) -> Result<Lookup, i64> {
        let mut directory = if path.first() == Some(&b'/') {
            ROOT
        } else {
            start
        };
        let trailing_slash = path.last() == Some(&b'/');
        let components: Vec<&[u8]> = path
            .split(|byte| *byte == b'/')
            .filter(|component| !component.is_empty())
            .collect();
        if components.is_empty() {
            return Ok(Lookup {
                parent: directory,
                name: Vec::new(),
                node: Some(directory),
                trailing_slash,
            });
        }
        for (index, component) in components.iter().enumerate() {
            if component.len() > NAME_MAX {
                return Err(ENAMETOOLONG);
            }
            if !self.is_directory(directory) {
                return Err(ENOTDIR);
            }
            if !self.permits(directory, 1) {
                return Err(EACCES);
            }
            let last = index + 1 == components.len();
            let next = match *component {
                b"." => Some(directory),
                b".." => match &self.node(directory)?.contents {
                    Contents::Directory { parent, .. } => Some(*parent),
                    _ => return Err(ENOTDIR),
                },
                name => self.entry(directory, name),
            };
            let dotted = matches!(*component, b"." | b"..");
            if last {
                let mut node = next;
                if let Some(found) = node
                    && (follow || trailing_slash)
                    && let Contents::Symlink(target) = &self.node(found)?.contents
                {
                    *depth += 1;
                    if *depth > SYMLINK_DEPTH {
                        return Err(ELOOP);
                    }
                    let resolved = self.walk(directory, target, true, depth)?;
                    node = resolved.node;
                    if node.is_none() {
                        return Ok(resolved);
                    }
                }
                if trailing_slash && node.is_some_and(|node| !self.is_directory(node)) {
                    return Err(ENOTDIR);
                }
                return Ok(Lookup {
                    parent: directory,
                    name: if dotted {
                        Vec::new()
                    } else {
                        component.to_vec()
                    },
                    node,
                    trailing_slash,
                });
            }
            let Some(mut found) = next else {
                return Err(ENOENT);
            };
            if let Contents::Symlink(target) = &self.node(found)?.contents {
                *depth += 1;
                if *depth > SYMLINK_DEPTH {
                    return Err(ELOOP);
                }
                found = self
                    .walk(directory, target, true, depth)?
                    .node
                    .ok_or(ENOENT)?;
            }
            directory = found;
        }
        unreachable!("the final component returns")
    }

    fn existing(&self, at: i32, path: &[u8], follow: bool) -> Result<NodeId, i64> {
        self.lookup(at, path, follow)?.node.ok_or(ENOENT)
    }

    // The parent of a component this call creates or removes, which must be a
    // writable, searchable directory; `.`/`..` and the root name no entry.
    fn creatable(&self, lookup: &Lookup) -> Result<(), i64> {
        if lookup.name.is_empty() {
            return Err(if lookup.node.is_some() {
                EEXIST
            } else {
                ENOENT
            });
        }
        if !self.permits(lookup.parent, 2) || !self.permits(lookup.parent, 1) {
            return Err(EACCES);
        }
        Ok(())
    }

    fn insert_node(&mut self, contents: Contents, permissions: u32) -> NodeId {
        let now = now();
        let node = Node {
            contents,
            permissions,
            uid: 0,
            gid: 0,
            links: 1,
            descriptions: 0,
            accessed: now,
            modified: now,
            changed: now,
            flock: None,
        };
        if let Some(free) = self.nodes.iter().position(Option::is_none) {
            self.nodes[free] = Some(node);
            free
        } else {
            self.nodes.push(Some(node));
            self.nodes.len() - 1
        }
    }

    fn link_entry(&mut self, directory: NodeId, name: &[u8], node: NodeId) -> Result<(), i64> {
        let now = now();
        let parent = self.node_mut(directory)?;
        let Contents::Directory { entries, .. } = &mut parent.contents else {
            return Err(ENOTDIR);
        };
        let index = entries
            .binary_search_by(|(entry, _)| entry.as_slice().cmp(name))
            .err()
            .ok_or(EEXIST)?;
        entries.insert(index, (name.to_vec(), node));
        parent.modified = now;
        parent.changed = now;
        Ok(())
    }

    fn unlink_entry(&mut self, directory: NodeId, name: &[u8]) -> Result<NodeId, i64> {
        let now = now();
        let parent = self.node_mut(directory)?;
        let Contents::Directory { entries, .. } = &mut parent.contents else {
            return Err(ENOTDIR);
        };
        let index = entries
            .binary_search_by(|(entry, _)| entry.as_slice().cmp(name))
            .map_err(|_| ENOENT)?;
        let (_, node) = entries.remove(index);
        parent.modified = now;
        parent.changed = now;
        Ok(node)
    }

    // Drop one name or description; the node's storage goes with the last.
    fn release(&mut self, node: NodeId) {
        let Some(entry) = self.nodes.get(node).and_then(Option::as_ref) else {
            return;
        };
        // A directory, like a file, lives while a name or description holds it.
        if entry.links == 0
            && entry.descriptions == 0
            && node != ROOT
            && let Some(Node {
                contents: Contents::File(bytes),
                ..
            }) = self.nodes[node].take()
        {
            self.stored -= bytes.len();
        }
    }

    fn allocate_guest(&mut self, description: usize) -> Result<i64, i64> {
        let index = match self
            .guests
            .iter()
            .position(|guest| matches!(guest, Guest::Free))
        {
            Some(index) => index,
            None if self.guests.len() < DESCRIPTOR_LIMIT => {
                self.guests.push(Guest::Free);
                self.guests.len() - 1
            }
            None => {
                self.close_description(description);
                return Err(EMFILE);
            }
        };
        self.guests[index] = Guest::Open(description);
        Ok(index as i64)
    }

    fn open_description(
        &mut self,
        node: NodeId,
        readable: bool,
        writable: bool,
        append: bool,
    ) -> Result<i64, i64> {
        self.node_mut(node)?.descriptions += 1;
        let description = Description {
            node,
            readable,
            writable,
            append,
            offset: 0,
            references: 1,
        };
        let index = if let Some(free) = self.descriptions.iter().position(Option::is_none) {
            self.descriptions[free] = Some(description);
            free
        } else {
            self.descriptions.push(Some(description));
            self.descriptions.len() - 1
        };
        self.allocate_guest(index)
    }

    fn close_description(&mut self, description: usize) {
        let Some(open) = self
            .descriptions
            .get_mut(description)
            .and_then(Option::as_mut)
        else {
            return;
        };
        open.references -= 1;
        if open.references != 0 {
            return;
        }
        let node = open.node;
        self.descriptions[description] = None;
        if let Ok(entry) = self.node_mut(node) {
            entry.descriptions -= 1;
            if let Some((_, holders)) = &mut entry.flock {
                holders.retain(|holder| *holder != description);
                if holders.is_empty() {
                    entry.flock = None;
                }
            }
        }
        self.release(node);
    }

    fn close_guest(&mut self, guest: i32) -> Result<(), i64> {
        let index = usize::try_from(guest).map_err(|_| EBADF)?;
        match self.guests.get(index) {
            Some(Guest::ModeledStandard) => {
                self.guests[index] = Guest::Free;
                Ok(())
            }
            Some(Guest::Open(description)) => {
                let description = *description;
                self.guests[index] = Guest::Free;
                self.close_description(description);
                Ok(())
            }
            _ => Err(EBADF),
        }
    }

    fn openat(&mut self, at: i32, path: &[u8], flags: i64, mode: i64) -> Result<i64, i64> {
        let (directory_flag, nofollow_flag) = if self.arm64 {
            (0o40000, 0o100000)
        } else {
            (0o200000, 0o400000)
        };
        let access = flags & O_ACCMODE;
        if access == O_ACCMODE {
            return Err(EINVAL);
        }
        let readable = access != O_WRONLY;
        let writable = access == O_WRONLY || access == O_RDWR;
        let create = flags & O_CREAT != 0;
        let exclusive = create && flags & O_EXCL != 0;
        let follow = flags & nofollow_flag == 0 && !exclusive;
        let lookup = self.lookup(at, path, follow)?;
        let node = match lookup.node {
            Some(node) => {
                if exclusive {
                    return Err(EEXIST);
                }
                node
            }
            None if create => {
                if lookup.trailing_slash {
                    return Err(EISDIR);
                }
                self.creatable(&lookup)?;
                let permissions = (mode as u32) & 0o7777 & !UMASK;
                let node = self.insert_node(Contents::File(Vec::new()), permissions);
                self.link_entry(lookup.parent, &lookup.name, node)?;
                return self.open_description(node, readable, writable, flags & O_APPEND != 0);
            }
            None => return Err(ENOENT),
        };
        match self.node(node)?.contents {
            Contents::Symlink(_) => return Err(ELOOP),
            Contents::Directory { .. } => {
                if writable || create {
                    return Err(EISDIR);
                }
            }
            Contents::File(_) => {
                if flags & directory_flag != 0 {
                    return Err(ENOTDIR);
                }
            }
        }
        if (readable && !self.permits(node, 4)) || (writable && !self.permits(node, 2)) {
            return Err(EACCES);
        }
        if writable && flags & O_TRUNC != 0 {
            self.resize(node, 0)?;
        }
        self.open_description(node, readable, writable, flags & O_APPEND != 0)
    }

    fn resize(&mut self, node: NodeId, length: usize) -> Result<(), i64> {
        let stored = self.stored;
        let entry = self.node_mut(node)?;
        let Contents::File(bytes) = &mut entry.contents else {
            return Err(EINVAL);
        };
        let grown = stored - bytes.len() + length;
        if grown > CAPACITY {
            return Err(if length > CAPACITY { EFBIG } else { ENOSPC });
        }
        bytes.resize(length, 0);
        let now = now();
        entry.modified = now;
        entry.changed = now;
        self.stored = grown;
        Ok(())
    }

    fn lseek(&mut self, guest: i32, offset: i64, whence: i64) -> Result<i64, i64> {
        let description = self.description_of(guest)?;
        let node = self.descriptions[description].as_ref().ok_or(EBADF)?.node;
        let end = match &self.node(node)?.contents {
            Contents::File(bytes) => bytes.len() as i64,
            _ => 0,
        };
        let current = self.descriptions[description].as_ref().ok_or(EBADF)?.offset as i64;
        let base = match whence {
            0 => 0,
            1 => current,
            2 => end,
            _ => return Err(EINVAL),
        };
        let next = base
            .checked_add(offset)
            .filter(|next| *next >= 0)
            .ok_or(EINVAL)?;
        self.description_mut(description)?.offset = next as u64;
        Ok(next)
    }

    fn transfer_in(&mut self, guest: i32, buffer: &mut [u8], at: Option<u64>) -> Result<i64, i64> {
        let description = self.description_of(guest)?;
        let open = self.descriptions[description].as_ref().ok_or(EBADF)?;
        if !open.readable {
            return Err(EBADF);
        }
        let (node, offset) = (open.node, at.unwrap_or(open.offset));
        let now = now();
        let entry = self.node_mut(node)?;
        let Contents::File(bytes) = &entry.contents else {
            return Err(EISDIR);
        };
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(bytes.len());
        let count = (bytes.len() - start).min(buffer.len());
        buffer[..count].copy_from_slice(&bytes[start..start + count]);
        entry.accessed = now;
        if at.is_none() {
            self.description_mut(description)?.offset = offset + count as u64;
        }
        Ok(count as i64)
    }

    fn transfer_out(&mut self, guest: i32, input: &[u8], at: Option<u64>) -> Result<i64, i64> {
        let description = self.description_of(guest)?;
        let open = self.descriptions[description].as_ref().ok_or(EBADF)?;
        if !open.writable {
            return Err(EBADF);
        }
        let (node, append, offset) = (open.node, open.append, open.offset);
        let length = match &self.node(node)?.contents {
            Contents::File(bytes) => bytes.len(),
            _ => return Err(EISDIR),
        };
        // Like Linux, an append-mode description writes at the end even for
        // a positioned write.
        let start = if append {
            length
        } else {
            usize::try_from(at.unwrap_or(offset)).map_err(|_| EFBIG)?
        };
        if input.is_empty() {
            return Ok(0);
        }
        let end = start.checked_add(input.len()).ok_or(EFBIG)?;
        if end > length {
            self.resize(node, end)?;
        }
        let now = now();
        let entry = self.node_mut(node)?;
        if let Contents::File(bytes) = &mut entry.contents {
            bytes[start..end].copy_from_slice(input);
        }
        entry.modified = now;
        entry.changed = now;
        if at.is_none() {
            self.description_mut(description)?.offset = end as u64;
        }
        Ok(input.len() as i64)
    }

    fn mkdirat(&mut self, at: i32, path: &[u8], mode: i64) -> Result<(), i64> {
        let lookup = self.lookup(at, path, false)?;
        if lookup.node.is_some() {
            return Err(EEXIST);
        }
        self.creatable(&lookup)?;
        let node = self.insert_node(
            Contents::Directory {
                parent: lookup.parent,
                entries: Vec::new(),
            },
            (mode as u32) & 0o7777 & !UMASK,
        );
        self.link_entry(lookup.parent, &lookup.name, node)
    }

    fn unlinkat(&mut self, at: i32, path: &[u8], flags: i64) -> Result<(), i64> {
        let lookup = self.lookup(at, path, false)?;
        let Some(node) = lookup.node else {
            return Err(ENOENT);
        };
        let remove_directory = flags & AT_REMOVEDIR != 0;
        if lookup.name.is_empty() {
            return Err(if node == ROOT { EBUSY } else { EINVAL });
        }
        match &self.node(node)?.contents {
            Contents::Directory { entries, .. } => {
                if !remove_directory {
                    return Err(EISDIR);
                }
                if !entries.is_empty() {
                    return Err(ENOTEMPTY);
                }
            }
            _ if remove_directory => return Err(ENOTDIR),
            _ => {}
        }
        if lookup.trailing_slash && !remove_directory {
            return Err(ENOTDIR);
        }
        self.creatable(&lookup)?;
        self.unlink_entry(lookup.parent, &lookup.name)?;
        let now = now();
        let entry = self.node_mut(node)?;
        entry.links = entry.links.saturating_sub(1);
        entry.changed = now;
        self.release(node);
        Ok(())
    }

    fn stat(&self, node: NodeId, buffer: &mut [u8]) -> Result<(), i64> {
        let entry = self.node(node)?;
        let (kind, size, links) = match &entry.contents {
            Contents::File(bytes) => (S_IFREG, bytes.len() as i64, entry.links),
            Contents::Directory { entries, .. } => {
                let subdirectories = entries
                    .iter()
                    .filter(|(_, child)| self.is_directory(*child))
                    .count() as u64;
                (
                    S_IFDIR,
                    (entries.len() as i64 + 2) * DIRECTORY_ENTRY_SIZE,
                    2 + subdirectories,
                )
            }
            Contents::Symlink(target) => (S_IFLNK, target.len() as i64, entry.links),
        };
        let blocks = match &entry.contents {
            Contents::File(bytes) => (bytes.len() as u64).div_ceil(PAGE) * (PAGE / 512),
            _ => 0,
        };
        let mode = kind | entry.permissions;
        let inode = node as u64 + 1;
        if buffer.len() < self.metadata_extent() {
            return Err(EINVAL);
        }
        buffer.fill(0);
        let mut put = |offset: usize, bytes: &[u8]| {
            buffer[offset..offset + bytes.len()].copy_from_slice(bytes);
        };
        if self.arm64 {
            put(8, &inode.to_le_bytes());
            put(16, &mode.to_le_bytes());
            put(20, &(links as u32).to_le_bytes());
            put(24, &entry.uid.to_le_bytes());
            put(28, &entry.gid.to_le_bytes());
            put(48, &size.to_le_bytes());
            put(56, &(PAGE as i32).to_le_bytes());
            put(64, &(blocks as i64).to_le_bytes());
        } else {
            put(8, &inode.to_le_bytes());
            put(16, &links.to_le_bytes());
            put(24, &mode.to_le_bytes());
            put(28, &entry.uid.to_le_bytes());
            put(32, &entry.gid.to_le_bytes());
            put(48, &size.to_le_bytes());
            put(56, &(PAGE as i64).to_le_bytes());
            put(64, &(blocks as i64).to_le_bytes());
        }
        for (offset, time) in [
            (72, entry.accessed),
            (88, entry.modified),
            (104, entry.changed),
        ] {
            put(offset, &time.seconds.to_le_bytes());
            put(offset + 8, &time.nanoseconds.to_le_bytes());
        }
        Ok(())
    }

    /*
        Emit `linux_dirent64` records from the description's position: `.` and
        `..` first, then entries by name. A record that does not fit ends the
        batch; if not even one fits the buffer is too small.
    */
    fn getdents64(&mut self, guest: i32, buffer: &mut [u8]) -> Result<i64, i64> {
        let description = self.description_of(guest)?;
        let open = self.descriptions[description].as_ref().ok_or(EBADF)?;
        let (node, mut position) = (open.node, open.offset as usize);
        let Contents::Directory { parent, entries } = &self.node(node)?.contents else {
            return Err(ENOTDIR);
        };
        let mut written = 0usize;
        loop {
            let (name, child): (&[u8], NodeId) = match position {
                0 => (b".", node),
                1 => (b"..", *parent),
                index => match entries.get(index - 2) {
                    Some((name, child)) => (name, *child),
                    None => break,
                },
            };
            let length = (19 + name.len() + 1).next_multiple_of(8);
            if written + length > buffer.len() {
                if written == 0 {
                    return Err(EINVAL);
                }
                break;
            }
            let kind: u8 = match self.node(child).map(|entry| &entry.contents) {
                Ok(Contents::Directory { .. }) => 4,
                Ok(Contents::Symlink(_)) => 10,
                _ => 8,
            };
            let record = &mut buffer[written..written + length];
            record.fill(0);
            record[0..8].copy_from_slice(&(child as u64 + 1).to_le_bytes());
            record[8..16].copy_from_slice(&(position as i64 + 1).to_le_bytes());
            record[16..18].copy_from_slice(&(length as u16).to_le_bytes());
            record[18] = kind;
            record[19..19 + name.len()].copy_from_slice(name);
            written += length;
            position += 1;
        }
        self.description_mut(description)?.offset = position as u64;
        Ok(written as i64)
    }

    fn chmod(&mut self, node: NodeId, mode: i64) -> Result<(), i64> {
        let now = now();
        let entry = self.node_mut(node)?;
        entry.permissions = (mode as u32) & 0o7777;
        entry.changed = now;
        Ok(())
    }

    // Without CAP_CHOWN an owner may keep its own uid and gid; `-1` keeps one.
    fn chown(&mut self, node: NodeId, uid: i64, gid: i64) -> Result<(), i64> {
        let keeps = |id: i64| id == -1 || id == 0 || id == i64::from(u32::MAX);
        if !keeps(uid) || !keeps(gid) {
            return Err(EPERM);
        }
        let now = now();
        let entry = self.node_mut(node)?;
        entry.changed = now;
        Ok(())
    }

    fn renameat(&mut self, from_at: i32, from: &[u8], to_at: i32, to: &[u8]) -> Result<(), i64> {
        let source = self.lookup(from_at, from, false)?;
        let destination = self.lookup(to_at, to, false)?;
        let Some(node) = source.node else {
            return Err(ENOENT);
        };
        if source.name.is_empty() || destination.name.is_empty() {
            return Err(EBUSY);
        }
        self.creatable(&source)?;
        if !self.permits(destination.parent, 2) || !self.permits(destination.parent, 1) {
            return Err(EACCES);
        }
        let moving_directory = self.is_directory(node);
        if !moving_directory && (source.trailing_slash || destination.trailing_slash) {
            return Err(ENOTDIR);
        }
        if destination.node == Some(node) {
            return Ok(());
        }
        if moving_directory {
            // A directory cannot move beneath itself.
            let mut ancestor = destination.parent;
            loop {
                if ancestor == node {
                    return Err(EINVAL);
                }
                let Contents::Directory { parent, .. } = &self.node(ancestor)?.contents else {
                    break;
                };
                if ancestor == ROOT {
                    break;
                }
                ancestor = *parent;
            }
        }
        if let Some(replaced) = destination.node {
            match (&self.node(replaced)?.contents, moving_directory) {
                (Contents::Directory { entries, .. }, true) => {
                    if !entries.is_empty() {
                        return Err(ENOTEMPTY);
                    }
                }
                (Contents::Directory { .. }, false) => return Err(EISDIR),
                (_, true) => return Err(ENOTDIR),
                _ => {}
            }
            self.unlink_entry(destination.parent, &destination.name)?;
            let entry = self.node_mut(replaced)?;
            entry.links = entry.links.saturating_sub(1);
            self.release(replaced);
        }
        self.unlink_entry(source.parent, &source.name)?;
        self.link_entry(destination.parent, &destination.name, node)?;
        let now = now();
        let entry = self.node_mut(node)?;
        entry.changed = now;
        if let Contents::Directory { parent, .. } = &mut entry.contents {
            *parent = destination.parent;
        }
        Ok(())
    }

    fn linkat(
        &mut self,
        from_at: i32,
        from: &[u8],
        to_at: i32,
        to: &[u8],
        flags: i64,
    ) -> Result<(), i64> {
        let node = self.existing(from_at, from, flags & AT_SYMLINK_FOLLOW != 0)?;
        if self.is_directory(node) {
            return Err(EPERM);
        }
        let destination = self.lookup(to_at, to, false)?;
        if destination.node.is_some() {
            return Err(EEXIST);
        }
        self.creatable(&destination)?;
        self.link_entry(destination.parent, &destination.name, node)?;
        let now = now();
        let entry = self.node_mut(node)?;
        entry.links += 1;
        entry.changed = now;
        Ok(())
    }

    fn symlinkat(&mut self, target: &[u8], at: i32, link: &[u8]) -> Result<(), i64> {
        if target.is_empty() {
            return Err(ENOENT);
        }
        let lookup = self.lookup(at, link, false)?;
        if lookup.node.is_some() {
            return Err(EEXIST);
        }
        self.creatable(&lookup)?;
        let node = self.insert_node(Contents::Symlink(target.to_vec()), 0o777);
        self.link_entry(lookup.parent, &lookup.name, node)
    }

    fn ftruncate(&mut self, guest: i32, length: i64) -> Result<(), i64> {
        let description = self.description_of(guest)?;
        let open = self.descriptions[description].as_ref().ok_or(EBADF)?;
        if !open.writable || length < 0 {
            return Err(EINVAL);
        }
        let length = usize::try_from(length).map_err(|_| EFBIG)?;
        self.resize(open.node, length)
    }

    fn utimens(&mut self, guest: i32, times: &[u8]) -> Result<(), i64> {
        let node = self.node_of(guest)?;
        let field = |index: usize| {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&times[index * 8..index * 8 + 8]);
            i64::from_le_bytes(bytes)
        };
        if times.len() < 32 {
            return Err(EINVAL);
        }
        let now = now();
        let resolve = |seconds: i64, nanoseconds: i64| -> Result<Option<Time>, i64> {
            match nanoseconds {
                UTIME_OMIT => Ok(None),
                UTIME_NOW => Ok(Some(now)),
                0..=999_999_999 => Ok(Some(Time {
                    seconds,
                    nanoseconds,
                })),
                _ => Err(EINVAL),
            }
        };
        let accessed = resolve(field(0), field(1))?;
        let modified = resolve(field(2), field(3))?;
        let entry = self.node_mut(node)?;
        if let Some(accessed) = accessed {
            entry.accessed = accessed;
        }
        if let Some(modified) = modified {
            entry.modified = modified;
        }
        entry.changed = now;
        Ok(())
    }

    /*
        BSD locks belong to the open file description. A conflicting request
        from another description fails with `EWOULDBLOCK` when nonblocking; a
        blocking one could never be released in this single-threaded guest, so
        it reports `EDEADLK` rather than hang the runner.
    */
    fn flock(&mut self, guest: i32, operation: i64) -> Result<(), i64> {
        let description = self.description_of(guest)?;
        let node = self.descriptions[description].as_ref().ok_or(EBADF)?.node;
        let entry = self.node_mut(node)?;
        let requested = match operation & !LOCK_NB {
            LOCK_SH => LockKind::Shared,
            LOCK_EX => LockKind::Exclusive,
            LOCK_UN => {
                if let Some((_, holders)) = &mut entry.flock {
                    holders.retain(|holder| *holder != description);
                    if holders.is_empty() {
                        entry.flock = None;
                    }
                }
                return Ok(());
            }
            _ => return Err(EINVAL),
        };
        let conflict = entry.flock.as_ref().is_some_and(|(kind, holders)| {
            holders.iter().any(|holder| *holder != description)
                && (*kind == LockKind::Exclusive || requested == LockKind::Exclusive)
        });
        if conflict {
            return Err(if operation & LOCK_NB != 0 {
                EAGAIN
            } else {
                EDEADLK
            });
        }
        match &mut entry.flock {
            Some((kind, holders)) => {
                *kind = requested;
                if !holders.contains(&description) {
                    holders.push(description);
                }
            }
            None => entry.flock = Some((requested, vec![description])),
        }
        Ok(())
    }
}

fn now() -> Time {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Time {
        seconds: elapsed.as_secs() as i64,
        nanoseconds: i64::from(elapsed.subsec_nanos()),
    }
}

#[cfg(test)]
mod tests {
    use super::{KernelOperand as K, LinuxFilesystemOperation as Op, PrivateLinuxFilesystem};
    use crate::host_filesystem::LINUX_AT_FDCWD;
    use std::ffi::CString;

    fn path(text: &str) -> CString {
        CString::new(text).expect("test paths have no NUL")
    }

    fn open(fs: &mut PrivateLinuxFilesystem, name: &str, flags: i64) -> i64 {
        let name = path(name);
        fs.call(
            Op::Openat,
            &mut [
                K::Directory(LINUX_AT_FDCWD),
                K::Path(&name),
                K::Value(flags),
                K::Value(0o644),
            ],
        )
    }

    #[test]
    fn written_bytes_read_back_and_state_starts_empty() {
        let mut fs = PrivateLinuxFilesystem::new();
        assert_eq!(open(&mut fs, "/notes.txt", 0), -2);
        let fd = open(&mut fs, "/notes.txt", 0o1101);
        assert_eq!(fd, 3, "the lowest descriptor after the modeled streams");
        assert_eq!(fs.write(fd as i32, b"hello"), 5);
        assert_eq!(fs.close(fd as i32), 0);
        let fd = open(&mut fs, "notes.txt", 0);
        let mut buffer = [0u8; 16];
        assert_eq!(fs.read(fd as i32, &mut buffer), 5);
        assert_eq!(&buffer[..5], b"hello");
        assert_eq!(PrivateLinuxFilesystem::new().nodes.len(), 1);
    }

    #[test]
    fn directories_enumerate_and_refuse_removal_while_populated() {
        let mut fs = PrivateLinuxFilesystem::new();
        let dir = path("/data");
        assert_eq!(
            fs.call(
                Op::Mkdirat,
                &mut [K::Directory(LINUX_AT_FDCWD), K::Path(&dir), K::Value(0o777)]
            ),
            0
        );
        let fd = open(&mut fs, "/data/a", 0o101);
        fs.close(fd as i32);
        assert_eq!(
            fs.call(
                Op::Unlinkat,
                &mut [K::Directory(LINUX_AT_FDCWD), K::Path(&dir), K::Value(0x200)]
            ),
            -39
        );
        let fd = open(&mut fs, "/data", 0o200000);
        let mut buffer = [0u8; 256];
        let written = fs.call(
            Op::Getdents64,
            &mut [K::Descriptor(fd as i32), K::OutputView(&mut buffer)],
        );
        assert_eq!(written, 72, "`.`, `..` and `a`, 24 bytes each");
        assert_eq!(&buffer[67..68], b"a");
        let mut stat = [0u8; 144];
        assert_eq!(
            fs.call(
                Op::Fstat,
                &mut [K::Descriptor(fd as i32), K::Output(&mut stat)]
            ),
            0
        );
        assert_eq!(
            u32::from_le_bytes(stat[24..28].try_into().unwrap()),
            0o040755
        );
        assert_eq!(fs.close(fd as i32), 0);
        assert!(
            open(&mut fs, "/data/a", 0) >= 0,
            "closing a listing keeps its directory"
        );
    }

    #[test]
    fn nested_file_metadata_reports_its_length() {
        let mut fs = PrivateLinuxFilesystem::new();
        for directory in ["/notes", "/notes/current"] {
            let directory = path(directory);
            assert_eq!(
                fs.call(
                    Op::Mkdirat,
                    &mut [
                        K::Directory(LINUX_AT_FDCWD),
                        K::Path(&directory),
                        K::Value(0o755)
                    ]
                ),
                0
            );
        }
        let fd = open(&mut fs, "/notes/current/a.txt", 0o1101 | 0o2000000);
        assert_eq!(fs.write(fd as i32, b"alpha"), 5);
        assert_eq!(fs.close(fd as i32), 0);
        let file = path("/notes/current/a.txt");
        let mut stat = [0u8; 144];
        assert_eq!(
            fs.call(
                Op::Newfstatat,
                &mut [
                    K::Directory(LINUX_AT_FDCWD),
                    K::Path(&file),
                    K::Output(&mut stat),
                    K::Value(0)
                ]
            ),
            0
        );
        assert_eq!(i64::from_le_bytes(stat[48..56].try_into().unwrap()), 5);
    }

    #[test]
    fn readonly_files_refuse_writing_opens() {
        let mut fs = PrivateLinuxFilesystem::new();
        let fd = open(&mut fs, "/locked", 0o101);
        assert_eq!(
            fs.call(Op::Fchmod, &mut [K::Descriptor(fd as i32), K::Value(0o444)]),
            0
        );
        fs.close(fd as i32);
        assert_eq!(open(&mut fs, "/locked", 1), -13);
    }
}
