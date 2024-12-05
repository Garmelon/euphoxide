//! Models the [packets][0] sent between the server and client.
//!
//! [0]: https://euphoria.leet.nu/heim/api#packets

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::PacketType;

/// A "raw" packet.
///
/// This packet closely matches the [packet representation defined in the
/// API][0]. It can contain arbitrary data in the form of a JSON [`Value`]. It
/// can also contain both data and an error at the same time.
///
/// In order to interpret this packet, you probably want to convert it to a
/// [`ParsedPacket`] using [`ParsedPacket::from_packet`].
///
/// [0]: https://euphoria.leet.nu/heim/api#packets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Packet {
    /// Client-generated id for associating replies with commands.
    pub id: Option<String>,
    /// The type of the command, reply, or event.
    pub r#type: PacketType,
    /// The payload of the command, reply, or event.
    pub data: Option<Value>,
    /// This field appears in replies if a command fails.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// This field appears in replies to warn the client that it may be
    /// flooding.
    ///
    /// The client should slow down its command rate.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub throttled: bool,
    /// If throttled is true, this field describes why.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub throttled_reason: Option<String>,
}

/// Models the relationship between command and reply types.
///
/// This trait is useful for type-safe command-reply APIs.
pub trait Command {
    /// The type of reply one can expect from the server when sending this
    /// command.
    type Reply;
}

macro_rules! packets {
    ( $( $mod:ident::$name:ident, )*) => {
        /// A big enum containing most types of packet data.
        #[derive(Debug, Clone)]
        #[non_exhaustive]
        pub enum Data {
            $( $name(super::$mod::$name), )*
            /// A valid type of packet data that this library does not model as
            /// a struct.
            Unimplemented(PacketType, Value),
        }

        impl Data {
            /// Interpret a JSON [`Value`] as packet data of a specific [`PacketType`].
            ///
            /// This method may fail if the data is invalid.
            pub fn from_value(ptype: PacketType, value: Value) -> serde_json::Result<Self> {
                Ok(match ptype {
                    $( PacketType::$name => Self::$name(serde_json::from_value(value)?), )*
                    _ => Self::Unimplemented(ptype, value),
                })
            }

            /// Convert the packet data into a JSON [`Value`].
            ///
            /// This method may fail if the data fails to serialize.
            pub fn into_value(self) -> serde_json::Result<Value> {
                Ok(match self {
                    $( Self::$name(p) => serde_json::to_value(p)?, )*
                    Self::Unimplemented(_, value) => value,
                })
            }

            /// The [`PacketType`] of this packet data.
            pub fn packet_type(&self) -> PacketType {
                match self {
                    $( Self::$name(_) => PacketType::$name, )*
                    Self::Unimplemented(ptype, _) => *ptype,
                }
            }
        }

        $(
            impl From<super::$mod::$name> for Data {
                fn from(p: super::$mod::$name) -> Self {
                    Self::$name(p)
                }
            }

            impl TryFrom<Data> for super::$mod::$name{
                type Error = ();

                fn try_from(value: Data) -> Result<Self, Self::Error> {
                    match value {
                        Data::$name(p) => Ok(p),
                        _ => Err(())
                    }
                }
            }
        )*
    };
}

macro_rules! commands {
    ( $( $cmd:ident => $rpl:ident, )* ) => {
        $(
            impl Command for super::$cmd {
                type Reply = super::$rpl;
            }
        )*
    };
}

packets! {
    // Events
    events::BounceEvent,
    events::DisconnectEvent,
    events::EditMessageEvent,
    events::HelloEvent,
    events::JoinEvent,
    events::LoginEvent,
    events::LogoutEvent,
    events::NetworkEvent,
    events::NickEvent,
    events::PartEvent,
    events::PingEvent,
    events::PmInitiateEvent,
    events::SendEvent,
    events::SnapshotEvent,
    // Session commands
    session_cmds::Auth,
    session_cmds::AuthReply,
    session_cmds::Ping,
    session_cmds::PingReply,
    // Chat room commands
    room_cmds::GetMessage,
    room_cmds::GetMessageReply,
    room_cmds::Log,
    room_cmds::LogReply,
    room_cmds::Nick,
    room_cmds::NickReply,
    room_cmds::PmInitiate,
    room_cmds::PmInitiateReply,
    room_cmds::Send,
    room_cmds::SendReply,
    room_cmds::Who,
    room_cmds::WhoReply,
    // Account commands
    account_cmds::ChangeEmail,
    account_cmds::ChangeEmailReply,
    account_cmds::ChangeName,
    account_cmds::ChangeNameReply,
    account_cmds::ChangePassword,
    account_cmds::ChangePasswordReply,
    account_cmds::Login,
    account_cmds::LoginReply,
    account_cmds::Logout,
    account_cmds::LogoutReply,
    account_cmds::RegisterAccount,
    account_cmds::RegisterAccountReply,
    account_cmds::ResendVerificationEmail,
    account_cmds::ResendVerificationEmailReply,
    account_cmds::ResetPassword,
    account_cmds::ResetPasswordReply,
}

