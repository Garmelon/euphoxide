//! A convenient way to keep a [`ServerConfig`] and some [`Instance`]s.

use std::collections::HashMap;

use super::instance::{Instance, ServerConfig};

/// A convenient way to keep a [`ServerConfig`] and some [`Instance`]s.
pub struct Instances {
    server_config: ServerConfig,
    instances: HashMap<String, Instance>,
}

impl Instances {
    pub fn new(server_config: ServerConfig) -> Self {
        Self {
            server_config,
            instances: HashMap::new(),
        }
    }

    pub fn server_config(&self) -> &ServerConfig {
        &self.server_config
    }

    pub fn instances(&self) -> impl Iterator<Item = &Instance> {
        self.instances.values()
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Get an instance by its name.
    pub fn get(&self, name: &str) -> Option<&Instance> {
        self.instances.get(name)
    }

    /// Add a new instance.
    ///
    /// If an instance with the same name exists already, it will be replaced by
    /// the new instance.
    pub fn add(&mut self, instance: Instance) {
        self.instances
            .insert(instance.config().name.clone(), instance);
    }

    /// Remove an instance by its name.
    pub fn remove(&mut self, name: &str) -> Option<Instance> {
        self.instances.remove(name)
    }

    /// Remove all stopped instances.
    ///
    /// This function should be called regularly.
    pub fn purge(&mut self) {
        self.instances.retain(|_, i| !i.stopped());
    }
}
