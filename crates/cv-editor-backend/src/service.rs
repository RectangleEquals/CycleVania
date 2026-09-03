//! **The service** — one handler, both shapes.
//!
//! ⚠ **Every request goes through here regardless of where it came from**, which is what makes
//! transport parity a property rather than a promise. A desktop shell calling in-process would be a
//! second path, and the second path is the one nobody exercises.
//!
//! # A write is scoped to the project, and that is enforced rather than trusted
//!
//! ⚠ **A path arriving over a socket is not a path the backend may open.** *"It is only ever our own
//! editor"* is true right up until it is a tablet on a network, and the check costs one comparison.

use crate::auth::{Auth, AuthError, Origin};
use crate::protocol::{failed, Envelope, Request, Response, PROTOCOL_VERSION};
use cv_assets::project::{self, Descriptor};
use cv_compile::{compile, Severity};
use cv_cvb::parse::parse;
use std::path::{Path, PathBuf};

/// The editor backend.
pub struct Service {
    auth: Auth,
    project: Option<Descriptor>,
    next_id: u64,
}

impl Service {
    /// A backend serving loopback only.
    pub fn loopback() -> Self {
        Service {
            auth: Auth::loopback_only(),
            project: None,
            next_id: 0,
        }
    }

    /// A backend that also listens on the LAN, with a pairing code.
    pub fn with_pairing(code: impl Into<String>, now: u64) -> Self {
        Service {
            auth: Auth::with_pairing(code, now),
            project: None,
            next_id: 0,
        }
    }

    /// The pairing code to print on startup.
    pub fn pairing_code(&self) -> Option<&str> {
        self.auth.pairing_code()
    }

    /// The open project, if any.
    pub fn project(&self) -> Option<&Descriptor> {
        self.project.as_ref()
    }

    /// The next request id a client should use.
    pub fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Handle one request.
    ///
    /// ⚠ **`origin` is the only thing the handler learns about the connection**, and it is used for
    /// exactly one decision: whether a pairing code is required. Everything after that is identical.
    pub fn handle(
        &mut self,
        origin: Origin,
        session_token: Option<&str>,
        request: Envelope<Request>,
        now: u64,
    ) -> Envelope<Response> {
        let id = request.id;
        let body = self.dispatch(origin, session_token, request.body, now);
        Envelope::new(id, body)
    }

    fn dispatch(
        &mut self,
        origin: Origin,
        session_token: Option<&str>,
        request: Request,
        now: u64,
    ) -> Response {
        if request.needs_session() {
            let Some(session_token) = session_token else {
                return failed("not_authorised", AuthError::NotAuthorised);
            };
            if let Err(e) = self.auth.authorise(session_token) {
                return failed("not_authorised", e);
            }
        }

        match request {
            Request::Hello {
                version,
                label,
                code,
            } => {
                // ⚠ **Refused rather than adapted.** A backend that accepted an older client would
                // have to know what changed, which is a compatibility layer nobody has written.
                if version != PROTOCOL_VERSION {
                    return failed(
                        "version",
                        format!("this backend speaks protocol {PROTOCOL_VERSION}, not {version}"),
                    );
                }
                match self.auth.connect(origin, code.as_deref(), &label, now) {
                    Ok(session) => Response::Welcome {
                        version: PROTOCOL_VERSION,
                        session_token: session.session_token,
                        paired: origin == Origin::Lan,
                    },
                    Err(e) => failed("auth", e),
                }
            }

            Request::OpenProject { path } => match project::load(Path::new(&path)) {
                Ok(d) => {
                    let response = Response::Project {
                        path: d.path.display().to_string(),
                        cyclevania: d.cyclevania.clone(),
                        content_root: d.content_root.clone(),
                    };
                    self.project = Some(d);
                    response
                }
                Err(e) => failed("no_project", e),
            },

            Request::ReadFile { path } => match self.resolve(&path) {
                Err(e) => e,
                Ok(full) => match std::fs::read_to_string(&full) {
                    Ok(body) => Response::File { path, body },
                    Err(e) => failed("no_file", e),
                },
            },

            Request::WriteFile { path, body } => match self.resolve(&path) {
                Err(e) => e,
                Ok(full) => {
                    if let Some(parent) = full.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    match std::fs::write(&full, &body) {
                        Ok(()) => Response::Written {
                            path,
                            bytes: body.len(),
                        },
                        Err(e) => failed("write_failed", e),
                    }
                }
            },

            Request::Check => {
                let Some(descriptor) = &self.project else {
                    return failed("no_project", "open a project first");
                };
                let files = project::files_under(&descriptor.schematic_dir(), &[".cvs"]);
                let mut errors = 0usize;
                let mut findings = Vec::new();
                for file in &files {
                    let name = file
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let Ok(src) = std::fs::read_to_string(file) else {
                        errors += 1;
                        findings.push(format!("{name}: unreadable"));
                        continue;
                    };
                    match parse(&src) {
                        Err(e) => {
                            errors += 1;
                            findings.push(format!("{name}: {e}"));
                        }
                        Ok(doc) => {
                            for finding in &compile(&doc).findings().findings {
                                if finding.severity == Severity::Error {
                                    errors += 1;
                                }
                                findings.push(format!("{name}: {finding}"));
                            }
                        }
                    }
                }
                Response::Checked {
                    schematics: files.len(),
                    errors,
                    findings,
                }
            }

            Request::ListSessions => Response::Sessions {
                labels: self
                    .auth
                    .sessions()
                    .iter()
                    .map(|s| s.label.clone())
                    .collect(),
            },

            Request::Revoke { session_token } => match self.auth.revoke(&session_token) {
                Ok(()) => Response::Done,
                Err(e) => failed("not_authorised", e),
            },

            Request::Goodbye => {
                if let Some(t) = session_token {
                    let _ = self.auth.revoke(t);
                }
                Response::Done
            }
        }
    }

