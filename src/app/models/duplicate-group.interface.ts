import { DuplicateFile } from './duplicate-file.interface';

export interface DuplicateGroup {
  size: number;
  /** Lo que se libera dejando una sola copia. */
  wastedSize: number;
  files: DuplicateFile[];
}
