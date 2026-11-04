export interface DuplicateFile {
  path: string;
  directoryId: number;
  /** Segundos Unix. */
  modified: number;
}
