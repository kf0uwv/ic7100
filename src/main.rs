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

//! Control an Icom IC-7100.
//!
//! The wiring layer, and the only place a concrete transport is named —
//! the same Rule 5 the other radios in this fleet follow. `radio` is
//! generic over its session; this chooses one.
//!
//! ```text
//! ic7100 --port /dev/ttyUSB0            # read what the radio is doing
//! ic7100 --port /dev/ttyUSB0 --scan     # find radios on the bus
//! ```

use cat_framework::capabilities::SUnitScale;
use cat_framework::civ::CivFormat;
use cat_transport_serial::{SerialCatSession, SerialConfig, SerialPort};
use radio::capabilities::IC7100_S_UNITS;
use radio::Ic7100;

struct Args {
    port: String,
    baud: u32,
    address: u8,
    scan: bool,
    /// `--tune <hz>`: move the dial before reporting.
    tune: Option<u64>,
}

fn usage() -> ! {
    eprintln!(
        "Usage: ic7100 --port <serial-port> [--baud <rate>] [--address <hex>] [--scan]\n\
         \n\
           --port     Serial port path. The IC-7100's [REMOTE] jack through a\n\
                      CT-17, or its USB connector -- the USB bridge presents\n\
                      two ports and CI-V is on the one the manual calls USB1.\n\
           --baud     Default 19200, this radio's factory CI-V rate.\n\
           --address  The radio's CI-V address in hex. Default 88, which is\n\
                      an IC-7100 as it leaves the factory. Up to four radios\n\
                      share a bus and differ only by this.\n\
           --tune     Move the dial to this frequency, in Hz, first.\n\
           --console <host:port>\n\
                      The terminal console, against `ic7100 server\n\
                      --console-port`.\n\
           --scan     Ask every plausible address who is there, and report\n\
                      what answers. For a bus whose addresses nobody wrote\n\
                      down."
    );
    std::process::exit(1);
}

fn parse_args() -> Args {
    let mut port = None;
    let mut baud = 19_200u32;
    let mut address = 0x88u8;
    let mut scan = false;
    let mut tune = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--port" => port = it.next(),
            "--baud" => baud = it.next().and_then(|v| v.parse().ok()).unwrap_or(19_200),
            "--address" => {
                address = it
                    .next()
                    .and_then(|v| u8::from_str_radix(v.trim_start_matches("0x"), 16).ok())
                    .unwrap_or(0x88)
            }
            "--scan" => scan = true,
            "--tune" => tune = it.next().and_then(|v| v.parse().ok()),
            _ => usage(),
        }
    }
    match port {
        Some(port) => Args {
            port,
            baud,
            address,
            scan,
            tune,
        },
        None => usage(),
    }
}

/// The session, framed for CI-V.
///
/// `with_format`, not `new`: the default scanner reads until `;`, which a
/// CI-V frame never contains. A session built the default way waits for a
/// semicolon forever, and the symptom is a program that prints nothing and
/// does not exit.
fn open(args: &Args) -> SerialCatSession<SerialPort, CivFormat> {
    let port = SerialPort::open(
        &args.port,
        SerialConfig {
            baud_rate: args.baud,
            ..SerialConfig::default()
        },
    )
    .unwrap_or_else(|e| {
        eprintln!("error: could not open {}: {e}", args.port);
        std::process::exit(1);
    });
    SerialCatSession::with_format(port, CivFormat::for_radio(args.address))
}

#[cfg(target_os = "linux")]
// `timer_enabled`: the broker times out a request that the radio never
// answers, and without a timer the runtime panics the moment it tries --
// with a message about the runtime rather than about the timeout, which
// is a long way from the cause.
#[monoio::main(driver = "fusion", timer_enabled = true)]
async fn main() {
    // `ic7100 server ...` is its own argument grammar, so it is taken
    // before the rest is parsed.
    if std::env::args().nth(1).as_deref() == Some("server") {
        run_server().await;
        return;
    }
    // `--console <host:port>`: the terminal console, over the console
    // protocol. Network-only, because that is the shape people already
    // run for WSJT-X and there is nothing this radio needs a second one
    // for.
    if std::env::args().nth(1).as_deref() == Some("--console") {
        let addr = std::env::args().nth(2).unwrap_or_else(|| {
            eprintln!("error: --console needs a host:port");
            std::process::exit(1);
        });
        if let Err(e) = ui::run(&addr) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return;
    }
    let args = parse_args();
    if args.scan {
        scan_bus(&args).await;
        return;
    }
    report(&args).await;
}

