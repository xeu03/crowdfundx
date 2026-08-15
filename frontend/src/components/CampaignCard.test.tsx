import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { CampaignCard } from './CampaignCard';
import type { CampaignInfo } from '../lib/types';

const info: CampaignInfo = {
  name: 'Moonbase One',
  address: 'CD5ZQSFQJ5DFB6PNZ5PZ7BW7PQ3Q6MLLFF2ODPQG4J5VTW6EWF3AAYHT',
  creator: 'GBBMLHKAWLHAPFZZQJBPVX6HVSKDBE7WPTTFTUIFRW6S4VZCLI6B6BLA',
  token: 'CAJXGFIU32R2SF4BVXV2EB2XSSUPUBQMNXWJWB5GYS7WE76TFPPR7Q7P',
  goal: 1_000_000_000n, // 100 CFX
  deadline: Math.floor(Date.now() / 1000) + 86_400,
  milestones: [500_000_000n, 500_000_000n],
  created_at: 1_752_600_000,
};

describe('CampaignCard', () => {
  it('renders name, goal, deadline and milestone count', () => {
    render(
      <MemoryRouter>
        <CampaignCard info={info} />
      </MemoryRouter>,
    );
    expect(screen.getByText('Moonbase One')).toBeInTheDocument();
    expect(screen.getByText('100 CFX')).toBeInTheDocument();
    expect(screen.getByText(/2 milestones scheduled/)).toBeInTheDocument();
    expect(screen.getByText(/by GBBM…6BLA/)).toBeInTheDocument();
  });

  it('links to the campaign detail page', () => {
    render(
      <MemoryRouter>
        <CampaignCard info={info} />
      </MemoryRouter>,
    );
    expect(screen.getByTestId('campaign-card')).toHaveAttribute(
      'href',
      `/campaign/${info.address}`,
    );
  });
});
