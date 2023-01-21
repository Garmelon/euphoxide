use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::time::Duration;
use std::{error, result};

use tokio::sync::oneshot::{self, Receiver, Sender};

#[derive(Debug)]
pub enum Error {
    TimedOut,
    Canceled,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimedOut => write!(f, "timed out"),
            Self::Canceled => write!(f, "canceled"),
        }
    }
}

impl error::Error for Error {}

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub struct PendingReply<R> {
    timeout: Duration,
    result: Receiver<R>,
}

impl<R> PendingReply<R> {
    pub async fn get(self) -> Result<R> {
        match tokio::time::timeout(self.timeout, self.result).await {
            Err(_) => Err(Error::TimedOut),
            Ok(Err(_)) => Err(Error::Canceled),
            Ok(Ok(value)) => Ok(value),
        }
    }
}

#[derive(Debug)]
pub struct Replies<I, R> {
    timeout: Duration,
    pending: HashMap<I, Sender<R>>,
}

impl<I, R> Replies<I, R> {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            pending: HashMap::new(),
        }
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn wait_for(&mut self, id: I) -> PendingReply<R>
    where
        I: Eq + Hash,
    {
        let (tx, rx) = oneshot::channel();
        self.pending.insert(id, tx);
        PendingReply {
            timeout: self.timeout,
            result: rx,
        }
    }

    pub fn complete(&mut self, id: &I, result: R)
    where
        I: Eq + Hash,
    {
        if let Some(tx) = self.pending.remove(id) {
            let _ = tx.send(result);
        }
    }

    pub fn purge(&mut self) {
        self.pending.retain(|_, tx| !tx.is_closed());
    }
}
