use std::borrow::Cow;

use json_patch::Patch;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Messages {
    ServerSignal(ServerSignalMessage),
    BiDirectional(BiDirectionalMessage),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ServerSignalMessage {
    Establish(String),
    EstablishResponse((String, Value)),
    Update(SignalUpdate),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum BiDirectionalMessage {
    Establish(String),
    EstablishResponse((String, Value)),
    Update(SignalUpdate),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalUpdate {
    name: Cow<'static, str>,
    patch: Patch,
}

impl SignalUpdate {
    /// Creates a new [`SignalUpdate`] from an old and new instance of `T`.
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
        Ok(SignalUpdate {
            name: name.into(),
            patch,
        })
    }

    /// Creates a new [`SignalUpdate`] from two json values.
    pub fn new_from_json(name: impl Into<Cow<'static, str>>, old: &Value, new: &Value) -> Self {
        let patch = json_patch::diff(old, new);
        SignalUpdate {
            name: name.into(),
            patch,
        }
    }

    pub fn new_from_patch(name: impl Into<Cow<'static, str>>, patch: &Patch) -> Self {
        SignalUpdate {
            name: name.into(),
            patch: patch.clone(),
        }
    }

    pub(crate) fn get_patch(&self) -> &Patch {
        &self.patch
    }

    pub(crate) fn get_name(&self) -> &Cow<'static, str> {
        &self.name
    }
}
