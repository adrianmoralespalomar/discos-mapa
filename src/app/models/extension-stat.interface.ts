import { FileCategory } from './file-category.type';

export interface ExtensionStat {
  extension: string;
  category: FileCategory;
  size: number;
  count: number;
}
