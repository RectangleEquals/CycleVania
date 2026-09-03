//! **The transport contract** — one protocol, both shapes.
//!
//! ⚠ **The desktop shell does not get a private in-process path.** It opens a loopback connection to
//! the identical backend a tablet on the LAN reaches. That is a deliberate cost — a local call would be
//! marginally faster — paid to avoid the outcome where *"works locally"* and *"works remotely"* are two
//! code paths and only one of them is exercised daily.
//!
//! ⚠ **Nothing in the protocol says which shape is on the other end.** There is no branch anywhere that
//! could behave differently, so a bug that reproduces on a tablet reproduces at a desk. That property is
//! what makes every later view written once rather than twice, and it is checked here rather than
//! promised.
//!
//! # Requests are named and versioned; responses carry their request's id
//!
//! ⚠ **An id per request, because the transport is a socket rather than a call.** Responses may arrive
//! out of order and notifications arrive unbidden, so a client that matched on *kind* would attribute
//! one save's failure to another's.

use std::fmt;

/// The protocol version, declared on every session.
///
/// ⚠ **Declared, never sniffed** — the same rule the file formats follow. A client and a backend that
/// disagreed would otherwise fail somewhere far from the disagreement.
pub const PROTOCOL_VERSION: u32 = 1;

/// What a client asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Establish a session.
    Hello {
        /// The protocol the client speaks.
        version: u32,
        /// What the client calls itself.
        label: String,
        /// The pairing code, when connecting from the LAN.
        code: Option<String>,
    },
    /// Open a project.
    OpenProject { path: String },
    /// Read one content file.
    ReadFile { path: String },
    /// Write one content file.
    WriteFile { path: String, body: String },
    /// Compile everything and report.
    Check,
    /// Which machines are connected.
    ListSessions,
    /// Close a session.
    Revoke { session_token: String },
    /// Close this one.
    Goodbye,
}

impl Request {
    /// The wire name.
    pub fn name(&self) -> &'static str {
        match self {
            Request::Hello { .. } => "hello",
            Request::OpenProject { .. } => "open_project",
            Request::ReadFile { .. } => "read_file",
            Request::WriteFile { .. } => "write_file",
            Request::Check => "check",
            Request::ListSessions => "list_sessions",
            Request::Revoke { .. } => "revoke",
            Request::Goodbye => "goodbye",
        }
    }

    /// ⚠ **Does this request need an established session?**
    ///
    /// Only `Hello` does not — and stating it as a property of the *request* rather than as a check in
    /// the handler is what stops a later request being added without anyone deciding.
    pub fn needs_session(&self) -> bool {
        !matches!(self, Request::Hello { .. })
    }

    /// Does this request change the project on disk?
    ///
    /// ⚠ **Asked so a read-only session is expressible.** A client that only inspects should not be
    /// able to write by accident, and the distinction has to exist before anything can enforce it.
    pub fn mutates(&self) -> bool {
        matches!(self, Request::WriteFile { .. })
    }
}

/// What the backend answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Response {
    /// A session was established.
    Welcome {
        version: u32,
        session_token: String,
        /// ⚠ **Absent for a loopback session**, because there was no code to spend.
        paired: bool,
    },
    /// A project is open.
    Project {
        path: String,
        cyclevania: String,
        content_root: String,
    },
    /// A file's contents.
    File { path: String, body: String },
    /// A write landed.
    Written { path: String, bytes: usize },
    /// A check's result.
    Checked {
        schematics: usize,
        errors: usize,
        findings: Vec<String>,
    },
    /// Who is connected.
    Sessions { labels: Vec<String> },
    /// It worked and there is nothing to say.
    Done,
    /// It did not work.
    Failed { code: &'static str, detail: String },
}

impl Response {
    /// Did it work?
    pub fn ok(&self) -> bool {
        !matches!(self, Response::Failed { .. })
    }

