import { Component, computed, inject, signal } from '@angular/core';
import { FILE_CATEGORY_STYLES } from '../../models/file-category-styles.const';
import { LargeFile } from '../../models/large-file.interface';
import { RISK_LEVEL_STYLES, UNCLASSIFIED_COLOR } from '../../models/risk-level-styles.const';
import { FORGOTTEN_FILE_AGE_SECONDS } from '../../models/treemap-settings.const';
import { AgePipe } from '../../pipes/age.pipe';
import { BytesPipe } from '../../pipes/bytes.pipe';
import { DiskScanStore } from '../../services/disk-scan-store.service';

@Component({
  selector: 'app-largest-files',
  imports: [AgePipe, BytesPipe],
  templateUrl: './largest-files.component.html',
  styleUrl: './largest-files.component.scss',
})
export class LargestFilesComponent {
  protected readonly store = inject(DiskScanStore);
  protected readonly fileCategoryStyles = FILE_CATEGORY_STYLES;
  protected readonly onlyForgotten = signal<boolean>(false);

  protected readonly visibleFiles = computed<LargeFile[]>(() => {
    const files = this.store.summary()?.topFiles ?? [];
    if (!this.onlyForgotten()) return files;
    const forgottenBefore = Date.now() / 1000 - FORGOTTEN_FILE_AGE_SECONDS;
    return files.filter((file) => file.modified && file.modified < forgottenBefore);
  });

  onForgottenToggleChanged(event: Event): void {
    this.onlyForgotten.set((event.target as HTMLInputElement).checked);
  }

  protected riskColor(file: LargeFile): string {
    return file.risk ? RISK_LEVEL_STYLES[file.risk.level].color : UNCLASSIFIED_COLOR;
  }

  protected riskLabel(file: LargeFile): string {
    return file.risk ? `${RISK_LEVEL_STYLES[file.risk.level].label}: ${file.risk.reason}` : 'Sin regla conocida';
  }

  onShowFolderButtonClicked(file: LargeFile): void {
    if (file.directoryId !== null) this.store.openDirectory(file.directoryId);
  }

  onOpenInExplorerButtonClicked(file: LargeFile): void {
    this.store.revealInExplorer(file.path);
  }
}
