mod api;
mod app;
mod state;
mod ui;

use state::{Action, Store};
use ui::{UIApp, UIEvent};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("vpn {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let runtime = tokio::runtime::Runtime::new()?;

    ratatui::run(|terminal| {
        let mut app = app::App::new();
        runtime.block_on(app.run(terminal))
    })
}
