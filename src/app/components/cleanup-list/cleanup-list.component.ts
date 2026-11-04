import { Component, computed, inject, signal } from '@angular/core';
import { CleanupCandidate } from '../../models/cleanup-candidate.interface';
import { RiskLevelTotal } from '../../models/risk-level-total.interface';
import { RISK_LEVEL_STYLES, SUGGESTED_RISK_LEVELS } from '../../models/risk-level-styles.const';
import { RiskLevel } from '../../models/risk-level.type';
import { AgePipe } from '../../pipes/age.pipe';
import { BytesPipe } from '../../pipes/bytes.pipe';
import { DiskScanStore } from '../../services/disk-scan-store.service';

@Component({
  selector: 'app-cleanup-list',
  imports: [AgePipe, BytesPipe],
  templateUrl: './cleanup-list.component.html',
  styleUrl: './cleanup-list.component.scss',
})
export class CleanupListComponent {
  protected readonly store = inject(DiskScanStore);
  protected readonly riskLevelStyles = RISK_LEVEL_STYLES;
  protected readonly selectedLevel = signal<RiskLevel | undefined>(undefined);

  protected readonly levelTotals = computed<RiskLevelTotal[]>(() => {
    const summary = this.store.summary();
    if (!summary) return [];
    const totals: Partial<Record<RiskLevel, number>> = { safe: summary.recoverableSafe, probable: summary.recoverableProbable, review: summary.reviewSize };
    return SUGGESTED_RISK_LEVELS.map((level) => ({ level, size: totals[level] ?? 0, count: summary.candidates.filter((candidate) => candidate.risk.level === level).length }));
  });

  protected readonly visibleCandidates = computed<CleanupCandidate[]>(() => {
    const candidates = this.store.summary()?.candidates ?? [];
    const level = this.selectedLevel();
    return level ? candidates.filter((candidate) => candidate.risk.level === level) : candidates;
  });

  onLevelFilterClicked(level: RiskLevel): void {
    this.selectedLevel.update((selected) => (selected === level ? undefined : level));
  }

  onShowInMapButtonClicked(candidate: CleanupCandidate): void {
    if (candidate.directoryId !== null) this.store.openDirectory(candidate.directoryId);
  }

  onOpenInExplorerButtonClicked(candidate: CleanupCandidate): void {
    this.store.revealInExplorer(candidate.path);
  }
}
