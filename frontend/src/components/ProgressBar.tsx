export type ProgressTone = 'primary' | 'success' | 'danger';

interface ProgressBarProps {
  /** 0–100 (values outside the range are clamped). */
  value: number;
  tone?: ProgressTone;
  label?: string;
}

/** A11y-friendly progress bar with an optional screen-reader label. */
export function ProgressBar({ value, tone = 'primary', label }: ProgressBarProps) {
  const clamped = Math.min(100, Math.max(0, value));
  return (
    <div
      className={`progress ${tone === 'danger' ? 'progress--danger' : ''} ${
        tone === 'success' ? 'progress--success' : ''
      }`}
      role="progressbar"
      aria-valuenow={clamped}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-label={label ?? 'Progress'}
      data-testid="progressbar"
    >
      <div className="progress__fill" style={{ width: `${clamped}%` }} />
    </div>
  );
}
