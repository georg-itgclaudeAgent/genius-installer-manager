import { describe, it, expect } from "vitest";
import { newApps } from "./newApps";

describe("newApps", () => {
  it("first launch: everything except PR Extension (which every earlier install had) is new", () => {
    const r = newApps(["com.attract.pr-extension", "com.attract.genius-cut"], null);
    expect(r.newIds).toEqual(["com.attract.genius-cut"]);
    expect(r.seen).toEqual(["com.attract.pr-extension", "com.attract.genius-cut"]);
  });

  it("an app added to the registry later shows as new, once", () => {
    const seen = ["com.attract.pr-extension", "com.attract.genius-cut"];
    const r = newApps([...seen, "com.attract.new-app"], seen);
    expect(r.newIds).toEqual(["com.attract.new-app"]);
    expect(newApps([...seen, "com.attract.new-app"], r.seen).newIds).toEqual([]);
  });

  it("an app removed from the registry doesn't make anything new", () => {
    expect(newApps(["com.attract.pr-extension"], ["com.attract.pr-extension", "com.attract.genius-cut"]).newIds).toEqual([]);
  });

  it("a corrupt stored list is treated as a first launch", () => {
    expect(newApps(["com.attract.pr-extension", "com.attract.genius-cut"], "garbage" as any).newIds).toEqual(["com.attract.genius-cut"]);
  });
});
