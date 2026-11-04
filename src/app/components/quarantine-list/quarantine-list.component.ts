import { DatePipe } from '@angular/common';
import { Component, inject, OnInit, signal } from '@angular/core';
import { QuarantineBatch } from '../../models/quarantine-batch.interface';
import { BytesPipe } from '../../pipes/bytes.pipe';
import { DiskScanStore } from '../../services/disk-scan-store.service';
import { formatBytes } from '../../utils/format-bytes';

@Component({
  selector: 'app-quarantine-list',
  imports: [BytesPipe, DatePipe],
  templateUrl: './quarantine-list.component.html',
  styleUrl: './quarantine-list.component.scss',
})
export class QuarantineListComponent implements OnInit {
  protected readonly store = inject(DiskScanStore);
  /** Lote que espera la segunda pulsación para borrarse para siempre. */
  protected readonly batchAwaitingPurge = signal<string | undefined>(undefined);
  protected readonly busyBatch = signal<string | undefined>(undefined);
  protected readonly expandedBatch = signal<string | undefined>(undefined);
  protected readonly lastMessage = signal<string | undefined>(undefined);

  ngOnInit(): void {
    this.store.loadQuarantine();
  }

  onBatchToggleClicked(batch: QuarantineBatch): void {
    this.expandedBatch.update((expanded) => (expanded === batch.path ? undefined : batch.path));
  }

  async onRestoreButtonClicked(batch: QuarantineBatch): Promise<void> {
    this.busyBatch.set(batch.path);
    const report = await this.store.restoreQuarantineBatch(batch.path);
    this.busyBatch.set(undefined);
    if (!report) return;
    const failures = report.failures.length ? ` ${report.failures.length} no se han podido restaurar.` : '';
    const restored = report.moved.length === 1 ? 'Restaurado 1 elemento' : `Restaurados ${report.moved.length} elementos`;
    this.lastMessage.set(`${restored} (${formatBytes(report.movedSize)}).${failures} Vuelve a analizar para verlo en el mapa.`);
  }

  onPurgeButtonClicked(batch: QuarantineBatch): void {
    this.batchAwaitingPurge.set(batch.path);
  }

  onPurgeCancelButtonClicked(): void {
    this.batchAwaitingPurge.set(undefined);
  }

  async onPurgeConfirmButtonClicked(batch: QuarantineBatch): Promise<void> {
    this.batchAwaitingPurge.set(undefined);
    this.busyBatch.set(batch.path);
    const report = await this.store.purgeQuarantineBatch(batch.path);
    this.busyBatch.set(undefined);
    if (report && !report.error) this.lastMessage.set(`Borrado para siempre: ${formatBytes(report.freedSize)} liberados.`);
  }

  onOpenInExplorerButtonClicked(batch: QuarantineBatch): void {
    this.store.revealInExplorer(batch.path);
  }
}
