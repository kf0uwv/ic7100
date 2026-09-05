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

//! Reading CI-V frames off a byte stream, and answering them.
//!
//! # Framing is the whole job
//!
//! An ASCII protocol ends a frame with `;` and a reader can split on it.
//! CI-V frames start with two `FE` bytes and end with `FD`, arriving in
//! whatever chunks the serial driver feels like — so a reader has to find
//! the boundaries itself, and has to survive starting mid-frame, which it
//! will whenever a controller opens the port while the bus is busy.
//!
//! The scanner below therefore hunts for the preamble rather than assuming
//! the buffer starts at one, and discards anything before it. A reader
//! that trusted its first byte would mis-frame once and then stay
//! mis-framed.

use cat_framework::civ::{END_OF_MESSAGE, PREAMBLE};

/// One complete frame taken off the front of `buffer`, if there is one.
///
/// Returns the frame and how many bytes to drain, including any junk
/// skipped before it.
pub fn take_frame(buffer: &[u8]) -> Option<(Vec<u8>, usize)> {
    // Find `FE FE`. Anything before it is the tail of something we joined
    // late, or another controller's traffic.
    let start = (0..buffer.len().saturating_sub(1))
        .find(|&i| buffer[i] == PREAMBLE && buffer[i + 1] == PREAMBLE)?;
    let end = buffer[start..]
        .iter()
        .position(|&b| b == END_OF_MESSAGE)
        .map(|i| start + i)?;
    Some((buffer[start..=end].to_vec(), end + 1))
}

/// How much junk to keep before giving up on finding a preamble.
///
/// A buffer that never yields a frame would otherwise grow without bound
/// on a line carrying something that is not CI-V at all.
pub const MAX_BUFFER: usize = 4096;

/// Drop leading bytes that can no longer begin a frame.
pub fn trim(buffer: &mut Vec<u8>) {
    if buffer.len() <= MAX_BUFFER {
        return;
    }
    // Keep the tail: a frame in flight is at the end, not the start.
    let keep = buffer.len() - MAX_BUFFER / 2;
    buffer.drain(..keep);
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: &[u8] = &[0xFE, 0xFE, 0x88, 0xE0, 0x03, 0xFD];

    #[test]
    fn a_whole_frame_is_taken_and_measured() {
        let (frame, used) = take_frame(FRAME).unwrap();
        assert_eq!(frame, FRAME);
        assert_eq!(used, FRAME.len());
    }

    #[test]
    fn a_partial_frame_waits_for_the_rest() {
        // Serial bytes arrive in whatever chunks the driver feels like. A
        // reader that answered half a frame would answer the wrong
        // command.
        assert!(take_frame(&FRAME[..4]).is_none());
        assert!(take_frame(&[0xFE]).is_none());
        assert!(take_frame(&[]).is_none());
    }

    #[test]
    fn junk_before_the_preamble_is_skipped() {
        // A controller that opens the port mid-frame joins in the middle
        // of somebody else's traffic. One that trusted its first byte
        // would mis-frame once and then stay mis-framed.
        let mut noisy = vec![0x11, 0x22, 0x33];
        noisy.extend_from_slice(FRAME);
        let (frame, used) = take_frame(&noisy).unwrap();
        assert_eq!(frame, FRAME);
        assert_eq!(used, noisy.len());
    }

    #[test]
    fn a_trailing_partial_frame_is_left_for_next_time() {
        let mut stream = FRAME.to_vec();
        stream.extend_from_slice(&[0xFE, 0xFE, 0x88]);
        let (frame, used) = take_frame(&stream).unwrap();
        assert_eq!(frame, FRAME);
        assert_eq!(&stream[used..], &[0xFE, 0xFE, 0x88]);
    }

    #[test]
    fn two_frames_come_out_one_at_a_time() {
        let mut stream = FRAME.to_vec();
        stream.extend_from_slice(FRAME);
        let (first, used) = take_frame(&stream).unwrap();
        assert_eq!(first, FRAME);
        let (second, _) = take_frame(&stream[used..]).unwrap();
        assert_eq!(second, FRAME);
    }

    #[test]
    fn a_line_that_is_not_civ_does_not_grow_without_bound() {
        let mut buffer = vec![0x55; MAX_BUFFER * 2];
        trim(&mut buffer);
        assert!(buffer.len() <= MAX_BUFFER, "{}", buffer.len());
    }
}
