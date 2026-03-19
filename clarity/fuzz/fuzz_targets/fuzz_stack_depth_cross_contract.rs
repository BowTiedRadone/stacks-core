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

//! Fuzz target 3i: deploy N contracts calling each other and invoke the
//! chain, testing stack depth accumulation across `contract-call?`
//! boundaries.

#![no_main]

use clarity::vm::contexts::OwnedEnvironment;
use clarity::vm::database::MemoryBackingStore;
use clarity::vm::types::{
    PrincipalData, QualifiedContractIdentifier, StandardPrincipalData, Value,
};
use clarity::vm::{ClarityVersion, max_call_stack_depth_for_epoch};
use libfuzzer_sys::{arbitrary, fuzz_target};
use stacks_common::types::StacksEpochId;

/// Which epoch regime to evaluate under.
#[derive(Debug, Clone, Copy)]
enum FuzzEpoch {
    /// Pre-Epoch34: call-stack limit = 64.
    Legacy,
    /// Epoch34+: call-stack limit = 128.
    Epoch34,
}

#[derive(Debug)]
struct CrossContractInput {
    epoch: FuzzEpoch,
    /// Number of contracts in the chain (1..=80).
    num_contracts: u8,
    /// Extra nesting depth inside the leaf contract (0..=10).
    leaf_depth: u8,
}

impl arbitrary::Arbitrary<'_> for CrossContractInput {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        let epoch = if bool::arbitrary(u)? {
            FuzzEpoch::Legacy
        } else {
            FuzzEpoch::Epoch34
        };

        let limit = match epoch {
            FuzzEpoch::Legacy => 64u8,
            FuzzEpoch::Epoch34 => 128u8,
        };

        // Each contract-call? costs 2 frames, so the boundary chain length
        // is roughly limit / 2.  Bias toward that boundary 50% of the time.
        let boundary = limit / 2;
        let num_contracts = if bool::arbitrary(u)? {
            let lo = boundary.saturating_sub(3);
            let hi = boundary.saturating_add(3).min(80);
            u.int_in_range(lo..=hi)?
        } else {
            u.int_in_range(1..=80)?
        };

        let leaf_depth = u.int_in_range(0u8..=10)?;

        Ok(CrossContractInput {
            epoch,
            num_contracts,
            leaf_depth,
        })
    }
}

/// Build a nested `(begin ... (ok 1))` expression for the leaf contract.
fn generate_leaf_body(extra_depth: u8) -> String {
    let mut s = "(ok 1)".to_string();
    for _ in 0..extra_depth {
        s = format!("(begin {s})");
    }
    s
}

/// Generate a chain of N contracts.  Contract 0 is the leaf; contract N-1
/// is the entry point that the fuzzer calls.
fn generate_contract_chain(n: u8, leaf_depth: u8) -> Vec<(String, String)> {
    let mut contracts = Vec::with_capacity(n as usize);

    let leaf_body = generate_leaf_body(leaf_depth);
    contracts.push((
        "contract-0".to_string(),
        format!("(define-public (call-me) {leaf_body})"),
    ));

    for i in 1..n {
        let prev = format!("contract-{}", i - 1);
        contracts.push((
            format!("contract-{i}"),
            format!("(define-public (call-me) (contract-call? .{prev} call-me))"),
        ));
    }

    contracts
}

/// Set up a `MemoryBackingStore` and return an `OwnedEnvironment` ready
/// to deploy contracts under the given epoch.  Uses the top-level pattern
/// (no `begin()`) because `execute_in_env` manages its own transactions.
fn setup_env(store: &mut MemoryBackingStore, epoch: StacksEpochId) -> OwnedEnvironment<'_, '_> {
    let mut db = store.as_clarity_db();
    db.begin();
    db.set_clarity_epoch_version(epoch).unwrap();
    db.commit().unwrap();
    if epoch.clarity_uses_tip_burn_block() {
        db.begin();
        db.set_tenure_height(1).unwrap();
        db.commit().unwrap();
    }
    if epoch.uses_marfed_block_time() {
        db.begin();
        db.setup_block_metadata(Some(1)).unwrap();
        db.commit().unwrap();
    }
    OwnedEnvironment::new(db, epoch)
}

fuzz_target!(|input: CrossContractInput| {
    let epoch = match input.epoch {
        FuzzEpoch::Legacy => StacksEpochId::Epoch21,
        FuzzEpoch::Epoch34 => StacksEpochId::Epoch34,
    };

    let contracts = generate_contract_chain(input.num_contracts, input.leaf_depth);

    let mut store = MemoryBackingStore::new();
    let mut owned_env = setup_env(&mut store, epoch);

    let version = ClarityVersion::Clarity2;

    // Deploy every contract in the chain.
    for (name, source) in &contracts {
        let contract_id = QualifiedContractIdentifier::local(name).unwrap();
        // Deployment may fail for very deep leaf bodies; that is acceptable.
        if owned_env
            .initialize_versioned_contract(contract_id, version, source, None)
            .is_err()
        {
            return;
        }
    }

    // Call the outermost contract to exercise the full chain.
    let entry_name = format!("contract-{}", input.num_contracts - 1);
    let entry_id = QualifiedContractIdentifier::local(&entry_name).unwrap();
    let sender = PrincipalData::Standard(StandardPrincipalData::transient());

    let result = owned_env.execute_transaction(sender, None, entry_id, "call-me", &[]);

    // Oracle: verify the outcome matches expectations.
    let limit = max_call_stack_depth_for_epoch(epoch);
    let n = u64::from(input.num_contracts);
    let leaf = u64::from(input.leaf_depth);
    // Each contract-call? costs 2 frames; the leaf adds extra nesting.
    let total_depth = 2 * n - 1 + leaf;

    match result {
        Ok((value, ..)) => {
            // If the call succeeded, depth must have been within limits.
            assert!(
                total_depth <= limit,
                "call succeeded at depth {total_depth} but limit is {limit}"
            );
            // Successful cross-contract calls return (ok 1).
            assert_eq!(
                value,
                Value::okay(Value::Int(1)).unwrap(),
                "unexpected return value from contract chain"
            );
        }
        Err(_) => {
            // Errors are fine — the chain exceeded the depth limit or some
            // other constraint was hit.  The real property: we did not panic.
        }
    }
    // Reaching this point without a panic or stack overflow is the primary
    // property under test.
});
