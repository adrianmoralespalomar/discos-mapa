import { Component, inject } from '@angular/core';
import { CleanupDialogComponent } from './components/cleanup-dialog/cleanup-dialog.component';
import { CleanupListComponent } from './components/cleanup-list/cleanup-list.component';
import { DriveListComponent } from './components/drive-list/drive-list.component';
import { DuplicateListComponent } from './components/duplicate-list/duplicate-list.component';
import { FileTypesComponent } from './components/file-types/file-types.component';
import { InfoBarComponent } from './components/info-bar/info-bar.component';
import { LargestFilesComponent } from './components/largest-files/largest-files.component';
import { ProjectListComponent } from './components/project-list/project-list.component';
import { QuarantineListComponent } from './components/quarantine-list/quarantine-list.component';
import { ScanProgressComponent } from './components/scan-progress/scan-progress.component';
import { TreemapComponent } from './components/treemap/treemap.component';
import { FILE_CATEGORY_STYLES } from './models/file-category-styles.const';
import { RISK_LEVEL_STYLES, RISK_LEVELS_IN_ORDER, UNCLASSIFIED_COLOR } from './models/risk-level-styles.const';
import { SidePanelTab } from './models/side-panel-tab.type';
import { SIDE_PANEL_TABS } from './models/side-panel-tabs.const';
import { TreemapColorMode } from './models/treemap-color-mode.type';
import { DiskScanStore } from './services/disk-scan-store.service';

@Component({
  selector: 'app-root',
  imports: [
    CleanupDialogComponent,
    CleanupListComponent,
    DriveListComponent,
    DuplicateListComponent,
    FileTypesComponent,
    InfoBarComponent,
    LargestFilesComponent,
    ProjectListComponent,
    QuarantineListComponent,
    ScanProgressComponent,
    TreemapComponent,
  ],
  templateUrl: './app.html',
  styleUrl: './app.scss',
})
export class App {
  protected readonly store = inject(DiskScanStore);
  protected readonly fileCategoryStyles = Object.values(FILE_CATEGORY_STYLES);
  protected readonly riskLevels = RISK_LEVELS_IN_ORDER.map((level) => RISK_LEVEL_STYLES[level]);
  protected readonly unclassifiedColor = UNCLASSIFIED_COLOR;
  protected readonly sidePanelTabs = SIDE_PANEL_TABS;

  onErrorDismissButtonClicked(): void {
    this.store.errorMessage.set(undefined);
  }

  onNewScanButtonClicked(): void {
    this.store.backToDrives();
  }

  onBreadcrumbClicked(directoryId: number): void {
    this.store.openDirectory(directoryId);
  }

  onColorModeButtonClicked(colorMode: TreemapColorMode): void {
    this.store.colorMode.set(colorMode);
  }

  onSidePanelTabClicked(tab: SidePanelTab): void {
    this.store.sidePanelTab.set(tab);
  }
}
