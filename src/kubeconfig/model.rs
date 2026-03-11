//! Selective deserialization structs for kubeconfig files.

use serde::Deserialize;

/// Lightweight read-only view of a kubeconfig file.
///
/// Only deserializes the fields needed for context listing and switching.
/// Clusters, users, auth providers, and embedded certificates are skipped
/// by serde automatically and never allocated.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KubeConfigView {
    /// The active context name, if set.
    #[serde(rename = "current-context", default)]
    pub current_context: Option<String>,

    /// Context entries in file order.
    #[serde(default)]
    pub contexts: Vec<ContextEntry>,
}

/// A named context entry from the kubeconfig.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ContextEntry {
    /// The context name.
    pub name: String,

    /// Optional context details (cluster, namespace, user).
    pub context: Option<ContextFields>,
}

/// Fields within a context entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ContextFields {
    /// The default namespace for this context.
    pub namespace: Option<String>,
    /// The cluster this context targets.
    pub cluster: Option<String>,
    /// The user credential this context authenticates with.
    pub user: Option<String>,
}

impl KubeConfigView {
    /// Returns context names in file order.
    #[must_use]
    pub fn context_names(&self) -> Vec<&str> {
        let mut names = Vec::with_capacity(self.contexts.len());
        for entry in &self.contexts {
            names.push(entry.name.as_str());
        }
        names
    }

    /// Returns the current context name, if set.
    #[must_use]
    pub fn current_context(&self) -> Option<&str> {
        self.current_context.as_deref()
    }

    /// Checks whether a context with the given name exists.
    #[must_use]
    pub fn context_exists(&self, name: &str) -> bool {
        self.contexts.iter().any(|c| c.name == name)
    }
}
