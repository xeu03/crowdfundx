import { useCallback, useState, type FormEvent } from 'react';
import { Link, useParams } from 'react-router-dom';
import { ErrorState } from '../components/ErrorState';
import { EventFeed } from '../components/EventFeed';
import { ProgressBar } from '../components/ProgressBar';
import { Skeleton } from '../components/Skeleton';
import { StatusBadge } from '../components/StatusBadge';
import { useCampaign } from '../hooks/useCampaign';
import { useEventStream } from '../hooks/useEventStream';
import { useToast } from '../hooks/useToast';
import {
  closeFailedTx,
  contributeTx,
  extendDeadlineTx,
  refundTx,
  releaseMilestoneTx,
} from '../lib/contracts';
import { formatCFX, formatDeadline, parseCFX, progressPercent, shortAddress, timeLeft } from '../lib/format';
import type { DecodedEvent, Status } from '../lib/types';

interface DetailProps {
  walletAddress: string | null;
}

export function CampaignDetail({ walletAddress }: DetailProps) {
  const { id = '' } = useParams();
  const { push } = useToast();
  const { state, contribution, loading, error, reload } = useCampaign(id, walletAddress);
  const [events, setEvents] = useState<DecodedEvent[]>([]);
  const [amountInput, setAmountInput] = useState('');
  const [inputError, setInputError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [extendInput, setExtendInput] = useState('');

  const onEvent = useCallback(
    (event: DecodedEvent) => {
      setEvents((current) => [event, ...current].slice(0, 50));
      // Any state-changing event invalidates the snapshot.
      reload();
    },
    [reload],
  );

  useEventStream({
    contractIds: id ? [id] : [],
    onEvent,
    enabled: id.length > 0,
  });

  const run = async (action: string, fn: () => Promise<{ hash: string }>) => {
    if (!walletAddress) {
      push('error', 'Connect your wallet first');
      return;
    }
    setBusy(action);
    try {
      const { hash } = await fn();
      push('success', `Transaction confirmed — ${hash.slice(0, 10)}…`);
      reload();
    } catch (err) {
      push('error', err instanceof Error ? err.message : 'Transaction failed');
    } finally {
      setBusy(null);
    }
  };

  const handleContribute = (e: FormEvent) => {
    e.preventDefault();
    setInputError(null);
    try {
      const amount = parseCFX(amountInput);
      void run('contribute', () => contributeTx(walletAddress!, id, amount));
    } catch (err) {
      setInputError(err instanceof Error ? err.message : 'Invalid amount');
    }
  };

  const isCreator = state !== null && walletAddress !== null && state.config.creator === walletAddress;
  const isExpired =
    state !== null && state.config.deadline * 1000 < Date.now() && state.config.status === 'Active';
  const canRefund =
    state !== null && state.config.status === 'Refunding' && contribution > 0n;

  if (loading && state === null) {
    return (
      <div className="container">
        <Skeleton variant="card" height="16rem" />
      </div>
    );
  }

  if (error || state === null) {
    return (
      <div className="container">
        <ErrorState message={error ?? 'Campaign not found'} onRetry={reload} />
      </div>
    );
  }

  const { config, milestones } = state;
  const percent = progressPercent(config.total_raised, config.goal);
  const status: Status = config.status;

  return (
    <div className="container detail-layout">
      <div className="detail-main">
        <Link to="/" className="back-link">
          ← All campaigns
        </Link>
        <div className="detail-header">
          <h1>{config.name}</h1>
          <StatusBadge status={status} />
        </div>
        <p className="detail-creator">by {shortAddress(config.creator)}</p>

        <div className="card detail-card">
          <div className="detail-raised">
            <strong>{formatCFX(config.total_raised)} CFX</strong>
            <span>
              raised of {formatCFX(config.goal)} goal · {timeLeft(config.deadline)}
            </span>
          </div>
          <ProgressBar value={percent} tone={percent >= 100 ? 'success' : 'primary'} label="Funding progress" />
          <p className="detail-deadline">
            Deadline: {formatDeadline(config.deadline)} ·{' '}
            {config.refunded_total > 0n && `${formatCFX(config.refunded_total)} refunded`}
          </p>

          <h2 className="detail-subtitle">Milestones</h2>
          <ol className="milestones">
            {milestones.map((m, index) => (
              <li
                key={index}
                className={`milestone ${m.released ? 'milestone--released' : ''}`}
              >
                <span className="milestone__index">{index + 1}</span>
                <span className="milestone__amount">{formatCFX(m.amount)} CFX</span>
                <span className="milestone__state">
                  {m.released ? 'Paid out' : 'Pending'}
                </span>
              </li>
            ))}
          </ol>
        </div>

        {isCreator && status === 'Funded' && (
          <div className="card detail-card">
            <h2 className="detail-subtitle">Creator panel</h2>
            <p className="detail-hint">
              Release each milestone to move its share of the vault to your wallet.
            </p>
            <div className="milestone-actions">
              {milestones.map((m, index) =>
                m.released ? null : (
                  <button
                    key={index}
                    type="button"
                    className="button button--primary"
                    disabled={busy !== null}
                    onClick={() =>
                      void run(`milestone-${index}`, () => releaseMilestoneTx(walletAddress!, id, index))
                    }
                  >
                    {busy === `milestone-${index}` ? 'Releasing…' : `Release milestone ${index + 1}`}
                  </button>
                ),
              )}
            </div>
          </div>
        )}

        {isCreator && status === 'Active' && (
          <div className="card detail-card">
            <h2 className="detail-subtitle">Creator panel</h2>
            <form
              className="extend-form"
              onSubmit={(e) => {
                e.preventDefault();
                const unix = Math.floor(new Date(extendInput).getTime() / 1000);
                if (!Number.isFinite(unix) || unix * 1000 <= Date.now()) {
                  push('error', 'Pick a future date');
                  return;
                }
                void run('extend', () => extendDeadlineTx(walletAddress!, id, unix));
              }}
            >
              <input
                type="datetime-local"
                className="input"
                value={extendInput}
                onChange={(e) => setExtendInput(e.target.value)}
                aria-label="New deadline"
              />
              <button type="submit" className="button button--ghost" disabled={busy !== null}>
                {busy === 'extend' ? 'Extending…' : 'Extend deadline'}
              </button>
            </form>
          </div>
        )}

        {isExpired && (
          <div className="card detail-card detail-card--warning">
            <p>This campaign ended below its goal. Close it to open refunds.</p>
            <button
              type="button"
              className="button button--danger"
              disabled={busy !== null}
              onClick={() => void run('close', () => closeFailedTx(walletAddress!, id))}
            >
              {busy === 'close' ? 'Closing…' : 'Close campaign & open refunds'}
            </button>
          </div>
        )}

        {canRefund && (
          <div className="card detail-card detail-card--warning">
            <p>
              You contributed {formatCFX(contribution)}. Claim your refund before the
              vault is drained.
            </p>
            <button
              type="button"
              className="button button--danger"
              disabled={busy !== null}
              onClick={() => void run('refund', () => refundTx(walletAddress!, id))}
            >
              {busy === 'refund' ? 'Claiming…' : 'Claim refund'}
            </button>
          </div>
        )}
      </div>

      <aside className="detail-side">
        {status === 'Active' && (
          <form className="card contribute-card" onSubmit={handleContribute}>
            <h2>Back this campaign</h2>
            {contribution > 0n && (
              <p className="detail-hint">Your contribution: {formatCFX(contribution)} CFX</p>
            )}
            <label className="field-label" htmlFor="amount">
              Amount (CFX)
            </label>
            <input
              id="amount"
              className="input input--large"
              inputMode="decimal"
              placeholder="0.00"
              value={amountInput}
              onChange={(e) => setAmountInput(e.target.value)}
              disabled={busy !== null}
            />
            {inputError && (
              <p className="field-error" role="alert">
                {inputError}
              </p>
            )}
            <button
              type="submit"
              className="button button--primary button--block"
              disabled={busy !== null || !walletAddress}
            >
              {busy === 'contribute'
                ? 'Submitting…'
                : walletAddress
                  ? 'Contribute'
                  : 'Connect wallet to contribute'}
            </button>
            <p className="detail-hint">
              All-or-nothing: if the goal isn't met by the deadline, every
              contribution is fully refundable.
            </p>
          </form>
        )}

        <div className="card event-feed-card">
          <EventFeed events={events} />
        </div>
      </aside>
    </div>
  );
}
