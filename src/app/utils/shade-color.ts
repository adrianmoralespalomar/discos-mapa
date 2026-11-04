/** Aclara (amount > 0) u oscurece (amount < 0) un color hexadecimal `#rrggbb`. */
export function shadeColor(hexColor: string, amount: number): string {
  const target = amount > 0 ? 255 : 0;
  const shaded = hexChannels(hexColor).map((channel) => Math.round(channel + (target - channel) * Math.abs(amount)));
  return `rgb(${shaded.join(', ')})`;
}

export function withAlpha(hexColor: string, alpha: number): string {
  return `rgba(${hexChannels(hexColor).join(', ')}, ${alpha})`;
}

function hexChannels(hexColor: string): number[] {
  return [1, 3, 5].map((start) => parseInt(hexColor.slice(start, start + 2), 16));
}
