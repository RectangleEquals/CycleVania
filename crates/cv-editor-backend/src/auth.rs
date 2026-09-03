//! **LAN auth** — a pairing code, not an account.
//!
//! ⚠ **The threat is a colleague's laptop on the same office network, not the internet.** The backend
//! binds to a LAN so a developer can drive it from a tablet or a second machine; it is never a public
//! service, and designing it as one would buy an account system nobody wants and a password nobody
//! would set.
//!
//! | | |
//! |---|---|
//! | **loopback** | ⚠ **no token required** — a process that can reach `127.0.0.1` can already read the project off disk, so a check there protects nothing and costs a login on every launch |
//! | **anything else** | a **pairing code**, printed on startup, exchanged once for a session token |
//!
//! ⚠ **The code is short-lived and single-use.** It is read off a screen and typed on a tablet, so it
//! is six characters — which is only safe *because* it expires and is spent on first use. A code that
//! stayed valid would be a six-character password on a machine that has a filesystem.
//!
//! ⚠ **The backend refuses a non-loopback bind with no pairing configured**, rather than serving
//! openly. Defaulting to open would make the failure mode of an unconfigured backend *exposure*, and a
//! tool whose insecure state is its quiet one gets shipped that way.
//!
//! # Deliberately absent
//!
//! ⚠ User accounts, roles, TLS termination, and any notion of a remote *project*. The backend serves
//! one project on one machine's disk. A team wanting shared editing wants version control, which they
//! already have.

use std::collections::BTreeMap;
use std::fmt;

/// Where a connection came from.
///
/// ⚠ **Two cases and no third.** A "trusted network" tier would be a decision nobody can make
/// correctly from inside a process — the machine cannot tell an office LAN from a café's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The same machine.
    Loopback,
    /// Somewhere else on the network.
    Lan,
}

/// How long a pairing code lives, in seconds.
///
/// ⚠ **Minutes, not hours.** It exists for the interval between reading it off one screen and typing it
/// into another.
pub const PAIRING_TTL_SECS: u64 = 300;

/// How many characters a pairing code has.
pub const PAIRING_LEN: usize = 6;

/// Why a connection was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthError {
    /// The backend is bound beyond loopback with no pairing configured.
    ///
    /// ⚠ **A refusal, not a warning.** The failure mode of an unconfigured backend must not be
    /// *exposure*.
    OpenBindRefused,
    /// The pairing code does not match.
    BadCode,
    /// The pairing code has expired.
    CodeExpired,
    /// The pairing code has already been spent.
    ///
    /// ⚠ **Distinct from [`BadCode`](AuthError::BadCode).** *"Someone already used this"* is a fact
    /// the person holding the screen needs, and it is how they notice a code was read by someone else.
    CodeSpent,
    /// No token, or a token that is not current.
    NotAuthorised,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::OpenBindRefused => write!(
                f,
                "refusing to serve beyond loopback with no pairing configured — an unconfigured \
                 backend must fail closed"
            ),
            AuthError::BadCode => write!(f, "that pairing code does not match"),
            AuthError::CodeExpired => write!(f, "that pairing code has expired"),
            AuthError::CodeSpent => write!(
                f,
                "that pairing code has already been used — if it was not you, close the session"
            ),
            AuthError::NotAuthorised => write!(f, "not authorised"),
        }
    }
}

impl std::error::Error for AuthError {}

/// One connected client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    /// The opaque token.
    pub session_token: String,
    /// Where it connected from.
    pub origin: Origin,
    /// What it called itself, for the *"which machines are connected"* list.
    pub label: String,
}

/// A pairing code awaiting exchange.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Pairing {
    code: String,
    issued_at: u64,
    spent: bool,
}

/// The backend's authorisation state.
///
/// ⚠ **Time is passed in rather than read.** A clock inside this type would make every expiry test a
/// sleep, and the one behaviour worth testing hardest is the one that only happens after a delay.
#[derive(Clone, Debug, Default)]
pub struct Auth {
    /// Whether the listener is bound beyond loopback.
    lan_enabled: bool,
    pairing: Option<Pairing>,
    sessions: BTreeMap<String, Session>,
    next: u64,
}

impl Auth {
    /// A backend serving loopback only.
    pub fn loopback_only() -> Self {
        Auth::default()
    }

    /// A backend that also listens on the LAN, with a pairing code issued now.
    pub fn with_pairing(code: impl Into<String>, now: u64) -> Self {
        Auth {
            lan_enabled: true,
            pairing: Some(Pairing {
                code: code.into(),
                issued_at: now,
                spent: false,
            }),
            ..Auth::default()
        }
    }

    /// ⚠ **Bind beyond loopback with no pairing** — which is refused.
    pub fn lan_without_pairing() -> Self {
        Auth {
            lan_enabled: true,
            ..Auth::default()
        }
    }

