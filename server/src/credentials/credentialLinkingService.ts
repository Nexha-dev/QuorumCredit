/**
 * Issue #1766: Credential Linking Service
 *
 * Related credentials are not linked today: every credential in
 * `credentialStore` is an island, so a caller can never ask "which credential
 * backs this one" or "what superseded this old document". This module owns the
 * credential relationship graph, the validation rules that guard it, and the
 * traversal helpers the HTTP layer exposes.
 *
 * The graph lives next to `credentialStore` (which owns the credentials
 * themselves). In production this would be a `credential_links` table with a
 * unique constraint on `(source_id, target_id, type)`; keeping the rules in one
 * place here means the storage swap never changes the validation contract.
 */

import { credentialStore, type Credential } from "./credentialStore.js";
import { metrics } from "../http/metricsRegistry.js";

/**
 * Relationship kinds a link may declare.
 *
 * - `supports`      — target provides supporting evidence for source.
 * - `derived_from`  — source was issued on the strength of target.
 * - `supersedes`    — source replaces an older credential of the same type.
 * - `same_subject`  — both credentials attest to the same subject.
 * - `related`       — generic association, no stronger claim than "these two
 *                     belong together".
 */
export type CredentialLinkType =
  | "supports"
  | "derived_from"
  | "supersedes"
  | "same_subject"
  | "related";

export const LINK_TYPES: readonly CredentialLinkType[] = [
  "supports",
  "derived_from",
  "supersedes",
  "same_subject",
  "related",
];

/** Link types that carry direction — these must stay acyclic. */
const DIRECTIONAL_TYPES: readonly CredentialLinkType[] = [
  "supports",
  "derived_from",
  "supersedes",
];

/** Link types that are symmetric — `(a, b)` and `(b, a)` are the same edge. */
const SYMMETRIC_TYPES: readonly CredentialLinkType[] = ["same_subject", "related"];

/**
 * Link types whose two endpoints must belong to the same holder: a credential
 * that supersedes, derives from, or duplicates the subject of another is only
 * meaningful inside one holder's portfolio.
 */
const SAME_HOLDER_TYPES: readonly CredentialLinkType[] = [
  "derived_from",
  "supersedes",
  "same_subject",
];

export type LinkRejectionCode =
  | "unknown_link_type"
  | "credential_not_found"
  | "credential_revoked"
  | "self_link"
  | "duplicate_link"
  | "cycle_detected"
  | "holder_mismatch"
  | "type_mismatch"
  | "temporal_violation"
  | "cross_holder_not_acknowledged";

export interface CredentialLink {
  id: string;
  /** The credential the relationship is declared from. */
  sourceId: string;
  /** The credential it points at. */
  targetId: string;
  type: CredentialLinkType;
  /** True when the edge is stored unordered (symmetric link type). */
  symmetric: boolean;
  /** Unix seconds. */
  createdAt: number;
  createdBy?: string;
  metadata: Record<string, unknown>;
}

export interface LinkValidationResult {
  valid: boolean;
  code?: LinkRejectionCode;
  reason?: string;
}

export interface LinkRequest {
  sourceId: string;
  targetId: string;
  type: string;
  createdBy?: string;
  metadata?: Record<string, unknown>;
  /**
   * Explicit acknowledgement required to link two credentials held by
   * different holders through a non-holder-scoped link type (`related`,
   * `supports`). Without it the link is rejected with
   * `cross_holder_not_acknowledged`.
   */
  allowCrossHolder?: boolean;
}

export type LinkResult =
  | { ok: true; link: CredentialLink }
  | { ok: false; code: LinkRejectionCode; reason: string };

export interface RelationshipGraphNode {
  credentialId: string;
  holderId: string;
  type: Credential["type"];
  status: Credential["status"];
  /** Hops from the credential the graph was requested for. */
  depth: number;
}

export interface RelationshipGraphEdge {
  linkId: string;
  sourceId: string;
  targetId: string;
  type: CredentialLinkType;
  /** Orientation of the stored edge relative to the traversal. */
  direction: "outgoing" | "incoming";
}

