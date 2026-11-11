use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, strum::Display, strum::EnumString)]
pub enum ProbeChannel {
    EioIn,
    EioOut,
    Ela(u32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Probe {
    pub channel: ProbeChannel,
    pub signal_name: String,
}
