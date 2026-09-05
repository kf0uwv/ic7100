// Copyright 2026 Matt Franklin
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Share one IC-7100 over the network.
//!
//! Thin wiring over `cat-rigctl`/`cat-server`, exactly as the other two
//! radios do: the broker, the listeners and the protocols are shared, and
//! what is here is this radio's shape.
//!
//! Both listeners at once — rigctl for WSJT-X and the console protocol
//! for a GUI or a remote TUI. They are not alternatives.

mod console;
mod rigctl_radio;

pub use cat_rigctl::ServerConfig;
use console::ConsoleIc7100;
use rigctl_radio::Ic7100Rigctl;

/// Bring up the broker and every listener `config` asks for.
#[cfg(target_os = "linux")]
pub async fn run<S>(session: S, config: ServerConfig) -> std::io::Result<()>
where
    S: cat_transport_core::CatSession + 'static,
    S::Error: std::error::Error + 'static,
{
    let shared = config
        .native_port
        .map(|_| cat_rigctl::native_bridge::NativeShared::new(&radio::capabilities::IC7100));
    if let Some(shared) = shared.clone() {
        // What this radio's console should look like, authored by the
        // crate that knows the radio (radio-cat-rs ADR 0020).
        shared.set_layout(radio::console_layout::layout());
        shared.set_theme(radio::console_layout::theme());
    }
    cat_rigctl::run_with_native(
        session,
        &radio::IC7100_COMMAND_TABLE,
        config,
        |s| Ic7100Rigctl::new(radio::Ic7100::new(s)),
        |s| ConsoleIc7100(radio::Ic7100::new(s)),
        shared,
    )
    .await
}
