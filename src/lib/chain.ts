import {
  decodeAbiParameters,
  parseAbiParameters,
  toFunctionSelector,
} from "viem";

/// Where the contracts live. Robinhood Chain testnet, because the spend
/// circuit's trusted setup is reproducible and therefore unsafe for real value.
export const chain = {
  name: "Robinhood Chain Testnet",
  id: 46630,
  rpc: process.env.EREBUS_CHAIN_RPC ?? "https://rpc.testnet.chain.robinhood.com",
  explorer: "https://explorer.testnet.chain.robinhood.com",
  registry:
    process.env.EREBUS_REGISTRY ??
    "0x1afa15F03e8d4f656374864750E0b62CCB6C8ad7",
  feePool:
    process.env.EREBUS_FEE_POOL ??
    "0x7e4E497aa102FdE094431F81BEFec6652A98b799",
  verifier:
    process.env.EREBUS_VERIFIER ??
    "0x53f1a479D2a56548A87d5EE7D647BD73ECE80B73",
} as const;

export function explorerAddress(address: string) {
  return `${chain.explorer}/address/${address}`;
}

export type MixNode = {
  key: string;
  endpoint: string;
  stake: bigint;
  operator: string;
  withdrawableAt: bigint;
};

export type Snapshot = {
  epoch: bigint;
  seed: string;
  nodes: MixNode[];
  minStake: bigint;
  epochLength: bigint;
  registered: bigint;
  denomination: bigint;
  /// Notes deposited in the fee pool: the anonymity set a spend hides in.
  notes: bigint;
};

const SNAPSHOT_OUTPUT = parseAbiParameters(
  "uint256 epoch, bytes32 seed, (bytes32 key, string endpoint, uint256 stake, address operator, uint64 withdrawableAt)[] nodes",
);

/// One `eth_call`, read at most once a minute.
///
/// The registry is the only shared source of truth in Erebus, so this page
/// reads it directly rather than through anything of ours: no API of ours sits
/// between the contract and what you see, and you can repeat the same call.
async function call(to: string, signature: string) {
  const response = await fetch(chain.rpc, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "eth_call",
      params: [{ to, data: toFunctionSelector(signature) }, "latest"],
    }),
    next: { revalidate: 60 },
  });
  if (!response.ok) throw new Error(`rpc ${response.status}`);
  const body = (await response.json()) as {
    result?: string;
    error?: { message: string };
  };
  if (body.error) throw new Error(body.error.message);
  if (!body.result) throw new Error("rpc returned no result");
  return body.result as `0x${string}`;
}

function scalar(data: `0x${string}`) {
  return BigInt(data);
}

/// The live node set, or `null` when the chain cannot be reached — a page that
/// invents numbers when the RPC is down is worse than one that says so.
export async function readNetwork(): Promise<Snapshot | null> {
  try {
    const [snapshot, minStake, epochLength, registered, denomination, notes] =
      await Promise.all([
        call(chain.registry, "snapshot()"),
        call(chain.registry, "minStake()"),
        call(chain.registry, "epochLength()"),
        call(chain.registry, "count()"),
        call(chain.feePool, "denomination()"),
        call(chain.feePool, "leaves()"),
      ]);

    const [epoch, seed, nodes] = decodeAbiParameters(
      SNAPSHOT_OUTPUT,
      snapshot,
    ) as [bigint, string, MixNode[]];

    return {
      epoch,
      seed,
      nodes: nodes.map((node) => ({ ...node })),
      minStake: scalar(minStake),
      epochLength: scalar(epochLength),
      registered: scalar(registered),
      denomination: scalar(denomination),
      notes: scalar(notes),
    };
  } catch {
    return null;
  }
}

export function formatEth(wei: bigint) {
  const whole = wei / 10n ** 18n;
  const rest = (wei % 10n ** 18n).toString().padStart(18, "0").slice(0, 4);
  return `${whole}.${rest} ETH`;
}
