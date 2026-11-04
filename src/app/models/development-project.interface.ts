import { RegenerableFolder } from './regenerable-folder.interface';

export interface DevelopmentProject {
  directoryId: number;
  name: string;
  path: string;
  kind: string;
  /** Segundos Unix del último cambio fuera de las carpetas regenerables. */
  lastActivity: number;
  regenerableSize: number;
  folders: RegenerableFolder[];
}
