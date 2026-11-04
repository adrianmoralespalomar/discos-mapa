import { PastScan } from './past-scan.interface';

export interface ScannedRoot {
  rootPath: string;
  lastScan: PastScan;
  scanCount: number;
}
