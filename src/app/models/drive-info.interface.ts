export interface DriveInfo {
  mountPoint: string;
  label: string;
  fileSystem: string;
  totalSpace: number;
  availableSpace: number;
  removable: boolean;
}
