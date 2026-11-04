import { Component, DestroyRef, ElementRef, afterNextRender, afterRenderEffect, computed, inject, signal, viewChild } from '@angular/core';
import { HierarchyRectangularNode, hierarchy, treemap, treemapSquarify } from 'd3-hierarchy';
import { DIRECTORY_COLOR, FILE_CATEGORY_STYLES, GROUP_COLOR } from '../../models/file-category-styles.const';
import { RISK_LEVEL_STYLES, UNCLASSIFIED_COLOR } from '../../models/risk-level-styles.const';
import { TreeNode } from '../../models/tree-node.interface';
import { TreemapColorMode } from '../../models/treemap-color-mode.type';
import { TREEMAP_HEADER_HEIGHT, TREEMAP_MIN_HEADER_HEIGHT, TREEMAP_MIN_HEADER_WIDTH } from '../../models/treemap-settings.const';
import { DiskScanStore } from '../../services/disk-scan-store.service';
import { effectiveRisk } from '../../utils/effective-risk';
import { formatBytes } from '../../utils/format-bytes';
import { shadeColor, withAlpha } from '../../utils/shade-color';

type LayoutNode = HierarchyRectangularNode<TreeNode>;

const FRAME_COLOR = '#cfd8de';
const GAP_COLOR = '#e9eef0';
const HEADER_TEXT_COLOR = '#1e2d3d';
const LEAF_TEXT_COLOR = 'rgba(255, 255, 255, 0.94)';
const HOVER_COLOR = '#ffffff';
const CLICK_TARGET_COLOR = '#1e2d3d';
const MIN_LABEL_WIDTH = 54;
const MIN_LABEL_HEIGHT = 18;
const MIN_SIZE_LABEL_HEIGHT = 32;

@Component({
  selector: 'app-treemap',
  templateUrl: './treemap.component.html',
  styleUrl: './treemap.component.scss',
  host: { '(window:keydown)': 'onWindowKeyPressed($event)' },
})
export class TreemapComponent {
  private readonly destroyRef = inject(DestroyRef);
  private readonly hostElement = inject<ElementRef<HTMLElement>>(ElementRef);
  private readonly store = inject(DiskScanStore);

  private readonly mapCanvas = viewChild.required<ElementRef<HTMLCanvasElement>>('mapCanvas');
  private readonly overlayCanvas = viewChild.required<ElementRef<HTMLCanvasElement>>('overlayCanvas');
  private readonly canvasSize = signal({ width: 0, height: 0 });
  private readonly hoveredLayoutNode = signal<LayoutNode | undefined>(undefined);

  private readonly layout = computed<LayoutNode | undefined>(() => {
    const node = this.store.currentNode();
    const { width, height } = this.canvasSize();
    if (!node || !width || !height) return undefined;
    const root = hierarchy(node, (treeNode) => treeNode.children).sum((treeNode) => (treeNode.children.length ? 0 : treeNode.size));
    root.sort((a, b) => (b.value ?? 0) - (a.value ?? 0));
    return treemap<TreeNode>()
      .tile(treemapSquarify)
      .size([width, height])
      .paddingInner(1)
      .paddingOuter((layoutNode) => (layoutNode.depth === 0 ? 0 : 2))
      .paddingTop((layoutNode) => (layoutNode.depth > 0 && this.hasRoomForHeader(layoutNode) ? TREEMAP_HEADER_HEIGHT : 2))
      .round(true)(root);
  });

  constructor() {
    afterNextRender(() => this.observeHostSize());
    afterRenderEffect(() => this.drawMap(this.layout(), this.store.colorMode()));
    afterRenderEffect(() => this.drawOverlay(this.layout(), this.hoveredLayoutNode()));
  }

  private observeHostSize(): void {
    const resizeObserver = new ResizeObserver(([entry]) =>
      this.canvasSize.set({ width: Math.floor(entry.contentRect.width), height: Math.floor(entry.contentRect.height) }),
    );
    resizeObserver.observe(this.hostElement.nativeElement);
    this.destroyRef.onDestroy(() => resizeObserver.disconnect());
  }

  private hasRoomForHeader(layoutNode: LayoutNode): boolean {
    return layoutNode.x1 - layoutNode.x0 >= TREEMAP_MIN_HEADER_WIDTH && layoutNode.y1 - layoutNode.y0 >= TREEMAP_MIN_HEADER_HEIGHT;
  }

