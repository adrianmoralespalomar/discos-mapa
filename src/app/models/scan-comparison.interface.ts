import { GrowthEntry } from './growth-entry.interface';
import { PastScan } from './past-scan.interface';

export interface ScanComparison {
  rootPath: string;
  previousScan: PastScan | null;
  grown: GrowthEntry[];
  /** Del más reciente al más antiguo, incluido el actual. */
  scans: PastScan[];
}
