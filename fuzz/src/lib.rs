use std::slice;
use std::str::from_utf8;

use blockstack_lib::clarity::vm::ast::{ASTRules, build_ast_with_rules};
use blockstack_lib::clarity::vm::costs::LimitedCostTracker;
use blockstack_lib::clarity::vm::errors::RuntimeErrorType;
use blockstack_lib::clarity::vm::types::{QualifiedContractIdentifier, StandardPrincipalData};
use blockstack_lib::clarity::vm::{ClarityVersion, ContractName};
use blockstack_lib::core::StacksEpochId;

// We use #[no_mangle] to ensure the symbol name is exactly as libFuzzer expects.
#[no_mangle]
pub extern "C" fn LLVMFuzzerTestOneInput(data: *const u8, size: usize) -> i32 {
    // If no data, nothing to do.
    if data.is_null() {
        return 0;
    }
    // Convert the fuzzer input to a byte slice.
    let bytes: &[u8] = unsafe { slice::from_raw_parts(data, size) };

    let mut cost_tracker: LimitedCostTracker = LimitedCostTracker::new_free();

    // Enforce UTF-8 validity. If input is not valid UTF-8, skip it.
    if let Ok(contract_content) = from_utf8(bytes) {
        println!("obtained contract: {}", contract_content);
        // Generate a contract id
        let contract_id = QualifiedContractIdentifier::new(
            StandardPrincipalData::transient(),
            ContractName::from("generated-contract"),
        );

        // Build the contract AST and map error.
        let _rules_ast_result = build_ast_with_rules(
            &contract_id,
            contract_content,
            &mut cost_tracker,
            ClarityVersion::latest(),
            StacksEpochId::latest(),
            ASTRules::PrecheckSize,
        )
        .map_err(|e| {
            println!("parse error: {}", e);
            RuntimeErrorType::ASTError(e)
        });
    }

    0 // Return 0 to indicate no fatal error to libFuzzer (crashes are caught by sanitizer).
}
