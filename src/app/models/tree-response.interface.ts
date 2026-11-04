import { Ancestor } from './ancestor.interface';
import { TreeNode } from './tree-node.interface';

export interface TreeResponse {
  node: TreeNode;
  ancestors: Ancestor[];
}
