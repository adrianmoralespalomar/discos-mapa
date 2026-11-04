import { HierarchyNode } from 'd3-hierarchy';
import { RiskInfo } from '../models/risk-info.interface';
import { TreeNode } from '../models/tree-node.interface';

/** Regla propia del nodo o la de su antepasado más cercano. */
export function effectiveRisk(node: HierarchyNode<TreeNode>): RiskInfo | null {
  return node.ancestors().find((ancestor) => ancestor.data.risk)?.data.risk ?? null;
}
