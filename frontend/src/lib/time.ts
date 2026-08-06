/**
 * 时间工具
 */
export function nowMs(): number {
  return Date.now();
}

export function startOfDayMs(d: Date = new Date()): number {
  const x = new Date(d);
  x.setHours(0, 0, 0, 0);
  return x.getTime();
}

export function startOfWeekMs(d: Date = new Date()): number {
  const x = new Date(d);
  const day = x.getDay(); // 0=周日
  const diff = (day + 6) % 7; // 周一为起点
  x.setDate(x.getDate() - diff);
  x.setHours(0, 0, 0, 0);
  return x.getTime();
}

export function startOfMonthMs(d: Date = new Date()): number {
  return new Date(d.getFullYear(), d.getMonth(), 1).getTime();
}

export function endOfDayMs(d: Date = new Date()): number {
  const x = new Date(d);
  x.setHours(23, 59, 59, 999);
  return x.getTime();
}

export function formatClock(ms: number): string {
  const totalSec = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(totalSec / 60).toString().padStart(2, '0');
  const s = (totalSec % 60).toString().padStart(2, '0');
  return `${m}:${s}`;
}