    /// May this backend accept a LAN connection at all?
    pub fn accepts_lan(&self) -> Result<(), AuthError> {
        if self.lan_enabled && self.pairing.is_none() {
            return Err(AuthError::OpenBindRefused);
        }
        Ok(())
    }

    /// The current pairing code, for printing on startup.
    pub fn pairing_code(&self) -> Option<&str> {
        self.pairing
            .as_ref()
            .filter(|p| !p.spent)
            .map(|p| p.code.as_str())
    }

    /// Connect.
    ///
    /// ⚠ **Loopback needs no code**, and passing one is neither required nor an error: the check would
    /// protect nothing, and rejecting a needless code would be a rule with no purpose that a client
    /// author has to learn.
    pub fn connect(
        &mut self,
        origin: Origin,
        code: Option<&str>,
        label: &str,
        now: u64,
    ) -> Result<Session, AuthError> {
        if origin == Origin::Lan {
            self.accepts_lan()?;
            let Some(pairing) = self.pairing.as_mut() else {
                return Err(AuthError::OpenBindRefused);
            };
            if pairing.spent {
                return Err(AuthError::CodeSpent);
            }
            if now.saturating_sub(pairing.issued_at) > PAIRING_TTL_SECS {
                return Err(AuthError::CodeExpired);
            }
            // ⚠ **Expiry before match**, so an expired code reports *expired* rather than *wrong* —
            // otherwise the person holding the screen goes looking for a typo.
            if code != Some(pairing.code.as_str()) {
                return Err(AuthError::BadCode);
            }
            pairing.spent = true;
        }

        self.next += 1;
        let session_token = format!("s{:016x}", cv_determinism::hash::mix64(self.next));
        let session = Session {
            session_token: session_token.clone(),
            origin,
            label: label.to_string(),
        };
        self.sessions.insert(session_token, session.clone());
        Ok(session)
    }

    /// Is this token current?
    pub fn authorise(&self, session_token: &str) -> Result<&Session, AuthError> {
        self.sessions
            .get(session_token)
            .ok_or(AuthError::NotAuthorised)
    }

    /// ⚠ **Which machines are connected** — visible, because the honest answer to a colleague joining
    /// by accident is that the person at the keyboard closes it.
    pub fn sessions(&self) -> Vec<&Session> {
        self.sessions.values().collect()
    }

    /// Close one.
    pub fn revoke(&mut self, session_token: &str) -> Result<(), AuthError> {
        self.sessions
            .remove(session_token)
            .map(|_| ())
            .ok_or(AuthError::NotAuthorised)
    }

