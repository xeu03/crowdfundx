import { Link } from 'react-router-dom';
import { formatCFX, formatDeadline, shortAddress, timeLeft } from '../lib/format';
import type { CampaignInfo } from '../lib/types';

interface CampaignCardProps {
  info: CampaignInfo;
}

/**
 * Registry-level campaign summary. Live raised amounts are only available
 * from the campaign contract itself, so the card shows goal/schedule and
 * links to the detail page for the full picture.
 */
export function CampaignCard({ info }: CampaignCardProps) {
  return (
    <Link
      to={`/campaign/${info.address}`}
      className="card campaign-card"
      data-testid="campaign-card"
    >
      <div className="campaign-card__header">
        <h3 className="campaign-card__name">{info.name}</h3>
        <span className="campaign-card__arrow" aria-hidden="true">
          →
        </span>
      </div>
      <p className="campaign-card__creator">by {shortAddress(info.creator)}</p>
      <div className="campaign-card__goal">
        <span className="campaign-card__goal-label">Goal</span>
        <strong>{formatCFX(info.goal)} CFX</strong>
      </div>
      <p className="campaign-card__deadline">
        {timeLeft(info.deadline)} · ends {formatDeadline(info.deadline)}
      </p>
      <p className="campaign-card__milestones">
        {info.milestones.length} milestone{info.milestones.length === 1 ? '' : 's'} scheduled
      </p>
    </Link>
  );
}
