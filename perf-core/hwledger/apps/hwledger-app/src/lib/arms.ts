export const ABLATION_VARIANTS = new Set(['stock', 'ours']);

export function isAblationVariant(variant: string | undefined | null): boolean {
  return Boolean(variant && ABLATION_VARIANTS.has(variant));
}

export function isAuxRole(variant: string | undefined | null): boolean {
  return Boolean(variant) && !isAblationVariant(variant);
}

export function auxRoleLabel(arm: string): string {
  const a = arm.toLowerCase();
  if (a.includes('minimax') || a.includes('judge') || a.includes('eval')) {
    return `${arm} (judge / eval)`;
  }
  if (a.includes('distill')) {
    return `${arm} (distiller)`;
  }
  return `${arm} (aux)`;
}

export function auxVariants(variants: Iterable<string>): string[] {
  return [...variants].filter(isAuxRole).sort();
}
