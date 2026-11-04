import { RiskLevel } from './risk-level.type';

export interface RiskInfo {
  level: RiskLevel;
  category: string;
  reason: string;
  tip: string | null;
  /** La regla viene de una carpeta superior. */
  inherited: boolean;
}
