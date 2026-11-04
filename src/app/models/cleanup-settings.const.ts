/** Tamaño mínimo de los ficheros que se comparan al buscar duplicados. */
export const DUPLICATE_MIN_SIZE_BYTES = 1024 * 1024;

/** Opciones de antigüedad para considerar un proyecto inactivo, en días. */
export const PROJECT_INACTIVITY_OPTIONS_DAYS = [30, 90, 180, 365];
export const DEFAULT_PROJECT_INACTIVITY_DAYS = 90;

/** Se muestra al intentar apartar algo de una carpeta de red o de un disco extraíble. */
export const CLEANUP_UNAVAILABLE_MESSAGE = 'En carpetas de red y discos extraíbles solo se puede mirar: la cuarentena funciona en los discos fijos del equipo.';
