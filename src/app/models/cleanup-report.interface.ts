import { CleanupFailure } from './cleanup-failure.interface';
import { QuarantinedItem } from './quarantined-item.interface';

export interface CleanupReport {
  moved: QuarantinedItem[];
  failures: CleanupFailure[];
  movedSize: number;
}
