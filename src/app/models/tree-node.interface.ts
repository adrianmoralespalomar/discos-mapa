import { FileCategory } from './file-category.type';
import { RiskInfo } from './risk-info.interface';
import { TreeNodeKind } from './tree-node-kind.type';

export interface TreeNode {
  /** Solo las carpetas tienen id. */
  id: number | null;
  kind: TreeNodeKind;
  name: string;
  path: string;
  size: number;
  fileCount: number;
  /** Segundos Unix de la última modificación (en carpetas, la más reciente de su contenido). */
  modified: number;
  inaccessible: boolean;
  cloudOnly: boolean;
  hasHiddenChildren: boolean;
  category: FileCategory | null;
  risk: RiskInfo | null;
  children: TreeNode[];
}
