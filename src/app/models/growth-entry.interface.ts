export interface GrowthEntry {
  name: string;
  path: string;
  directoryId: number;
  previousSize: number;
  currentSize: number;
  /** No existía (o era pequeña) en el análisis anterior. */
  isNew: boolean;
}
