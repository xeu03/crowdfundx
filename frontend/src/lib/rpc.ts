import { rpc } from '@stellar/stellar-sdk';
import { RPC_URL } from '../config';

/**
 * Single shared RPC server instance. Keeping one client lets us rely on
 * per-endpoint retry handling in one place and makes mocking trivial in
 * tests.
 */
export const server = new rpc.Server(RPC_URL);

/** Fetch with a timeout so a hung RPC never freezes the UI forever. */
export async function fetchWithTimeout(
  url: string,
  init: RequestInit = {},
  timeoutMs = 15_000,
): Promise<Response> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, { ...init, signal: controller.signal });
  } finally {
    clearTimeout(timer);
  }
}
