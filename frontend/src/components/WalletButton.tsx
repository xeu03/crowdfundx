import { shortAddress } from '../lib/format';
import { useToast } from '../hooks/useToast';
import type { WalletState } from '../hooks/useWallet';

export function WalletButton({ wallet }: { wallet: WalletState }) {
  const { push } = useToast();
  const { address, connecting, error, connect, disconnect } = wallet;

  if (address) {
    return (
      <div className="wallet-chip">
        <span className="wallet-chip__address" title={address}>
          {shortAddress(address)}
        </span>
        <button
          type="button"
          className="button button--small button--ghost"
          onClick={() => {
            disconnect();
            push('info', 'Wallet disconnected');
          }}
        >
          Disconnect
        </button>
      </div>
    );
  }

  return (
    <div className="wallet-connect">
      {error && (
        <button
          type="button"
          className="button button--small button--danger-quiet"
          title={error}
          onClick={() => push('error', error)}
        >
          ⚠
        </button>
      )}
      <button
        type="button"
        className="button button--primary button--small"
        onClick={() => void connect()}
        disabled={connecting}
        data-testid="connect-wallet"
      >
        {connecting ? 'Connecting…' : 'Connect Wallet'}
      </button>
    </div>
  );
}