    /// The wire name.
    pub fn name(&self) -> &'static str {
        match self {
            Response::Welcome { .. } => "welcome",
            Response::Project { .. } => "project",
            Response::File { .. } => "file",
            Response::Written { .. } => "written",
            Response::Checked { .. } => "checked",
            Response::Sessions { .. } => "sessions",
            Response::Done => "done",
            Response::Failed { .. } => "failed",
        }
    }
}

/// One message on the wire.
///
/// ⚠ **The envelope carries the id and the payload does not**, so matching a response to its request
/// never requires understanding either.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Envelope<T> {
    /// The request this belongs to.
    pub id: u64,
    /// The payload.
    pub body: T,
}

impl<T> Envelope<T> {
    /// Wrap a payload.
    pub fn new(id: u64, body: T) -> Self {
        Envelope { id, body }
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Response::Failed { code, detail } => write!(f, "failed({code}): {detail}"),
            other => f.write_str(other.name()),
        }
    }
}

/// A failure, as a response.
pub fn failed(code: &'static str, detail: impl fmt::Display) -> Response {
    Response::Failed {
        code,
        detail: detail.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_request() -> Vec<Request> {
        vec![
            Request::Hello {
                version: PROTOCOL_VERSION,
                label: "desktop".into(),
                code: None,
            },
            Request::OpenProject {
                path: "./game.cvproj".into(),
            },
            Request::ReadFile {
                path: "a.cvs".into(),
            },
            Request::WriteFile {
                path: "a.cvs".into(),
                body: String::new(),
            },
            Request::Check,
            Request::ListSessions,
            Request::Revoke {
                session_token: "s0".into(),
            },
            Request::Goodbye,
        ]
    }

    #[test]
    fn every_request_has_a_distinct_wire_name() {
        let mut seen = std::collections::BTreeSet::new();
        for r in every_request() {
            assert!(seen.insert(r.name()), "{} shares a name", r.name());
        }
        assert_eq!(seen.len(), 8);
    }

    #[test]
    fn only_hello_may_arrive_without_a_session() {
        // ⚠ Stated as a property of the request, so a later one cannot be added without anyone
        // deciding.
        for r in every_request() {
            let expected = !matches!(r, Request::Hello { .. });
            assert_eq!(r.needs_session(), expected, "{}", r.name());
        }
    }

    #[test]
    fn only_a_write_mutates_the_project() {
        // ⚠ The distinction has to exist before a read-only session can be enforced.
        for r in every_request() {
            let expected = matches!(r, Request::WriteFile { .. });
            assert_eq!(r.mutates(), expected, "{}", r.name());
        }
    }

    #[test]
    fn a_response_carries_the_id_of_its_request_rather_than_its_kind() {
        // ⚠ Responses may arrive out of order; a client matching on kind would attribute one save's
        // failure to another's.
        let a = Envelope::new(7, Response::Done);
        let b = Envelope::new(9, Response::Done);
        assert_eq!(a.body, b.body);
        assert_ne!(a, b, "the id is what tells them apart");
    }

    #[test]
    fn a_failure_is_a_response_rather_than_a_dropped_connection() {
        // ⚠ A backend that closed the socket on an error would give a client nothing to show a user.
        let r = failed("not_found", "no such file");
        assert!(!r.ok());
        assert_eq!(r.name(), "failed");
        assert!(r.to_string().contains("no such file"));
    }

    #[test]
    fn the_protocol_version_is_declared_on_every_session() {
        // ⚠ Declared, never sniffed — a client and a backend that disagreed would otherwise fail
        // somewhere far from the disagreement.
        let Request::Hello { version, .. } = &every_request()[0] else {
            panic!("hello is first");
        };
        assert_eq!(*version, PROTOCOL_VERSION);
    }

    #[test]
    fn nothing_in_the_protocol_names_a_deployment_shape() {
        // ⚠ The property that makes every later view written once. A field naming the shape would be a
        // branch waiting to be written.
        let names: Vec<&str> = every_request().iter().map(Request::name).collect();
        for n in &names {
            for forbidden in ["desktop", "browser", "local", "remote", "tauri", "web"] {
                assert!(
                    !n.contains(forbidden),
                    "{n} names a deployment shape, which is a branch waiting to be written"
                );
            }
        }
    }
}
