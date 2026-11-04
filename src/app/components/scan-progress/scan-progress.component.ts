import { DecimalPipe } from '@angular/common';
import { Component, inject, signal } from '@angular/core';
import { BytesPipe } from '../../pipes/bytes.pipe';
import { DiskScanStore } from '../../services/disk-scan-store.service';

@Component({
  selector: 'app-scan-progress',
  imports: [BytesPipe, DecimalPipe],
  templateUrl: './scan-progress.component.html',
  styleUrl: './scan-progress.component.scss',
})
export class ScanProgressComponent {
  protected readonly store = inject(DiskScanStore);
  protected readonly stopping = signal<boolean>(false);

  onStopButtonClicked(): void {
    this.stopping.set(true);
    this.store.cancelScan();
  }
}
