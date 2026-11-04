import { CleanupAction } from './cleanup-action.type';
import { RiskLevel } from './risk-level.type';

export interface RiskInfo {
  level: RiskLevel;
  category: string;
  reason: string;
  tip: string | null;
  action: CleanupAction;
  /** La regla viene de una carpeta superior. */
  inherited: boolean;
}
