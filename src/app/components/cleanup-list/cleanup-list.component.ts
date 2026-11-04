import { Component, computed, inject, signal } from '@angular/core';
import { CLEANABLE_ACTIONS } from '../../models/cleanable-actions.const';
import { CleanupCandidate } from '../../models/cleanup-candidate.interface';
import { CleanupSelection } from '../../models/cleanup-selection.interface';
import { CLEANUP_UNAVAILABLE_MESSAGE } from '../../models/cleanup-settings.const';
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
  protected readonly cleanupUnavailableMessage = CLEANUP_UNAVAILABLE_MESSAGE;
  protected readonly riskLevelStyles = RISK_LEVEL_STYLES;
  protected readonly selectedLevel = signal<RiskLevel | undefined>(undefined);
  protected readonly selectedPaths = signal<Set<string>>(new Set());

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

  /** Lo marcado que sigue existiendo en el resumen (tras apartar algo, desaparece de la lista). */
  protected readonly selectedCandidates = computed<CleanupCandidate[]>(() => {
    const selectedPaths = this.selectedPaths();
    return (this.store.summary()?.candidates ?? []).filter((candidate) => selectedPaths.has(candidate.path));
  });

  protected readonly selectedSize = computed<number>(() => this.selectedCandidates().reduce((total, candidate) => total + candidate.size, 0));

  protected isCleanable(candidate: CleanupCandidate): boolean {
    return CLEANABLE_ACTIONS.includes(candidate.risk.action);
  }

  onLevelFilterClicked(level: RiskLevel): void {
    this.selectedLevel.update((selected) => (selected === level ? undefined : level));
  }

  onSelectSafeButtonClicked(): void {
    const safePaths = (this.store.summary()?.candidates ?? [])
      .filter((candidate) => candidate.risk.level === 'safe' && this.isCleanable(candidate))
      .map((candidate) => candidate.path);
    this.selectedPaths.set(new Set(safePaths));
  }

  onCandidateCheckboxChanged(candidate: CleanupCandidate, event: Event): void {
    const checked = (event.target as HTMLInputElement).checked;
    this.selectedPaths.update((paths) => {
      const updated = new Set(paths);
      if (checked) updated.add(candidate.path);
      else updated.delete(candidate.path);
      return updated;
    });
  }

  onShowInMapButtonClicked(candidate: CleanupCandidate): void {
    if (candidate.directoryId !== null) this.store.openDirectory(candidate.directoryId);
  }

  onOpenInExplorerButtonClicked(candidate: CleanupCandidate): void {
    this.store.revealInExplorer(candidate.path);
  }

  onMoveToQuarantineButtonClicked(): void {
    const selections: CleanupSelection[] = this.selectedCandidates().map((candidate) => ({
      path: candidate.path,
      name: candidate.name,
      size: candidate.size,
      action: candidate.risk.action,
    }));
    this.store.requestCleanup(selections);
  }
}
