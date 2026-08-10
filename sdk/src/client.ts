import { GatewaySocket, type SocketFactory } from "./transport.js";
import {
  MixClient,
  encode_expect,
  encode_send,
  loadWasm,
  type WasmSource,
} from "./wasm.js";

export interface ConnectOptions {
  /** The gateway's WebSocket URL, for example `ws://127.0.0.1:8080`. */
  gateway: string;
  /**
   * Mean delay each hop is told to hold the packet, in milliseconds. Larger
   * means a bigger anonymity set and a slower answer; the client chooses it, and
   * pays for it, alone.
   */
  meanDelayMs?: number;
  /** How long to wait for a reply before giving up. */
  timeoutMs?: number;
  /**
   * A registry obtained somewhere the gateway cannot forge, if you have one.
   * Defaults to the one the gateway serves.
   */
  registry?: string;
  /** Where to load the WebAssembly from, when the default resolution fails. */
  wasm?: WasmSource;
  /** A WebSocket implementation, for runtimes without a global one. */
  socket?: SocketFactory;
}

interface Waiting {
  resolve(body: Uint8Array | undefined): void;
  reject(reason: Error): void;
  timer: ReturnType<typeof setTimeout>;
}

const hex = (bytes: Uint8Array) =>
  Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");

/** An exponentially distributed interval, for traffic with no rhythm to read. */
const exponential = (meanMs: number) => -Math.log(1 - Math.random()) * meanMs;

/**
 * A client of the Erebus mixnet.
 *
 * Path selection, packet construction, reply blocks, and reply decryption all
 * happen here, in WebAssembly, on this machine. The gateway is a courier: it
 * sees that you are sending something and which entry node you chose, which is
 * what your network link sees anyway, and nothing else.
 */
export class ErebusClient {
  private readonly waiting = new Map<string, Waiting>();
  private cover: ReturnType<typeof setTimeout> | null = null;
  private closed = false;

  private constructor(
    private readonly socket: GatewaySocket,
    private readonly mix: MixClient,
    private readonly timeoutMs: number,
  ) {}

  static async connect(options: ConnectOptions): Promise<ErebusClient> {
    await loadWasm(options.wasm);

    let self: ErebusClient | undefined;
    const socket = await GatewaySocket.open(
      options.gateway,
      {
        onDelivery: (frame) => self?.accept(frame),
        onClose: () => self?.fail(new Error("the gateway closed the socket")),
      },
      options.socket,
    );

    const mix = new MixClient(
      options.registry ?? socket.greeting.registry,
      options.meanDelayMs ?? 50,
      socket.greeting.tag,
    );
    self = new ErebusClient(socket, mix, options.timeoutMs ?? 20_000);
    return self;
  }

  /** The address the mixnet delivers this client's replies to. */
  get replyAddress(): string {
    return this.socket.greeting.tag;
  }

  /** Requests still waiting for an answer. */
  get inFlight(): number {
    return this.mix.in_flight;
  }

  /**
   * Sends `body` to a destination service and waits for the answer to come back
   * over a return path the destination never sees.
   */
  async request(destination: string, body: Uint8Array): Promise<Uint8Array> {
    const outgoing = this.mix.request(destination, body);
    const id = outgoing.id;
    if (!id) throw new Error("a request must carry a reply block");

    const answer = this.expect(id);
    this.dispatch(id, outgoing.first_hop, outgoing.packet);
    const body_ = await answer;
    if (!body_) throw new Error("the reply carried no body");
    return body_;
  }

  /** Sends something nothing can answer, for when nothing has to. */
  send(destination: string, body: Uint8Array): void {
    const outgoing = this.mix.send(destination, body);
    this.dispatch(null, outgoing.first_hop, outgoing.packet);
  }

  /**
   * Times a packet routed from this client back to itself. A probe that never
   * returns is evidence that a hop on the path stopped forwarding.
   */
  async probe(): Promise<number> {
    const outgoing = this.mix.probe();
    const id = outgoing.id;
    if (!id) throw new Error("a probe must carry an id");

    const started = performance.now();
    const returned = this.expect(id);
    this.dispatch(id, outgoing.first_hop, outgoing.packet);
    await returned;
    return performance.now() - started;
  }

  /**
   * Sends packets that carry nothing at exponentially spaced intervals, so that
   * sending something is not itself the signal.
   */
  startCoverTraffic(destination: string, meanIntervalMs = 5_000): void {
    this.stopCoverTraffic();
    const tick = () => {
      if (this.closed) return;
      const outgoing = this.mix.cover(destination);
      this.dispatch(null, outgoing.first_hop, outgoing.packet);
      this.cover = setTimeout(tick, exponential(meanIntervalMs));
    };
    this.cover = setTimeout(tick, exponential(meanIntervalMs));
  }

  stopCoverTraffic(): void {
    if (this.cover !== null) {
      clearTimeout(this.cover);
      this.cover = null;
    }
  }

  close(): void {
    this.closed = true;
    this.stopCoverTraffic();
    this.fail(new Error("the client was closed"));
    this.socket.close();
  }

  private expect(id: Uint8Array): Promise<Uint8Array | undefined> {
    const key = hex(id);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.waiting.delete(key);
        this.mix.forget(id);
        reject(new Error(`no reply within ${this.timeoutMs} ms`));
      }, this.timeoutMs);
      this.waiting.set(key, { resolve, reject, timer });
    });
  }

  private dispatch(
    id: Uint8Array | null,
    firstHop: Uint8Array,
    packet: Uint8Array,
  ): void {
    if (id) this.socket.send(encode_expect(id));
    this.socket.send(encode_send(firstHop, packet));
  }

  /** Opens a delivered frame and hands it to whoever asked for it. */
  private accept(frame: Uint8Array): void {
    let delivery;
    try {
      delivery = this.mix.accept(frame);
    } catch {
      // A frame this client is not waiting for. The gateway is not trusted to
      // send only frames that belong here, so this is expected, not fatal.
      return;
    }
    const pending = this.waiting.get(hex(delivery.id));
    if (!pending) return;
    this.waiting.delete(hex(delivery.id));
    clearTimeout(pending.timer);
    pending.resolve(delivery.body);
  }

  private fail(reason: Error): void {
    for (const [key, pending] of this.waiting) {
      clearTimeout(pending.timer);
      pending.reject(reason);
      this.waiting.delete(key);
    }
  }
}
