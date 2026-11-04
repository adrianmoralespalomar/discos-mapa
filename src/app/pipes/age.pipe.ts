import { Pipe, PipeTransform } from '@angular/core';
import { formatAge } from '../utils/format-age';

@Pipe({ name: 'age' })
export class AgePipe implements PipeTransform {
  transform(unixSeconds: number): string {
    return formatAge(unixSeconds);
  }
}
