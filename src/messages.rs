use serde::{Deserialize, Serialize};

use crate::ServerSignalUpdate;


#[derive(Debug,Clone,Serialize,Deserialize)]
pub enum Messages {
    Establish(String),
    Update(ServerSignalUpdate)
}