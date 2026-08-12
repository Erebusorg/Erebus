// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";

import {NodeRegistry} from "../src/NodeRegistry.sol";

contract NodeRegistryTest is Test {
    NodeRegistry private registry;

    address private constant ARBITER = address(0xA11CE);
    address private constant TREASURY = address(0x7EA);
    address private constant ALICE = address(0xA1);
    address private constant BOB = address(0xB0B);

    uint256 private constant MIN_STAKE = 1 ether;
    uint64 private constant UNBONDING = 7 days;
    uint64 private constant EPOCH = 1 hours;

    bytes32 private constant KEY = bytes32(uint256(1));
    string private constant ENDPOINT = "203.0.113.7:9000";

    function setUp() public {
        registry = new NodeRegistry(MIN_STAKE, UNBONDING, EPOCH, ARBITER, TREASURY);
        vm.deal(ALICE, 100 ether);
        vm.deal(BOB, 100 ether);
        // A block with a hash to seed from, and a timestamp inside an epoch.
        vm.roll(1000);
        vm.warp(EPOCH * 500);
    }

    function _register(address operator, bytes32 key, string memory endpoint) private {
        vm.prank(operator);
        registry.register{value: MIN_STAKE}(key, endpoint);
    }

    function test_a_registered_node_is_in_the_snapshot_with_its_endpoint() public {
        _register(ALICE, KEY, ENDPOINT);

        (,, NodeRegistry.Node[] memory nodes) = registry.snapshot();
        assertEq(nodes.length, 1);
        assertEq(nodes[0].key, KEY);
        assertEq(nodes[0].endpoint, ENDPOINT);
        assertEq(nodes[0].stake, MIN_STAKE);
        assertEq(nodes[0].operator, ALICE);
        assertTrue(registry.isActive(KEY));
    }

    function test_a_bond_below_the_minimum_is_refused() public {
        vm.prank(ALICE);
        vm.expectRevert(
            abi.encodeWithSelector(NodeRegistry.StakeTooSmall.selector, MIN_STAKE - 1, MIN_STAKE)
        );
        registry.register{value: MIN_STAKE - 1}(KEY, ENDPOINT);
    }

    function test_a_key_can_only_be_registered_once() public {
        _register(ALICE, KEY, ENDPOINT);
        vm.prank(BOB);
        vm.expectRevert(abi.encodeWithSelector(NodeRegistry.AlreadyRegistered.selector, KEY));
        registry.register{value: MIN_STAKE}(KEY, ENDPOINT);
    }

    /// An endpoint that cannot fit in a delivery tag could be registered but
    /// never routed to, so it is refused here rather than failing silently in
    /// the mixnet.
    function test_an_endpoint_too_long_for_a_delivery_tag_is_refused() public {
        string memory tooLong = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.example:9000";
        assertGt(bytes(tooLong).length, registry.MAX_ENDPOINT());

        vm.prank(ALICE);
        vm.expectRevert(
            abi.encodeWithSelector(NodeRegistry.EndpointRejected.selector, bytes(tooLong).length)
        );
        registry.register{value: MIN_STAKE}(KEY, tooLong);

        vm.prank(ALICE);
        vm.expectRevert(abi.encodeWithSelector(NodeRegistry.EndpointRejected.selector, 0));
        registry.register{value: MIN_STAKE}(KEY, "");
    }

    function test_only_the_operator_moves_or_retires_its_node() public {
        _register(ALICE, KEY, ENDPOINT);

        vm.prank(BOB);
        vm.expectRevert(abi.encodeWithSelector(NodeRegistry.NotTheOperator.selector, KEY));
        registry.setEndpoint(KEY, "198.51.100.4:9000");

        vm.prank(BOB);
        vm.expectRevert(abi.encodeWithSelector(NodeRegistry.NotTheOperator.selector, KEY));
        registry.announceExit(KEY);

        vm.prank(ALICE);
        registry.setEndpoint(KEY, "198.51.100.4:9000");
        (,, NodeRegistry.Node[] memory nodes) = registry.snapshot();
        assertEq(nodes[0].endpoint, "198.51.100.4:9000");
    }

    function test_a_node_leaving_is_dropped_from_the_snapshot_immediately() public {
        _register(ALICE, KEY, ENDPOINT);

        vm.prank(ALICE);
        registry.announceExit(KEY);

        (,, NodeRegistry.Node[] memory nodes) = registry.snapshot();
        assertEq(nodes.length, 0, "clients still route through a node that is leaving");
        assertFalse(registry.isActive(KEY));
        // But it is still on the record, and still slashable.
        assertEq(registry.nodeOf(KEY).stake, MIN_STAKE);
        assertEq(registry.count(), 1);
    }

    function test_a_bond_cannot_be_taken_back_before_the_unbonding_period() public {
        _register(ALICE, KEY, ENDPOINT);

        vm.prank(ALICE);
        vm.expectRevert(abi.encodeWithSelector(NodeRegistry.NotLeaving.selector, KEY));
        registry.withdraw(KEY);

        vm.prank(ALICE);
        registry.announceExit(KEY);
        uint64 until_ = uint64(block.timestamp) + UNBONDING;

        vm.prank(ALICE);
        vm.expectRevert(abi.encodeWithSelector(NodeRegistry.StillBonded.selector, until_));
        registry.withdraw(KEY);

        vm.warp(until_);
        uint256 before = ALICE.balance;
        vm.prank(ALICE);
        registry.withdraw(KEY);

        assertEq(ALICE.balance, before + MIN_STAKE);
        assertEq(registry.count(), 0);
    }

    /// The point of the unbonding period: misbehaving and leaving in the same
    /// block must not put the bond out of reach.
    function test_a_node_that_announced_an_exit_is_still_slashable() public {
        _register(ALICE, KEY, ENDPOINT);
        vm.prank(ALICE);
        registry.announceExit(KEY);

        vm.prank(ARBITER);
        registry.slash(KEY, MIN_STAKE / 2, "dropped every probe for an epoch");

        assertEq(registry.nodeOf(KEY).stake, MIN_STAKE / 2);
        assertEq(TREASURY.balance, MIN_STAKE / 2);

        vm.warp(block.timestamp + UNBONDING);
        uint256 before = ALICE.balance;
        vm.prank(ALICE);
        registry.withdraw(KEY);
        assertEq(ALICE.balance, before + MIN_STAKE / 2, "the operator got back what was slashed");
    }

    function test_only_the_arbiter_slashes_and_the_reason_is_on_the_record() public {
        _register(ALICE, KEY, ENDPOINT);

        vm.prank(BOB);
        vm.expectRevert(NodeRegistry.NotTheArbiter.selector);
        registry.slash(KEY, MIN_STAKE, "because I say so");

        vm.expectEmit(true, false, false, true);
        emit NodeRegistry.Slashed(KEY, MIN_STAKE / 4, "loop probes never returned");
        vm.prank(ARBITER);
        registry.slash(KEY, MIN_STAKE / 4, "loop probes never returned");
    }

    function test_slashing_past_the_bond_takes_the_bond_and_no_more() public {
        _register(ALICE, KEY, ENDPOINT);

        vm.prank(ARBITER);
        registry.slash(KEY, MIN_STAKE * 10, "everything");

        assertEq(registry.nodeOf(KEY).stake, 0);
        assertEq(TREASURY.balance, MIN_STAKE);

        vm.prank(ARBITER);
        vm.expectRevert(NodeRegistry.NothingToSlash.selector);
        registry.slash(KEY, 1, "again");
    }

    /// Being slashed below the minimum takes a node out of the set clients
    /// select from, without deleting it: topping the bond back up puts it back.
    function test_a_node_slashed_below_the_minimum_stops_being_selected() public {
        _register(ALICE, KEY, ENDPOINT);

        vm.prank(ARBITER);
        registry.slash(KEY, 1, "a little");

        (,, NodeRegistry.Node[] memory nodes) = registry.snapshot();
        assertEq(nodes.length, 0);
        assertFalse(registry.isActive(KEY));

        vm.prank(BOB);
        registry.addStake{value: 1}(KEY);
        assertTrue(registry.isActive(KEY), "a topped-up node is selectable again");
    }

    function test_a_snapshot_carries_the_epoch_and_a_seed_recorded_in_it() public {
        _register(ALICE, KEY, ENDPOINT);

        (uint256 epoch, bytes32 seed,) = registry.snapshot();
        assertEq(epoch, block.timestamp / EPOCH);
        assertTrue(seed != bytes32(0), "registering did not record the epoch seed");
        assertEq(seed, registry.seedOf(epoch));
    }

    function test_a_new_epoch_gets_a_new_seed_and_the_old_one_is_kept() public {
        _register(ALICE, KEY, ENDPOINT);
        (uint256 first, bytes32 firstSeed,) = registry.snapshot();

        vm.warp(block.timestamp + EPOCH);
        vm.roll(block.number + 1);

        (uint256 second, bytes32 unseeded,) = registry.snapshot();
        assertEq(second, first + 1);
        assertEq(unseeded, bytes32(0), "an epoch nobody touched should have no seed yet");

        registry.seedEpoch();
        (, bytes32 secondSeed,) = registry.snapshot();
        assertTrue(secondSeed != bytes32(0));
        assertTrue(secondSeed != firstSeed, "the layers would not be reshuffled");
        assertEq(registry.seedOf(first), firstSeed, "an epoch's seed changed after the fact");
    }

    function test_a_seed_is_recorded_once_per_epoch() public {
        _register(ALICE, KEY, ENDPOINT);
        (uint256 epoch, bytes32 seed,) = registry.snapshot();

        vm.roll(block.number + 1);
        registry.seedEpoch();
        assertEq(registry.seedOf(epoch), seed, "a later block changed this epoch's seed");
    }

    function test_the_snapshot_holds_every_active_node_and_nothing_else() public {
        _register(ALICE, bytes32(uint256(1)), "10.0.0.1:9000");
        _register(ALICE, bytes32(uint256(2)), "10.0.0.2:9000");
        _register(BOB, bytes32(uint256(3)), "10.0.0.3:9000");

        vm.prank(ALICE);
        registry.announceExit(bytes32(uint256(2)));

        (,, NodeRegistry.Node[] memory nodes) = registry.snapshot();
        assertEq(nodes.length, 2);
        assertEq(nodes[0].key, bytes32(uint256(1)));
        assertEq(nodes[1].key, bytes32(uint256(3)));
    }

    function test_the_active_count_follows_what_the_snapshot_holds() public {
        assertEq(registry.activeNodes(ALICE), 0);

        _register(ALICE, bytes32(uint256(1)), "10.0.0.1:9000");
        _register(ALICE, bytes32(uint256(2)), "10.0.0.2:9000");
        assertEq(registry.activeNodes(ALICE), 2);

        // Slashed below the minimum: out of the set, so out of the count.
        vm.prank(ARBITER);
        registry.slash(bytes32(uint256(1)), MIN_STAKE, "test");
        assertEq(registry.activeNodes(ALICE), 1);

        // And back in once the bond is topped up again.
        vm.deal(ALICE, MIN_STAKE);
        vm.prank(ALICE);
        registry.addStake{value: MIN_STAKE}(bytes32(uint256(1)));
        assertEq(registry.activeNodes(ALICE), 2);

        vm.prank(ALICE);
        registry.announceExit(bytes32(uint256(2)));
        assertEq(registry.activeNodes(ALICE), 1, "leaving stops the count at once");

        vm.warp(block.timestamp + UNBONDING + 1);
        vm.prank(ALICE);
        registry.withdraw(bytes32(uint256(2)));
        assertEq(registry.activeNodes(ALICE), 1, "withdrawing does not double count the exit");
    }

    function test_an_unregistered_key_is_not_a_node() public {
        assertFalse(registry.isActive(KEY));
        vm.expectRevert(abi.encodeWithSelector(NodeRegistry.NotRegistered.selector, KEY));
        registry.nodeOf(KEY);

        vm.prank(ALICE);
        vm.expectRevert(NodeRegistry.BadKey.selector);
        registry.register{value: MIN_STAKE}(bytes32(0), ENDPOINT);
    }

    function testFuzz_a_snapshot_never_carries_a_node_below_the_minimum(uint96 slashed) public {
        _register(ALICE, KEY, ENDPOINT);
        vm.assume(slashed > 0);

        vm.prank(ARBITER);
        registry.slash(KEY, slashed, "fuzz");

        (,, NodeRegistry.Node[] memory nodes) = registry.snapshot();
        uint256 left = registry.nodeOf(KEY).stake;
        assertEq(nodes.length, left >= MIN_STAKE ? 1 : 0);
    }
}
