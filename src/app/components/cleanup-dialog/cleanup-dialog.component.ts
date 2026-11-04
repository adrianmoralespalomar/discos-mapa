import { Component, computed, inject } from '@angular/core';
import { BytesPipe } from '../../pipes/bytes.pipe';
import { DiskScanStore } from '../../services/disk-scan-store.service';

@Component({
  selector: 'app-cleanup-dialog',
  imports: [BytesPipe],
  templateUrl: './cleanup-dialog.component.html',
  styleUrl: './cleanup-dialog.component.scss',
  host: { '(document:keydown.escape)': 'onEscapeKeyPressed()' },
})
export class CleanupDialogComponent {
  protected readonly store = inject(DiskScanStore);

  protected readonly pendingSize = computed<number>(() => (this.store.pendingCleanup() ?? []).reduce((total, selection) => total + selection.size, 0));

  onEscapeKeyPressed(): void {
    if (!this.store.cleanupInProgress()) this.store.closeCleanupDialog();
  }

  onConfirmButtonClicked(): void {
    this.store.confirmCleanup();
  }

  onCancelButtonClicked(): void {
    this.store.closeCleanupDialog();
  }

  onOpenQuarantineButtonClicked(): void {
    this.store.closeCleanupDialog();
    this.store.sidePanelTab.set('quarantine');
  }
}
