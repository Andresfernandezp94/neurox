import { apiGet } from './client';
import type { Health } from '../types';

export function getHealth(): Promise<Health> {
  return apiGet<Health>('/health');
}
