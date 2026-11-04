import { FileCategory } from './file-category.type';
import { RiskInfo } from './risk-info.interface';

export interface LargeFile {
  name: string;
  path: string;
  directoryId: number | null;
  size: number;
  modified: number;
  category: FileCategory;
  risk: RiskInfo | null;
}