export interface RelationshipGraph {
  rootCredentialId: string;
  depth: number;
  nodes: RelationshipGraphNode[];
  edges: RelationshipGraphEdge[];
  /** True when the traversal stopped early because `depth` was reached. */
  truncated: boolean;
}

export interface LinkStats {
  total: number;
  byType: Record<CredentialLinkType, number>;
  credentialsCovered: number;
  directional: number;
  symmetric: number;
}

function isKnownLinkType(type: string): type is CredentialLinkType {
  return (LINK_TYPES as readonly string[]).includes(type);
}

function isSymmetricType(type: CredentialLinkType): boolean {
  return SYMMETRIC_TYPES.includes(type);
}

function isDirectionalType(type: CredentialLinkType): boolean {
  return DIRECTIONAL_TYPES.includes(type);
}

function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

export class CredentialLinkingService {
  private links = new Map<string, CredentialLink>();
  /** credentialId -> link ids where the credential is the source. */
  private outgoing = new Map<string, Set<string>>();
  /** credentialId -> link ids where the credential is the target. */
  private incoming = new Map<string, Set<string>>();
  private linkCounter = 0;

  /**
   * Validate a proposed link without creating it. The HTTP layer runs this
   * first so that a rejected link can be answered with a precise code.
   */
  validateLink(request: LinkRequest): LinkValidationResult {
    const { sourceId, targetId, type } = request;

    if (!isKnownLinkType(type)) {
      return {
        valid: false,
        code: "unknown_link_type",
        reason: `type must be one of ${LINK_TYPES.join(", ")}`,
      };
    }

    const source = credentialStore.getCredential(sourceId);
    if (!source) {
      return {
        valid: false,
        code: "credential_not_found",
        reason: `source credential ${sourceId} not found`,
      };
    }

    const target = credentialStore.getCredential(targetId);
    if (!target) {
      return {
        valid: false,
        code: "credential_not_found",
        reason: `target credential ${targetId} not found`,
      };
    }

    if (source.id === target.id) {
      return {
        valid: false,
        code: "self_link",
        reason: "a credential cannot be linked to itself",
      };
    }

    const revoked = [source, target].find((c) => c.status === "revoked");
    if (revoked) {
      return {
        valid: false,
        code: "credential_revoked",
        reason: `credential ${revoked.id} is revoked and cannot take part in a link`,
      };
    }

    if (this.findLink(sourceId, targetId, type)) {
      return {
        valid: false,
        code: "duplicate_link",
        reason: `a ${type} link between ${sourceId} and ${targetId} already exists`,
      };
    }

    const holderCheck = this.validateHolders(source, target, type, request.allowCrossHolder === true);
    if (!holderCheck.valid) return holderCheck;

    const typeCheck = this.validateTypeRules(source, target, type);
    if (!typeCheck.valid) return typeCheck;

    if (isDirectionalType(type) && this.reachable(targetId, sourceId, type)) {
      return {
        valid: false,
        code: "cycle_detected",
        reason: `linking ${sourceId} -> ${targetId} (${type}) would create a cycle`,
      };
    }

    return { valid: true };
  }

  /**
   * Create a link, or return the rejection code explaining why it cannot exist.
   */
  linkCredential(request: LinkRequest): LinkResult {
    const validation = this.validateLink(request);
    if (!validation.valid) {
      metrics.incCounter("qc_credential_links_rejected_total");
      return {
        ok: false,
        code: validation.code as LinkRejectionCode,
        reason: validation.reason ?? "link rejected",
      };
    }

    const type = request.type as CredentialLinkType;
    const symmetric = isSymmetricType(type);
    const [sourceId, targetId] = symmetric
      ? [request.sourceId, request.targetId].sort()
      : [request.sourceId, request.targetId];

    const link: CredentialLink = {
      id: `link_${++this.linkCounter}_${Date.now()}`,
      sourceId,
      targetId,
      type,
      symmetric,
      createdAt: nowSeconds(),
      createdBy: request.createdBy,
      metadata: request.metadata ?? {},
    };

    this.links.set(link.id, link);
    this.addToIndex(this.outgoing, sourceId, link.id);
    this.addToIndex(this.incoming, targetId, link.id);

    metrics.incCounter("qc_credential_links_created_total");
    metrics.incLabeledCounter("qc_credential_links_by_type_total", "type", type);
    return { ok: true, link };
  }

