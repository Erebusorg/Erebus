// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {MiMC} from "./MiMC.sol";

interface INodeRegistry {
    function activeNodes(address operator) external view returns (uint256);
}

interface ISpendVerifier {
    function verify(uint256[8] calldata proof, uint256[3] calldata input)
        external
        view
        returns (bool);
}

/// @title The Erebus shielded fee pool.
///
/// @notice Pays mix nodes without telling them, or anyone watching, who paid.
///
/// A payer deposits a fixed amount under a commitment. Later — from a different
/// address, at a different time, over the mixnet if it likes — someone proves in
/// zero knowledge that *some* deposit in this pool is theirs and directs its
/// value at a set of nodes. The pool learns that one of its deposits was spent
/// and which nodes got paid. It does not learn which deposit, and the deposit
/// transaction and the spend transaction share no field an observer can join on.
///
/// The fixed denomination is not a convenience: with variable amounts the value
/// would identify the deposit, and the proof would be pointless.
///
/// Payouts only reach operators who run a node the network is currently routing
/// through, which the registry answers. That does not make the pool a reward for
/// work — see below — but it does stop it being a way to move value between two
/// addresses with the deposit's origin erased.
///
/// What this does not do: meter traffic. It cannot tell whether a node carried
/// the packets it is being paid for — a payer chooses what to spend and on whom.
/// Enforcing payment per packet needs a per-hop credential the node can check
/// while it forwards, which is a protocol change, not a contract change.
contract FeePool {
    /// Depth of the commitment tree: 2^20 ≈ 1.05M notes. Every level is a hash
    /// the prover constrains, so this is paid for in proving time by spenders.
    uint256 public constant DEPTH = 20;
    /// How many past roots stay spendable. A spend proves membership in a root
    /// that may be a few deposits stale by the time it lands, and rejecting
    /// those would make every deposit a denial of service against every
    /// in-flight spend.
    uint256 public constant ROOT_HISTORY = 30;

    bytes internal constant EMPTY_SEED = "erebus.fees.empty.v1";

    /// The one amount a deposit can be.
    uint256 public immutable denomination;
    ISpendVerifier public immutable verifier;
    /// Who counts as a node.
    INodeRegistry public immutable registry;

    /// The rightmost node the tree knows at each level, which is all an append
    /// needs.
    uint256[DEPTH] private _frontier;
    uint256[DEPTH + 1] private _zeros;
    uint256[ROOT_HISTORY] private _roots;
    uint256 private _rootAt;

    uint256 public leaves;
    mapping(uint256 => bool) public deposited;
    mapping(uint256 => bool) public spent;
    /// What a node may withdraw. Pull, not push: a node whose address reverts
    /// on receive must not be able to make spending fail for everyone else.
    mapping(address => uint256) public earned;

    event Deposited(uint256 indexed commitment, uint256 leaf, uint256 root);
    event Spent(uint256 indexed nullifierHash, address[] recipients, uint256[] amounts);
    event Claimed(address indexed node, uint256 amount);

    error WrongAmount(uint256 sent, uint256 required);
    error NotReduced();
    error AlreadyDeposited();
    error TreeFull();
    error UnknownRoot();
    error AlreadySpent();
    error PayoutMismatch();
    error PayoutNotDenomination(uint256 total, uint256 required);
    error ProofRejected();
    error NothingToClaim();
    error NotANode(address recipient);
    error Expired(uint256 deadline, uint256 now_);

    constructor(uint256 denomination_, ISpendVerifier verifier_, INodeRegistry registry_) {
        require(denomination_ > 0, "pool: zero denomination");
        require(address(verifier_) != address(0), "pool: no verifier");
        require(address(registry_) != address(0), "pool: no registry");
        denomination = denomination_;
        verifier = verifier_;
        registry = registry_;

        _zeros[0] = uint256(keccak256(EMPTY_SEED)) % MiMC.R;
        for (uint256 i = 1; i <= DEPTH; i++) {
            _zeros[i] = MiMC.hash(_zeros[i - 1], _zeros[i - 1]);
        }
        for (uint256 i = 0; i < DEPTH; i++) {
            _frontier[i] = _zeros[i];
        }
        _roots[0] = _zeros[DEPTH];
    }

    /// Funds a note. `commitment` is `MiMC(nullifier, secret)`, and the payer
    /// keeps both halves; nothing here can be spent without them.
    function deposit(uint256 commitment) external payable {
        if (msg.value != denomination) revert WrongAmount(msg.value, denomination);
        if (commitment >= MiMC.R) revert NotReduced();
        if (deposited[commitment]) revert AlreadyDeposited();

        deposited[commitment] = true;
        (uint256 leaf, uint256 newRoot) = _append(commitment);
        emit Deposited(commitment, leaf, newRoot);
    }

    /// Directs one deposit's worth of value at `recipients`, on proof that the
    /// caller holds a note in the pool.
    ///
    /// @dev Anyone may submit this: the proof carries the authorisation, so the
    /// payer does not have to appear on chain at all. Submitting it from an
    /// address that also funded a deposit is the obvious way to undo the
    /// anonymity this buys.
    function spend(
        uint256 root,
        uint256 nullifierHash,
        uint256 deadline,
        address[] calldata recipients,
        uint256[] calldata amounts,
        uint256[8] calldata proof
    ) external {
        if (recipients.length == 0 || recipients.length != amounts.length) revert PayoutMismatch();
        // A proof is a bearer instrument until its nullifier is spent. The
        // deadline is how a payer stops one that never landed — dropped by a
        // relayer, censored, or held back — from being someone else's to submit
        // months later against a route that has since changed hands.
        if (block.timestamp > deadline) revert Expired(deadline, block.timestamp);
        if (!isKnownRoot(root)) revert UnknownRoot();
        if (spent[nullifierHash]) revert AlreadySpent();

        uint256 total;
        for (uint256 i = 0; i < amounts.length; i++) {
            total += amounts[i];
            if (registry.activeNodes(recipients[i]) == 0) revert NotANode(recipients[i]);
        }
        // A partial spend would leave change the pool cannot represent, and an
        // over-spend would pay out of someone else's deposit.
        if (total != denomination) revert PayoutNotDenomination(total, denomination);

        uint256 payout = _payoutHash(deadline, recipients, amounts);
        if (!verifier.verify(proof, [root, nullifierHash, payout])) revert ProofRejected();

        spent[nullifierHash] = true;
        for (uint256 i = 0; i < recipients.length; i++) {
            earned[recipients[i]] += amounts[i];
        }
        emit Spent(nullifierHash, recipients, amounts);
    }

    /// Takes what this node has been paid.
    function claim() external {
        uint256 amount = earned[msg.sender];
        if (amount == 0) revert NothingToClaim();
        earned[msg.sender] = 0;

        emit Claimed(msg.sender, amount);
        (bool sent,) = msg.sender.call{value: amount}("");
        require(sent, "pool: transfer failed");
    }

    function currentRoot() external view returns (uint256) {
        return _roots[_rootAt];
    }

    function isKnownRoot(uint256 candidate) public view returns (bool) {
        if (candidate == 0) return false;
        for (uint256 i = 0; i < ROOT_HISTORY; i++) {
            if (_roots[i] == candidate) return true;
        }
        return false;
    }

    /// The root of an empty subtree of the given height, which is what a client
    /// pads its own copy of the tree with.
    function zeroAt(uint256 height) external view returns (uint256) {
        require(height <= DEPTH, "pool: height");
        return _zeros[height];
    }

    /// The public input that binds a proof to one payout on one deployment.
    ///
    /// @dev Chain id and pool address are in the preimage so a proof cannot be
    /// replayed onto another chain or another pool holding the same note.
    function payoutHash(uint256 deadline, address[] calldata recipients, uint256[] calldata amounts)
        external
        view
        returns (uint256)
    {
        return _payoutHash(deadline, recipients, amounts);
    }

    function _payoutHash(
        uint256 deadline,
        address[] calldata recipients,
        uint256[] calldata amounts
    ) private view returns (uint256) {
        bytes memory data = abi.encodePacked(
            block.chainid, uint256(uint160(address(this))), deadline, recipients.length
        );
        for (uint256 i = 0; i < recipients.length; i++) {
            data = abi.encodePacked(data, uint256(uint160(recipients[i])));
        }
        for (uint256 i = 0; i < amounts.length; i++) {
            data = abi.encodePacked(data, amounts[i]);
        }
        return uint256(keccak256(data)) % MiMC.R;
    }

    function _append(uint256 leaf) private returns (uint256 index, uint256 newRoot) {
        index = leaves;
        if (index >= 2 ** DEPTH) revert TreeFull();

        uint256 at = index;
        uint256 node = leaf;
        for (uint256 level = 0; level < DEPTH; level++) {
            if (at % 2 == 0) {
                // Left child: remember it, and pair with the empty subtree for
                // now. The sibling that eventually arrives will redo this level.
                _frontier[level] = node;
                node = MiMC.hash(node, _zeros[level]);
            } else {
                node = MiMC.hash(_frontier[level], node);
            }
            at /= 2;
        }

        leaves = index + 1;
        _rootAt = (_rootAt + 1) % ROOT_HISTORY;
        _roots[_rootAt] = node;
        newRoot = node;
    }
}
