import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ProgressBar } from './ProgressBar';

describe('ProgressBar', () => {
  it('renders the fill width from the value', () => {
    const { container } = render(<ProgressBar value={42} label="Funding" />);
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '42');
    const fill = container.querySelector('.progress__fill') as HTMLElement;
    expect(fill.style.width).toBe('42%');
  });

  it('clamps out-of-range values', () => {
    const { rerender } = render(<ProgressBar value={250} />);
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '100');
    rerender(<ProgressBar value={-10} />);
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '0');
  });

  it('applies the success tone when funded', () => {
    const { container } = render(<ProgressBar value={100} tone="success" />);
    expect(container.querySelector('.progress--success')).not.toBeNull();
  });
});
