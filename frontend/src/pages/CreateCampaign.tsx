import { useState, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { useToast } from '../hooks/useToast';
import { createCampaignTx } from '../lib/contracts';
import { formatCFX, parseCFX } from '../lib/format';
import { isConfigured } from '../config';

interface CreateProps {
  walletAddress: string | null;
}

interface MilestoneRow {
  amount: string;
}

/** Minimum lead time for a new campaign deadline. */
const MIN_DEADLINE_MS = 3_600_000;

export function CreateCampaign({ walletAddress }: CreateProps) {
  const navigate = useNavigate();
  const { push } = useToast();
  const [name, setName] = useState('');
  const [goal, setGoal] = useState('');
  const [deadline, setDeadline] = useState('');
  const [rows, setRows] = useState<MilestoneRow[]>([
    { amount: '' },
    { amount: '' },
  ]);
  const [fieldError, setFieldError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const milestoneTotal = rows.reduce((sum, row) => {
    try {
      return sum + parseCFX(row.amount);
    } catch {
      return sum;
    }
  }, 0n);

  const addRow = () => setRows((current) => [...current, { amount: '' }]);
  const removeRow = (index: number) =>
    setRows((current) => current.filter((_, i) => i !== index));
  const updateRow = (index: number, value: string) =>
    setRows((current) =>
      current.map((row, i) => (i === index ? { ...row, amount: value } : row)),
    );

  const validate = (): { goalRaw: bigint; deadlineUnix: number; milestonesRaw: bigint[] } | null => {
    setFieldError(null);
    if (name.trim().length < 3) {
      setFieldError('Give the campaign a name (3+ characters).');
      return null;
    }
    let goalRaw: bigint;
    try {
      goalRaw = parseCFX(goal);
    } catch {
      setFieldError('Enter a valid funding goal.');
      return null;
    }
    const deadlineUnix = Math.floor(new Date(deadline).getTime() / 1000);
    if (!Number.isFinite(deadlineUnix) || deadlineUnix * 1000 < Date.now() + MIN_DEADLINE_MS) {
      setFieldError('Pick a deadline at least an hour from now.');
      return null;
    }
    if (rows.length < 1 || rows.some((row) => row.amount.trim() === '')) {
      setFieldError('Add at least one milestone amount.');
      return null;
    }
    const milestonesRaw: bigint[] = [];
    try {
      for (const row of rows) milestonesRaw.push(parseCFX(row.amount));
    } catch {
      setFieldError('Every milestone must be a valid amount.');
      return null;
    }
    if (milestoneTotal !== goalRaw) {
      setFieldError(
        `Milestones sum to ${formatCFX(milestoneTotal)} CFX — they must exactly equal the goal (${formatCFX(goalRaw)} CFX).`,
      );
      return null;
    }
    return { goalRaw, deadlineUnix, milestonesRaw };
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!walletAddress) {
      push('error', 'Connect your wallet first');
      return;
    }
    if (!isConfigured) {
      push('error', 'App is not pointed at a deployment — check frontend/.env');
      return;
    }
    const values = validate();
    if (!values) return;
    setSubmitting(true);
    try {
      const { hash } = await createCampaignTx(walletAddress, {
        name: name.trim(),
        goal: values.goalRaw,
        deadline: values.deadlineUnix,
        milestones: values.milestonesRaw,
      });
      push('success', `Campaign deployed — ${hash.slice(0, 10)}…`);
      navigate('/');
    } catch (err) {
      push('error', err instanceof Error ? err.message : 'Deployment failed');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="container create-layout">
      <div className="create-intro">
        <h1>Start a campaign</h1>
        <p>
          Set a goal, schedule milestone payouts, and let the factory deploy your
          campaign contract on-chain. If the goal isn't reached in time, every
          backer can claim a full refund.
        </p>
      </div>

      <form className="card create-form" onSubmit={(e) => void handleSubmit(e)}>
        <label className="field-label" htmlFor="name">
          Campaign name
        </label>
        <input
          id="name"
          className="input"
          placeholder="e.g. Moonbase One"
          value={name}
          onChange={(e) => setName(e.target.value)}
          maxLength={32}
        />

        <div className="form-row">
          <div>
            <label className="field-label" htmlFor="goal">
              Funding goal (CFX)
            </label>
            <input
              id="goal"
              className="input"
              inputMode="decimal"
              placeholder="1000.00"
              value={goal}
              onChange={(e) => setGoal(e.target.value)}
            />
          </div>
          <div>
            <label className="field-label" htmlFor="deadline">
              Deadline
            </label>
            <input
              id="deadline"
              className="input"
              type="datetime-local"
              value={deadline}
              onChange={(e) => setDeadline(e.target.value)}
            />
          </div>
        </div>

        <fieldset className="milestones-editor">
          <legend className="field-label">Milestone payouts</legend>
          <p className="detail-hint">
            Amounts are released to you in order once the goal is funded. The
            milestone amounts must sum exactly to the goal.
          </p>
          {rows.map((row, index) => (
            <div className="milestone-row" key={index}>
              <span className="milestone-row__index">{index + 1}</span>
              <input
                className="input"
                inputMode="decimal"
                placeholder="500.00"
                aria-label={`Milestone ${index + 1} amount`}
                value={row.amount}
                onChange={(e) => updateRow(index, e.target.value)}
              />
              <button
                type="button"
                className="button button--ghost button--small"
                onClick={() => removeRow(index)}
                disabled={rows.length <= 1}
                aria-label={`Remove milestone ${index + 1}`}
              >
                ✕
              </button>
            </div>
          ))}
          <button type="button" className="button button--ghost button--small" onClick={addRow}>
            + Add milestone
          </button>
        </fieldset>

        <p className="milestone-summary" data-testid="milestone-summary">
          Milestones: {formatCFX(milestoneTotal)} CFX / goal:{' '}
          {goal.trim() === '' ? '—' : `${goal} CFX`}
        </p>

        {fieldError && (
          <p className="field-error" role="alert">
            {fieldError}
          </p>
        )}

        <button
          type="submit"
          className="button button--primary button--large button--block"
          disabled={submitting || !walletAddress}
        >
          {submitting
            ? 'Deploying contract…'
            : walletAddress
              ? 'Launch campaign'
              : 'Connect wallet to launch'}
        </button>
      </form>
    </div>
  );
}
