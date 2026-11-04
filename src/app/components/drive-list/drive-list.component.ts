import { Component, inject, OnInit } from '@angular/core';
import { DriveInfo } from '../../models/drive-info.interface';
import { ScannedRoot } from '../../models/scanned-root.interface';
import { AgePipe } from '../../pipes/age.pipe';
import { BytesPipe } from '../../pipes/bytes.pipe';
import { DiskScanStore } from '../../services/disk-scan-store.service';

@Component({
  selector: 'app-drive-list',
  imports: [AgePipe, BytesPipe],
  templateUrl: './drive-list.component.html',
  styleUrl: './drive-list.component.scss',
})
export class DriveListComponent implements OnInit {
  protected readonly store = inject(DiskScanStore);

  ngOnInit(): void {
    this.store.loadDrives();
    this.store.loadScannedRoots();
  }

  protected usedFraction(drive: DriveInfo): number {
    return drive.totalSpace ? (drive.totalSpace - drive.availableSpace) / drive.totalSpace : 0;
  }

  onDriveClicked(drive: DriveInfo): void {
    this.store.startScan(drive.mountPoint);
  }

  onPickFolderButtonClicked(): void {
    this.store.pickFolderAndScan();
  }

  onNetworkPathSubmitted(event: SubmitEvent, networkPath: string): void {
    event.preventDefault();
    const trimmedPath = networkPath.trim();
    if (trimmedPath) this.store.startScan(trimmedPath);
  }

  onScannedRootClicked(root: ScannedRoot): void {
    this.store.startScan(root.rootPath);
  }
}
