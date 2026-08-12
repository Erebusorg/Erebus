// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";

import {FeePool, INodeRegistry, ISpendVerifier} from "../src/FeePool.sol";
import {SpendVerifier} from "../src/SpendVerifier.sol";

/// Deploys the shielded fee pool and the verifier it calls.
///
///   DENOMINATION=10000000000000000 REGISTRY=0x… \
///   forge script script/DeployFees.s.sol --rpc-url $RPC --broadcast
///
/// The registry is not optional: the pool only pays operators who run a node the
/// network is currently routing through.
///
/// The verifier is generated from the circuit — regenerate it with
/// `cargo run --release --manifest-path mixnet/Cargo.toml -p erebus-fees --
/// export-verifier` from the repo root rather than editing
/// it, or proofs will stop verifying.
contract DeployFees is Script {
    function run() external returns (FeePool pool, SpendVerifier verifier) {
        uint256 denomination = vm.envOr("DENOMINATION", uint256(0.01 ether));
        address registry = vm.envAddress("REGISTRY");

        vm.startBroadcast();
        verifier = new SpendVerifier();
        pool = new FeePool(denomination, ISpendVerifier(address(verifier)), INodeRegistry(registry));
        vm.stopBroadcast();

        console.log("SpendVerifier", address(verifier));
        console.log("FeePool", address(pool));
        console.log("denomination", denomination);
        console.log("registry", registry);
    }
}
