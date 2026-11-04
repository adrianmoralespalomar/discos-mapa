import { CleanupAction } from './cleanup-action.type';

export interface CleanupRequest {
  path: string;
  action: CleanupAction;
}
