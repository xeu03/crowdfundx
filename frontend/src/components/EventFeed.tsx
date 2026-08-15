import { formatCFX, shortAddress } from '../lib/format';
import type { DecodedEvent } from '../lib/types';

/** Decoded event values are `unknown` — coerce to bigint safely. */
const asBigInt = (value: unknown): bigint =>
  typeof value === 'bigint' || typeof value === 'number' || typeof value === 'string'
    ? BigInt(value)
    : 0n;

const FRIENDLY: Record<string, (e: DecodedEvent) => string> = {
  contributed: (e) =>
    `${shortAddress(String(e.topics[0] ?? ''))} contributed ${formatCFX(asBigInt(e.data.amount))}`,
  goal_reached: () => '🎉 Goal reached!',
  milestone_released: (e) => `Milestone #${String(e.data.index ?? '?')} released to creator`,
  refunded: (e) =>
    `${shortAddress(String(e.topics[0] ?? ''))} claimed ${formatCFX(asBigInt(e.data.amount))}`,
  failed: () => 'Campaign closed under goal — refunds open',
  extended: (e) => `Deadline extended to ${new Date(Number(asBigInt(e.data.new_deadline)) * 1000).toLocaleDateString()}`,
  campaign_created: (e) => `New campaign “${String(e.data.name ?? '')}” launched`,
  contribution_tracked: (e) => `Platform total updated (+${formatCFX(asBigInt(e.data.amount))})`,
};

function describe(event: DecodedEvent): string {
  const describeFn = FRIENDLY[event.name];
  return describeFn ? describeFn(event) : event.name;
}

interface EventFeedProps {
  events: DecodedEvent[];
  emptyText?: string;
}

/** Live feed of contract events, newest first. */
export function EventFeed({ events, emptyText = 'No activity yet' }: EventFeedProps) {
  return (
    <div className="event-feed" data-testid="event-feed">
      <h3 className="event-feed__title">Live activity</h3>
      {events.length === 0 ? (
        <p className="event-feed__empty">{emptyText}</p>
      ) : (
        <ul className="event-feed__list">
          {events.map((event) => (
            <li key={event.id} className="event-feed__item">
              <span className="event-feed__dot" aria-hidden="true" />
              <div>
                <p className="event-feed__text">{describe(event)}</p>
                <span className="event-feed__meta">
                  #{event.ledger.toLocaleString()} · {event.name}
                </span>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
