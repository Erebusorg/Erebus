// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title MiMC over the BN254 scalar field, in Feistel mode.
///
/// @notice The hash the commitment tree is built from. It exists in this shape
/// because the same function has to be computed inside a Groth16 circuit, where
/// keccak would cost millions of constraints; `x^5` costs three per round there
/// and a handful of `mulmod`s here.
///
/// The round constants are a keccak chain from a fixed label rather than a
/// stored table: 110 keccaks of a single word is cheaper than 110 `SLOAD`s, and
/// it means the Rust prover and this library derive the same constants from the
/// same two lines of code instead of from a file that could drift.
library MiMC {
    /// The BN254 scalar field, which is the field the circuit is written over.
    uint256 internal constant R =
        21888242871839275222246405745257275088548364400416034343698204186575808495617;

    /// `ceil(log_5(R))`: the point at which the round function's algebraic
    /// degree reaches the field size.
    uint256 internal constant ROUNDS = 110;

    bytes internal constant CONSTANT_SEED = "erebus.mimc.v1";

    /// Compresses two field elements into one.
    function hash(uint256 l, uint256 r) internal pure returns (uint256) {
        require(l < R && r < R, "mimc: not reduced");

        uint256 digest = uint256(keccak256(CONSTANT_SEED));
        for (uint256 i = 0; i < ROUNDS; i++) {
            uint256 t = addmod(l, digest % R, R);
            uint256 t2 = mulmod(t, t, R);
            uint256 t5 = mulmod(mulmod(t2, t2, R), t, R);

            if (i == ROUNDS - 1) {
                r = addmod(r, t5, R);
            } else {
                uint256 next = addmod(r, t5, R);
                r = l;
                l = next;
            }
            // The next constant is the keccak of this one's 32 bytes. Hashed in
            // scratch space because `abi.encodePacked` in a loop this hot costs
            // more than the round function it feeds.
            assembly ("memory-safe") {
                mstore(0x00, digest)
                digest := keccak256(0x00, 0x20)
            }
        }
        return l;
    }
}