  private drawMap(root: LayoutNode | undefined, colorMode: TreemapColorMode): void {
    const context = this.prepareCanvas(this.mapCanvas().nativeElement);
    if (!context || !root) return;
    context.fillStyle = GAP_COLOR;
    context.fillRect(0, 0, root.x1, root.y1);
    context.textBaseline = 'middle';
    for (const layoutNode of root.descendants()) {
      if (layoutNode.depth === 0) continue;
      if (layoutNode.children) this.drawDirectoryFrame(context, layoutNode, colorMode);
      else this.drawLeaf(context, layoutNode, colorMode);
    }
  }

  private prepareCanvas(canvas: HTMLCanvasElement): CanvasRenderingContext2D | null {
    const { width, height } = this.canvasSize();
    const pixelRatio = window.devicePixelRatio || 1;
    canvas.width = width * pixelRatio;
    canvas.height = height * pixelRatio;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    const context = canvas.getContext('2d');
    context?.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
    context?.clearRect(0, 0, width, height);
    return context;
  }

  private drawDirectoryFrame(context: CanvasRenderingContext2D, layoutNode: LayoutNode, colorMode: TreemapColorMode): void {
    const width = layoutNode.x1 - layoutNode.x0;
    const height = layoutNode.y1 - layoutNode.y0;
    context.fillStyle = FRAME_COLOR;
    context.fillRect(layoutNode.x0, layoutNode.y0, width, height);
    if (colorMode === 'risk') {
      const risk = effectiveRisk(layoutNode);
      context.fillStyle = withAlpha(risk ? RISK_LEVEL_STYLES[risk.level].color : UNCLASSIFIED_COLOR, 0.35);
      context.fillRect(layoutNode.x0, layoutNode.y0, width, height);
    }
    if (!this.hasRoomForHeader(layoutNode)) return;
    context.font = '600 11px "Barlow Semi Condensed", sans-serif';
    context.fillStyle = HEADER_TEXT_COLOR;
    const label = `${layoutNode.data.name}   ${formatBytes(layoutNode.data.size)}`;
    this.fillTruncatedText(context, label, layoutNode.x0 + 4, layoutNode.y0 + TREEMAP_HEADER_HEIGHT / 2 + 1, width - 8);
  }

  private drawLeaf(context: CanvasRenderingContext2D, layoutNode: LayoutNode, colorMode: TreemapColorMode): void {
    const width = layoutNode.x1 - layoutNode.x0;
    const height = layoutNode.y1 - layoutNode.y0;
    if (width < 1 || height < 1) return;
    const baseColor = this.leafColor(layoutNode, colorMode);
    // Sombreado tipo "cojín": más claro arriba a la izquierda, como en WinDirStat.
    const gradient = context.createLinearGradient(layoutNode.x0, layoutNode.y0, layoutNode.x1, layoutNode.y1);
    gradient.addColorStop(0, shadeColor(baseColor, 0.28));
    gradient.addColorStop(1, shadeColor(baseColor, -0.22));
    context.fillStyle = gradient;
    context.fillRect(layoutNode.x0, layoutNode.y0, width, height);
    if (layoutNode.data.kind === 'group') this.drawGroupHatching(context, layoutNode);
    if (width < MIN_LABEL_WIDTH || height < MIN_LABEL_HEIGHT) return;
    context.font = '500 11px "Barlow", sans-serif';
    context.fillStyle = LEAF_TEXT_COLOR;
    this.fillTruncatedText(context, layoutNode.data.name, layoutNode.x0 + 4, layoutNode.y0 + 9, width - 8);
    if (height < MIN_SIZE_LABEL_HEIGHT) return;
    context.font = '400 11px "Barlow Semi Condensed", sans-serif';
    this.fillTruncatedText(context, formatBytes(layoutNode.data.size), layoutNode.x0 + 4, layoutNode.y0 + 23, width - 8);
  }

  private leafColor(layoutNode: LayoutNode, colorMode: TreemapColorMode): string {
    const treeNode = layoutNode.data;
    if (colorMode === 'risk') {
      const risk = effectiveRisk(layoutNode);
      return risk ? RISK_LEVEL_STYLES[risk.level].color : UNCLASSIFIED_COLOR;
    }
    if (treeNode.kind === 'group') return GROUP_COLOR;
    if (treeNode.kind === 'directory') return DIRECTORY_COLOR;
    return FILE_CATEGORY_STYLES[treeNode.category ?? 'other'].color;
  }

