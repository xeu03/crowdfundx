import { describe, expect, it } from 'vitest';
import {
  formatCFX,
  formatDeadline,
  parseCFX,
  progressPercent,
  shortAddress,
  timeLeft,
} from './format';

describe('formatCFX', () => {
  it('formats raw units with 7 decimals', () => {
    expect(formatCFX(0n)).toBe('0');
    expect(formatCFX(12_500_000n)).toBe('1.25');
    expect(formatCFX(1_000_000_000n)).toBe('100');
  });

  it('rounds to the requested decimal count', () => {
    expect(formatCFX(1_234_567n, 2)).toBe('0.12');
    expect(formatCFX(1_234_567n, 5)).toBe('0.12345');
  });

  it('handles zero and negative values', () => {
    expect(formatCFX(-5_000_000n)).toBe('-0.5');
  });
});

describe('parseCFX', () => {
  it('parses decimal strings into raw units', () => {
    expect(parseCFX('1.25')).toBe(12_500_000n);
    expect(parseCFX('100')).toBe(1_000_000_000n);
    expect(parseCFX('0.0000001')).toBe(1n);
  });

  it('rejects invalid input', () => {
    expect(() => parseCFX('')).toThrow();
    expect(() => parseCFX('abc')).toThrow();
    expect(() => parseCFX('-3')).toThrow();
    expect(() => parseCFX('0')).toThrow(); // must be positive
  });
});

describe('shortAddress', () => {
  it('shortens long Stellar addresses', () => {
    const address = 'GBBMLHKAWLHAPFZZQJBPVX6HVSKDBE7WPTTFTUIFRW6S4VZCLI6B6BLA';
    expect(shortAddress(address)).toBe('GBBM…6BLA');
  });

  it('leaves short strings alone', () => {
    expect(shortAddress('short')).toBe('short');
  });
});

describe('timeLeft', () => {
  const now = 1_752_600_000_000;

  it('reports days/hours/minutes remaining', () => {
    expect(timeLeft(now / 1000 + 3 * 86_400 + 4 * 3_600, now)).toBe('3d 4h left');
    expect(timeLeft(now / 1000 + 2 * 3_600 + 30 * 60, now)).toBe('2h 30m left');
    expect(timeLeft(now / 1000 + 5 * 60, now)).toBe('5m left');
  });

  it('reports ended campaigns', () => {
    expect(timeLeft(now / 1000 - 1, now)).toBe('Ended');
  });

  it('accepts BigInt deadlines from the RPC (u64 regression)', () => {
    // Contract u64 values arrive as BigInt — must not throw
    // "Cannot mix BigInt and other types".
    expect(timeLeft(BigInt(now / 1000 + 3_600), now)).toBe('1h 0m left');
    expect(timeLeft(BigInt(now / 1000 - 1), now)).toBe('Ended');
    expect(formatDeadline(BigInt(1_782_547_200))).toMatch(/\d{4}/);
  });
});

describe('formatDeadline', () => {
  it('formats a unix timestamp as a date', () => {
    expect(formatDeadline(1_782_547_200)).toMatch(/\d{4}/); // contains a year
  });
});

describe('progressPercent', () => {
  it('computes and clamps percentages', () => {
    expect(progressPercent(250n, 1000n)).toBe(25);
    expect(progressPercent(1000n, 1000n)).toBe(100);
    expect(progressPercent(1500n, 1000n)).toBe(100);
    expect(progressPercent(0n, 0n)).toBe(0);
  });
});