  /** Remove a link by id. Returns false when the id is unknown. */
  unlink(linkId: string): boolean {
    const link = this.links.get(linkId);
    if (!link) return false;
    this.removeFromIndex(this.outgoing, link.sourceId, linkId);
    this.removeFromIndex(this.incoming, link.targetId, linkId);
    this.links.delete(linkId);
    metrics.incCounter("qc_credential_links_removed_total");
    return true;
  }

  /** Remove a specific relationship between two credentials. */
  unlinkByPair(sourceId: string, targetId: string, type: CredentialLinkType): boolean {
    const link = this.findLink(sourceId, targetId, type);
    if (!link) return false;
    return this.unlink(link.id);
  }

  /**
   * Drop every link touching a credential. Called when a credential is
   * revoked so the graph never keeps dangling relationships behind.
   */
  removeLinksForCredential(credentialId: string): number {
    const ids = new Set<string>([
      ...(this.outgoing.get(credentialId) ?? []),
      ...(this.incoming.get(credentialId) ?? []),
    ]);
    for (const id of ids) this.unlink(id);
    return ids.size;
  }

  getLink(linkId: string): CredentialLink | undefined {
    return this.links.get(linkId);
  }

  /**
   * Find the stored link between two credentials, honouring the fact that
   * symmetric edges are stored once in sorted order.
   */
  findLink(sourceId: string, targetId: string, type: string): CredentialLink | undefined {
    if (!isKnownLinkType(type)) return undefined;
    const [a, b] = isSymmetricType(type) ? [sourceId, targetId].sort() : [sourceId, targetId];
    for (const link of this.links.values()) {
      if (link.type === type && link.sourceId === a && link.targetId === b) return link;
    }
    return undefined;
  }

  /** Every link touching a credential, split by orientation. */
  getLinksForCredential(credentialId: string): {
    credentialId: string;
    outgoing: CredentialLink[];
    incoming: CredentialLink[];
    total: number;
  } {
    metrics.incCounter("qc_credential_link_queries_total");
    const { outgoing, incoming } = this.linksForCredential(credentialId);
    return { credentialId, outgoing, incoming, total: outgoing.length + incoming.length };
  }

  /** Credentials directly linked to this one, optionally filtered by type. */
  getLinkedCredentials(credentialId: string, type?: CredentialLinkType): Credential[] {
    const { outgoing, incoming } = this.getLinksForCredential(credentialId);
    const seen = new Set<string>();
    const result: Credential[] = [];

    for (const link of [...outgoing, ...incoming]) {
      if (type && link.type !== type) continue;
      const otherId = link.sourceId === credentialId ? link.targetId : link.sourceId;
      if (seen.has(otherId)) continue;
      seen.add(otherId);
      const credential = credentialStore.getCredential(otherId);
      if (credential) result.push(credential);
    }

    return result;
  }

