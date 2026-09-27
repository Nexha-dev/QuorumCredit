import { describe, it, expect, beforeEach } from "vitest";
import { credentialStore, type Credential } from "../src/credentials/credentialStore.js";
import {
  CredentialLinkingService,
  LINK_TYPES,
  type CredentialLinkType,
} from "../src/credentials/credentialLinkingService.js";

const FAR_FUTURE = Math.floor(Date.now() / 1000) + 365 * 24 * 60 * 60;
let counter = 0;

/** Each test gets its own graph; credentials come from the shared store. */
let service: CredentialLinkingService;

/**
 * `issueCredential` stamps `issuedAt` with the current second, which makes the
 * temporal rules non-deterministic. Pin it explicitly instead.
 */
function issue(
  holderId: string,
  type: Credential["type"] = "education",
  issuedAt?: number,
  status?: Credential["status"]
): Credential {
  const credential = credentialStore.issueCredential(
    holderId,
    type,
    `issuer-${++counter}`,
    FAR_FUTURE
  );
  if (issuedAt !== undefined) credential.issuedAt = issuedAt;
  if (status) credential.status = status;
  return credential;
}

const now = Math.floor(Date.now() / 1000);

beforeEach(() => {
  service = new CredentialLinkingService();
});

describe("credentialLinkingService — link creation", () => {
  it("creates a directional link and reports it from both endpoints", () => {
    const education = issue("holder-a", "education", now - 1000);
    const professional = issue("holder-a", "professional", now - 500);

    const result = service.linkCredential({
      sourceId: professional.id,
      targetId: education.id,
      type: "derived_from",
      createdBy: "verifier-1",
      metadata: { reason: "degree required for licence" },
    });

    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.link.sourceId).toBe(professional.id);
    expect(result.link.targetId).toBe(education.id);
    expect(result.link.symmetric).toBe(false);
    expect(result.link.createdBy).toBe("verifier-1");

    const fromProfessional = service.getLinksForCredential(professional.id);
    expect(fromProfessional.outgoing).toHaveLength(1);
    expect(fromProfessional.incoming).toHaveLength(0);
    expect(fromProfessional.total).toBe(1);

    const fromEducation = service.getLinksForCredential(education.id);
    expect(fromEducation.incoming).toHaveLength(1);
    expect(fromEducation.outgoing).toHaveLength(0);
  });

  it("lists the credentials linked to a credential, filtered by type", () => {
    const education = issue("holder-b", "education", now - 1000);
    const professional = issue("holder-b", "professional", now - 500);
    const related = issue("holder-b", "financial", now - 400);

    service.linkCredential({
      sourceId: professional.id,
      targetId: education.id,
      type: "derived_from",
    });
    service.linkCredential({
      sourceId: professional.id,
      targetId: related.id,
      type: "related",
    });

    const all = service.getLinkedCredentials(professional.id).map((c) => c.id);
    expect(new Set(all)).toEqual(new Set([education.id, related.id]));

    const derived = service.getLinkedCredentials(professional.id, "derived_from").map((c) => c.id);
    expect(derived).toEqual([education.id]);
  });

  it("stores symmetric links once, so (a,b) and (b,a) are the same edge", () => {
    const first = issue("holder-c", "education", now - 1000);
    const second = issue("holder-c", "education", now - 900);

    const forward = service.linkCredential({
      sourceId: first.id,
      targetId: second.id,
      type: "same_subject",
    });
    expect(forward.ok).toBe(true);

    const backward = service.linkCredential({
      sourceId: second.id,
      targetId: first.id,
      type: "same_subject",
    });
    expect(backward.ok).toBe(false);
    if (backward.ok) return;
    expect(backward.code).toBe("duplicate_link");

    expect(service.getStats().total).toBe(1);
    expect(service.getStats().symmetric).toBe(1);
  });
});

