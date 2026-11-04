import { Injectable, signal } from '@angular/core';
import { Channel, invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { Ancestor } from '../models/ancestor.interface';
import { AppScreen } from '../models/app-screen.type';
import { CleanupReport } from '../models/cleanup-report.interface';
import { CleanupRequest } from '../models/cleanup-request.interface';
import { CleanupResult } from '../models/cleanup-result.interface';
import { CleanupSelection } from '../models/cleanup-selection.interface';
import { DriveInfo } from '../models/drive-info.interface';
import { PurgeReport } from '../models/purge-report.interface';
import { QuarantineBatch } from '../models/quarantine-batch.interface';
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
  readonly cleanupInProgress = signal<boolean>(false);
  readonly cleanupReport = signal<CleanupReport | undefined>(undefined);
  readonly colorMode = signal<TreemapColorMode>('category');
  readonly currentNode = signal<TreeNode | undefined>(undefined);
  readonly drives = signal<DriveInfo[]>([]);
  readonly errorMessage = signal<string | undefined>(undefined);
  readonly hoveredNode = signal<TreeNode | undefined>(undefined);
  /** Regla del elemento señalado, propia o heredada de su carpeta. */
  readonly hoveredRisk = signal<RiskInfo | null>(null);
  /** Lo que espera confirmación para ir a la cuarentena. */
  readonly pendingCleanup = signal<CleanupSelection[] | undefined>(undefined);
  readonly progress = signal<ScanProgress | undefined>(undefined);
  readonly quarantineBatches = signal<QuarantineBatch[]>([]);
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

  /** Abre la confirmación; no se mueve nada hasta `confirmCleanup`. */
  requestCleanup(selections: CleanupSelection[]): void {
    const cleanable = selections.filter((selection) => selection.action !== 'manual');
    if (!cleanable.length) return;
    this.cleanupReport.set(undefined);
    this.pendingCleanup.set(cleanable);
  }

  async confirmCleanup(): Promise<void> {
    const selections = this.pendingCleanup();
    if (!selections?.length) return;
    this.cleanupInProgress.set(true);
    try {
      const requests: CleanupRequest[] = selections.map(({ path, action }) => ({ path, action }));
      const result = await invoke<CleanupResult>('quarantine_items', { requests });
      this.summary.set(result.summary);
      this.cleanupReport.set(result.report);
      const currentNodeId = this.currentNode()?.id;
      if (currentNodeId !== null && currentNodeId !== undefined) await this.openDirectory(currentNodeId);
    } catch (error) {
      this.errorMessage.set(`No se ha podido apartar: ${error}`);
      this.pendingCleanup.set(undefined);
    } finally {
      this.cleanupInProgress.set(false);
    }
  }

  closeCleanupDialog(): void {
    this.pendingCleanup.set(undefined);
    this.cleanupReport.set(undefined);
  }

  async loadQuarantine(): Promise<void> {
    try {
      this.quarantineBatches.set(await invoke<QuarantineBatch[]>('list_quarantine'));
    } catch (error) {
      this.errorMessage.set(`No se ha podido leer la cuarentena: ${error}`);
    }
  }

  async restoreQuarantineBatch(batchPath: string): Promise<CleanupReport | undefined> {
    try {
      const report = await invoke<CleanupReport>('restore_quarantine_batch', { batchPath });
      await this.loadQuarantine();
      return report;
    } catch (error) {
      this.errorMessage.set(`No se ha podido restaurar: ${error}`);
      return undefined;
    }
  }

  async purgeQuarantineBatch(batchPath: string): Promise<PurgeReport | undefined> {
    try {
      const report = await invoke<PurgeReport>('purge_quarantine_batch', { batchPath });
      if (report.error) this.errorMessage.set(report.error);
      await this.loadQuarantine();
      return report;
    } catch (error) {
      this.errorMessage.set(`No se ha podido vaciar: ${error}`);
      return undefined;
    }
  }
}
