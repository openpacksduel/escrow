import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

const EXPECTED_FILES = {
  "openpacksduel_escrow.json":
    "f16eda95787367db629051203dac8a5db61794f1c048528ecfecd868245e070d",
  "build-manifest.json":
    "1555ead5de9c038d80658dcdd58abba0e6d37ccf6d1f5925fc06b20cb957d4ef",
} as const;

describe("checked IDL provenance", () => {
  test("matches the verified workflow artifact", async () => {
    const [checksums, manifestBytes, provenanceBytes] = await Promise.all([
      readFile(new URL("../idl/SHA256SUMS", import.meta.url), "utf8"),
      readFile(new URL("../idl/build-manifest.json", import.meta.url)),
      readFile(new URL("../idl/provenance.json", import.meta.url)),
    ]);
    const manifest = JSON.parse(manifestBytes.toString());
    const provenance = JSON.parse(provenanceBytes.toString());

    for (const [file, expectedHash] of Object.entries(EXPECTED_FILES)) {
      const bytes = await readFile(new URL(`../idl/${file}`, import.meta.url));
      expect(createHash("sha256").update(bytes).digest("hex")).toBe(
        expectedHash,
      );
      expect(checksums).toContain(`${expectedHash}  ${file}`);
      expect(provenance.files[file]).toBe(expectedHash);
    }
    expect(manifest.sourceSha).toBe(provenance.sourceSha);
    expect(manifest.idl.sha256).toBe(
      EXPECTED_FILES["openpacksduel_escrow.json"],
    );
    expect(provenance.workflowRunId).toBe(29458570612);
    expect(provenance.artifactSha256SumsFileSha256).toBe(
      "56170c9830591a7900592bed94cb1e8affc043f435da80bf9b6df620e7123f39",
    );
  });
});
