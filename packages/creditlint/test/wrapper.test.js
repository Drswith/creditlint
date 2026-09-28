"use strict";

const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const wrapper = path.resolve(__dirname, "..", "bin", "creditlint.js");
const {
  candidateBinaries,
  platformPackageName,
  resolveBinary,
} = require("../bin/creditlint.js");

function makeFakeBinary() {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "creditlint-wrapper-"));
  const fakeBinary = path.join(tempDir, "creditlint-fake");

  fs.writeFileSync(
    fakeBinary,
    [
      "#!/usr/bin/env node",
      "const args = process.argv.slice(2);",
      "console.log(JSON.stringify(args));",
      "process.exit(args.includes('--fail') ? 7 : 0);",
      "",
    ].join("\n"),
  );
  fs.chmodSync(fakeBinary, 0o755);

  return { fakeBinary, tempDir };
}

test("CREDITLINT_BIN override receives user arguments", () => {
  const { fakeBinary, tempDir } = makeFakeBinary();

  try {
    const result = spawnSync(process.execPath, [wrapper, "check", "--stdin"], {
      env: { ...process.env, CREDITLINT_BIN: fakeBinary },
      encoding: "utf8",
    });

    assert.equal(result.status, 0);
    assert.match(result.stdout, /\["check","--stdin"\]/);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test("native exit code is forwarded", () => {
  const { fakeBinary, tempDir } = makeFakeBinary();

  try {
    const result = spawnSync(process.execPath, [wrapper, "--fail"], {
      env: { ...process.env, CREDITLINT_BIN: fakeBinary },
      encoding: "utf8",
    });

    assert.equal(result.status, 7);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test("missing native binary exits with code 2", () => {
  const missingBinary = path.join(os.tmpdir(), "creditlint-missing-native");
  const result = spawnSync(process.execPath, [wrapper, "--version"], {
    env: { ...process.env, CREDITLINT_BIN: missingBinary },
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /could not find a native creditlint binary/);
  assert.match(result.stderr, /CREDITLINT_BIN/);
});

test("CREDITLINT_BIN override is the only candidate when set", () => {
  const overridePath = path.join(os.tmpdir(), "creditlint-override");
  const candidates = candidateBinaries({
    env: { CREDITLINT_BIN: overridePath },
    platform: "darwin",
    arch: "arm64",
  });

  assert.deepEqual(candidates, [
    {
      kind: "env",
      path: overridePath,
    },
  ]);
});

test("current supported platform maps to native package name", () => {
  assert.equal(platformPackageName("darwin", "arm64"), "creditlint-darwin-arm64");
  assert.equal(platformPackageName("darwin", "x64"), "creditlint-darwin-x64");
  assert.equal(platformPackageName("linux", "arm64"), "creditlint-linux-arm64");
  assert.equal(platformPackageName("linux", "x64"), "creditlint-linux-x64");
  assert.equal(platformPackageName("win32", "x64"), "creditlint-windows-x64");
});

test("installed platform package candidate is checked before local fallbacks", () => {
  const candidates = candidateBinaries({
    env: {},
    platform: process.platform,
    arch: process.arch,
    repoRoot: "/repo",
  });

  assert.equal(candidates[0].kind, "platform-package");
  assert.match(candidates[0].packageName, /^creditlint-/);
  assert.equal(candidates.at(-2).kind, "cargo-release");
  assert.equal(candidates.at(-1).kind, "cargo-debug");
});

test("unsupported platform reports platform key", () => {
  const resolved = resolveBinary({
    env: {},
    platform: "freebsd",
    arch: "riscv64",
    repoRoot: "/definitely-missing-creditlint-repo",
  });

  assert.equal(resolved.binary, undefined);
  assert.equal(resolved.platformKey, "freebsd-riscv64");
  assert.match(resolved.error, /unsupported platform: freebsd-riscv64/);
});

test("supported platform without native binary reports missing optional package", () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "creditlint-empty-repo-"));

  try {
    const resolved = resolveBinary({
      env: {},
      platform: "linux",
      arch: "x64",
      baseDir: tempDir,
      repoRoot: tempDir,
    });

    assert.equal(resolved.binary, undefined);
    assert.equal(resolved.packageName, "creditlint-linux-x64");
    assert.match(resolved.error, /optional package creditlint-linux-x64 may not be installed/);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test("strip-workspace-protocol replaces workspace: prefixes with concrete versions", () => {
  const { stripWorkspaceProtocol, stripWorkspaceProtocolFile } = require("../../../scripts/strip-workspace-protocol.js");

  const stripped = stripWorkspaceProtocol({
    optionalDependencies: {
      "creditlint-darwin-arm64": "workspace:0.1.2",
      "creditlint-darwin-x64": "workspace:^1.0.0",
      "creditlint-linux-arm64": "workspace:*",
    },
    dependencies: {
      "normal-dep": "^2.0.0",
    },
  });

  assert.equal(stripped.optionalDependencies["creditlint-darwin-arm64"], "0.1.2");
  assert.equal(stripped.optionalDependencies["creditlint-darwin-x64"], "^1.0.0");
  assert.equal(stripped.optionalDependencies["creditlint-linux-arm64"], "*");
  assert.equal(stripped.dependencies["normal-dep"], "^2.0.0");
});

test("strip-workspace-protocol file writes concrete versions to disk", () => {
  const { stripWorkspaceProtocolFile } = require("../../../scripts/strip-workspace-protocol.js");
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "creditlint-strip-"));
  const pkgPath = path.join(tempDir, "package.json");

  try {
    fs.writeFileSync(
      pkgPath,
      JSON.stringify({
        name: "test-pkg",
        version: "1.0.0",
        optionalDependencies: {
          "creditlint-darwin-arm64": "workspace:1.0.0",
        },
      }),
    );

    stripWorkspaceProtocolFile(pkgPath);

    const result = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
    assert.equal(result.optionalDependencies["creditlint-darwin-arm64"], "1.0.0");
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});