/// Read what the radio is doing and print it.
async fn report(args: &Args) {
    let mut radio = Ic7100::at_address(open(args), args.address);

    let address = match radio.civ_address().await {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: no answer from address {:02X}: {e}", args.address);
            eprintln!("hint: --scan asks every address who is there");
            std::process::exit(1);
        }
    };

    if let Some(hz) = args.tune {
        if let Err(e) = radio.set_frequency(hz).await {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }

    println!("Icom IC-7100 at CI-V address {address:02X}");

    match radio.frequency().await {
        Ok(hz) => println!("  frequency  {}", format_hz(hz)),
        Err(e) => println!("  frequency  — ({e})"),
    }
    match radio.mode().await {
        Ok((mode, filter)) => println!("  mode       {} {:?}", mode.label(), filter),
        Err(e) => println!("  mode       — ({e})"),
    }
    match radio.split_and_duplex().await {
        Ok((split, duplex)) => println!(
            "  split      {}   duplex {duplex:?}",
            if split { "on" } else { "off" }
        ),
        Err(e) => println!("  split      — ({e})"),
    }

    // All seven, which is what makes this radio worth a console of its
    // own. Read together so they describe one moment.
    match radio.meters().await {
        Ok(m) => {
            let scale: SUnitScale = IC7100_S_UNITS;
            println!("  meters");
            println!("    S    {:>3}  {}", m.s, scale.label(u16::from(m.s)));
            println!("    PO   {:>3}", m.po);
            println!("    SWR  {:>3}", m.swr);
            println!("    ALC  {:>3}", m.alc);
            println!("    COMP {:>3}", m.comp);
            println!("    Vd   {:>3}", m.vd);
            println!("    Id   {:>3}", m.id);
        }
        Err(e) => println!("  meters     — ({e})"),
    }
}

/// Ask every plausible address who is there.
///
/// The thing a bus needs and a point-to-point protocol never does. Icom
/// assigns addresses per model, so a shack with three Icoms has three
/// different ones and nobody wrote them down.
async fn scan_bus(args: &Args) {
    println!("scanning the CI-V bus for radios...");
    let mut found = 0;
    // One port, re-aimed. Opening one per address would open two hundred
    // of them to ask a single question each, which on a real serial port
    // is slow enough to look like a hang.
    let mut radio = Ic7100::new(open(args));
    // 0x00 is a broadcast and 0xE0-0xEF are controllers; neither is a
    // radio, so neither is asked.
    for address in 0x01..=0xDFu8 {
        radio.set_address(address);
        if let Ok(reported) = radio.civ_address().await {
            println!("  {address:02X}  answered (reports {reported:02X})");
            found += 1;
        }
    }
    if found == 0 {
        println!("  nothing answered. Check the cable, the baud rate, and that");
        println!("  the radio's CI-V transceive setting is not holding the bus.");
    }
}

/// Share this radio over the network.
///
/// Both listeners at once: rigctl for WSJT-X and the console protocol for
/// a GUI or a remote console. They are not alternatives, and a server that
/// made them so would force an operator to choose between logging and
/// looking.
async fn run_server() {
    let mut port = None;
    let mut baud = 19_200u32;
    let mut address = 0x88u8;
    let mut config = server::ServerConfig::default();

    // One pass, taking each flag's value as it goes. The first attempt at
    // this re-scanned `env::args()` per flag, which worked and read
    // terribly.
    let mut it = std::env::args().skip(2);
    while let Some(arg) = it.next() {
        let mut number = || it.next().and_then(|v| v.parse::<u16>().ok());
        match arg.as_str() {
            "--port" => port = it.next(),
            "--baud" => baud = it.next().and_then(|v| v.parse().ok()).unwrap_or(19_200),
            "--address" => {
                address = it
                    .next()
                    .and_then(|v| u8::from_str_radix(v.trim_start_matches("0x"), 16).ok())
                    .unwrap_or(0x88)
            }
            "--rigctl-port" => config.rigctl_port = number(),
            "--console-port" => config.native_port = number(),
            "--raw-tcp-port" => config.raw_tcp_port = number(),
            "--raw-udp-port" => config.raw_udp_port = number(),
            other => {
                eprintln!("error: unknown option {other}");
                server_usage();
            }
        }
    }

    let Some(port_path) = port else {
        server_usage()
    };

    if config.rigctl_port.is_none()
        && config.native_port.is_none()
        && config.raw_tcp_port.is_none()
        && config.raw_udp_port.is_none()
    {
        eprintln!("error: at least one listener is required");
        server_usage();
    }

    let session = open(&Args {
        port: port_path,
        baud,
        address,
        scan: false,
        tune: None,
    });

    if let Err(e) = server::run(session, config).await {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}

fn server_usage() -> ! {
    eprintln!(
        "Usage: ic7100 server --port <serial-port> [--baud <rate>] [--address <hex>]\n\
         \n\
           --rigctl-port   A Hamlib rigctld-compatible listener, for WSJT-X\n\
           --console-port  The typed console protocol, for a GUI or a remote\n\
                           console\n\
           --raw-tcp-port  cat-server's raw length-prefixed TCP protocol\n\
           --raw-udp-port  its enveloped UDP protocol\n\
         \n\
         At least one listener is required. They are not alternatives --\n\
         rigctl and a console can both be bound, and usually should be."
    );
    std::process::exit(1);
}

fn format_hz(hz: u64) -> String {
    format!(
        "{:>3}.{:03}.{:03} MHz",
        hz / 1_000_000,
        (hz / 1_000) % 1_000,
        hz % 1_000
    )
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("ic7100: this build targets Linux (io_uring); see the other radios' Windows notes");
    std::process::exit(1);
}
