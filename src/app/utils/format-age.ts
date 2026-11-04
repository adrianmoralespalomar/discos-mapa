const SECONDS_PER_DAY = 24 * 3600;

/** "hoy", "hace 3 días", "hace 2 años"… a partir de segundos Unix. */
export function formatAge(unixSeconds: number): string {
  if (!unixSeconds) return 'sin fecha';
  const days = Math.floor((Date.now() / 1000 - unixSeconds) / SECONDS_PER_DAY);
  if (days < 1) return 'hoy';
  if (days < 30) return days === 1 ? 'hace 1 día' : `hace ${days} días`;
  const months = Math.floor(days / 30);
  if (months < 12) return months === 1 ? 'hace 1 mes' : `hace ${months} meses`;
  const years = Math.floor(days / 365);
  return years === 1 ? 'hace 1 año' : `hace ${years} años`;
}
