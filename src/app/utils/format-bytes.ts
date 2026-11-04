const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];
const BYTE_STEP = 1024;

export function formatBytes(bytes: number): string {
  if (bytes < BYTE_STEP) return `${bytes} B`;
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(BYTE_STEP)), BYTE_UNITS.length - 1);
  const value = bytes / BYTE_STEP ** exponent;
  const decimals = value >= 100 ? 0 : 1;
  return `${value.toLocaleString('es-ES', { maximumFractionDigits: decimals, minimumFractionDigits: decimals })} ${BYTE_UNITS[exponent]}`;
}
