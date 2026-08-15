/**
 * Wire-format types shared with the Soroban contracts. Values are decoded
 * from SCVal via `scValToNative`: i128 -> bigint, u32/u64 -> number,
 * contracttype structs -> plain objects, Address -> string.
 */

export type Status = 'Active' | 'Funded' | 'Refunding' | 'Closed';

export interface Milestone {
  amount: bigint;
  released: boolean;
}

export interface CampaignInfo {
  name: string;
  address: string;
  creator: string;
  token: string;
  goal: bigint;
  deadline: number; // unix seconds
  milestones: bigint[];
  created_at: number;
}

export interface CampaignConfig {
  name: string;
  creator: string;
  token: string;
  factory: string;
  goal: bigint;
  deadline: number;
  total_raised: bigint;
  refunded_total: bigint;
  status: Status;
}

export interface CampaignState {
  config: CampaignConfig;
  milestones: Milestone[];
}

export interface FactoryStats {
  campaignCount: number;
  totalRaised: bigint;
  creationFee: bigint;
}

/** Decoded Soroban event, shaped for the UI. */
export interface DecodedEvent {
  id: string;
  ledger: number;
  contractId: string;
  /** Event name, e.g. "contributed". */
  name: string;
  /** Named data map, e.g. { amount: 1000n, total_raised: 3000n }. */
  data: Record<string, unknown>;
  /** Extra topic values beyond the two fixed ones (e.g. contributor). */
  topics: unknown[];
}

export interface TxResult {
  hash: string;
  ledger: number;
}