describe("credentialLinkingService — validation", () => {
  it("rejects an unknown link type", () => {
    const a = issue("holder-d");
    const b = issue("holder-d");
    const result = service.linkCredential({
      sourceId: a.id,
      targetId: b.id,
      type: "teleports_to",
    });
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.code).toBe("unknown_link_type");
  });

  it("exposes the full set of link types it accepts", () => {
    expect([...LINK_TYPES]).toEqual([
      "supports",
      "derived_from",
      "supersedes",
      "same_subject",
      "related",
    ]);
  });

  it("rejects a missing credential, naming which side is missing", () => {
    const a = issue("holder-e");
    const missingSource = service.validateLink({
      sourceId: "cred_does_not_exist",
      targetId: a.id,
      type: "related",
    });
    expect(missingSource.valid).toBe(false);
    expect(missingSource.code).toBe("credential_not_found");
    expect(missingSource.reason).toContain("cred_does_not_exist");

    const missingTarget = service.validateLink({
      sourceId: a.id,
      targetId: "cred_missing_target",
      type: "related",
    });
    expect(missingTarget.code).toBe("credential_not_found");
    expect(missingTarget.reason).toContain("cred_missing_target");
  });

  it("rejects linking a credential to itself", () => {
    const a = issue("holder-f");
    const result = service.linkCredential({ sourceId: a.id, targetId: a.id, type: "related" });
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.code).toBe("self_link");
  });

  it("rejects links involving a revoked credential", () => {
    const active = issue("holder-g");
    const revoked = issue("holder-g", "education", now - 100, "revoked");

    const result = service.linkCredential({
      sourceId: active.id,
      targetId: revoked.id,
      type: "related",
    });
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.code).toBe("credential_revoked");
    expect(result.reason).toContain(revoked.id);
  });

  it("rejects a duplicate link", () => {
    const a = issue("holder-h", "education", now - 1000);
    const b = issue("holder-h", "professional", now - 500);

    expect(
      service.linkCredential({ sourceId: b.id, targetId: a.id, type: "derived_from" }).ok
    ).toBe(true);
    const duplicate = service.linkCredential({
      sourceId: b.id,
      targetId: a.id,
      type: "derived_from",
    });
    expect(duplicate.ok).toBe(false);
    if (duplicate.ok) return;
    expect(duplicate.code).toBe("duplicate_link");
  });
});

describe("credentialLinkingService — holder rules", () => {
  it("requires the same holder for holder-scoped link types", () => {
    const mine = issue("holder-i", "education", now - 1000);
    const theirs = issue("holder-j", "professional", now - 500);

    const result = service.linkCredential({
      sourceId: theirs.id,
      targetId: mine.id,
      type: "derived_from",
    });
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.code).toBe("holder_mismatch");
  });

  it("requires an explicit acknowledgement before crossing holders", () => {
    const mine = issue("holder-k", "education", now - 1000);
    const theirs = issue("holder-l", "professional", now - 500);

    const refused = service.linkCredential({
      sourceId: mine.id,
      targetId: theirs.id,
      type: "supports",
    });
    expect(refused.ok).toBe(false);
    if (refused.ok) return;
    expect(refused.code).toBe("cross_holder_not_acknowledged");

    const accepted = service.linkCredential({
      sourceId: mine.id,
      targetId: theirs.id,
      type: "supports",
      allowCrossHolder: true,
    });
    expect(accepted.ok).toBe(true);

    // Same-holder links never need the flag.
    const sameHolder = issue("holder-m", "education", now - 1000);
    const sameHolderTwo = issue("holder-m", "education", now - 900);
    expect(
      service.linkCredential({
        sourceId: sameHolder.id,
        targetId: sameHolderTwo.id,
        type: "same_subject",
      }).ok
    ).toBe(true);
  });
});

