import { DecimalPipe } from '@angular/common';
import { Component, computed, inject } from '@angular/core';
import { FILE_CATEGORY_STYLES } from '../../models/file-category-styles.const';
import { RISK_LEVEL_STYLES } from '../../models/risk-level-styles.const';
import { AgePipe } from '../../pipes/age.pipe';
import { BytesPipe } from '../../pipes/bytes.pipe';
import { DiskScanStore } from '../../services/disk-scan-store.service';

@Component({
  selector: 'app-info-bar',
  imports: [AgePipe, BytesPipe, DecimalPipe],
  templateUrl: './info-bar.component.html',
  styleUrl: './info-bar.component.scss',
})
export class InfoBarComponent {
  protected readonly store = inject(DiskScanStore);
  protected readonly fileCategoryStyles = FILE_CATEGORY_STYLES;
  protected readonly riskLevelStyles = RISK_LEVEL_STYLES;

  protected readonly scanDurationSeconds = computed<number>(() => (this.store.summary()?.durationMs ?? 0) / 1000);
}
