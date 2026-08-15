import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { EventFeed } from './EventFeed';
import type { DecodedEvent } from '../lib/types';

const contributed: DecodedEvent = {
  id: 'e1',
  ledger: 12345,
  contractId: 'CAAA',
  name: 'contributed',
  data: { amount: 12_500_000n, total_raised: 25_000_000n },
  topics: ['GBBMLHKAWLHAPFZZQJBPVX6HVSKDBE7WPTTFTUIFRW6S4VZCLI6B6BLA'],
};

describe('EventFeed', () => {
  it('shows an empty state when there are no events', () => {
    render(<EventFeed events={[]} emptyText="Nothing here yet" />);
    expect(screen.getByText('Nothing here yet')).toBeInTheDocument();
  });

  it('renders events newest-first with human descriptions', () => {
    render(<EventFeed events={[contributed]} />);
    expect(screen.getByText(/GBBM…6BLA contributed 1.25/)).toBeInTheDocument();
    expect(screen.getByText(/#12,345 · contributed/)).toBeInTheDocument();
  });

  it('falls back to the raw event name for unknown kinds', () => {
    render(
      <EventFeed
        events={[{ ...contributed, id: 'e2', name: 'something_new', data: { value: 1 } }]}
      />,
    );
    expect(screen.getByText('something_new')).toBeInTheDocument();
  });
});