  /**
   * Breadth-first traversal of the relationship graph around a credential.
   *
   * Traversal is undirected: a relationship is a relationship regardless of
   * which end you start from. Each link is reported once, oriented relative to
   * the credential it was reached from, so a client can rebuild the direction
   * of the underlying claim.
   */
  getRelationshipGraph(credentialId: string, maxDepth = 2): RelationshipGraph {
    metrics.incCounter("qc_credential_graph_queries_total");
    const depthLimit = Math.max(0, Math.floor(maxDepth));
    const root = credentialStore.getCredential(credentialId);

    const nodes: RelationshipGraphNode[] = [];
    const edges: RelationshipGraphEdge[] = [];
    const visitedCredits = new Set<string>([credentialId]);
    const seenEdges = new Set<string>();

    if (root) nodes.push(this.toNode(root, 0));

    let frontier = [credentialId];
    for (let depth = 1; depth <= depthLimit && frontier.length > 0; depth++) {
      const next: string[] = [];
      for (const current of frontier) {
        const { outgoing, incoming } = this.getLinksForCredential(current);
        for (const link of outgoing) {
          const edge = this.toEdge(link, "outgoing", seenEdges);
          if (edge) edges.push(edge);
        }
        for (const link of incoming) {
          const edge = this.toEdge(link, "incoming", seenEdges);
          if (edge) edges.push(edge);
        }

        for (const link of [...outgoing, ...incoming]) {
          const otherId = link.sourceId === current ? link.targetId : link.sourceId;
          if (visitedCredits.has(otherId)) continue;
          visitedCredits.add(otherId);
          const credential = credentialStore.getCredential(otherId);
          if (credential) nodes.push(this.toNode(credential, depth));
          next.push(otherId);
        }
      }
      frontier = next;
    }

    // Anything sitting at the boundary with a neighbour we never walked is
    // graph the caller did not get to see.
    const truncated = frontier.some((id) => {
      const { outgoing, incoming } = this.linksForCredential(id);
      return [...outgoing, ...incoming].some((link) => {
        const otherId = link.sourceId === id ? link.targetId : link.sourceId;
        return !visitedCredits.has(otherId);
      });
    });

    return {
      rootCredentialId: credentialId,
      depth: depthLimit,
      nodes,
      edges,
      truncated,
    };
  }

  /**
   * Can `fromId` reach `toId`?
   *
   * With `restrictToType` the walk is filtered to that link type: directional
   * types are followed source → target only (this is what cycle detection
   * needs), symmetric types are followed in both directions because the edge
   * carries no direction. Without it the walk is undirected across every link
   * type. The starting credential never counts as its own reachable node.
   */
  reachable(fromId: string, toId: string, restrictToType?: CredentialLinkType): boolean {
    if (fromId === toId) return false;

    const seen = new Set<string>([fromId]);
    const queue = [fromId];

    while (queue.length > 0) {
      const current = queue.shift() as string;
      const linkIds = this.candidateLinkIds(current, restrictToType);

      for (const linkId of linkIds) {
        const link = this.links.get(linkId);
        if (!link) continue;
        const next = link.sourceId === current ? link.targetId : link.sourceId;
        if (next === toId) return true;
        if (seen.has(next)) continue;
        seen.add(next);
        queue.push(next);
      }
    }

    return false;
  }

  /** Aggregate counters for dashboards and the analytics service. */
  getStats(): LinkStats {
    const byType = LINK_TYPES.reduce((acc, type) => {
      acc[type] = 0;
      return acc;
    }, {} as Record<CredentialLinkType, number>);

    const covered = new Set<string>();
    let directional = 0;
    let symmetric = 0;

    for (const link of this.links.values()) {
      byType[link.type] += 1;
      covered.add(link.sourceId);
      covered.add(link.targetId);
      if (link.symmetric) symmetric += 1;
      else directional += 1;
    }

    return {
      total: this.links.size,
      byType,
      credentialsCovered: covered.size,
      directional,
      symmetric,
    };
  }

  /** Test/ops helper: drop the whole graph. */
  clear(): void {
    this.links.clear();
    this.outgoing.clear();
    this.incoming.clear();
  }

  // ── internals ────────────────────────────────────────────────────────────

  private addToIndex(index: Map<string, Set<string>>, key: string, linkId: string): void {
    const set = index.get(key);
    if (set) set.add(linkId);
    else index.set(key, new Set([linkId]));
  }

  private removeFromIndex(index: Map<string, Set<string>>, key: string, linkId: string): void {
    const set = index.get(key);
    if (!set) return;
    set.delete(linkId);
    if (set.size === 0) index.delete(key);
  }

