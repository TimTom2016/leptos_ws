use std::borrow::Cow;

use json_patch::Patch;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Messages {
    ServerSignal(ServerSignalMessage),
    // Hier können weitere Nachrichtentypen hinzugefügt werden
    // ChatMessage(ChatMessage),
    // StateSync(StateSyncMessage),
    // etc.
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ServerSignalMessage {
    Establish(String),
    EstablishResponse((String, Value)),
    Update(ServerSignalUpdate),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSignalUpdate {
    name: Cow<'static, str>,
    patch: Patch,
}

impl ServerSignalUpdate {
    /// Creates a new [`ServerSignalUpdate`] from an old and new instance of `T`.
    pub fn new<T>(
        name: impl Into<Cow<'static, str>>,
        old: &T,
        new: &T,
    ) -> Result<Self, serde_json::Error>
    where
        T: Serialize,
    {
        let left = serde_json::to_value(old)?;
        let right = serde_json::to_value(new)?;
        let patch = json_patch::diff(&left, &right);
        Ok(ServerSignalUpdate {
            name: name.into(),
            patch,
        })
    }

    /// Creates a new [`ServerSignalUpdate`] from two json values.
    pub fn new_from_json(name: impl Into<Cow<'static, str>>, old: &Value, new: &Value) -> Self {
        let patch = json_patch::diff(old, new);
        ServerSignalUpdate {
            name: name.into(),
            patch,
        }
    }

    pub(crate) fn get_patch(&self) -> &Patch {
        &self.patch
    }

    pub(crate) fn get_name(&self) -> &Cow<'static, str> {
        &self.name
    }
}