  private drawGroupHatching(context: CanvasRenderingContext2D, layoutNode: LayoutNode): void {
    const height = layoutNode.y1 - layoutNode.y0;
    context.save();
    context.beginPath();
    context.rect(layoutNode.x0, layoutNode.y0, layoutNode.x1 - layoutNode.x0, height);
    context.clip();
    context.beginPath();
    context.strokeStyle = 'rgba(255, 255, 255, 0.4)';
    context.lineWidth = 1;
    for (let offset = layoutNode.x0 - height; offset < layoutNode.x1; offset += 6) {
      context.moveTo(offset, layoutNode.y1);
      context.lineTo(offset + height, layoutNode.y0);
    }
    context.stroke();
    context.restore();
  }

  private fillTruncatedText(context: CanvasRenderingContext2D, text: string, x: number, y: number, maxWidth: number): void {
    if (maxWidth <= 0) return;
    if (context.measureText(text).width <= maxWidth) return context.fillText(text, x, y);
    let truncated = text;
    while (truncated.length > 1 && context.measureText(`${truncated}…`).width > maxWidth) truncated = truncated.slice(0, -1);
    if (truncated.length > 1) context.fillText(`${truncated}…`, x, y);
  }

  private drawOverlay(root: LayoutNode | undefined, hoveredLayoutNode: LayoutNode | undefined): void {
    const context = this.prepareCanvas(this.overlayCanvas().nativeElement);
    // Tras cambiar de carpeta, el nodo señalado pertenece al mapa anterior.
    if (!context || !root || !hoveredLayoutNode || hoveredLayoutNode.ancestors().at(-1) !== root) return;
    const clickTarget = this.clickTargetOf(hoveredLayoutNode);
    if (clickTarget && clickTarget !== hoveredLayoutNode) this.strokeNode(context, clickTarget, CLICK_TARGET_COLOR);
    this.strokeNode(context, hoveredLayoutNode, HOVER_COLOR);
  }

  private strokeNode(context: CanvasRenderingContext2D, layoutNode: LayoutNode, color: string): void {
    context.strokeStyle = color;
    context.lineWidth = 2;
    context.strokeRect(layoutNode.x0 + 1, layoutNode.y0 + 1, layoutNode.x1 - layoutNode.x0 - 2, layoutNode.y1 - layoutNode.y0 - 2);
  }

  /** Carpeta de primer nivel bajo el puntero: es a la que se entra al hacer clic. */
  private clickTargetOf(layoutNode: LayoutNode): LayoutNode | undefined {
    return layoutNode.ancestors().find((ancestor) => ancestor.depth === 1);
  }

  onCanvasMouseMoved(event: MouseEvent): void {
    const layoutNode = this.layoutNodeAt(event.offsetX, event.offsetY);
    if (layoutNode === this.hoveredLayoutNode()) return;
    this.hoveredLayoutNode.set(layoutNode);
    this.store.setHoveredNode(layoutNode?.data, layoutNode ? effectiveRisk(layoutNode) : null);
  }

  private layoutNodeAt(x: number, y: number): LayoutNode | undefined {
    let found: LayoutNode | undefined;
    let candidates: LayoutNode[] | undefined = this.layout()?.children;
    while (candidates) {
      const match: LayoutNode | undefined = candidates.find((child) => x >= child.x0 && x < child.x1 && y >= child.y0 && y < child.y1);
      if (!match) break;
      found = match;
      candidates = match.children;
    }
    return found;
  }

  onCanvasMouseLeft(): void {
    this.hoveredLayoutNode.set(undefined);
    this.store.setHoveredNode(undefined, null);
  }

  onCanvasClicked(event: MouseEvent): void {
    const layoutNode = this.layoutNodeAt(event.offsetX, event.offsetY);
    const target = layoutNode && this.clickTargetOf(layoutNode);
    if (target?.data.kind === 'directory' && target.data.id !== null) this.store.openDirectory(target.data.id);
  }

  onCanvasRightClicked(event: MouseEvent): void {
    event.preventDefault();
    const layoutNode = this.layoutNodeAt(event.offsetX, event.offsetY);
    if (layoutNode) this.store.revealInExplorer(layoutNode.data.path);
  }

  onWindowKeyPressed(event: KeyboardEvent): void {
    if ((event.target as HTMLElement | null)?.closest('input, textarea')) return;
    if (event.key === 'Backspace' || event.key === 'Escape') this.store.openParentDirectory();
  }
}
