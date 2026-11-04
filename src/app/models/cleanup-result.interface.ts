import { CleanupReport } from './cleanup-report.interface';
import { ScanSummary } from './scan-summary.interface';

export interface CleanupResult {
  report: CleanupReport;
  summary: ScanSummary;
}
