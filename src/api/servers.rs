//! Provider-agnostic WireGuard server entry shared by Surfshark and Proton.

#[derive(Clone, Debug, PartialEq)]
pub struct Server {
    pub name: String,
    pub country: String,
    pub location: String,
    pub load: u32,
    pub wg_public_key: String,
    pub endpoint_host: String,
}

impl Server {
    /// Place label: `Country, Location` (falls back when either is empty).
    pub fn display_name(&self) -> String {
        match (self.country.is_empty(), self.location.is_empty()) {
            (false, false) => format!("{}, {}", self.country, self.location),
            (false, true) => self.country.clone(),
            (true, false) => self.location.clone(),
            (true, true) => self.name.clone(),
        }
    }

    /// List / banner label: `Country, Location (unique-id)`.
    pub fn connected_label(&self) -> String {
        let place = self.display_name();
        if self.name.is_empty() || place == self.name {
            place
        } else {
            format!("{place} ({})", self.name)
        }
    }
}
