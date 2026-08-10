import init, {
  MixClient,
  decode_deliver,
  decode_hello,
  encode_expect,
  encode_send,
} from "../pkg/erebus_sdk.js";

/**
 * Where to load the WebAssembly from. A bundler that emits the `.wasm` as an
 * asset gives you a URL; Node gives you bytes.
 */
export type WasmSource = Parameters<typeof init>[0];

let started: Promise<unknown> | null = null;

/** Loads the packet code once per page, however many clients are created. */
export async function loadWasm(source?: WasmSource): Promise<void> {
  if (!started) {
    started = init(source);
  }
  await started;
}

export { MixClient, decode_deliver, decode_hello, encode_expect, encode_send };