  /**
   * Link ids a traversal may walk out of a credential. Without a type filter
   * both orientations are walked (the graph is undirected). With a filter:
   * directional types are followed source → target only — a `derived_from`
   * claim is not symmetric — while symmetric types are walked both ways.
   */
  private candidateLinkIds(credentialId: string, restrictToType?: CredentialLinkType): string[] {
    const outgoing = [...(this.outgoing.get(credentialId) ?? [])];
    const followBothWays = restrictToType === undefined || isSymmetricType(restrictToType);
    const candidates = followBothWays
      ? [...outgoing, ...(this.incoming.get(credentialId) ?? [])]
      : outgoing;

    return restrictToType
      ? candidates.filter((id) => this.links.get(id)?.type === restrictToType)
      : candidates;
  }

  private collect(ids: Set<string> | undefined): CredentialLink[] {
    if (!ids) return [];
    return [...ids]
      .map((id) => this.links.get(id))
      .filter((link): link is CredentialLink => link !== undefined);
  }

  /** Index lookup without touching query metrics (used by internal walks). */
  private linksForCredential(credentialId: string): {
    outgoing: CredentialLink[];
    incoming: CredentialLink[];
  } {
    return {
      outgoing: this.collect(this.outgoing.get(credentialId)),
      incoming: this.collect(this.incoming.get(credentialId)),
    };
  }

  private toNode(credential: Credential, depth: number): RelationshipGraphNode {
    return {
      credentialId: credential.id,
      holderId: credential.holderId,
      type: credential.type,
      status: credential.status,
      depth,
    };
  }

  private toEdge(
    link: CredentialLink,
    direction: "outgoing" | "incoming",
    seen: Set<string>
  ): RelationshipGraphEdge | undefined {
    if (seen.has(link.id)) return undefined;
    seen.add(link.id);
    return {
      linkId: link.id,
      sourceId: link.sourceId,
      targetId: link.targetId,
      type: link.type,
      direction,
    };
  }

  private validateHolders(
    source: Credential,
    target: Credential,
    type: CredentialLinkType,
    allowCrossHolder: boolean
  ): LinkValidationResult {
    if (source.holderId === target.holderId) return { valid: true };

    if (SAME_HOLDER_TYPES.includes(type)) {
      return {
        valid: false,
        code: "holder_mismatch",
        reason: `a ${type} link requires both credentials to belong to the same holder`,
      };
    }

    if (!allowCrossHolder) {
      return {
        valid: false,
        code: "cross_holder_not_acknowledged",
        reason:
          `linking ${source.id} (holder ${source.holderId}) to ${target.id} ` +
          `(holder ${target.holderId}) crosses holders; resend with allowCrossHolder: true`,
      };
    }

    return { valid: true };
  }

  private validateTypeRules(
    source: Credential,
    target: Credential,
    type: CredentialLinkType
  ): LinkValidationResult {
    if (type === "same_subject" && source.type !== target.type) {
      return {
        valid: false,
        code: "type_mismatch",
        reason: `same_subject requires matching credential types (${source.type} vs ${target.type})`,
      };
    }

    if (type === "supersedes") {
      if (source.type !== target.type) {
        return {
          valid: false,
          code: "type_mismatch",
          reason: `supersedes requires matching credential types (${source.type} vs ${target.type})`,
        };
      }
      if (source.issuedAt <= target.issuedAt) {
        return {
          valid: false,
          code: "temporal_violation",
          reason: `supersedes requires the source to be newer (${source.issuedAt} <= ${target.issuedAt})`,
        };
      }
    }

    if (type === "derived_from" && target.issuedAt > source.issuedAt) {
      return {
        valid: false,
        code: "temporal_violation",
        reason: `derived_from requires the supporting credential to predate the derived one (${target.issuedAt} > ${source.issuedAt})`,
      };
    }

    return { valid: true };
  }
}

export const credentialLinkingService = new CredentialLinkingService();
