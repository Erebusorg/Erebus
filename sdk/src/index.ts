export { ErebusClient, type ConnectOptions } from "./client.js";
export {
  ErebusProvider,
  ProviderRpcError,
  type ProviderEvent,
  type ProviderOptions,
  type RequestArguments,
} from "./provider.js";
export {
  GatewaySocket,
  type Greeting,
  type SocketFactory,
  type SocketLike,
} from "./transport.js";
export { loadWasm, type WasmSource } from "./wasm.js";
