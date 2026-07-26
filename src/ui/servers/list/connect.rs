//! Connect helpers: fastest server and connected-state matching.

use crate::{Action, Store, api::Server};

use super::ServerList;

impl ServerList {
    pub(in crate::ui::servers) fn connect_fastest(state: &Store) -> Action {
        let fastest = state
            .servers
            .iter()
            .filter_map(|s| {
                state
                    .latencies
                    .get(&s.endpoint_host)
                    .copied()
                    .flatten()
                    .map(|ms| (ms, s))
            })
            .min_by_key(|(ms, _)| *ms)
            .map(|(_, s)| s.clone());
        let target = fastest.or_else(|| state.servers.iter().min_by_key(|s| s.load).cloned());
        match target {
            Some(server) => Action::Connect(server),
            None => Action::Error("no servers loaded".into()),
        }
    }

    pub(in crate::ui::servers) fn is_connected(state: &Store, s: &Server) -> bool {
        let Some(c) = state.connected.as_ref() else {
            return false;
        };
        // Unique logical id: Proton `NL#54`, Surfshark connection hostname.
        if c == &s.name || c == &s.connected_label() {
            return true;
        }
        // Peer pubkey uniquely identifies the physical tunnel (Proton EntryIPs
        // are shared across many logicals; city/country alone is not unique).
        if let Some(pk) = crate::utils::wg::conf_peer_public_key(&crate::utils::conf_path()) {
            return pk == s.wg_public_key;
        }
        // Legacy label / endpoint-only restore when conf isn't readable.
        c == &s.display_name() || c.contains(&s.endpoint_host)
    }
}
