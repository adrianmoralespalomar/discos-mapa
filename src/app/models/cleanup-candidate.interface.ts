import { RiskInfo } from './risk-info.interface';

export interface CleanupCandidate {
  /** Vacío cuando la sugerencia es un fichero suelto. */
  directoryId: number | null;
  name: string;
  path: string;
  size: number;
  fileCount: number;
  modified: number;
  risk: RiskInfo;
}
