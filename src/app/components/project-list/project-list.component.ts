import { Component, computed, inject, OnInit, signal } from '@angular/core';
import { CleanupSelection } from '../../models/cleanup-selection.interface';
import { DEFAULT_PROJECT_INACTIVITY_DAYS, PROJECT_INACTIVITY_OPTIONS_DAYS } from '../../models/cleanup-settings.const';
import { DevelopmentProject } from '../../models/development-project.interface';
import { SECONDS_PER_DAY } from '../../models/format-units.const';
import { AgePipe } from '../../pipes/age.pipe';
import { BytesPipe } from '../../pipes/bytes.pipe';
import { DiskScanStore } from '../../services/disk-scan-store.service';

@Component({
  selector: 'app-project-list',
  imports: [AgePipe, BytesPipe],
  templateUrl: './project-list.component.html',
  styleUrl: './project-list.component.scss',
})
export class ProjectListComponent implements OnInit {
  protected readonly store = inject(DiskScanStore);
  protected readonly inactivityOptions = PROJECT_INACTIVITY_OPTIONS_DAYS;
  protected readonly inactivityDays = signal<number>(DEFAULT_PROJECT_INACTIVITY_DAYS);

  protected readonly inactiveProjects = computed<DevelopmentProject[]>(() => {
    const activeAfter = Date.now() / 1000 - this.inactivityDays() * SECONDS_PER_DAY;
    return this.store.projects().filter((project) => project.lastActivity < activeAfter);
  });

  protected readonly inactiveSize = computed<number>(() => this.inactiveProjects().reduce((total, project) => total + project.regenerableSize, 0));

  ngOnInit(): void {
    this.store.loadProjects();
  }

  onInactivityChanged(event: Event): void {
    this.inactivityDays.set(Number((event.target as HTMLSelectElement).value));
  }

  onCleanAllButtonClicked(): void {
    this.store.requestCleanup(this.inactiveProjects().flatMap((project) => this.projectSelections(project)));
  }

  private projectSelections(project: DevelopmentProject): CleanupSelection[] {
    return project.folders.map((folder) => ({ path: folder.path, name: `${project.name}\\${folder.name}`, size: folder.size, action: 'item' }));
  }

  onShowInMapButtonClicked(project: DevelopmentProject): void {
    this.store.openDirectory(project.directoryId);
  }

  onCleanProjectButtonClicked(project: DevelopmentProject): void {
    this.store.requestCleanup(this.projectSelections(project));
  }
}
