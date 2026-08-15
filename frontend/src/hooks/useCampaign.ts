import { useCallback, useEffect, useState } from 'react';
import { fetchCampaignState, fetchContribution } from '../lib/contracts';
import type { CampaignState } from '../lib/types';

export interface CampaignDetailState {
  state: CampaignState | null;
  contribution: bigint;
  loading: boolean;
  error: string | null;
  reload: () => void;
}

/**
 * Loads a single campaign's state and (when connected) the viewer's
 * contribution to it.
 */
export function useCampaign(campaignId: string, walletAddress: string | null): CampaignDetailState {
  const [state, setState] = useState<CampaignState | null>(null);
  const [contribution, setContribution] = useState<bigint>(0n);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const campaignState = await fetchCampaignState(campaignId);
      setState(campaignState);
      if (walletAddress) {
        setContribution(await fetchContribution(campaignId, walletAddress));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not load campaign');
    } finally {
      setLoading(false);
    }
  }, [campaignId, walletAddress]);

  useEffect(() => {
    void load();
  }, [load]);

  return { state, contribution, loading, error, reload: load };
}
