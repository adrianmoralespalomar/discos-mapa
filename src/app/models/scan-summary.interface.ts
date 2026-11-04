import { CategoryStat } from './category-stat.interface';
import { CleanupCandidate } from './cleanup-candidate.interface';
import { ExtensionStat } from './extension-stat.interface';
import { LargeFile } from './large-file.interface';

export interface ScanSummary {
  rootId: number;
  rootPath: string;
  totalSize: number;
  fileCount: number;
  directoryCount: number;
  inaccessibleCount: number;
  durationMs: number;
  cancelled: boolean;
  topFiles: LargeFile[];
  extensions: ExtensionStat[];
  categories: CategoryStat[];
  candidates: CleanupCandidate[];
  recoverableSafe: number;
  recoverableProbable: number;
  reviewSize: number;
}
