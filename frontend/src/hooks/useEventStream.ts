import { useEffect, useRef, useState } from 'react';
import { rpc, scValToNative } from '@stellar/stellar-sdk';
import { server } from '../lib/rpc';
import { EVENT_POLL_MS } from '../config';
import type { DecodedEvent } from '../lib/types';

export type StreamStatus = 'idle' | 'streaming' | 'error';

interface EventStreamOptions {
  contractIds: string[];
  /** Optional fixed two-topic filter, e.g. ["factory", "campaign_created"]. */
  topics?: [string, string];
  onEvent: (event: DecodedEvent) => void;
  enabled: boolean;
}

/** Decode an RPC event into the UI shape. */
export function decodeEvent(raw: rpc.Api.EventResponse): DecodedEvent {
  const topicValues = raw.topic.map(scValToNative);
  const name = typeof topicValues[1] === 'string' ? topicValues[1] : 'unknown';
  const value = scValToNative(raw.value);
  // scValToNative returns a key-value object for maps (v16+), but tolerate a
  // JS Map as well.
  const entries: [string, unknown][] =
    value instanceof Map
      ? [...value.entries()].map(([k, v]) => [String(k), v])
      : value !== null && typeof value === 'object'
        ? Object.entries(value as Record<string, unknown>)
        : [];
  return {
    id: raw.id,
    ledger: raw.ledger,
    contractId: raw.contractId ? String(raw.contractId) : '',
    name,
    data: Object.fromEntries(entries),
    topics: topicValues.slice(2),
  };
}

/**
 * Polls the RPC `getEvents` endpoint with cursor pagination and calls
 * `onEvent` for every new event. Reconnects automatically after errors —
 * the cursor guarantees no duplicate processing and no gaps.
 */
export function useEventStream({
  contractIds,
  topics,
  onEvent,
  enabled,
}: EventStreamOptions): StreamStatus {
  const [status, setStatus] = useState<StreamStatus>('idle');
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  const key = `${contractIds.join(',')}|${topics?.join(',') ?? ''}`;

  useEffect(() => {
    if (!enabled || contractIds.length === 0) {
      setStatus('idle');
      return;
    }
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let cursor: string | undefined;

    const filters: rpc.Api.EventFilter[] = [
      topics
        ? { type: 'contract', contractIds, topics: [[...topics]] }
        : { type: 'contract', contractIds },
    ];

    const poll = async () => {
      try {
        const request = cursor
          ? { filters, cursor, limit: 100 }
          : { filters, startLedger: 1, limit: 100 };
        const res = await server.getEvents(request as rpc.Api.GetEventsRequest);
        if (cancelled) return;
        cursor = res.cursor;
        for (const event of res.events) {
          onEventRef.current(decodeEvent(event));
        }
        setStatus('streaming');
      } catch {
        if (!cancelled) setStatus('error');
      } finally {
        if (!cancelled) {
          timer = setTimeout(poll, EVENT_POLL_MS);
        }
      }
    };

    void poll();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, key]);

  return status;
}
