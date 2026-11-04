import { CleanupAction } from './cleanup-action.type';

/** Elemento elegido para apartar, con lo necesario para enseñarlo en la confirmación. */
export interface CleanupSelection {
  path: string;
  name: string;
  size: number;
  action: CleanupAction;
}
