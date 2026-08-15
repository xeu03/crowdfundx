import { useCallback, useEffect, useState } from 'react';
import { fetchCampaigns, fetchFactoryStats } from '../lib/contracts';
import { isConfigured } from '../config';
import type { CampaignInfo, FactoryStats } from '../lib/types';

export interface CampaignsState {
  campaigns: CampaignInfo[];
  stats: FactoryStats | null;
  loading: boolean;
  error: string | null;
  reload: () => void;
}

/**
 * Loads the campaign registry + platform stats, with a `refreshKey` bump to
 * trigger reloads from the event stream (new campaign created, contribution
 * tracked, ...).
 */
export function useCampaigns(refreshKey = 0): CampaignsState {
  const [campaigns, setCampaigns] = useState<CampaignInfo[]>([]);
  const [stats, setStats] = useState<FactoryStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!isConfigured) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const [campaignList, factoryStats] = await Promise.all([
        fetchCampaigns(),
        fetchFactoryStats(),
      ]);
      setCampaigns(campaignList);
      setStats(factoryStats);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not load campaigns');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load, refreshKey]);

  return { campaigns, stats, loading, error, reload: load };
}
