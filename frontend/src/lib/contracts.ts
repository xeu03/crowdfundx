import {
  Address,
  Contract,
  Transaction,
  TransactionBuilder,
  nativeToScVal,
  scValToNative,
  xdr,
  type TransactionSource,
} from '@stellar/stellar-sdk';
import { signTransaction } from '@stellar/freighter-api';
import { server } from './rpc';
import {
  FACTORY_ADDRESS,
  NETWORK_PASSPHRASE,
  TOKEN_ADDRESS,
  TX_TIMEOUT_MS,
} from '../config';
import type { CampaignInfo, CampaignState, FactoryStats, TxResult } from './types';

const BASE_FEE = '100000';

// ---------------------------------------------------------------------------
// SCVal helpers (i128/u64/u32 encode as native values via nativeToScVal)
// ---------------------------------------------------------------------------

export const scvI128 = (v: bigint): xdr.ScVal => nativeToScVal(v, { type: 'i128' });
export const scvU64 = (v: number): xdr.ScVal => nativeToScVal(v, { type: 'u64' });
export const scvU32 = (v: number): xdr.ScVal => nativeToScVal(v, { type: 'u32' });
export const scvVec = (items: xdr.ScVal[]): xdr.ScVal => xdr.ScVal.scvVec(items);

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * A simulated read against a contract. Reads don't need a real signature —
 * only the simulated retval matters — so a throwaway source account is fine.
 */
