use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Provider {
    #[default]
    Surfshark,
    Proton,
}

impl Provider {
    pub fn label(self) -> &'static str {
        match self {
            Provider::Surfshark => "Surfshark",
            Provider::Proton => "ProtonVPN",
        }
    }

    pub fn storage_file(self) -> &'static str {
        match self {
            Provider::Surfshark => "surfshark.json",
            Provider::Proton => "proton.json",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Provider::Surfshark => Provider::Proton,
            Provider::Proton => Provider::Surfshark,
        }
    }
}
