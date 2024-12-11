use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};

use cookie::CookieJar;
use euphoxide::{
    api::ParsedPacket,
    client::{conn::ClientConnHandle, state::State},
};

use crate::Instance;

pub enum BotEvent<I> {
    Started {
        instance: Instance<I>,
    },
    Connected {
        instance: Instance<I>,
        conn: ClientConnHandle,
        state: State,
    },
    Joined {
        instance: Instance<I>,
        conn: ClientConnHandle,
        state: State,
    },
    Packet {
        instance: Instance<I>,
        conn: ClientConnHandle,
        state: State,
        packet: ParsedPacket,
    },
    Disconnected {
        instance: Instance<I>,
    },
    Stopped {
        instance: Instance<I>,
    },
}

impl<I> BotEvent<I> {
    pub fn instance(&self) -> &Instance<I> {
        match self {
            Self::Started { instance } => instance,
            Self::Connected { instance, .. } => instance,
            Self::Joined { instance, .. } => instance,
            Self::Packet { instance, .. } => instance,
            Self::Disconnected { instance } => instance,
            Self::Stopped { instance } => instance,
        }
    }
}

pub struct Bot<I> {
    cookies: Arc<Mutex<CookieJar>>,
    instances: Arc<RwLock<HashMap<I, Instance<I>>>>,
}
