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

//! A virtual IC-7100.
//!
//! CI-V on a pseudo-terminal, so a console can be developed and tested
//! against a radio nobody has to own. Linux/Unix only — it hosts a PTY,
//! which has no Windows equivalent.
//!
//! # It has to be wrong in the same ways a real radio is
//!
//! An emulator whose meters read a constant, or whose S-meter disagreed
//! with the band it claimed to be receiving, would let a console pass
//! every test and fail on hardware. So this one's meters are computed from
//! a synthetic band at the dial, the same way `ts570d`'s are — see
//! [`meter`].

pub mod civ_loop;
pub mod meter;
pub mod pty;

/// What can go wrong bringing an emulator up.
#[derive(Debug, thiserror::Error)]
pub enum EmulatorError {
    #[error("could not open a pseudo-terminal: {0}")]
    Pty(String),
    #[error("serial error: {0}")]
    Serial(#[from] serialport::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
