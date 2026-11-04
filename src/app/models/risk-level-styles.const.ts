import { RiskLevel } from './risk-level.type';
import { RiskLevelStyle } from './risk-level-style.interface';

export const RISK_LEVEL_STYLES: Record<RiskLevel, RiskLevelStyle> = {
  safe: { label: 'Seguro', description: 'Se regenera solo o es basura', color: '#3f8f5f' },
  probable: { label: 'Probable', description: 'Se puede regenerar, con algo de coste', color: '#c9a227' },
  review: { label: 'Revisar', description: 'Depende de ti', color: '#d4732c' },
  protected: { label: 'No tocar', description: 'Necesario para el sistema o irrecuperable', color: '#b8403b' },
};

export const RISK_LEVELS_IN_ORDER: RiskLevel[] = ['safe', 'probable', 'review', 'protected'];

/** Niveles que aparecen en la lista de sugerencias. */
export const SUGGESTED_RISK_LEVELS: RiskLevel[] = ['safe', 'probable', 'review'];

/** Color de lo que no tiene ninguna regla. */
export const UNCLASSIFIED_COLOR = '#9aa6ae';
