import {
  isConnected,
  requestAccess,
  getAddress,
  isAllowed,
  setAllowed,
} from '@stellar/freighter-api';

export class WalletError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'WalletError';
  }
}

const INSTALL_HINT = 'Freighter is not installed. Install it from freighter.app and reload.';
const REJECTED_HINT = 'Connection request was rejected.';

/** Connect Freighter and return the public key, with friendly errors. */
export async function connectWallet(): Promise<string> {
  let connected: boolean;
  try {
    ({ isConnected: connected } = await isConnected());
  } catch {
    throw new WalletError(INSTALL_HINT);
  }
  if (!connected) throw new WalletError(INSTALL_HINT);

  try {
    const { address } = await requestAccess();
    // Persist permission so the extension auto-allows future connections.
    try {
      const { isAllowed: allowed } = await isAllowed();
      if (!allowed) await setAllowed();
    } catch {
      // Non-fatal: older extension versions may not expose isAllowed/setAllowed.
    }
    return address;
  } catch {
    throw new WalletError(REJECTED_HINT);
  }
}

/** Return the currently connected address, or null when none. */
export async function getWalletAddress(): Promise<string | null> {
  try {
    const { isConnected: connected } = await isConnected();
    if (!connected) return null;
    const { address } = await getAddress();
    return address;
  } catch {
    return null;
  }
}
