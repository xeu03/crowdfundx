# CrowdfundX — Frontend Integration Evidence

> The dApp UI lives in `frontend/` (React 19 + TypeScript + Vite) and talks to
> the Soroban contracts deployed on testnet:
> factory `CB46HW3YW5XVMBQLHOSKTLQ5SQBPDPIUDPG2U6JOCDGMHLBNLI5PHJO7`,
> token `CC4WNTYPTFUQIUUJXL4WRGH2LOCGNGX3ALLFHZZ7QUGFA4OVXGCAT2S7`.
> Live demo: https://aquamarine-semolina-ff4c7e.netlify.app/

## File map

```
frontend/src/lib/rpc.ts           shared Soroban RPC server (@stellar/stellar-sdk rpc.Server)
frontend/src/lib/wallet.ts        Freighter integration (isConnected/requestAccess/getAddress/isAllowed/setAllowed)
frontend/src/lib/contracts.ts     all contract calls: reads + build→prepare→sign→submit→poll
frontend/src/lib/format.ts        CFX decimals, deadlines, addresses
frontend/src/hooks/useWallet.ts   wallet state lifecycle (connect/disconnect/errors)
frontend/src/hooks/useCampaigns.ts  registry + stats loading
frontend/src/hooks/useCampaign.ts   single campaign state + viewer contribution
frontend/src/hooks/useEventStream.ts  live getEvents streaming with cursor pagination
frontend/src/pages/Explore.tsx       campaign grid + platform stats (calls get_campaigns/get_stats)
frontend/src/pages/CampaignDetail.tsx  contribute/refund/release-milestone/close-failed/extend-deadline
frontend/src/pages/CreateCampaign.tsx  create_campaign form with milestone validation
frontend/src/components/*          Header, WalletButton, CampaignCard, ProgressBar, EventFeed, …
```

## 1. Freighter wallet connection

`frontend/src/lib/wallet.ts` — the connect flow:

```ts
import {
  isConnected, requestAccess, getAddress, isAllowed, setAllowed,
} from '@stellar/freighter-api';

export async function connectWallet(): Promise<string> {
  const { isConnected: connected } = await isConnected();
  if (!connected) throw new WalletError('Freighter is not installed…');
  const { address } = await requestAccess();
  const { isAllowed: allowed } = await isAllowed();
  if (!allowed) await setAllowed();       // persist permission
  return address;
}

export async function getWalletAddress(): Promise<string | null> {
  const { isConnected: connected } = await isConnected();
  if (!connected) return null;
  const { address } = await getAddress();
  return address;
}
```

`frontend/src/components/WalletButton.tsx` renders the connect/disconnect UI;
`frontend/src/hooks/useWallet.ts` manages the lifecycle and auto-detects an
existing session on mount.

## 2. Transaction signing

`frontend/src/lib/contracts.ts` — every mutation goes through
`build → prepareTransaction → signTransaction (Freighter) → sendTransaction → poll`:

```ts
import { Address, Contract, Transaction, TransactionBuilder, nativeToScVal, scValToNative, xdr } from '@stellar/stellar-sdk';
import { signTransaction } from '@stellar/freighter-api';
import { server } from './rpc';                       // new rpc.Server(RPC_URL)

export async function submitAndPoll(
  signer: string, contractId: string, method: string, args: xdr.ScVal[],
): Promise<TxResult> {
  const source = await server.getAccount(signer);
  const op = new Contract(contractId).call(method, ...args);
  const tx = new TransactionBuilder(source, { fee: '100000', networkPassphrase: NETWORK_PASSPHRASE })
    .addOperation(op).setTimeout(60).build();

  const prepared = await server.prepareTransaction(tx);
  const { signedTxXdr } = await signTransaction(prepared.toXDR(), {
    networkPassphrase: NETWORK_PASSPHRASE, address: signer,
  });
  const signed = new Transaction(signedTxXdr, NETWORK_PASSPHRASE);
  const sent = await server.sendTransaction(signed);
  // poll server.getTransaction(sent.hash) until SUCCESS/FAILED, decoding
  // invokeHostFunctionResult for readable error messages
}
```

SCVal conversions used throughout:

```ts
export const scvI128 = (v: bigint) => nativeToScVal(v, { type: 'i128' });
export const scvU64  = (v: number) => nativeToScVal(v, { type: 'u64' });
export const scvU32  = (v: number) => nativeToScVal(v, { type: 'u32' });
// reads decode with scValToNative(sim.result.retval)
```

## 3. UI actions → contract functions (cross-check)

| UI action | Page | Contract function | Frontend caller |
| --- | --- | --- | --- |
| Explore grid + stats | `Explore.tsx` | factory `get_campaigns`, `get_stats` | `fetchCampaigns()`, `fetchFactoryStats()` |
| Campaign detail snapshot | `CampaignDetail.tsx` | campaign `get_state`, `get_contribution` | `fetchCampaignState()`, `fetchContribution()` |
| Back a campaign | `CampaignDetail.tsx` | campaign `contribute(from, amount)` | `contributeTx()` |
| Release a milestone | `CampaignDetail.tsx` | campaign `release_milestone(index)` | `releaseMilestoneTx()` |
| Claim refund | `CampaignDetail.tsx` | campaign `refund(contributor)` | `refundTx()` |
| Close failed campaign | `CampaignDetail.tsx` | campaign `close_failed()` | `closeFailedTx()` |
| Extend deadline | `CampaignDetail.tsx` | campaign `extend_deadline(new_deadline)` | `extendDeadlineTx()` |
| Launch a campaign | `CreateCampaign.tsx` | factory `create_campaign(creator, name, token, goal, deadline, milestones)` | `createCampaignTx()` |
| Token balance (faucet) | lib | token `balance`, `mint` | `fetchTokenBalance()`, `mintFaucetTx()` |

The argument order above matches the Soroban contracts in
`contracts/{factory,campaign,token}/src/lib.rs` exactly — verified by the
deployed factory/campaign on testnet (see `deployment.json` and the
transaction hash in the README checklist).

## 4. Live event streaming

`frontend/src/hooks/useEventStream.ts` polls `server.getEvents()` with cursor
pagination and decodes contract events into UI updates:

```ts
const filters: rpc.Api.EventFilter[] = [
  { type: 'contract', contractIds, topics: [['factory', 'campaign_created']] },
];
const res = await server.getEvents({ filters, cursor, limit: 100 });
// decodeEvent(): scValToNative topics -> name, scValToNative value -> data map
```

## 5. Tests

`frontend/src/**/*.test.{ts,tsx}` — 24 Vitest tests (`npm test`), covering
format utils, ProgressBar, CampaignCard, EventFeed, and the event-stream hook
(cursor pagination, unmount cleanup, RPC error recovery). CI runs them on
every push (`.github/workflows/ci.yml`).
