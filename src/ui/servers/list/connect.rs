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
        state.connected.as_deref() == Some(s.name.as_str())
    }
}
