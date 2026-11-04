import { DuplicateGroup } from './duplicate-group.interface';

export interface DuplicatesResult {
  groups: DuplicateGroup[];
  wastedSize: number;
  comparedFiles: number;
  cancelled: boolean;
}
