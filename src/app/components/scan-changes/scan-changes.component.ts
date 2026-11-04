import { DatePipe } from '@angular/common';
import { Component, computed, inject } from '@angular/core';
import { GrowthEntry } from '../../models/growth-entry.interface';
import { PastScan } from '../../models/past-scan.interface';
import { BytesPipe } from '../../pipes/bytes.pipe';
import { DiskScanStore } from '../../services/disk-scan-store.service';

@Component({
  selector: 'app-scan-changes',
  imports: [BytesPipe, DatePipe],
  templateUrl: './scan-changes.component.html',
  styleUrl: './scan-changes.component.scss',
})
export class ScanChangesComponent {
  protected readonly store = inject(DiskScanStore);

  /** Diferencia de tamaño total con el análisis anterior (negativa si ha bajado). */
  protected readonly totalChange = computed<number>(() => {
    const comparison = this.store.comparison();
    return comparison?.previousScan ? comparison.scans[0].totalSize - comparison.previousScan.totalSize : 0;
  });

  protected readonly largestScanSize = computed<number>(() => Math.max(1, ...(this.store.comparison()?.scans ?? []).map((scan) => scan.totalSize)));

  protected growthOf(entry: GrowthEntry): number {
    return entry.currentSize - entry.previousSize;
  }

  onShowInMapButtonClicked(entry: GrowthEntry): void {
    this.store.openDirectory(entry.directoryId);
  }

  protected scanBarPercent(scan: PastScan): number {
    return (scan.totalSize / this.largestScanSize()) * 100;
  }
}
