import { useCallback, useEffect, useState } from 'react';
import { connectWallet, getWalletAddress } from '../lib/wallet';

export interface WalletState {
  address: string | null;
  connecting: boolean;
  error: string | null;
  connect: () => Promise<void>;
  disconnect: () => void;
}

/**
 * Wallet lifecycle: auto-detects an existing Freighter session on mount,
 * connects on demand, and exposes errors for the UI to render.
 */
export function useWallet(): WalletState {
  const [address, setAddress] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getWalletAddress().then((addr) => {
      if (!cancelled) setAddress(addr);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const connect = useCallback(async () => {
    setConnecting(true);
    setError(null);
    try {
      const addr = await connectWallet();
      setAddress(addr);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not connect wallet');
    } finally {
      setConnecting(false);
    }
  }, []);

  const disconnect = useCallback(() => {
    setAddress(null);
    setError(null);
  }, []);

  return { address, connecting, error, connect, disconnect };
}
