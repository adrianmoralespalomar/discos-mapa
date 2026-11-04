import { QuarantinedItem } from './quarantined-item.interface';

export interface QuarantineBatch {
  path: string;
  /** Segundos Unix. */
  created: number;
  totalSize: number;
  items: QuarantinedItem[];
}