describe("credentialLinkingService — type-specific rules", () => {
  it("requires matching credential types for same_subject", () => {
    const education = issue("holder-n", "education", now - 1000);
    const professional = issue("holder-n", "professional", now - 500);

    const result = service.linkCredential({
      sourceId: education.id,
      targetId: professional.id,
      type: "same_subject",
    });
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.code).toBe("type_mismatch");
  });

  it("requires a supersedes link to be same-type and newer", () => {
    const older = issue("holder-o", "education", now - 1000);
    const newer = issue("holder-o", "education", now - 100);

    const wrongWay = service.linkCredential({
      sourceId: older.id,
      targetId: newer.id,
      type: "supersedes",
    });
    expect(wrongWay.ok).toBe(false);
    if (wrongWay.ok) return;
    expect(wrongWay.code).toBe("temporal_violation");

    const otherType = issue("holder-o", "professional", now + 10);
    const mismatched = service.linkCredential({
      sourceId: otherType.id,
      targetId: older.id,
      type: "supersedes",
    });
    expect(mismatched.ok).toBe(false);
    if (mismatched.ok) return;
    expect(mismatched.code).toBe("type_mismatch");

    expect(
      service.linkCredential({ sourceId: newer.id, targetId: older.id, type: "supersedes" }).ok
    ).toBe(true);
  });

  it("requires the supporting credential for derived_from to predate the derived one", () => {
    const recent = issue("holder-p", "education", now - 10);
    const derived = issue("holder-p", "professional", now - 1000);

    const result = service.linkCredential({
      sourceId: derived.id,
      targetId: recent.id,
      type: "derived_from",
    });
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.code).toBe("temporal_violation");
  });

  it("rejects directional links that would close a cycle", () => {
    const a = issue("holder-q", "education", now - 3000);
    const b = issue("holder-q", "education", now - 2000);
    const c = issue("holder-q", "education", now - 1000);

    expect(service.linkCredential({ sourceId: a.id, targetId: b.id, type: "supports" }).ok).toBe(
      true
    );
    expect(service.linkCredential({ sourceId: b.id, targetId: c.id, type: "supports" }).ok).toBe(
      true
    );

    // a -> b -> c already exists, so c -> a would close the loop.
    const cycle = service.linkCredential({
      sourceId: c.id,
      targetId: a.id,
      type: "supports",
    });
    expect(cycle.ok).toBe(false);
    if (cycle.ok) return;
    expect(cycle.code).toBe("cycle_detected");
    expect(service.getStats().total).toBe(2);
  });

  it("does not treat symmetric links as cycles", () => {
    const a = issue("holder-r", "education", now - 1000);
    const b = issue("holder-r", "education", now - 900);
    const c = issue("holder-r", "education", now - 800);

    expect(service.linkCredential({ sourceId: a.id, targetId: b.id, type: "same_subject" }).ok).toBe(
      true
    );
    expect(service.linkCredential({ sourceId: b.id, targetId: c.id, type: "same_subject" }).ok).toBe(
      true
    );
    expect(service.linkCredential({ sourceId: c.id, targetId: a.id, type: "same_subject" }).ok).toBe(
      true
    );
  });
});

describe("credentialLinkingService — graph traversal", () => {
  /** d -> c -> b -> a */
  function chain(): Credential[] {
    const a = issue("holder-s", "education", now - 4000);
    const b = issue("holder-s", "education", now - 3000);
    const c = issue("holder-s", "education", now - 2000);
    const d = issue("holder-s", "education", now - 1000);
    service.linkCredential({ sourceId: b.id, targetId: a.id, type: "derived_from" });
    service.linkCredential({ sourceId: c.id, targetId: b.id, type: "derived_from" });
    service.linkCredential({ sourceId: d.id, targetId: c.id, type: "derived_from" });
    return [a, b, c, d];
  }

  it("traverses the graph breadth-first, honouring the depth limit", () => {
    const [a, b, c, d] = chain();

    // a and c are both one hop from b; d is two hops.
    const depthOne = service.getRelationshipGraph(b.id, 1);
    expect(depthOne.nodes.map((n) => n.credentialId).sort()).toEqual([a.id, b.id, c.id].sort());
    expect(depthOne.edges).toHaveLength(2);
    expect(depthOne.truncated).toBe(true);

    const depthTwo = service.getRelationshipGraph(b.id, 2);
    expect(depthTwo.nodes.map((n) => n.credentialId).sort()).toEqual(
      [a.id, b.id, c.id, d.id].sort()
    );
    expect(depthTwo.nodes.find((n) => n.credentialId === a.id)?.depth).toBe(1);
    expect(depthTwo.nodes.find((n) => n.credentialId === c.id)?.depth).toBe(1);
    expect(depthTwo.nodes.find((n) => n.credentialId === d.id)?.depth).toBe(2);
    expect(depthTwo.edges).toHaveLength(3);
    expect(depthTwo.truncated).toBe(false);

    // A depth of 0 is the credential itself.
    const depthZero = service.getRelationshipGraph(b.id, 0);
    expect(depthZero.nodes.map((n) => n.credentialId)).toEqual([b.id]);
    expect(depthZero.edges).toHaveLength(0);
  });

  it("reports each edge once, oriented relative to the credential it was reached from", () => {
    const [a, b] = chain();
    const graph = service.getRelationshipGraph(a.id, 2);

    const link = graph.edges.find((e) => e.targetId === a.id && e.sourceId === b.id);
    expect(link?.direction).toBe("incoming");
    expect(graph.edges).toHaveLength(2);
  });

  it("walks the graph undirectionally but follows directional types one way when restricted", () => {
    const a = issue("holder-t", "education", now - 3000);
    const b = issue("holder-t", "education", now - 2000);
    const unrelated = issue("holder-t", "financial", now - 1000);

    service.linkCredential({ sourceId: b.id, targetId: a.id, type: "derived_from" });
    service.linkCredential({ sourceId: a.id, targetId: unrelated.id, type: "related" });

    // Unrestricted traversal ignores orientation and link type.
    expect(service.reachable(a.id, b.id)).toBe(true);
    expect(service.reachable(a.id, unrelated.id)).toBe(true);
    expect(service.reachable(b.id, unrelated.id)).toBe(true);

    // Restricting to a directional type follows source -> target only.
    expect(service.reachable(b.id, a.id, "derived_from")).toBe(true);
    expect(service.reachable(a.id, b.id, "derived_from")).toBe(false);
    expect(service.reachable(a.id, unrelated.id, "derived_from")).toBe(false);

    // Restricting to a symmetric type is still undirected.
    expect(service.reachable(unrelated.id, a.id, "related")).toBe(true);
  });

  it("returns a root-only graph for an unlinked credential", () => {
    const lonely = issue("holder-u");
    const graph = service.getRelationshipGraph(lonely.id, 3);
    expect(graph.nodes).toHaveLength(1);
    expect(graph.edges).toHaveLength(0);
    expect(graph.truncated).toBe(false);
  });
});

