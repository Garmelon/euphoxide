//! Account commands.
//!
//! These commands enable a client to register, associate, and dissociate with
//! an account. An account allows an identity to be shared across browsers and
//! devices, and is a prerequisite for room management

use serde::{Deserialize, Serialize};

use super::AccountId;

/// Change the primary email address associated with the signed in account.
///
/// The email address may need to be verified before the change is fully applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEmail {
    /// The new primary email address for the account.
    pub email: String,
    /// The account’s password.
    pub password: String,
}

/// Indicate that the primary email address has been changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEmailReply {
    /// True if authentication succeeded and the email was changed.
    pub success: bool,
    /// If [`Self::success`] was false, the reason for failure.
    pub reason: Option<String>,
    /// If true, a verification email will be sent out, and the user must verify
    /// the address before it becomes their primary address.
    pub verification_needed: bool,
}

/// Change the name associated with the signed in account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeName {
    /// The name to associate with the account.
    pub name: String,
}

/// Indicate a successful name change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeNameReply {
    /// The new name associated with the account.
    pub name: String,
}

/// Change the password of the signed in account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePassword {
    /// The current (and soon-to-be former) password.
    pub old_password: String,
    /// The new password.
    pub new_password: String,
}

/// Return the outcome of changing the password.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePasswordReply;

/// Attempt to log an anonymous session into an account.
///
/// The command will return an error if the session is already logged in.
///
/// If the login succeeds, the client should expect to receive a
/// [`DisconnectEvent`](super::DisconnectEvent) shortly after. The next
/// connection the client makes will be a logged in session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Login {
    /// The namespace of a personal identifier.
    pub namespace: String,
    /// The id of a personal identifier.
    pub id: String,
    /// The password for unlocking the account.
    pub password: String,
}

/// Return whether the session successfully logged into an account.
///
/// If this reply returns success, the client should expect to receive a
/// [`DisconnectEvent`](super::DisconnectEvent) shortly after. The next
/// connection the client makes will be a logged in session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginReply {
    /// True if the session is now logged in.
    pub success: bool,
    /// If [`Self::success`] was false, the reason why.
    pub reason: Option<String>,
    /// If [`Self::success`] was true, the id of the account the session logged
    /// into.
    pub account_id: Option<AccountId>,
}

/// Log a session out of an account.
///
/// The command will return an error if the session is not logged in.
///
/// If the logout is successful, the client should expect to receive a
/// [`DisconnectEvent`](super::DisconnectEvent) shortly after. The next
/// connection the client makes will be a logged out session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logout;

/// Confirm a logout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoutReply;

/// Create a new account and logs into it.
///
/// The command will return an error if the session is already logged in.
///
/// If the account registration succeeds, the client should expect to receive a
/// [`DisconnectEvent`](super::DisconnectEvent) shortly after. The next
/// connection the client makes will be a logged in session using the new
/// account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAccount {
    /// The namespace of a personal identifier.
    pub namespace: String,
    /// The id of a personal identifier.
    pub id: String,
    /// The password for unlocking the account.
    pub password: String,
}

/// Return whether the new account was registered.
///
/// If this reply returns success, the client should expect to receive a
/// [`DisconnectEvent`](super::DisconnectEvent) shortly after. The next
/// connection the client makes will be a logged in session, using the newly
/// created account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAccountReply {
    /// True if the session is now logged in.
    pub success: bool,
    /// If [`Self::success`] was false, the reason why.
    pub reason: Option<String>,
    /// If [`Self::success`] was true, the id of the account the session logged
    /// into.
    pub account_id: Option<AccountId>,
}

/// Force a new email to be sent for verifying an accounts primary email
/// address.
///
/// An error will be returned if the account has no unverified email addresses
/// associated with it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResendVerificationEmail;

/// Indicate that a verification email has been sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResendVerificationEmailReply;

/// Generate a password reset request.
///
/// An email will be sent to the owner of the given personal identifier, with
/// instructions and a confirmation code for resetting the password.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetPassword {
    pub namespace: String,
    pub id: String,
}

/// Confirm that the password reset is in progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetPasswordReply;
