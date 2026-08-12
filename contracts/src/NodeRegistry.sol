// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title The Erebus node registry.
///
/// @notice Who the mix nodes are, what they staked, and which epoch seed every
/// client should be deriving layers from. It is deliberately the only piece of
/// Erebus that is a single shared source of truth, and it holds as little as it
/// can get away with: a public key, an endpoint, and a bond.
///
/// What it is not: it does not assign layers, choose paths, or see traffic.
/// Layer assignment is a pure function of the epoch seed and each node's public
/// key, computed by every client independently (see `mixnet/crates/topology`),
/// so this contract cannot put a node where it wants it, and an operator cannot
/// buy the exit layer where the most valuable metadata is.
contract NodeRegistry {
    struct Node {
        /// X25519 public key. The node's identity in the Sphinx layer, so a key
        /// registered here is the key packets are actually onion-encrypted to.
        bytes32 key;
        /// Where to reach it, `host:port`.
        string endpoint;
        uint256 stake;
        /// The operator who may change the endpoint and withdraw the bond.
        address operator;
        /// When the bond becomes withdrawable, or zero while the node is serving.
        uint64 withdrawableAt;
    }

    /// An endpoint has to fit in a Sphinx delivery tag, which is 32 bytes of
    /// fixed routing space. A longer one could be registered but never routed to.
    uint256 public constant MAX_ENDPOINT = 32;

    /// The bond a node has to hold to be in the set clients select from.
    uint256 public immutable minStake;
    /// How long a bond stays slashable after the operator announces an exit.
    /// Without it, a node could misbehave and leave in the same block.
    uint64 public immutable unbondingPeriod;
    /// How long a layer assignment holds.
    uint64 public immutable epochLength;
    /// Who may slash. Intended to be governance, and separate from any operator.
    address public immutable arbiter;
    /// Where slashed stake goes. Never back to the arbiter, so slashing is not
    /// a way to be paid.
    address public immutable treasury;

    bytes32[] private _keys;
    mapping(bytes32 => Node) private _nodes;
    /// The seed for an epoch, recorded the first time anyone touches the
    /// registry in it.
    mapping(uint256 => bytes32) public seedOf;

    event Registered(bytes32 indexed key, address indexed operator, string endpoint, uint256 stake);
    event EndpointChanged(bytes32 indexed key, string endpoint);
    event StakeAdded(bytes32 indexed key, uint256 stake);
    event ExitAnnounced(bytes32 indexed key, uint64 withdrawableAt);
    event Withdrawn(bytes32 indexed key, uint256 amount);
    event Slashed(bytes32 indexed key, uint256 amount, string reason);
    event EpochSeeded(uint256 indexed epoch, bytes32 seed);

    error AlreadyRegistered(bytes32 key);
    error NotRegistered(bytes32 key);
    error NotTheOperator(bytes32 key);
    error StakeTooSmall(uint256 offered, uint256 required);
    error EndpointRejected(uint256 length);
    error BadKey();
    error Leaving(bytes32 key);
    error NotLeaving(bytes32 key);
    error StillBonded(uint64 until);
    error NotTheArbiter();
    error NothingToSlash();

    constructor(
        uint256 minStake_,
        uint64 unbondingPeriod_,
        uint64 epochLength_,
        address arbiter_,
        address treasury_
    ) {
        require(minStake_ > 0 && epochLength_ > 0, "registry: zero parameter");
        require(arbiter_ != address(0) && treasury_ != address(0), "registry: zero address");
        minStake = minStake_;
        unbondingPeriod = unbondingPeriod_;
        epochLength = epochLength_;
        arbiter = arbiter_;
        treasury = treasury_;
    }

    /// Joins the node set, bonding at least `minStake`.
    function register(bytes32 key, string calldata endpoint) external payable {
        if (key == bytes32(0)) revert BadKey();
        if (_nodes[key].key != bytes32(0)) revert AlreadyRegistered(key);
        if (msg.value < minStake) revert StakeTooSmall(msg.value, minStake);
        _checkEndpoint(endpoint);

        _nodes[key] = Node({
            key: key, endpoint: endpoint, stake: msg.value, operator: msg.sender, withdrawableAt: 0
        });
        _keys.push(key);
        _seed();

        emit Registered(key, msg.sender, endpoint, msg.value);
    }

    /// Moves a node. Clients pick it up at the next read; packets already in
    /// flight to the old address are lost, which is the operator's problem.
    function setEndpoint(bytes32 key, string calldata endpoint) external {
        Node storage node = _mine(key);
        if (node.withdrawableAt != 0) revert Leaving(key);
        _checkEndpoint(endpoint);
        node.endpoint = endpoint;
        _seed();
        emit EndpointChanged(key, endpoint);
    }

    /// Adds to a bond: the way back into the set after being slashed below
    /// `minStake`.
    function addStake(bytes32 key) external payable {
        Node storage node = _node(key);
        if (node.withdrawableAt != 0) revert Leaving(key);
        node.stake += msg.value;
        _seed();
        emit StakeAdded(key, node.stake);
    }

    /// Leaves the set. The node stops being selected immediately, and the bond
    /// stays slashable for `unbondingPeriod` so that leaving is not an escape.
    function announceExit(bytes32 key) external {
        Node storage node = _mine(key);
        if (node.withdrawableAt != 0) revert Leaving(key);
        node.withdrawableAt = uint64(block.timestamp) + unbondingPeriod;
        _seed();
        emit ExitAnnounced(key, node.withdrawableAt);
    }

    /// Takes back what is left of the bond, once it has stopped being slashable.
    function withdraw(bytes32 key) external {
        Node storage node = _mine(key);
        if (node.withdrawableAt == 0) revert NotLeaving(key);
        if (block.timestamp < node.withdrawableAt) revert StillBonded(node.withdrawableAt);

        uint256 amount = node.stake;
        address operator = node.operator;
        _remove(key);
        _seed();

        emit Withdrawn(key, amount);
        if (amount > 0) {
            (bool sent,) = operator.call{value: amount}("");
            require(sent, "registry: transfer failed");
        }
    }

    /// Takes stake from a node, with the reason on the record.
    ///
    /// @dev What is slashable is a policy question this contract does not
    /// answer: it records a decision made elsewhere. The evidence a mixnet can
    /// actually produce — loop probes that never return — is statistical, so
    /// automating the judgement here would be pretending to a certainty the
    /// protocol does not have.
    function slash(bytes32 key, uint256 amount, string calldata reason) external {
        if (msg.sender != arbiter) revert NotTheArbiter();
        Node storage node = _node(key);
        if (node.stake == 0 || amount == 0) revert NothingToSlash();

        uint256 taken = amount > node.stake ? node.stake : amount;
        node.stake -= taken;
        _seed();

        emit Slashed(key, taken, reason);
        (bool sent,) = treasury.call{value: taken}("");
        require(sent, "registry: transfer failed");
    }

    /// Records this epoch's seed if nobody has yet. Anyone may call it, and
    /// every state change above does it too.
    ///
    /// @dev The seed is the previous block hash, mixed with the epoch number. It
    /// is not unpredictable to whoever produces blocks — on an L2 that is the
    /// sequencer — so it is a defence against an operator choosing its own layer,
    /// not against the chain itself.
    function seedEpoch() external {
        _seed();
    }

    function currentEpoch() public view returns (uint256) {
        return block.timestamp / epochLength;
    }

    /// Everything a client needs to build a path, in one call.
    ///
    /// @dev `seed` is the current epoch's if it has been recorded, and zero
    /// otherwise. Clients derive layers from whatever this returns, so they
    /// agree either way; a node that wants the reshuffle it is owed can call
    /// `seedEpoch()`.
    function snapshot() external view returns (uint256 epoch, bytes32 seed, Node[] memory nodes) {
        epoch = currentEpoch();
        seed = seedOf[epoch];

        uint256 live;
        for (uint256 i = 0; i < _keys.length; i++) {
            if (_isActive(_nodes[_keys[i]])) live++;
        }

        nodes = new Node[](live);
        uint256 at;
        for (uint256 i = 0; i < _keys.length; i++) {
            Node storage node = _nodes[_keys[i]];
            if (_isActive(node)) nodes[at++] = node;
        }
    }

    /// A node by key, whether or not it is currently selectable.
    function nodeOf(bytes32 key) external view returns (Node memory) {
        return _node(key);
    }

    /// Whether clients should be routing through this node at all.
    function isActive(bytes32 key) external view returns (bool) {
        return _isActive(_nodes[key]);
    }

    function count() external view returns (uint256) {
        return _keys.length;
    }

    function _isActive(Node storage node) private view returns (bool) {
        return node.key != bytes32(0) && node.withdrawableAt == 0 && node.stake >= minStake;
    }

    function _node(bytes32 key) private view returns (Node storage node) {
        node = _nodes[key];
        if (node.key == bytes32(0)) revert NotRegistered(key);
    }

    function _mine(bytes32 key) private view returns (Node storage node) {
        node = _node(key);
        if (node.operator != msg.sender) revert NotTheOperator(key);
    }

    function _checkEndpoint(string calldata endpoint) private pure {
        uint256 length = bytes(endpoint).length;
        if (length == 0 || length > MAX_ENDPOINT) revert EndpointRejected(length);
    }

    function _remove(bytes32 key) private {
        for (uint256 i = 0; i < _keys.length; i++) {
            if (_keys[i] == key) {
                _keys[i] = _keys[_keys.length - 1];
                _keys.pop();
                break;
            }
        }
        delete _nodes[key];
    }

    function _seed() private {
        uint256 epoch = currentEpoch();
        if (seedOf[epoch] != bytes32(0)) return;
        bytes32 seed = keccak256(abi.encodePacked(epoch, blockhash(block.number - 1)));
        seedOf[epoch] = seed;
        emit EpochSeeded(epoch, seed);
    }
}
