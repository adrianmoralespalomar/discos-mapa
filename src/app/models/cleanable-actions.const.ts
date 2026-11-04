import { CleanupAction } from './cleanup-action.type';

/** Acciones que la app hace en bloque; lo demás se revisa a mano o con su herramienta oficial. */
export const CLEANABLE_ACTIONS: CleanupAction[] = ['item', 'contents'];
