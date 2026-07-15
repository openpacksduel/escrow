import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

describe("checked IDL provenance", () => {
  test("matches the verified workflow artifact", async () => {
    const idl = await readFile(
      new URL("../idl/openpacksduel_escrow.json", import.meta.url),
    );
    const manifest = await readFile(
      new URL("../idl/build-manifest.json", import.meta.url),
    );

    expect(createHash("sha256").update(idl).digest("hex")).toBe(
      "53ed60b44d5cef022db0301e5d6495ca3bf84486a048c7dd7ce5621a499762e0",
    );
    expect(createHash("sha256").update(manifest).digest("hex")).toBe(
      "0ea30ac7a9f95dc9fcb8eaa06a66294d895e9bc38b7149fc88620dfeb0b5afb1",
    );
  });
});
