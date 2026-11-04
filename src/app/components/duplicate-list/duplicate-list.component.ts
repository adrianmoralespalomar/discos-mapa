import { DecimalPipe } from '@angular/common';
import { Component, computed, inject } from '@angular/core';
import { CleanupSelection } from '../../models/cleanup-selection.interface';
import { DUPLICATE_MIN_SIZE_BYTES } from '../../models/cleanup-settings.const';
import { DuplicateFile } from '../../models/duplicate-file.interface';
import { DuplicateGroup } from '../../models/duplicate-group.interface';
import { AgePipe } from '../../pipes/age.pipe';
import { BytesPipe } from '../../pipes/bytes.pipe';
import { DiskScanStore } from '../../services/disk-scan-store.service';

@Component({
  selector: 'app-duplicate-list',
  imports: [AgePipe, BytesPipe, DecimalPipe],
  templateUrl: './duplicate-list.component.html',
  styleUrl: './duplicate-list.component.scss',
})
export class DuplicateListComponent {
  protected readonly store = inject(DiskScanStore);
  protected readonly minSize = DUPLICATE_MIN_SIZE_BYTES;

  /** Porcentaje de bytes ya comparados por contenido. */
  protected readonly hashedPercent = computed<number>(() => {
    const progress = this.store.duplicateProgress();
    return progress?.bytesToHash ? Math.min(100, (progress.hashedBytes / progress.bytesToHash) * 100) : 0;
  });

  onStopButtonClicked(): void {
    this.store.cancelDuplicates();
  }

  onSearchButtonClicked(): void {
    this.store.findDuplicates(DUPLICATE_MIN_SIZE_BYTES);
  }

  protected fileName(file: DuplicateFile): string {
    return file.path.slice(file.path.lastIndexOf('\\') + 1);
  }

  protected folderOf(file: DuplicateFile): string {
    return file.path.slice(0, file.path.lastIndexOf('\\'));
  }

  onShowFolderButtonClicked(file: DuplicateFile): void {
    this.store.openDirectory(file.directoryId);
  }

  onOpenInExplorerButtonClicked(file: DuplicateFile): void {
    this.store.revealInExplorer(file.path);
  }

  onMoveFileButtonClicked(file: DuplicateFile, group: DuplicateGroup): void {
    this.store.requestCleanup([this.fileSelection(file, group)]);
  }

  private fileSelection(file: DuplicateFile, group: DuplicateGroup): CleanupSelection {
    return { path: file.path, name: this.fileName(file), size: group.size, action: 'item' };
  }

  onKeepOldestButtonClicked(group: DuplicateGroup): void {
    // Los ficheros llegan ordenados del más antiguo al más reciente: se conserva el primero.
    this.store.requestCleanup(group.files.slice(1).map((file) => this.fileSelection(file, group)));
  }
}
