// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {FeePool, ISpendVerifier} from "../src/FeePool.sol";
import {MiMC} from "../src/MiMC.sol";

/// Accepts everything. Lets these tests cover the pool's own rules — the
/// denomination, the nullifier, the root history, the payout binding — without
/// producing a Groth16 proof per case. `test/Spend.t.sol` covers the real
/// verifier against a proof from the Rust prover.
contract AlwaysVerifier is ISpendVerifier {
    function verify(uint256[8] calldata, uint256[3] calldata) external pure returns (bool) {
        return true;
    }
}

contract NeverVerifier is ISpendVerifier {
    function verify(uint256[8] calldata, uint256[3] calldata) external pure returns (bool) {
        return false;
    }
}

/// A node whose payout address refuses money, to check it can only hurt itself.
contract Refuser {
    receive() external payable {
        revert("no");
    }

    function claim(FeePool pool) external {
        pool.claim();
    }
}

contract FeePoolTest is Test {
    uint256 constant DENOMINATION = 0.01 ether;

    FeePool pool;
    address payer = address(0xBEEF);
    address entry = address(0xE1);
    address relay = address(0xE2);
    address exit = address(0xE3);

    function setUp() public {
        pool = new FeePool(DENOMINATION, new AlwaysVerifier());
        vm.deal(payer, 10 ether);
    }

    function _nodes() private view returns (address[] memory recipients, uint256[] memory amounts) {
        recipients = new address[](3);
        recipients[0] = entry;
        recipients[1] = relay;
        recipients[2] = exit;

        amounts = new uint256[](3);
        amounts[0] = DENOMINATION / 3;
        amounts[1] = DENOMINATION / 3;
        amounts[2] = DENOMINATION - 2 * (DENOMINATION / 3);
    }

    function _deposit(uint256 commitment) private {
        vm.prank(payer);
        pool.deposit{value: DENOMINATION}(commitment);
    }

    function test_deposit_extends_the_tree() public {
        uint256 empty = pool.currentRoot();
        _deposit(1);

        assertEq(pool.leaves(), 1);
        assertTrue(pool.deposited(1));
        assertNotEq(pool.currentRoot(), empty);
        assertTrue(pool.isKnownRoot(empty), "the empty root stays spendable");
        assertEq(address(pool).balance, DENOMINATION);
    }

    function test_deposit_only_at_the_denomination() public {
        vm.prank(payer);
        vm.expectRevert(abi.encodeWithSelector(FeePool.WrongAmount.selector, 1 wei, DENOMINATION));
        pool.deposit{value: 1 wei}(1);
    }

    function test_deposit_rejects_a_repeated_commitment() public {
        _deposit(7);
        vm.prank(payer);
        vm.expectRevert(FeePool.AlreadyDeposited.selector);
        pool.deposit{value: DENOMINATION}(7);
    }

    function test_deposit_rejects_an_unreduced_commitment() public {
        vm.prank(payer);
        vm.expectRevert(FeePool.NotReduced.selector);
        pool.deposit{value: DENOMINATION}(MiMC.R);
    }

    function test_spend_credits_the_nodes_without_paying_them_yet() public {
        _deposit(1);
        (address[] memory recipients, uint256[] memory amounts) = _nodes();

        pool.spend(pool.currentRoot(), 42, recipients, amounts, [uint256(0), 0, 0, 0, 0, 0, 0, 0]);

        assertEq(pool.earned(entry), DENOMINATION / 3);
        assertEq(pool.earned(exit), DENOMINATION - 2 * (DENOMINATION / 3));
        assertEq(entry.balance, 0, "credit, not transfer");
        assertTrue(pool.spent(42));
    }

    function test_spend_is_single_use() public {
        _deposit(1);
        (address[] memory recipients, uint256[] memory amounts) = _nodes();
        uint256 root = pool.currentRoot();

        pool.spend(root, 42, recipients, amounts, [uint256(0), 0, 0, 0, 0, 0, 0, 0]);
        vm.expectRevert(FeePool.AlreadySpent.selector);
        pool.spend(root, 42, recipients, amounts, [uint256(0), 0, 0, 0, 0, 0, 0, 0]);
    }

    function test_spend_rejects_an_unknown_root() public {
        _deposit(1);
        (address[] memory recipients, uint256[] memory amounts) = _nodes();

        vm.expectRevert(FeePool.UnknownRoot.selector);
        pool.spend(999, 42, recipients, amounts, [uint256(0), 0, 0, 0, 0, 0, 0, 0]);
    }

    function test_spend_rejects_a_total_that_is_not_the_denomination() public {
        _deposit(1);
        (address[] memory recipients, uint256[] memory amounts) = _nodes();
        amounts[0] += 1;
        uint256 root = pool.currentRoot();

        vm.expectRevert(
            abi.encodeWithSelector(
                FeePool.PayoutNotDenomination.selector, DENOMINATION + 1, DENOMINATION
            )
        );
        pool.spend(root, 42, recipients, amounts, [uint256(0), 0, 0, 0, 0, 0, 0, 0]);
    }

    function test_spend_rejects_mismatched_arrays() public {
        _deposit(1);
        address[] memory recipients = new address[](2);
        uint256[] memory amounts = new uint256[](1);
        uint256 root = pool.currentRoot();

        vm.expectRevert(FeePool.PayoutMismatch.selector);
        pool.spend(root, 42, recipients, amounts, [uint256(0), 0, 0, 0, 0, 0, 0, 0]);
    }

    function test_spend_rejects_a_bad_proof() public {
        FeePool strict = new FeePool(DENOMINATION, new NeverVerifier());
        vm.prank(payer);
        strict.deposit{value: DENOMINATION}(1);
        (address[] memory recipients, uint256[] memory amounts) = _nodes();
        uint256 root = strict.currentRoot();

        vm.expectRevert(FeePool.ProofRejected.selector);
        strict.spend(root, 42, recipients, amounts, [uint256(0), 0, 0, 0, 0, 0, 0, 0]);
    }

    function test_claim_pays_once() public {
        _deposit(1);
        (address[] memory recipients, uint256[] memory amounts) = _nodes();
        pool.spend(pool.currentRoot(), 42, recipients, amounts, [uint256(0), 0, 0, 0, 0, 0, 0, 0]);

        vm.prank(entry);
        pool.claim();
        assertEq(entry.balance, DENOMINATION / 3);
        assertEq(pool.earned(entry), 0);

        vm.prank(entry);
        vm.expectRevert(FeePool.NothingToClaim.selector);
        pool.claim();
    }

    function test_a_node_that_refuses_money_cannot_block_a_spend() public {
        Refuser refuser = new Refuser();
        _deposit(1);

        address[] memory recipients = new address[](2);
        recipients[0] = address(refuser);
        recipients[1] = entry;
        uint256[] memory amounts = new uint256[](2);
        amounts[0] = DENOMINATION / 2;
        amounts[1] = DENOMINATION - DENOMINATION / 2;

        pool.spend(pool.currentRoot(), 42, recipients, amounts, [uint256(0), 0, 0, 0, 0, 0, 0, 0]);

        vm.expectRevert("pool: transfer failed");
        refuser.claim(pool);

        vm.prank(entry);
        pool.claim();
        assertEq(entry.balance, DENOMINATION - DENOMINATION / 2);
    }

    function test_the_payout_hash_binds_recipients_amounts_and_the_pool() public {
        (address[] memory recipients, uint256[] memory amounts) = _nodes();
        uint256 base = pool.payoutHash(recipients, amounts);

        address[] memory others = new address[](3);
        others[0] = entry;
        others[1] = relay;
        others[2] = address(0xDEAD);
        assertNotEq(base, pool.payoutHash(others, amounts));

        uint256[] memory split = new uint256[](3);
        split[0] = DENOMINATION;
        assertNotEq(base, pool.payoutHash(recipients, split));

        FeePool twin = new FeePool(DENOMINATION, new AlwaysVerifier());
        assertNotEq(base, twin.payoutHash(recipients, amounts), "another pool, another hash");

        vm.chainId(999);
        assertNotEq(base, pool.payoutHash(recipients, amounts), "another chain, another hash");
    }

    function test_old_roots_expire_out_of_the_history() public {
        _deposit(1);
        uint256 stale = pool.currentRoot();
        for (uint256 i = 0; i < pool.ROOT_HISTORY(); i++) {
            _deposit(100 + i);
        }
        assertFalse(pool.isKnownRoot(stale), "a root falls out after ROOT_HISTORY deposits");
        assertTrue(pool.isKnownRoot(pool.currentRoot()));
    }

    function test_zeros_are_the_empty_subtree_roots() public view {
        uint256 zero = pool.zeroAt(0);
        assertEq(pool.zeroAt(1), MiMC.hash(zero, zero));
        assertEq(pool.currentRoot(), pool.zeroAt(pool.DEPTH()), "an empty pool is the empty root");
    }

    function testFuzz_deposits_always_move_the_root(uint256 a, uint256 b) public {
        a = bound(a, 1, MiMC.R - 1);
        b = bound(b, 1, MiMC.R - 1);
        vm.assume(a != b);

        _deposit(a);
        uint256 first = pool.currentRoot();
        _deposit(b);
        assertNotEq(first, pool.currentRoot());
        assertEq(pool.leaves(), 2);
    }
}
