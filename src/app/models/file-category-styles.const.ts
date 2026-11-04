import { FileCategory } from './file-category.type';
import { FileCategoryStyle } from './file-category-style.interface';

export const FILE_CATEGORY_STYLES: Record<FileCategory, FileCategoryStyle> = {
  video: { label: 'Vídeo', color: '#c98a3c' },
  image: { label: 'Imagen', color: '#7fa36b' },
  audio: { label: 'Audio', color: '#9a6f9e' },
  document: { label: 'Documento', color: '#5b84b1' },
  archive: { label: 'Comprimido', color: '#c9b37a' },
  diskImage: { label: 'Disco virtual o ISO', color: '#b5634b' },
  code: { label: 'Código', color: '#3f9a98' },
  executable: { label: 'Programa', color: '#5d6b7a' },
  system: { label: 'Datos de sistema', color: '#8b9bb0' },
  other: { label: 'Otros', color: '#b3aca2' },
};

export const DIRECTORY_COLOR = '#6f8090';
export const GROUP_COLOR = '#c3c9cd';
