interface SkeletonProps {
  width?: string;
  height?: string;
  /** Render as a card-shaped block (default) or a plain bar. */
  variant?: 'card' | 'bar';
}

export function Skeleton({ width, height, variant = 'bar' }: SkeletonProps) {
  return (
    <div
      className={variant === 'card' ? 'skeleton skeleton--card' : 'skeleton'}
      style={{ width, height }}
      aria-hidden="true"
    />
  );
}

/** Grid of card skeletons shown while the campaign list loads. */
export function CampaignListSkeleton({ count = 3 }: { count?: number }) {
  return (
    <div className="card-grid" aria-busy="true" data-testid="campaign-skeleton">
      {Array.from({ length: count }, (_, i) => (
        <div className="card" key={i}>
          <Skeleton variant="bar" width="70%" height="1.2rem" />
          <Skeleton variant="bar" width="40%" height="0.9rem" />
          <Skeleton variant="bar" width="100%" height="0.6rem" />
          <Skeleton variant="bar" width="100%" height="2.2rem" />
        </div>
      ))}
    </div>
  );
}
