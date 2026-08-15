/**
 * Environment-driven configuration. All values can be overridden at build
 * time via `.env` (see `.env.example`), which keeps the same bundle pointing
 * at mainnet, testnet or a local sandbox without code changes.
 */
export const NETWORK_PASSPHRASE: string =
  import.meta.env.VITE_NETWORK_PASSPHRASE ?? 'Test SDF Network ; September 2015';

export const RPC_URL: string =
  import.meta.env.VITE_RPC_URL ?? 'https://soroban-testnet.stellar.org';

export const FACTORY_ADDRESS: string = import.meta.env.VITE_FACTORY_ADDRESS ?? '';
export const TOKEN_ADDRESS: string = import.meta.env.VITE_TOKEN_ADDRESS ?? '';

/** True when the app has not been pointed at a deployment yet. */
export const isConfigured = FACTORY_ADDRESS.length > 0 && TOKEN_ADDRESS.length > 0;

/** CFX token decimals (fixed in the token contract). */
export const CFX_DECIMALS = 7;

/** Poll interval for the event stream (ms). */
export const EVENT_POLL_MS = 4_000;

/** Simulation + polling budget for transactions (ms). */
export const TX_TIMEOUT_MS = 60_000;