describe("credentialLinkingService — lifecycle", () => {
  it("removes a link by id and by pair", () => {
    const a = issue("holder-v", "education", now - 1000);
    const b = issue("holder-v", "professional", now - 500);

    const created = service.linkCredential({
      sourceId: b.id,
      targetId: a.id,
      type: "derived_from",
    });
    expect(created.ok).toBe(true);
    if (!created.ok) return;

    expect(service.unlink("link_does_not_exist")).toBe(false);
    expect(service.unlink(created.link.id)).toBe(true);
    expect(service.getLinksForCredential(a.id).total).toBe(0);

    expect(service.linkCredential({ sourceId: b.id, targetId: a.id, type: "derived_from" }).ok).toBe(
      true
    );
    expect(service.unlinkByPair(b.id, a.id, "derived_from")).toBe(true);
    expect(service.unlinkByPair(b.id, a.id, "derived_from")).toBe(false);
  });

  it("drops every link touching a revoked credential", () => {
    const a = issue("holder-w", "education", now - 3000);
    const b = issue("holder-w", "education", now - 2000);
    const c = issue("holder-w", "education", now - 1000);

    service.linkCredential({ sourceId: b.id, targetId: a.id, type: "derived_from" });
    service.linkCredential({ sourceId: c.id, targetId: b.id, type: "derived_from" });
    service.linkCredential({ sourceId: c.id, targetId: a.id, type: "related" });
    expect(service.getStats().total).toBe(3);

    const removed = service.removeLinksForCredential(b.id);
    expect(removed).toBe(2);
    expect(service.getStats().total).toBe(1);
    expect(service.getLinksForCredential(b.id).total).toBe(0);
  });

  it("reports aggregate stats by link type", () => {
    const a = issue("holder-x", "education", now - 3000);
    const b = issue("holder-x", "education", now - 2000);
    const c = issue("holder-x", "professional", now - 1000);

    service.linkCredential({ sourceId: b.id, targetId: a.id, type: "same_subject" });
    service.linkCredential({ sourceId: c.id, targetId: a.id, type: "derived_from" });

    const stats = service.getStats();
    expect(stats.total).toBe(2);
    expect(stats.symmetric).toBe(1);
    expect(stats.directional).toBe(1);
    expect(stats.credentialsCovered).toBe(3);
    expect(stats.byType.same_subject).toBe(1);
    expect(stats.byType.derived_from).toBe(1);
    expect(stats.byType.supersedes).toBe(0);
  });

  it("validates without side effects", () => {
    const a = issue("holder-y", "education", now - 1000);
    const b = issue("holder-y", "professional", now - 500);

    const validation = service.validateLink({
      sourceId: b.id,
      targetId: a.id,
      type: "derived_from" as CredentialLinkType,
    });
    expect(validation.valid).toBe(true);
    expect(service.getStats().total).toBe(0);
  });
});
