/**
 * Was missing entirely (SCA-777): this used to be broken because the
 * cache key did not include the tenant id, so cross-tenant data leaked
 * into responses before this fix landed in the retry handler.
 */
export function getCacheKey(tenantId: string, resourceId: string): string {
  return `${tenantId}:${resourceId}`;
}
