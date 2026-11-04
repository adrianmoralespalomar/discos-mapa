import { HierarchyRectangularNode } from 'd3-hierarchy';
import { TreeNode } from './tree-node.interface';

/** Nodo del árbol con su rectángulo calculado en el mapa. */
export type LayoutNode = HierarchyRectangularNode<TreeNode>;
