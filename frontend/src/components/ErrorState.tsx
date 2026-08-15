interface ErrorStateProps {
  title?: string;
  message: string;
  onRetry?: () => void;
}

/** Full-width error panel with an optional retry action. */
export function ErrorState({ title = 'Something went wrong', message, onRetry }: ErrorStateProps) {
  return (
    <div className="error-state" role="alert" data-testid="error-state">
      <div className="error-state__icon" aria-hidden="true">
        !
      </div>
      <h3>{title}</h3>
      <p>{message}</p>
      {onRetry && (
        <button type="button" className="button button--ghost" onClick={onRetry}>
          Try again
        </button>
      )}
    </div>
  );
}
