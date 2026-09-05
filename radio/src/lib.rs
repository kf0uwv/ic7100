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

//! The Icom IC-7100, on the shared CAT framework.
//!
//! Every fact here is from the IC-7100 Full Manual in `docs/manuals/`, and
//! the comment beside it says where. That discipline is this repo's ADR
//! 0001 and matches what `ts570d` and `ft991a` already enforce: a
//! declaration that agreed with the radio by coincidence would be worse
//! than none, because everything downstream trusts it.

pub mod capabilities;
pub mod command;
pub mod console_layout;
pub mod ic7100;
pub mod ic7100_radio;
pub mod mode;
pub mod state;

pub use capabilities::IC7100;
pub use command::{Ic7100CommandId, IC7100_COMMAND_TABLE};
pub use ic7100::{Ic7100, RadioError, RadioResult};
pub use ic7100_radio::{Ic7100Error, Ic7100Event, Ic7100Radio};
pub use mode::{Filter, Mode};
pub use state::{Duplex, Ic7100State, Levels, Meters, Tuning, Vfo};
