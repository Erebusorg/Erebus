// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {FeePool, ISpendVerifier} from "../src/FeePool.sol";
import {MiMC} from "../src/MiMC.sol";
import {SpendVerifier} from "../src/SpendVerifier.sol";

/// The one test that proves the two halves are the same protocol: a proof, a
/// tree, and a payout hash produced by `mixnet/crates/fees` are replayed against
/// the deployed verifier and pool.
///
/// Regenerate with `cargo run --release -p erebus-fees -- fixture` from the repo
/// root after any change to the circuit, the hash, or the payout preimage. If
/// this test fails and the unit tests pass, Rust and Solidity have drifted.
contract SpendTest is Test {
    struct Fixture {
        uint256[] amounts;
        uint256 chainId;
        uint256[] commitments;
        uint256 denomination;
        uint256 emptyLeaf;
        uint256 emptyRoot;
        uint256 nullifierHash;
        uint256 oneTwo;
        address payable[] recipients;
        uint256 payout;
        address pool;
        uint256[8] proof;
        uint256 root;
        uint256 zeroOne;
    }

    Fixture fx;
    FeePool pool;

    function setUp() public {
        string memory json = vm.readFile("test/fixtures/spend.json");

        fx.chainId = vm.parseJsonUint(json, ".chainId");
        fx.pool = vm.parseJsonAddress(json, ".pool");
        fx.denomination = vm.parseJsonUint(json, ".denomination");
        fx.commitments = vm.parseJsonUintArray(json, ".commitments");
        fx.recipients = _payable(vm.parseJsonAddressArray(json, ".recipients"));
        fx.amounts = vm.parseJsonUintArray(json, ".amounts");
        fx.root = vm.parseJsonUint(json, ".root");
        fx.nullifierHash = vm.parseJsonUint(json, ".nullifierHash");
        fx.payout = vm.parseJsonUint(json, ".payout");
        uint256[] memory proof = vm.parseJsonUintArray(json, ".proof");
        for (uint256 i = 0; i < 8; i++) {
            fx.proof[i] = proof[i];
        }
        fx.zeroOne = vm.parseJsonUint(json, ".mimc.zeroOne");
        fx.oneTwo = vm.parseJsonUint(json, ".mimc.oneTwo");
        fx.emptyLeaf = vm.parseJsonUint(json, ".mimc.emptyLeaf");
        fx.emptyRoot = vm.parseJsonUint(json, ".mimc.emptyRoot");

        vm.chainId(fx.chainId);
        // The pool address is inside the payout preimage, so it has to be the
        // address the prover assumed.
        deployCodeTo(
            "FeePool.sol:FeePool", abi.encode(fx.denomination, new SpendVerifier()), fx.pool
        );
        pool = FeePool(fx.pool);
    }

    function _payable(address[] memory input) private pure returns (address payable[] memory out) {
        out = new address payable[](input.length);
        for (uint256 i = 0; i < input.length; i++) {
            out[i] = payable(input[i]);
        }
    }

    function _recipients() private view returns (address[] memory out) {
        out = new address[](fx.recipients.length);
        for (uint256 i = 0; i < fx.recipients.length; i++) {
            out[i] = fx.recipients[i];
        }
    }

    function _fund() private {
        deal(address(this), fx.denomination * fx.commitments.length);
        for (uint256 i = 0; i < fx.commitments.length; i++) {
            pool.deposit{value: fx.denomination}(fx.commitments[i]);
        }
    }

    function test_the_solidity_hash_matches_the_rust_hash() public view {
        assertEq(MiMC.hash(0, 1), fx.zeroOne);
        assertEq(MiMC.hash(1, 2), fx.oneTwo);
        assertEq(pool.zeroAt(0), fx.emptyLeaf);
        assertEq(pool.zeroAt(pool.DEPTH()), fx.emptyRoot);
    }

    function test_the_pool_builds_the_same_tree_as_the_prover() public {
        _fund();
        assertEq(pool.currentRoot(), fx.root, "same leaves, same root");
    }

    function test_the_pool_builds_the_same_payout_hash() public view {
        assertEq(pool.payoutHash(_recipients(), fx.amounts), fx.payout);
    }

    function test_a_real_proof_pays_the_nodes() public {
        _fund();

        pool.spend(fx.root, fx.nullifierHash, _recipients(), fx.amounts, fx.proof);

        for (uint256 i = 0; i < fx.recipients.length; i++) {
            assertEq(pool.earned(fx.recipients[i]), fx.amounts[i]);
            vm.prank(fx.recipients[i]);
            pool.claim();
            assertEq(fx.recipients[i].balance, fx.amounts[i]);
        }
        // One deposit's worth left the pool; the other three are untouched.
        assertEq(address(pool).balance, fx.denomination * (fx.commitments.length - 1));
    }

    function test_a_real_proof_is_rejected_after_the_first_spend() public {
        _fund();
        pool.spend(fx.root, fx.nullifierHash, _recipients(), fx.amounts, fx.proof);

        vm.expectRevert(FeePool.AlreadySpent.selector);
        pool.spend(fx.root, fx.nullifierHash, _recipients(), fx.amounts, fx.proof);
    }

    function test_a_real_proof_cannot_be_redirected_to_other_nodes() public {
        _fund();
        address[] memory greedy = _recipients();
        greedy[0] = address(0xBAD);

        vm.expectRevert(FeePool.ProofRejected.selector);
        pool.spend(fx.root, fx.nullifierHash, greedy, fx.amounts, fx.proof);
    }

    function test_a_real_proof_cannot_be_reshared_between_the_same_nodes() public {
        _fund();
        uint256[] memory skewed = new uint256[](3);
        skewed[0] = fx.denomination - 2;
        skewed[1] = 1;
        skewed[2] = 1;

        vm.expectRevert(FeePool.ProofRejected.selector);
        pool.spend(fx.root, fx.nullifierHash, _recipients(), skewed, fx.proof);
    }

    function test_a_tampered_proof_is_rejected() public {
        _fund();
        uint256[8] memory proof = fx.proof;
        proof[0] = proof[0] ^ 1;

        // A mangled G1 point is not on the curve, so the precompile itself
        // fails rather than the pairing coming out wrong.
        vm.expectRevert();
        pool.spend(fx.root, fx.nullifierHash, _recipients(), fx.amounts, proof);
    }

    function test_a_proof_from_another_pool_is_rejected() public {
        _fund();
        FeePool other = new FeePool(fx.denomination, ISpendVerifier(address(new SpendVerifier())));
        deal(address(this), fx.denomination * fx.commitments.length);
        for (uint256 i = 0; i < fx.commitments.length; i++) {
            other.deposit{value: fx.denomination}(fx.commitments[i]);
        }

        // Same notes, same tree, same root — but the payout hash names the pool.
        assertEq(other.currentRoot(), fx.root);
        vm.expectRevert(FeePool.ProofRejected.selector);
        other.spend(fx.root, fx.nullifierHash, _recipients(), fx.amounts, fx.proof);
    }

    function test_the_verifier_rejects_unreduced_inputs() public {
        SpendVerifier verifier = new SpendVerifier();
        uint256[3] memory input = [MiMC.R, fx.nullifierHash, fx.payout];

        vm.expectRevert(SpendVerifier.BadInput.selector);
        verifier.verify(fx.proof, input);
    }

    function test_the_verifier_accepts_the_fixture_directly() public {
        SpendVerifier verifier = new SpendVerifier();
        assertTrue(verifier.verify(fx.proof, [fx.root, fx.nullifierHash, fx.payout]));
    }
}
