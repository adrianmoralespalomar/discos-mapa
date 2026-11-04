import { DecimalPipe } from '@angular/common';
import { Component, computed, inject } from '@angular/core';
import { FILE_CATEGORY_STYLES } from '../../models/file-category-styles.const';
import { BytesPipe } from '../../pipes/bytes.pipe';
import { DiskScanStore } from '../../services/disk-scan-store.service';

@Component({
  selector: 'app-file-types',
  imports: [BytesPipe, DecimalPipe],
  templateUrl: './file-types.component.html',
  styleUrl: './file-types.component.scss',
})
export class FileTypesComponent {
  protected readonly store = inject(DiskScanStore);
  protected readonly fileCategoryStyles = FILE_CATEGORY_STYLES;

  /** Tamaño de la categoría más grande, para escalar las barras. */
  protected readonly largestCategorySize = computed<number>(() => Math.max(1, ...(this.store.summary()?.categories ?? []).map((category) => category.size)));
}
