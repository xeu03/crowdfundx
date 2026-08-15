import type { Status } from '../lib/types';

const STATUS_META: Record<Status, { label: string; className: string }> = {
  Active: { label: 'Active', className: 'status-badge--active' },
  Funded: { label: 'Funded', className: 'status-badge--funded' },
  Refunding: { label: 'Refunding', className: 'status-badge--refunding' },
  Closed: { label: 'Closed', className: 'status-badge--closed' },
};

export function StatusBadge({ status }: { status: Status }) {
  const meta = STATUS_META[status] ?? STATUS_META.Active;
  return <span className={`status-badge ${meta.className}`}>{meta.label}</span>;
}