commands! {
    // Session commands
    Auth => AuthReply,
    Ping => PingReply,
    // Chat room commands
    GetMessage => GetMessageReply,
    Log => LogReply,
    Nick => NickReply,
    PmInitiate => PmInitiateReply,
    Send => SendReply,
    Who => WhoReply,
    // Account commands
    ChangeEmail => ChangeEmailReply,
    ChangeName => ChangeNameReply,
    ChangePassword => ChangePasswordReply,
    Login => LoginReply,
    Logout => LogoutReply,
    RegisterAccount => RegisterAccountReply,
    ResendVerificationEmail => ResendVerificationEmailReply,
    ResetPassword => ResetPasswordReply,
}

/// A fully parsed and interpreted packet.
///
/// Compared to [`Packet`], this packet's representation more closely matches
/// the actual use of packets.
#[derive(Debug, Clone)]
pub struct ParsedPacket {
    /// Client-generated id for associating replies with commands.
    pub id: Option<String>,
    /// The type of the command, reply, or event.
    pub r#type: PacketType,
    /// The payload of the command, reply, or event, or an error message if the
    /// command failed.
    pub content: Result<Data, String>,
    /// A warning to the client that it may be flooding.
    ///
    /// The client should slow down its command rate.
    pub throttled: Option<String>,
}

impl ParsedPacket {
    /// Convert a [`Data`]-compatible value into a [`ParsedPacket`].
    pub fn from_data(id: Option<String>, data: impl Into<Data>) -> Self {
        let data = data.into();
        Self {
            id,
            r#type: data.packet_type(),
            content: Ok(data),
            throttled: None,
        }
    }

    /// Convert a [`Packet`] into a [`ParsedPacket`].
    ///
    /// This method may fail if the packet data is invalid.
    pub fn from_packet(packet: Packet) -> serde_json::Result<Self> {
        let id = packet.id;
        let r#type = packet.r#type;

        let content = if let Some(error) = packet.error {
            Err(error)
        } else {
            let data = packet.data.unwrap_or_default();
            Ok(Data::from_value(r#type, data)?)
        };

        let throttled = if packet.throttled {
            let reason = packet
                .throttled_reason
                .unwrap_or_else(|| "no reason given".to_string());
            Some(reason)
        } else {
            None
        };

        Ok(Self {
            id,
            r#type,
            content,
            throttled,
        })
    }

    /// Convert a [`ParsedPacket`] into a [`Packet`].
    ///
    /// This method may fail if the packet data fails to serialize.
    pub fn into_packet(self) -> serde_json::Result<Packet> {
        let id = self.id;
        let r#type = self.r#type;
        let throttled = self.throttled.is_some();
        let throttled_reason = self.throttled;

        Ok(match self.content {
            Ok(data) => Packet {
                id,
                r#type,
                data: Some(data.into_value()?),
                error: None,
                throttled,
                throttled_reason,
            },
            Err(error) => Packet {
                id,
                r#type,
                data: None,
                error: Some(error),
                throttled,
                throttled_reason,
            },
        })
    }
}

impl TryFrom<Packet> for ParsedPacket {
    type Error = serde_json::Error;

    fn try_from(value: Packet) -> Result<Self, Self::Error> {
        Self::from_packet(value)
    }
}

impl TryFrom<ParsedPacket> for Packet {
    type Error = serde_json::Error;

    fn try_from(value: ParsedPacket) -> Result<Self, Self::Error> {
        value.into_packet()
    }
}
