import type { ErebusClient } from "./client.js";

export interface RequestArguments {
  readonly method: string;
  readonly params?: readonly unknown[] | object;
}

export interface ProviderMessage {
  readonly type: string;
  readonly data: unknown;
}

export type ProviderEvent =
  | "connect"
  | "disconnect"
  | "chainChanged"
  | "accountsChanged"
  | "message";

/** The error shape EIP-1193 requires providers to throw. */
export class ProviderRpcError extends Error {
  constructor(
    readonly code: number,
    message: string,
    readonly data?: unknown,
  ) {
    super(message);
    this.name = "ProviderRpcError";
  }
}

export interface ProviderOptions {
  /** The exit-side JSON-RPC service, as `host:port`. */
  destination: string;
  /**
   * The chain id to answer `eth_chainId` with before the first round trip. Left
   * unset, the first `eth_chainId` is asked over the mixnet like anything else.
   */
  chainId?: string;
}

/** Methods that need a key. This provider is a transport and holds none. */
const NEEDS_A_WALLET = new Set([
  "eth_accounts",
  "eth_requestAccounts",
  "eth_sendTransaction",
  "eth_sign",
  "eth_signTransaction",
  "eth_signTypedData_v4",
  "personal_sign",
  "wallet_addEthereumChain",
  "wallet_switchEthereumChain",
]);

/**
 * An EIP-1193 provider whose transport is the mixnet.
 *
 * It is a transport and nothing more: it holds no keys and signs nothing, so
 * pair it with a local signer and hand it the signed transaction. What it
 * changes is who learns that the transaction came from you — the RPC endpoint
 * sees a request from an exit node it cannot link to an address, a session, or
 * to your other requests.
 */
export class ErebusProvider {
  readonly isErebus = true;
  private readonly listeners = new Map<string, Set<(payload: never) => void>>();
  private nextId = 1;
  private chainId?: string;

  constructor(
    private readonly client: ErebusClient,
    private readonly options: ProviderOptions,
  ) {
    this.chainId = options.chainId;
  }

  async request(args: RequestArguments): Promise<unknown> {
    const { method, params } = args;
    if (NEEDS_A_WALLET.has(method)) {
      throw new ProviderRpcError(
        4200,
        `${method} needs a key: Erebus is a transport and signs nothing. ` +
          "Sign locally and submit with eth_sendRawTransaction.",
      );
    }
    if (method === "eth_chainId" && this.chainId) {
      return this.chainId;
    }

    const id = this.nextId++;
    const body = new TextEncoder().encode(
      JSON.stringify({ jsonrpc: "2.0", id, method, params: params ?? [] }),
    );
    const answer = await this.client.request(this.options.destination, body);

    let parsed: {
      result?: unknown;
      error?: { code: number; message: string; data?: unknown };
    };
    try {
      parsed = JSON.parse(new TextDecoder().decode(answer));
    } catch {
      throw new ProviderRpcError(-32603, "the exit returned something that is not JSON-RPC");
    }
    if (parsed.error) {
      throw new ProviderRpcError(
        parsed.error.code,
        parsed.error.message,
        parsed.error.data,
      );
    }
    if (method === "eth_chainId" && typeof parsed.result === "string") {
      this.chainId = parsed.result;
    }
    return parsed.result;
  }

  on(event: ProviderEvent, listener: (payload: never) => void): this {
    const set = this.listeners.get(event) ?? new Set();
    set.add(listener);
    this.listeners.set(event, set);
    return this;
  }

  removeListener(event: ProviderEvent, listener: (payload: never) => void): this {
    this.listeners.get(event)?.delete(listener);
    return this;
  }

  /** Announces an event to whoever is listening. */
  emit(event: ProviderEvent, payload?: unknown): void {
    for (const listener of this.listeners.get(event) ?? []) {
      (listener as (payload: unknown) => void)(payload);
    }
  }
}
