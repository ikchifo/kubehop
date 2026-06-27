//! Shared helpers for `serde_yaml::Value` operations on kubeconfig documents.

use std::fs;
use std::path::Path;

use serde_yaml::Value;

use super::error::ContextError;
use crate::kubeconfig::KubeconfigError;

const KEY_CURRENT_CONTEXT: &str = "current-context";
const KEY_CONTEXTS: &str = "contexts";
const KEY_NAME: &str = "name";

/// Load a kubeconfig file into a generic YAML document.
pub(crate) fn load_yaml_doc(path: &Path) -> Result<Value, ContextError> {
    let raw = fs::read_to_string(path).map_err(|source| KubeconfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_yaml::from_str(&raw)
        .map_err(KubeconfigError::Parse)
        .map_err(Into::into)
}

/// Serialize and write a YAML document back to disk.
pub(crate) fn write_yaml_doc(path: &Path, doc: &Value) -> Result<(), ContextError> {
    let out = serde_yaml::to_string(doc).map_err(KubeconfigError::Parse)?;
    fs::write(path, out).map_err(|source| KubeconfigError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// Verify that `target` appears as a context name in the `contexts` array.
pub(super) fn validate_target_exists(doc: &Value, target: &str) -> Result<(), ContextError> {
    if context_exists(doc, target) {
        Ok(())
    } else {
        Err(ContextError::NotFound(target.to_owned()))
    }
}

pub(super) fn context_exists(doc: &Value, target: &str) -> bool {
    doc.get(KEY_CONTEXTS)
        .and_then(Value::as_sequence)
        .is_some_and(|contexts| {
            contexts.iter().any(|entry| {
                entry
                    .get(KEY_NAME)
                    .and_then(Value::as_str)
                    .is_some_and(|name| name == target)
            })
        })
}

/// Extract the `current-context` value from the document.
pub(crate) fn read_current_context(doc: &Value) -> Option<String> {
    doc.get(KEY_CURRENT_CONTEXT)
        .and_then(Value::as_str)
        .map(String::from)
}

/// Set `current-context` to `target` in the document mapping.
pub(super) fn set_current_context(doc: &mut Value, target: &str) {
    if let Value::Mapping(map) = doc {
        let key = Value::String(KEY_CURRENT_CONTEXT.to_owned());
        map.insert(key, Value::String(target.to_owned()));
    }
}

/// Remove the `current-context` key from the document mapping.
pub(super) fn remove_current_context(doc: &mut Value) {
    if let Value::Mapping(map) = doc {
        let key = Value::String(KEY_CURRENT_CONTEXT.to_owned());
        map.remove(&key);
    }
}