export async function simulateRead<T>(
  contractId: string,
  method: string,
  args: xdr.ScVal[] = [],
): Promise<T> {
  const address = 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF';
  const dummySource: TransactionSource = {
    accountId: () => address,
    sequenceNumber: () => '0',
    incrementSequenceNumber: () => {},
  };
  const op = new Contract(contractId).call(method, ...args);
  const tx = new TransactionBuilder(dummySource, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(op)
    .setTimeout(30)
    .build();
  const sim = await server.simulateTransaction(tx);
  if ('error' in sim && sim.error) {
    throw new Error(`Simulation failed: ${String(sim.error)}`);
  }
  const retval = 'result' in sim ? sim.result?.retval : undefined;
  if (!retval) throw new Error('Simulation returned no value');
  return scValToNative(retval) as T;
}

/** Extract a human-readable error from a failed transaction result. */
function decodeTxFailure(result: xdr.TransactionResult | undefined): string {
  if (!result) return 'Transaction failed';
  try {
    const results = result.result().results();
    const first = results[0];
    if (first) {
      const hostFn = first.tr().invokeHostFunctionResult();
      if (hostFn.switch().value !== 0) {
        return `Contract call failed (${hostFn.switch().name})`;
      }
    }
    return 'Transaction failed';
  } catch {
    return 'Transaction failed';
  }
}

/**
 * Build → prepare → wallet-sign → submit → poll a contract invocation.
 * Throws with a user-readable message on failure.
 */
export async function submitAndPoll(
  signer: string,
  contractId: string,
  method: string,
  args: xdr.ScVal[],
): Promise<TxResult> {
  const source = await server.getAccount(signer);
  const op = new Contract(contractId).call(method, ...args);
  const tx = new TransactionBuilder(source, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(op)
    .setTimeout(60)
    .build();

  const prepared = await server.prepareTransaction(tx);
  const { signedTxXdr } = await signTransaction(prepared.toXDR(), {
    networkPassphrase: NETWORK_PASSPHRASE,
    address: signer,
  });
  const signed = new Transaction(signedTxXdr, NETWORK_PASSPHRASE);
  const sent = await server.sendTransaction(signed);
  if (sent.status === 'ERROR') {
    throw new Error('Transaction rejected by the network');
  }

  const deadline = Date.now() + TX_TIMEOUT_MS;
  while (Date.now() < deadline) {
    const res = await server.getTransaction(sent.hash);
    if (res.status === 'SUCCESS') {
      return { hash: sent.hash, ledger: res.ledger };
    }
    if (res.status === 'FAILED') {
      throw new Error(decodeTxFailure(res.resultXdr));
    }
    await sleep(1_500);
  }
  throw new Error('Transaction timed out — check the explorer for its status');
}

// ---------------------------------------------------------------------------
// Platform queries (simulated reads)
// ---------------------------------------------------------------------------

export async function fetchFactoryStats(): Promise<FactoryStats> {
  const [count, totalRaised, fee] = await simulateRead<[number, bigint, bigint]>(
    FACTORY_ADDRESS,
    'get_stats',
  );
  return { campaignCount: count, totalRaised, creationFee: fee };
}

/**
 * The RPC decodes u64 contract values as BigInt. The UI treats ledger
 * timestamps as JS numbers, so coerce them at the data boundary.
 */
const asNumber = (v: unknown): number => Number(v as number | bigint);

export async function fetchCampaigns(): Promise<CampaignInfo[]> {
  const raw = await simulateRead<CampaignInfo[]>(FACTORY_ADDRESS, 'get_campaigns');
  return raw.map((info) => ({
    ...info,
    deadline: asNumber(info.deadline),
    created_at: asNumber(info.created_at),
  }));
}

export async function fetchCampaignState(campaignId: string): Promise<CampaignState> {
  const [config, milestones] = await simulateRead<
    [CampaignState['config'], CampaignState['milestones']]
  >(campaignId, 'get_state');
  return {
    config: { ...config, deadline: asNumber(config.deadline) },
    milestones,
  };
}

export async function fetchContribution(campaignId: string, contributor: string): Promise<bigint> {
  return simulateRead<bigint>(campaignId, 'get_contribution', [
    new Address(contributor).toScVal(),
  ]);
}

export async function fetchTokenBalance(owner: string): Promise<bigint> {
  return simulateRead<bigint>(TOKEN_ADDRESS, 'balance', [new Address(owner).toScVal()]);
}

// ---------------------------------------------------------------------------
// Mutations (signed transactions)
// ---------------------------------------------------------------------------

export async function createCampaignTx(
  signer: string,
  params: { name: string; goal: bigint; deadline: number; milestones: bigint[] },
): Promise<TxResult> {
  return submitAndPoll(signer, FACTORY_ADDRESS, 'create_campaign', [
    new Address(signer).toScVal(),
    nativeToScVal(params.name, { type: 'string' }),
    new Address(TOKEN_ADDRESS).toScVal(),
    scvI128(params.goal),
    scvU64(params.deadline),
    scvVec(params.milestones.map(scvI128)),
  ]);
}

export async function contributeTx(
  signer: string,
  campaignId: string,
  amount: bigint,
): Promise<TxResult> {
  return submitAndPoll(signer, campaignId, 'contribute', [
    new Address(signer).toScVal(),
    scvI128(amount),
  ]);
}

export async function releaseMilestoneTx(
  signer: string,
  campaignId: string,
  index: number,
): Promise<TxResult> {
  return submitAndPoll(signer, campaignId, 'release_milestone', [scvU32(index)]);
}

export async function refundTx(signer: string, campaignId: string): Promise<TxResult> {
  return submitAndPoll(signer, campaignId, 'refund', [new Address(signer).toScVal()]);
}

export async function closeFailedTx(signer: string, campaignId: string): Promise<TxResult> {
  return submitAndPoll(signer, campaignId, 'close_failed', []);
}

export async function extendDeadlineTx(
  signer: string,
  campaignId: string,
  newDeadline: number,
): Promise<TxResult> {
  return submitAndPoll(signer, campaignId, 'extend_deadline', [scvU64(newDeadline)]);
}

/** Demo faucet: admin-mints CFX so testers can try the platform. */
export async function mintFaucetTx(signer: string, to: string, amount: bigint): Promise<TxResult> {
  return submitAndPoll(signer, TOKEN_ADDRESS, 'mint', [
    new Address(to).toScVal(),
    scvI128(amount),
  ]);
}
