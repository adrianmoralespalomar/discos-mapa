import { Injectable, signal } from '@angular/core';
import { Channel, invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { Ancestor } from '../models/ancestor.interface';
import { AppScreen } from '../models/app-screen.type';
import { DriveInfo } from '../models/drive-info.interface';
import { RiskInfo } from '../models/risk-info.interface';
import { ScanProgress } from '../models/scan-progress.interface';
import { ScanSummary } from '../models/scan-summary.interface';
import { SidePanelTab } from '../models/side-panel-tab.type';
import { TreeNode } from '../models/tree-node.interface';
import { TreeResponse } from '../models/tree-response.interface';
import { TreemapColorMode } from '../models/treemap-color-mode.type';
import { TREEMAP_DEPTH } from '../models/treemap-settings.const';

/** Estado de la app y llamadas al backend de Tauri. */
@Injectable({ providedIn: 'root' })
export class DiskScanStore {
  readonly ancestors = signal<Ancestor[]>([]);
  readonly colorMode = signal<TreemapColorMode>('category');
  readonly currentNode = signal<TreeNode | undefined>(undefined);
  readonly drives = signal<DriveInfo[]>([]);
  readonly errorMessage = signal<string | undefined>(undefined);
  readonly hoveredNode = signal<TreeNode | undefined>(undefined);
  /** Regla del elemento señalado, propia o heredada de su carpeta. */
  readonly hoveredRisk = signal<RiskInfo | null>(null);
  readonly progress = signal<ScanProgress | undefined>(undefined);
  readonly scanningPath = signal<string>('');
  readonly screen = signal<AppScreen>('drives');
  readonly sidePanelTab = signal<SidePanelTab>('cleanup');
  readonly summary = signal<ScanSummary | undefined>(undefined);

  async loadDrives(): Promise<void> {
    try {
      this.drives.set(await invoke<DriveInfo[]>('list_drives'));
    } catch (error) {
      this.errorMessage.set(`No se han podido leer los discos: ${error}`);
    }
  }

  async pickFolderAndScan(): Promise<void> {
    const folder = await open({ directory: true, multiple: false, title: 'Elige la carpeta que quieres analizar' });
    if (typeof folder === 'string') await this.startScan(folder);
  }

  async startScan(path: string): Promise<void> {
    const onProgress = new Channel<ScanProgress>();
    onProgress.onmessage = (progress: ScanProgress) => this.progress.set(progress);
    this.errorMessage.set(undefined);
    this.progress.set(undefined);
    this.scanningPath.set(path);
    this.screen.set('scanning');
    try {
      const summary = await invoke<ScanSummary>('scan_path', { path, onProgress });
      this.summary.set(summary);
      await this.openDirectory(summary.rootId);
      this.screen.set('results');
    } catch (error) {
      this.errorMessage.set(`No se ha podido analizar ${path}: ${error}`);
      this.screen.set('drives');
    }
  }

  async cancelScan(): Promise<void> {
    await invoke('cancel_scan');
  }

  async openDirectory(directoryId: number): Promise<void> {
    const response = await invoke<TreeResponse>('get_tree', { nodeId: directoryId, depth: TREEMAP_DEPTH });
    this.setHoveredNode(undefined, null);
    this.currentNode.set(response.node);
    this.ancestors.set(response.ancestors);
  }

  setHoveredNode(node: TreeNode | undefined, risk: RiskInfo | null): void {
    this.hoveredNode.set(node);
    this.hoveredRisk.set(risk);
  }

  async openParentDirectory(): Promise<void> {
    const ancestors = this.ancestors();
    if (ancestors.length > 1) await this.openDirectory(ancestors[ancestors.length - 2].id);
  }

  async revealInExplorer(path: string): Promise<void> {
    try {
      await revealItemInDir(path);
    } catch (error) {
      this.errorMessage.set(`No se ha podido abrir el Explorador: ${error}`);
    }
  }

  backToDrives(): void {
    this.screen.set('drives');
    this.loadDrives();
  }
}