    /// A content-relative path, checked to stay inside the project.
    ///
    /// ⚠ **Refused rather than trusted.** *"It is only ever our own editor"* is true right up until it
    /// is a tablet on a network, and the check costs one comparison.
    fn resolve(&self, relative: &str) -> Result<PathBuf, Response> {
        let Some(descriptor) = &self.project else {
            return Err(failed("no_project", "open a project first"));
        };
        let root = descriptor.content_dir();
        let joined = root.join(relative);

        // A textual check, because the file need not exist yet — canonicalising would refuse every
        // write of a new file.
        let escapes = joined
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
            || Path::new(relative).is_absolute();
        if escapes {
            return Err(failed(
                "outside_project",
                format!("{relative} leaves the content root"),
            ));
        }
        Ok(joined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMATIC: &str = "\
Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=sch_01
   Begin Graph Name=\"grants\" Role=Hook Id=grf_01
      Begin Node Id=n_0001 Op=array.make Pos=(0,0)
         Pin (Name=out, Dir=Out, Type=Array<Ref'/Core/Unlock'>, To=(n_0002.value))
      End Node
      Begin Node Id=n_0002 Op=core.return Pos=(160,0)
         Pin (Name=value, Dir=In, Type=Array<Ref'/Core/Unlock'>)
      End Node
   End Graph
End Schematic
";

    fn skeleton(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cv-editor-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content/schematics")).unwrap();
        std::fs::write(
            dir.join("game.cvproj"),
            r#"{"cyclevania":"0.2.0","paths":{"contentRoot":"content"}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("content/schematics/hookshot.cvs"), SCHEMATIC).unwrap();
        dir
    }

    /// Connect and open, returning the session token.
    fn session(service: &mut Service, origin: Origin, code: Option<&str>, dir: &Path) -> String {
        let hello = Envelope::new(
            1,
            Request::Hello {
                version: PROTOCOL_VERSION,
                label: "test".into(),
                code: code.map(str::to_string),
            },
        );
        let Response::Welcome { session_token, .. } = service.handle(origin, None, hello, 0).body
        else {
            panic!("hello was refused");
        };
        let open = Envelope::new(
            2,
            Request::OpenProject {
                path: dir.join("game.cvproj").display().to_string(),
            },
        );
        assert!(service
            .handle(origin, Some(&session_token), open, 0)
            .body
            .ok());
        session_token
    }

    #[test]
    fn a_loopback_session_opens_reads_writes_and_checks() {
        let dir = skeleton("loopback");
        let mut service = Service::loopback();
        let session_token = session(&mut service, Origin::Loopback, None, &dir);

        let read = service.handle(
            Origin::Loopback,
            Some(&session_token),
            Envelope::new(
                3,
                Request::ReadFile {
                    path: "schematics/hookshot.cvs".into(),
                },
            ),
            0,
        );
        let Response::File { body, .. } = read.body else {
            panic!("expected a file");
        };
        assert!(body.contains("Begin Schematic"));

        let write = service.handle(
            Origin::Loopback,
            Some(&session_token),
            Envelope::new(
                4,
                Request::WriteFile {
                    path: "schematics/second.cvs".into(),
                    body: SCHEMATIC.replace("sch_01", "sch_02"),
                },
            ),
            0,
        );
        assert!(write.body.ok(), "{:?}", write.body);

        let checked = service.handle(
            Origin::Loopback,
            Some(&session_token),
            Envelope::new(5, Request::Check),
            0,
        );
        let Response::Checked {
            schematics, errors, ..
        } = checked.body
        else {
            panic!("expected a check");
        };
        assert_eq!(schematics, 2, "the written file is part of the project");
        assert_eq!(errors, 0);
    }

    #[test]
    fn the_same_session_works_identically_over_the_lan() {
        // ⚠ M16's green condition. Nothing but the pairing differs, and that is checked by running the
        // *same* sequence against the *same* handler.
        let dir = skeleton("parity");

        let mut local = Service::loopback();
        let local_session_token = session(&mut local, Origin::Loopback, None, &dir);
        let local_check = local
            .handle(
                Origin::Loopback,
                Some(&local_session_token),
                Envelope::new(9, Request::Check),
                0,
            )
            .body;

        let mut remote = Service::with_pairing("ABC123", 0);
        let remote_session_token = session(&mut remote, Origin::Lan, Some("ABC123"), &dir);
        let remote_check = remote
            .handle(
                Origin::Lan,
                Some(&remote_session_token),
                Envelope::new(9, Request::Check),
                0,
            )
            .body;

        assert_eq!(
            local_check, remote_check,
            "a tablet and a desk must see the same project"
        );
    }

    #[test]
    fn a_write_lands_identically_from_either_origin() {
        let dir = skeleton("write-parity");
        let write = |service: &mut Service, origin, session_token: &str, name: &str| {
            service
                .handle(
                    origin,
                    Some(session_token),
                    Envelope::new(
                        10,
                        Request::WriteFile {
                            path: format!("schematics/{name}.cvs"),
                            body: SCHEMATIC.to_string(),
                        },
                    ),
                    0,
                )
                .body
        };

        let mut local = Service::loopback();
        let lt = session(&mut local, Origin::Loopback, None, &dir);
        let mut remote = Service::with_pairing("ABC123", 0);
        let rt = session(&mut remote, Origin::Lan, Some("ABC123"), &dir);

        let a = write(&mut local, Origin::Loopback, &lt, "from_desk");
        let b = write(&mut remote, Origin::Lan, &rt, "from_desk");
        assert_eq!(a, b);
        assert!(dir.join("content/schematics/from_desk.cvs").exists());
    }

    #[test]
    fn a_request_without_a_session_is_refused() {
        let dir = skeleton("nosession");
        let mut service = Service::loopback();
        let r = service.handle(
            Origin::Loopback,
            None,
            Envelope::new(
                1,
                Request::OpenProject {
                    path: dir.join("game.cvproj").display().to_string(),
                },
            ),
            0,
        );
        assert!(!r.body.ok());
    }

    #[test]
    fn a_path_that_leaves_the_content_root_is_refused() {
        // ⚠ "It is only ever our own editor" is true right up until it is a tablet on a network.
        let dir = skeleton("escape");
        let mut service = Service::loopback();
        let session_token = session(&mut service, Origin::Loopback, None, &dir);

        for escape in ["../../etc/passwd", "schematics/../../secrets.txt"] {
            let r = service.handle(
                Origin::Loopback,
                Some(&session_token),
                Envelope::new(
                    3,
                    Request::ReadFile {
                        path: escape.into(),
                    },
                ),
                0,
            );
            let Response::Failed { code, .. } = r.body else {
                panic!("{escape} was not refused");
            };
            assert_eq!(code, "outside_project");
        }
    }

    #[test]
    fn a_write_of_a_new_file_is_allowed_even_though_it_does_not_exist_yet() {
        // ⚠ Canonicalising the path would have refused every new file, which is most of authoring.
        let dir = skeleton("newfile");
        let mut service = Service::loopback();
        let session_token = session(&mut service, Origin::Loopback, None, &dir);
        let r = service.handle(
            Origin::Loopback,
            Some(&session_token),
            Envelope::new(
                3,
                Request::WriteFile {
                    path: "schematics/deep/nested/new.cvs".into(),
                    body: SCHEMATIC.into(),
                },
            ),
            0,
        );
        assert!(r.body.ok(), "{:?}", r.body);
        assert!(dir.join("content/schematics/deep/nested/new.cvs").exists());
    }

    #[test]
    fn a_client_speaking_another_protocol_is_refused_rather_than_adapted() {
        // ⚠ Accepting an older client would need a compatibility layer nobody has written.
        let mut service = Service::loopback();
        let r = service.handle(
            Origin::Loopback,
            None,
            Envelope::new(
                1,
                Request::Hello {
                    version: PROTOCOL_VERSION + 1,
                    label: "future".into(),
                    code: None,
                },
            ),
            0,
        );
        let Response::Failed { code, .. } = r.body else {
            panic!("expected a refusal");
        };
        assert_eq!(code, "version");
    }

    #[test]
    fn a_lan_client_with_a_bad_code_never_gets_a_session() {
        let mut service = Service::with_pairing("ABC123", 0);
        let r = service.handle(
            Origin::Lan,
            None,
            Envelope::new(
                1,
                Request::Hello {
                    version: PROTOCOL_VERSION,
                    label: "tablet".into(),
                    code: Some("WRONG!".into()),
                },
            ),
            0,
        );
        assert!(!r.body.ok());
    }

    #[test]
    fn every_response_carries_the_id_of_its_request() {
        let dir = skeleton("ids");
        let mut service = Service::loopback();
        let session_token = session(&mut service, Origin::Loopback, None, &dir);
        for id in [3u64, 40, 500] {
            let r = service.handle(
                Origin::Loopback,
                Some(&session_token),
                Envelope::new(id, Request::ListSessions),
                0,
            );
            assert_eq!(r.id, id);
        }
    }

    #[test]
    fn connected_machines_are_listed_and_can_be_closed_from_the_one_that_granted_it() {
        let dir = skeleton("sessions");
        let mut service = Service::with_pairing("ABC123", 0);
        let desk = session(&mut service, Origin::Loopback, None, &dir);
        let tablet = session(&mut service, Origin::Lan, Some("ABC123"), &dir);

        let listed = service.handle(
            Origin::Loopback,
            Some(&desk),
            Envelope::new(7, Request::ListSessions),
            0,
        );
        let Response::Sessions { labels } = listed.body else {
            panic!("expected sessions");
        };
        assert_eq!(labels.len(), 2);

        let revoked = service.handle(
            Origin::Loopback,
            Some(&desk),
            Envelope::new(
                8,
                Request::Revoke {
                    session_token: tablet.clone(),
                },
            ),
            0,
        );
        assert!(revoked.body.ok());

        let after = service.handle(
            Origin::Lan,
            Some(&tablet),
            Envelope::new(9, Request::ListSessions),
            0,
        );
        assert!(!after.body.ok(), "a revoked session_token stops working");
    }

    #[test]
    fn goodbye_closes_the_session_it_arrived_on() {
        let dir = skeleton("goodbye");
        let mut service = Service::loopback();
        let session_token = session(&mut service, Origin::Loopback, None, &dir);
        assert!(service
            .handle(
                Origin::Loopback,
                Some(&session_token),
                Envelope::new(3, Request::Goodbye),
                0
            )
            .body
            .ok());
        assert!(!service
            .handle(
                Origin::Loopback,
                Some(&session_token),
                Envelope::new(4, Request::ListSessions),
                0
            )
            .body
            .ok());
    }

    #[test]
    fn a_check_before_a_project_is_open_says_so() {
        let mut service = Service::loopback();
        let hello = Envelope::new(
            1,
            Request::Hello {
                version: PROTOCOL_VERSION,
                label: "t".into(),
                code: None,
            },
        );
        let Response::Welcome { session_token, .. } =
            service.handle(Origin::Loopback, None, hello, 0).body
        else {
            panic!("hello");
        };
        let r = service.handle(
            Origin::Loopback,
            Some(&session_token),
            Envelope::new(2, Request::Check),
            0,
        );
        let Response::Failed { code, .. } = r.body else {
            panic!("expected a refusal");
        };
        assert_eq!(code, "no_project");
    }

    #[test]
    fn the_descriptor_round_trips_through_open() {
        let dir = skeleton("descriptor");
        let mut service = Service::loopback();
        session(&mut service, Origin::Loopback, None, &dir);
        let d = service.project().expect("a project is open");
        assert_eq!(d.cyclevania, "0.2.0");
        assert_eq!(d.content_root, "content");
    }
}
