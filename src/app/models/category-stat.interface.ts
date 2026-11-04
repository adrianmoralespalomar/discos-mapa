import { FileCategory } from './file-category.type';

export interface CategoryStat {
  category: FileCategory;
  size: number;
  count: number;
}
