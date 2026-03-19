// Copyright (C) 2026 Stacks Open Internet Foundation
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Fuzz target 3g: feed deeply nested Clarity expressions at both epoch
//! limits, checking that the interpreter never panics or overflows the
//! thread stack.

#![no_main]

use clarity::vm::{execute_with_parameters, ClarityVersion, max_call_stack_depth_for_epoch};
use libfuzzer_sys::{arbitrary, fuzz_target};
use stacks_common::types::StacksEpochId;

/// AST parser allows nesting up to `max_call_stack_depth + 5`.
const AST_DEPTH_BUFFER: u64 = 5;

/// Which epoch regime to evaluate under.
#[derive(Debug, Clone, Copy)]
enum FuzzEpoch {
    /// Pre-Epoch34: call-stack limit = 64.
    Legacy,
    /// Epoch34+: call-stack limit = 128.
    Epoch34,
}

/// Which AST nesting pattern to generate.
#[derive(Debug, Clone, Copy)]
enum NestingShape {
    NestedPlus,
    NestedBegin,
    NestedIf,
    NestedLet,
    NestedMap,
}

#[derive(Debug)]
struct StackDepthInput {
    epoch: FuzzEpoch,
    depth: u16,
    shape: NestingShape,
}

impl arbitrary::Arbitrary<'_> for StackDepthInput {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        let epoch = if bool::arbitrary(u)? {
            FuzzEpoch::Legacy
        } else {
            FuzzEpoch::Epoch34
        };

        let limit = match epoch {
            FuzzEpoch::Legacy => 64u16,
            FuzzEpoch::Epoch34 => 128u16,
        };

        // Bias toward boundary values 50% of the time.
        let depth = if bool::arbitrary(u)? {
            let lo = limit.saturating_sub(4);
            let hi = limit.saturating_add(6);
            u.int_in_range(lo..=hi)?
        } else {
            u.int_in_range(1..=200)?
        };

        let shape = match u.int_in_range(0u8..=4)? {
            0 => NestingShape::NestedPlus,
            1 => NestingShape::NestedBegin,
            2 => NestingShape::NestedIf,
            3 => NestingShape::NestedLet,
            _ => NestingShape::NestedMap,
        };

        Ok(StackDepthInput {
            epoch,
            depth,
            shape,
        })
    }
}

/// Build a Clarity program with the requested nesting depth and shape.
fn generate_program(depth: u16, shape: NestingShape) -> String {
    let depth = depth as usize;
    match shape {
        NestingShape::NestedPlus => {
            // (+ 1 (+ 1 (+ 1 ... 1))).
            let mut s = "1".to_string();
            for _ in 0..depth {
                s = format!("(+ 1 {s})");
            }
            s
        }
        NestingShape::NestedBegin => {
            let mut s = "1".to_string();
            for _ in 0..depth {
                s = format!("(begin {s})");
            }
            s
        }
        NestingShape::NestedIf => {
            // Only nest in the true branch to avoid exponential blowup.
            let mut s = "1".to_string();
            for _ in 0..depth {
                s = format!("(if true {s} 0)");
            }
            s
        }
        NestingShape::NestedLet => {
            let mut s = "1".to_string();
            for _ in 0..depth {
                s = format!("(let ((v 1)) {s})");
            }
            s
        }
        NestingShape::NestedMap => {
            // (map + (list 1 2 3) (map + (list 1 2 3) ...)).
            let mut s = "(list 1 2 3)".to_string();
            for _ in 0..depth {
                s = format!("(map + {s} (list 1 2 3))");
            }
            s
        }
    }
}

fuzz_target!(|input: StackDepthInput| {
    let epoch = match input.epoch {
        FuzzEpoch::Legacy => StacksEpochId::Epoch21,
        FuzzEpoch::Epoch34 => StacksEpochId::Epoch34,
    };

    let program = generate_program(input.depth, input.shape);

    // The call must never panic or overflow; any Err is acceptable.
    let result = execute_with_parameters(&program, ClarityVersion::Clarity2, epoch, false);

    let limit = max_call_stack_depth_for_epoch(epoch);
    let parser_limit = limit + AST_DEPTH_BUFFER;

    if u64::from(input.depth) > parser_limit {
        // Programs deeper than the parser limit must be rejected.
        assert!(
            result.is_err(),
            "depth {} exceeded parser limit {} but was not rejected",
            input.depth,
            parser_limit
        );
    }
    // Programs within the parser limit may succeed or fail — either is fine.
    // The real property: we reached this point without panicking.
});
