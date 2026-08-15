import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';
import { Contract, nativeToScVal, rpc } from '@stellar/stellar-sdk';
import { useEventStream, decodeEvent } from './useEventStream';
import { server } from '../lib/rpc';

vi.mock('../lib/rpc', () => ({
  server: {
    getEvents: vi.fn(),
  },
}));

const mockGetEvents = vi.mocked(server.getEvents);

const rawEvent = (id: string): rpc.Api.EventResponse => ({
  id,
  type: 'contract',
  ledger: 100,
  ledgerClosedAt: '2026-08-15T00:00:00Z',
  contractId: new Contract('CAJXGFIU32R2SF4BVXV2EB2XSSUPUBQMNXWJWB5GYS7WE76TFPPR7Q7P'),
  topic: [],
  value: nativeToScVal({ amount: 10n, total_raised: 20n }),
  transactionIndex: 0,
  operationIndex: 0,
  inSuccessfulContractCall: true,
  txHash: 'hash',
});

describe('decodeEvent', () => {
  it('decodes topic names and data maps', () => {
    const event = decodeEvent({
      ...rawEvent('e9'),
      topic: [
        nativeToScVal('campaign', { type: 'symbol' }),
        nativeToScVal('contributed', { type: 'symbol' }),
        nativeToScVal('GAAA', { type: 'symbol' }),
      ],
      value: nativeToScVal({ amount: 300n, total_raised: 300n }),
    });
    expect(event.name).toBe('contributed');
    expect(event.data.amount).toBe(300n);
    expect(event.topics).toEqual(['GAAA']);
  });
});

describe('useEventStream', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mockGetEvents.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('streams events via cursor pagination and stops on unmount', async () => {
    mockGetEvents
      .mockResolvedValueOnce({
        events: [rawEvent('e1')],
        cursor: 'cursor-1',
      } as never)
      .mockResolvedValueOnce({
        events: [rawEvent('e2')],
        cursor: 'cursor-2',
      } as never);

    const onEvent = vi.fn();
    const { unmount } = renderHook(() =>
      useEventStream({
        contractIds: ['CAAA'],
        onEvent,
        enabled: true,
      }),
    );

    // Flush the effect and the first poll's microtasks.
    await vi.advanceTimersByTimeAsync(0);
    expect(onEvent).toHaveBeenCalledTimes(1);

    // Second poll reuses the cursor.
    await vi.advanceTimersByTimeAsync(4_000);
    expect(onEvent).toHaveBeenCalledTimes(2);
    expect(mockGetEvents).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ cursor: 'cursor-1' }),
    );

    unmount();
    await vi.advanceTimersByTimeAsync(12_000);
    expect(mockGetEvents).toHaveBeenCalledTimes(2);
  });

  it('keeps polling after a transient error', async () => {
    mockGetEvents.mockRejectedValueOnce(new Error('RPC down'));
    mockGetEvents.mockResolvedValueOnce({ events: [rawEvent('e3')], cursor: 'c' } as never);
    mockGetEvents.mockResolvedValue({ events: [], cursor: 'c2' } as never);

    const onEvent = vi.fn();
    const { result } = renderHook(() =>
      useEventStream({ contractIds: ['CAAA'], onEvent, enabled: true }),
    );

    // First poll rejects, the retry fires on the next tick and succeeds.
    await vi.advanceTimersByTimeAsync(4_000);
    await vi.advanceTimersByTimeAsync(4_000);
    expect(onEvent).toHaveBeenCalledTimes(1);
    expect(result.current).toBe('streaming');
  });

  it('does nothing while disabled', async () => {
    const onEvent = vi.fn();
    renderHook(() =>
      useEventStream({ contractIds: ['CAAA'], onEvent, enabled: false }),
    );
    await vi.advanceTimersByTimeAsync(10_000);
    expect(mockGetEvents).not.toHaveBeenCalled();
  });
});
