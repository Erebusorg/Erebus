// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script} from "forge-std/Script.sol";
import {console} from "forge-std/console.sol";

import {NodeRegistry} from "../src/NodeRegistry.sol";

/// Deploys the registry.
///
/// The parameters are read from the environment so that a testnet deployment and
/// a local one are the same code path:
///
///   MIN_STAKE=1000000000000000 UNBONDING=604800 EPOCH=3600 \
///   ARBITER=0x... TREASURY=0x... \
///   forge script script/Deploy.s.sol --rpc-url $RPC --broadcast
contract Deploy is Script {
    function run() external returns (NodeRegistry registry) {
        uint256 minStake = vm.envOr("MIN_STAKE", uint256(0.001 ether));
        uint64 unbonding = uint64(vm.envOr("UNBONDING", uint256(7 days)));
        uint64 epoch = uint64(vm.envOr("EPOCH", uint256(1 hours)));
        address arbiter = vm.envOr("ARBITER", msg.sender);
        address treasury = vm.envOr("TREASURY", msg.sender);

        vm.startBroadcast();
        registry = new NodeRegistry(minStake, unbonding, epoch, arbiter, treasury);
        vm.stopBroadcast();

        console.log("NodeRegistry", address(registry));
    }
}