    /// Issue a fresh pairing code, invalidating any previous one.
    pub fn reissue(&mut self, code: impl Into<String>, now: u64) {
        self.lan_enabled = true;
        self.pairing = Some(Pairing {
            code: code.into(),
            issued_at: now,
            spent: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_connects_without_a_code() {
        // ⚠ A process that can reach 127.0.0.1 can already read the project off disk.
        let mut auth = Auth::loopback_only();
        let s = auth.connect(Origin::Loopback, None, "desktop", 0).unwrap();
        assert_eq!(s.origin, Origin::Loopback);
        assert!(auth.authorise(&s.session_token).is_ok());
    }

    #[test]
    fn loopback_with_a_needless_code_is_accepted_rather_than_refused() {
        // ⚠ Rejecting it would be a rule with no purpose that a client author has to learn.
        let mut auth = Auth::loopback_only();
        assert!(auth
            .connect(Origin::Loopback, Some("ABC123"), "desktop", 0)
            .is_ok());
    }

    #[test]
    fn a_lan_bind_with_no_pairing_is_refused_rather_than_served_openly() {
        // ⚠ The failure mode of an unconfigured backend must not be exposure.
        let mut auth = Auth::lan_without_pairing();
        assert_eq!(auth.accepts_lan(), Err(AuthError::OpenBindRefused));
        assert_eq!(
            auth.connect(Origin::Lan, Some("ABC123"), "tablet", 0),
            Err(AuthError::OpenBindRefused)
        );
        let err = AuthError::OpenBindRefused.to_string();
        assert!(err.contains("fail closed"));
    }

    #[test]
    fn a_lan_client_exchanges_the_pairing_code_for_a_session_token() {
        let mut auth = Auth::with_pairing("ABC123", 100);
        assert_eq!(auth.pairing_code(), Some("ABC123"));
        let s = auth
            .connect(Origin::Lan, Some("ABC123"), "tablet", 120)
            .unwrap();
        assert_eq!(s.origin, Origin::Lan);
        assert_eq!(s.label, "tablet");
        assert!(auth.authorise(&s.session_token).is_ok());
    }

    #[test]
    fn a_pairing_code_is_single_use() {
        // ⚠ A code that stayed valid would be a six-character password on a machine with a filesystem.
        let mut auth = Auth::with_pairing("ABC123", 100);
        auth.connect(Origin::Lan, Some("ABC123"), "tablet", 110)
            .unwrap();
        assert_eq!(
            auth.connect(Origin::Lan, Some("ABC123"), "laptop", 120),
            Err(AuthError::CodeSpent)
        );
        assert_eq!(auth.pairing_code(), None, "and it stops being displayed");
    }

    #[test]
    fn spent_and_wrong_are_different_answers() {
        // ⚠ "Someone already used this" is how the person holding the screen notices it was read by
        // someone else.
        assert_ne!(AuthError::CodeSpent, AuthError::BadCode);
        assert!(AuthError::CodeSpent
            .to_string()
            .contains("close the session"));
    }

    #[test]
    fn an_expired_code_reports_expired_rather_than_wrong() {
        // ⚠ Otherwise the person holding the screen goes looking for a typo.
        let mut auth = Auth::with_pairing("ABC123", 100);
        let late = 100 + PAIRING_TTL_SECS + 1;
        assert_eq!(
            auth.connect(Origin::Lan, Some("ABC123"), "tablet", late),
            Err(AuthError::CodeExpired)
        );
        assert_eq!(
            auth.connect(Origin::Lan, Some("WRONG!"), "tablet", late),
            Err(AuthError::CodeExpired),
            "expiry is checked before the match"
        );
    }

    #[test]
    fn a_wrong_code_inside_the_window_is_refused_and_does_not_spend_it() {
        let mut auth = Auth::with_pairing("ABC123", 100);
        assert_eq!(
            auth.connect(Origin::Lan, Some("WRONG!"), "tablet", 110),
            Err(AuthError::BadCode)
        );
        assert_eq!(
            auth.pairing_code(),
            Some("ABC123"),
            "a failed attempt must not burn the code, or a typo locks the tablet out"
        );
        assert!(auth
            .connect(Origin::Lan, Some("ABC123"), "tablet", 110)
            .is_ok());
    }

    #[test]
    fn a_lan_client_with_no_code_at_all_is_refused() {
        let mut auth = Auth::with_pairing("ABC123", 100);
        assert_eq!(
            auth.connect(Origin::Lan, None, "tablet", 110),
            Err(AuthError::BadCode)
        );
    }

    #[test]
    fn a_pairing_code_is_short_because_it_is_typed_off_a_screen() {
        assert_eq!(PAIRING_LEN, 6);
        // ⚠ Six characters is only safe *because* the window is short. A const comparison the
        // compiler folds away proves nothing, so this states the bound the way a reader would check it.
        const MAX_SAFE_TTL: u64 = 600;
        assert_eq!(
            PAIRING_TTL_SECS.min(MAX_SAFE_TTL),
            PAIRING_TTL_SECS,
            "a six-character code needs a window measured in minutes"
        );
    }

    #[test]
    fn every_session_is_listed_and_any_of_them_can_be_closed() {
        // ⚠ The honest answer to a colleague joining by accident is that the person at the keyboard
        // closes it — which requires seeing them.
        let mut auth = Auth::with_pairing("ABC123", 0);
        let desk = auth.connect(Origin::Loopback, None, "desktop", 0).unwrap();
        let tablet = auth
            .connect(Origin::Lan, Some("ABC123"), "tablet", 10)
            .unwrap();
        assert_eq!(auth.sessions().len(), 2);

        auth.revoke(&tablet.session_token).unwrap();
        assert_eq!(
            auth.authorise(&tablet.session_token),
            Err(AuthError::NotAuthorised)
        );
        assert!(auth.authorise(&desk.session_token).is_ok());
        assert_eq!(
            auth.revoke(&tablet.session_token),
            Err(AuthError::NotAuthorised)
        );
    }

    #[test]
    fn two_sessions_never_share_a_session_token() {
        let mut auth = Auth::loopback_only();
        let a = auth.connect(Origin::Loopback, None, "one", 0).unwrap();
        let b = auth.connect(Origin::Loopback, None, "two", 0).unwrap();
        assert_ne!(a.session_token, b.session_token);
    }

    #[test]
    fn reissuing_replaces_a_spent_code_with_a_fresh_one() {
        let mut auth = Auth::with_pairing("ABC123", 0);
        auth.connect(Origin::Lan, Some("ABC123"), "tablet", 0)
            .unwrap();
        assert_eq!(auth.pairing_code(), None);

        auth.reissue("XYZ789", 1000);
        assert_eq!(auth.pairing_code(), Some("XYZ789"));
        assert!(auth
            .connect(Origin::Lan, Some("XYZ789"), "laptop", 1000)
            .is_ok());
        assert_eq!(
            auth.sessions().len(),
            2,
            "reissuing pairs a new machine; it does not evict the old one"
        );
    }

    #[test]
    fn an_unknown_session_token_is_never_authorised() {
        let auth = Auth::loopback_only();
        assert_eq!(auth.authorise("s0000"), Err(AuthError::NotAuthorised));
    }
}
