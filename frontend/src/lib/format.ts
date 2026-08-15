import { CFX_DECIMALS } from '../config';

/** Convert raw CFX integer units (7 decimals) to a display string. */
export function formatCFX(raw: bigint, maxDecimals = 2): string {
  const divisor = 10n ** BigInt(CFX_DECIMALS);
  const negative = raw < 0n;
  const abs = negative ? -raw : raw;
  const whole = abs / divisor;
  const frac = abs % divisor;
  const fracStr = frac
    .toString()
    .padStart(CFX_DECIMALS, '0')
    .slice(0, maxDecimals)
    .replace(/0+$/, '');
  const sign = negative ? '-' : '';
  return fracStr.length > 0 ? `${sign}${whole}.${fracStr}` : `${sign}${whole}`;
}

/** Parse a user-entered CFX amount (e.g. "12.5") into raw integer units. */
export function parseCFX(input: string): bigint {
  const trimmed = input.trim();
  if (!/^\d*(\.\d*)?$/.test(trimmed) || trimmed === '' || trimmed === '.') {
    throw new Error('Invalid amount');
  }
  const [whole, frac = ''] = trimmed.split('.');
  const scale = 10n ** BigInt(CFX_DECIMALS);
  const w = BigInt(whole === '' ? 0 : whole) * scale;
  const f = BigInt((frac + '0000000').slice(0, CFX_DECIMALS) || '0');
  const value = w + f;
  if (value <= 0n) throw new Error('Amount must be positive');
  return value;
}

/** Shorten a Stellar address like GABC…XYZ4. */
export function shortAddress(address: string): string {
  if (address.length <= 12) return address;
  return `${address.slice(0, 4)}…${address.slice(-4)}`;
}

/** "3d 4h left", "Expired", etc. relative to `now`. */
export function timeLeft(deadline: number, now = Date.now()): string {
  const diff = deadline * 1000 - now;
  if (diff <= 0) return 'Ended';
  const days = Math.floor(diff / 86_400_000);
  const hours = Math.floor((diff % 86_400_000) / 3_600_000);
  const minutes = Math.floor((diff % 3_600_000) / 60_000);
  if (days > 0) return `${days}d ${hours}h left`;
  if (hours > 0) return `${hours}h ${minutes}m left`;
  return `${minutes}m left`;
}

/** "Aug 15, 2026" style deadline. */
export function formatDeadline(deadline: number): string {
  return new Date(deadline * 1000).toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}

/** Progress percentage clamped to [0, 100]. */
export function progressPercent(raised: bigint, goal: bigint): number {
  if (goal === 0n) return 0;
  const pct = Number((raised * 100n) / goal);
  return Math.min(100, Math.max(0, pct));
}
