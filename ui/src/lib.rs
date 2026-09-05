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

//! The IC-7100's terminal console.
//!
//! **Network-only**, and deliberately so. `ts570d`'s terminal console
//! grew a local serial path first and a network one later, and carries
//! both; this one starts where that ended up. A console talks to
//! `ic7100 server --console-port`, which is the same shape people already
//! run for WSJT-X.
//!
//! That keeps this crate to the loop: the console itself — the layout,
//! the panels, the key handling — is `cat-ui-ratatui`'s and shared with
//! every other radio. What is here is filling a `RadioDisplay` from the
//! protocol and drawing it.

use std::io::Stdout;
use std::time::{Duration, Instant};

use cat_native::{Client, Command, Event, ServerMessage, Streams};
use cat_ui::display::RadioDisplay;
use cat_ui_ratatui::console::{self, ConsoleKey, ConsoleView};
use crossterm::event::{self, Event as TermEvent, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

#[derive(Debug, thiserror::Error)]
pub enum UiError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not reach {addr}: {source}")]
    Connect {
        addr: String,
        #[source]
        source: cat_native::ClientError,
    },
}

/// How often to ask the radio what it is doing.
///
/// Ten times a second. On CI-V a state read costs several round trips —
/// there is no `IF;` equivalent — so asking faster would put a queue of
/// frequency reads in front of the traffic a readout depends on.
const POLL: Duration = Duration::from_millis(100);

/// Run the console against a server.
pub fn run(addr: &str) -> Result<(), UiError> {
    let mut client = Client::connect(addr, Streams::none()).map_err(|source| UiError::Connect {
        addr: addr.to_string(),
        source,
    })?;
    let caps = client.capabilities().clone();
    let mut view = ConsoleView::for_capabilities(&caps);
    let mut state = RadioDisplay {
        connected: true,
        initializing: true,
        ..RadioDisplay::default()
    };

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let result = event_loop(&mut terminal, &mut client, &caps, &mut view, &mut state);

    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    client: &mut Client,
    caps: &cat_native::CapabilitiesWire,
    view: &mut ConsoleView,
    state: &mut RadioDisplay,
) -> Result<(), UiError> {
    let mut last_poll = Instant::now() - POLL;

    loop {
        if last_poll.elapsed() >= POLL {
            last_poll = Instant::now();
            client.request_state();
        }

        while let Some(event) = client.try_event() {
            match event {
                Event::Reply(ServerMessage::State(radio)) => {
                    apply(state, caps, &radio);
                    state.initializing = false;
                }
                Event::Reply(ServerMessage::Error { code, message }) => {
                    // `NotReady` is the ordinary answer before the server
                    // has heard from the radio, and not worth a line an
                    // operator has to read past.
                    if !matches!(code, cat_native::ErrorCode::NotReady) {
                        view.message = Some(message);
                    }
                }
                Event::Reply(_) => {}
                Event::Disconnected(why) => {
                    state.connected = false;
                    view.message = Some(format!("connection lost: {why}"));
                }
            }
        }

        view.passband = state
            .mode_id
            .and_then(|mode| cat_ui::af::passband_for(caps, mode));

        terminal.draw(|f| {
            console::draw(f, f.size(), state, view, caps);
        })?;

        if event::poll(Duration::from_millis(10))? {
            if let TermEvent::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match console::handle_key(key, view, caps) {
                    ConsoleKey::Consumed | ConsoleKey::Rejected(_) => {}
                    ConsoleKey::Action(action) => apply_action(action, view, client),
                    // This radio has no signal sources: it declares no IF
                    // tap and there is nothing to attach. Reported rather
                    // than silently ignored.
                    ConsoleKey::Attach(_) | ConsoleKey::RefreshDevices => {
                        view.message =
                            Some("this radio has no signal sources to attach".to_string());
                    }
                    ConsoleKey::Passthrough => {
                        if matches!(key.code, crossterm::event::KeyCode::Char('q')) {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

/// Fill the console's view from what the server reported.
fn apply(
    state: &mut RadioDisplay,
    caps: &cat_native::CapabilitiesWire,
    radio: &cat_native::RadioState,
) {
    state.vfo_a_hz = radio.vfo_a_hz;
    state.vfo_b_hz = radio.vfo_b_hz;
    state.mode_id = Some(radio.mode);
    // The label the *radio* publishes, not one derived here: this radio
    // spells its digital mode "DV" and another spells the same `ModeId`
    // "C4FM", and an operator should read their own radio's word.
    state.mode = caps
        .modes
        .iter()
        .find(|m| m.id == radio.mode)
        .map(|m| m.label.clone())
        .unwrap_or_else(|| format!("{:?}", radio.mode));
    state.tx = radio.transmitting;
    state.split = radio.split;
    state.connected = true;
    if let Some(raw) = radio.meter(cat_native::MeterKind::S) {
        state.smeter = raw;
    }
}

/// Carry out what the command line asked for.
fn apply_action(action: cat_ui::command::Action, view: &mut ConsoleView, client: &Client) {
    use cat_ui::command::Action;
    match action {
        Action::Quit => {}
        // Handled inside `handle_key`; it never reaches here.
        Action::SelectTab(_) => {}
        Action::Radio(command) => {
            if let Command::SetFrequency { hz, .. } | Command::Retune { hz } = command {
                // The confirmed value stays on screen and the requested
                // one follows it until a poll confirms.
                view.pending_vfo_hz = Some(hz);
            }
            if !client.send(command) {
                view.message = Some("connection lost".to_string());
            }
        }
    }
}
