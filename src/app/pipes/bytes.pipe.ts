import { Pipe, PipeTransform } from '@angular/core';
import { formatBytes } from '../utils/format-bytes';

@Pipe({ name: 'bytes' })
export class BytesPipe implements PipeTransform {
  transform(bytes: number): string {
    return formatBytes(bytes);
  }
}
