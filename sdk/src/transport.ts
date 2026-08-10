import { decode_deliver, decode_hello } from "./wasm.js";

/**
 * The subset of `WebSocket` this SDK uses, so a caller can supply Node's
 * implementation, or a fake, without the SDK depending on either.
 */
export interface SocketLike {
  binaryType: string;
  send(data: ArrayBufferView | ArrayBuffer): void;
  close(): void;
  addEventListener(type: "open", listener: () => void): void;
  addEventListener(type: "close", listener: () => void): void;
  addEventListener(type: "error", listener: (event: unknown) => void): void;
  addEventListener(
    type: "message",
    listener: (event: { data: unknown }) => void,
  ): void;
}

export type SocketFactory = (url: string) => SocketLike;

/** What the gateway said about itself when the socket opened. */
export interface Greeting {
  /** The address replies are delivered to, on this client's behalf. */
  tag: string;
  /** The node set, as JSON. */
  registry: string;
}

export interface GatewayHandlers {
  onDelivery(frame: Uint8Array): void;
  onClose(): void;
}

function defaultFactory(url: string): SocketLike {
  const ctor = (globalThis as { WebSocket?: new (url: string) => SocketLike })
    .WebSocket;
  if (!ctor) {
    throw new Error(
      "no WebSocket in this runtime: pass `socket` to connect() (for example Node's `ws`)",
    );
  }
  return new ctor(url);
}

/** Reads whatever a WebSocket implementation calls a binary message. */
async function toBytes(data: unknown): Promise<Uint8Array | null> {
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (ArrayBuffer.isView(data)) {
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  }
  if (typeof Blob !== "undefined" && data instanceof Blob) {
    return new Uint8Array(await data.arrayBuffer());
  }
  return null;
}

/**
 * The socket to the gateway. It carries opaque bytes in both directions: this
 * class never looks inside a packet, because it cannot.
 */
export class GatewaySocket {
  private constructor(
    private readonly socket: SocketLike,
    readonly greeting: Greeting,
  ) {}

  /** Opens the socket and waits for the gateway's greeting. */
  static async open(
    url: string,
    handlers: GatewayHandlers,
    factory: SocketFactory = defaultFactory,
  ): Promise<GatewaySocket> {
    const socket = factory(url);
    socket.binaryType = "arraybuffer";

    const greeting = await new Promise<Greeting>((resolve, reject) => {
      let greeted = false;
      socket.addEventListener("error", () =>
        reject(new Error(`the gateway at ${url} could not be reached`)),
      );
      socket.addEventListener("close", () => {
        if (!greeted) reject(new Error("the gateway closed the socket"));
        handlers.onClose();
      });
      socket.addEventListener("message", (event) => {
        void (async () => {
          const bytes = await toBytes(event.data);
          if (!bytes) return;
          // A gateway is not trusted to send something readable: anything that
          // fails to decode has to become a rejected connection or a dropped
          // message, never an exception in a callback nobody awaits.
          if (greeted) {
            try {
              const frame = decode_deliver(bytes);
              if (frame) handlers.onDelivery(frame);
            } catch {
              // Not a delivery this client can read; the gateway may send
              // anything, and none of it is authoritative.
            }
            return;
          }
          let hello;
          try {
            hello = decode_hello(bytes);
          } catch {
            reject(
              new Error(
                `the gateway at ${url} sent a greeting that could not be read`,
              ),
            );
            return;
          }
          if (!hello) return;
          greeted = true;
          resolve({ tag: hello.tag, registry: hello.registry });
        })();
      });
    });

    return new GatewaySocket(socket, greeting);
  }

  send(message: Uint8Array): void {
    this.socket.send(message);
  }

  close(): void {
    this.socket.close();
  }
}
